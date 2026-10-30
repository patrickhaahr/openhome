//! Meal photos (`CONTEXT.md`, Meal Log): the user's photo of what they ate, kept with its Meal Log.
//!
//! A JPEG or PNG upload is decoded, turned upright from its EXIF orientation, limited to
//! [`LONG_SIDE`] px on its long side and re-encoded as AVIF with a [`THUMBNAIL_LONG_SIDE`] px
//! thumbnail. Re-encoding from pixels removes every piece of metadata (EXIF, GPS, XMP, ICC), and
//! the original upload is never written. Encoding uses libavif's aom encoder at speed 8 on one
//! thread, on a blocking worker outside the async runtime and one photo at a time, so the other
//! services on the host keep their CPU.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use base64::Engine;
use image::{
    DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits, RgbImage,
    imageops::FilterType, metadata::Orientation,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnectOptions;
use tokio::sync::Semaphore;
use uuid::Uuid;

/// The largest photo accepted, before base64 encoding.
pub const MAX_PHOTO_BYTES: usize = 10 * 1024 * 1024;
/// The request body limit of the routes that carry a photo; every other route keeps Axum's
/// default. A base64 photo of [`MAX_PHOTO_BYTES`] is about 13.4 MiB.
pub const PHOTO_BODY_LIMIT: usize = 15 * 1024 * 1024;
/// The stored photo's longest side, in pixels. Smaller photos keep their size.
pub const LONG_SIDE: u32 = 1_600;
/// The thumbnail's longest side, in pixels.
pub const THUMBNAIL_LONG_SIDE: u32 = 400;
/// The longest side a photo may have before it is decoded, against decompression bombs.
const MAX_SIDE: u32 = 12_000;
/// The most memory one decode may allocate: a 48-megapixel photo as RGBA, with headroom.
const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;
/// aom speed 8 of 0-10: about 0.6 s per 1,600 px photo on one Raspberry Pi 5 core.
const SPEED: u8 = 8;
/// libavif quality 0-100: about 235 KB for a 1,600 px photo.
const QUALITY: u8 = 60;
const DIR_NAME: &str = "meal-photos";

#[derive(Debug, thiserror::Error)]
pub enum PhotoError {
    #[error("photo must be a base64-encoded JPEG or PNG image")]
    Undecodable,
    #[error("photo must be at most 10 MB and at most 12000 px on each side")]
    TooLarge,
    #[error("the photo could not be encoded")]
    Encode,
    #[error("the photo could not be stored")]
    Io(#[from] std::io::Error),
}

/// A received JPEG or PNG within [`MAX_PHOTO_BYTES`], not yet decoded.
pub struct Upload {
    bytes: Vec<u8>,
    format: ImageFormat,
}

impl Upload {
    /// Accepts JPEG or PNG bytes, recognised by their signature rather than a declared type.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, PhotoError> {
        if bytes.len() > MAX_PHOTO_BYTES {
            return Err(PhotoError::TooLarge);
        }
        let format = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            ImageFormat::Jpeg
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            ImageFormat::Png
        } else {
            return Err(PhotoError::Undecodable);
        };
        Ok(Self { bytes, format })
    }

    /// Accepts standard base64, padded or not, optionally as a `data:image/...;base64,` URL.
    pub fn from_base64(text: &str) -> Result<Self, PhotoError> {
        let text = text.trim();
        let text = match text.split_once(";base64,") {
            Some((prefix, data)) if prefix.starts_with("data:") => data,
            _ => text,
        };
        // Reject before decoding anything that cannot decode to at most MAX_PHOTO_BYTES.
        if text.len() > MAX_PHOTO_BYTES.div_ceil(3) * 4 {
            return Err(PhotoError::TooLarge);
        }
        let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(text.trim_end_matches('='))
            .map_err(|_| PhotoError::Undecodable)?;
        Self::from_bytes(bytes)
    }

    /// The SHA-256 of the uploaded bytes, so a retry can tell the same photo from another.
    #[must_use]
    pub fn sha256(&self) -> String {
        use std::fmt::Write;
        Sha256::digest(&self.bytes)
            .iter()
            .fold(String::with_capacity(64), |mut hex, byte| {
                let _ = write!(hex, "{byte:02x}");
                hex
            })
    }
}

/// Pixel dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// A photo re-encoded as AVIF with its thumbnail; nothing is stored yet.
pub struct Encoded {
    photo: Vec<u8>,
    thumbnail: Vec<u8>,
    pub size: Size,
    pub thumbnail_size: Size,
}

/// Which stored file of a photo to read.
#[derive(Clone, Copy, Debug)]
pub enum Variant {
    Photo,
    Thumbnail,
}

/// The stored AVIF photos and thumbnails in the data directory, named by photo ID.
#[derive(Clone)]
pub struct MealPhotos {
    dir: PathBuf,
    /// One permit: encodes run one at a time.
    encoder: Arc<Semaphore>,
}

/// Where photos are stored: a `meal-photos` directory beside `app.db`.
pub fn dir_beside(database_url: &str) -> anyhow::Result<PathBuf> {
    let options = SqliteConnectOptions::from_str(database_url)?;
    let file = options.get_filename();
    anyhow::ensure!(
        file != Path::new(":memory:") && !file.as_os_str().is_empty(),
        "DATABASE_URL must name a file so meal photos can be stored beside it"
    );
    Ok(file
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(DIR_NAME))
}

impl MealPhotos {
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            encoder: Arc::new(Semaphore::new(1)),
        }
    }

    /// Decodes, resizes and re-encodes `upload` on a blocking worker, waiting for any encode
    /// already running. The permit moves into the worker, so an abandoned request still holds it
    /// until its encode ends.
    pub async fn encode(&self, upload: Upload) -> Result<Encoded, PhotoError> {
        let permit = Arc::clone(&self.encoder)
            .acquire_owned()
            .await
            .map_err(|_| PhotoError::Encode)?;
        tokio::task::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let encoded = encode(&upload);
            drop(permit);
            if let Ok(encoded) = &encoded {
                tracing::info!(
                    elapsed_ms = started.elapsed().as_millis(),
                    width = encoded.size.width,
                    height = encoded.size.height,
                    photo_bytes = encoded.photo.len(),
                    thumbnail_bytes = encoded.thumbnail.len(),
                    "Encoded a meal photo"
                );
            }
            encoded
        })
        .await
        .map_err(|_| PhotoError::Encode)?
    }

    /// Stores `encoded` under a new photo ID. Either both files are written or neither is.
    pub async fn store(&self, encoded: &Encoded) -> Result<String, PhotoError> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let id = Uuid::new_v4().to_string();
        let written = async {
            write(&self.path(&id, Variant::Photo), &encoded.photo).await?;
            write(&self.path(&id, Variant::Thumbnail), &encoded.thumbnail).await
        }
        .await;
        if let Err(error) = written {
            self.remove(&id).await;
            return Err(error.into());
        }
        Ok(id)
    }

    /// Removes a photo's files. A failure leaves an orphaned file behind and is only logged:
    /// the Meal Log no longer references it.
    pub async fn remove(&self, id: &str) {
        for variant in [Variant::Photo, Variant::Thumbnail] {
            match tokio::fs::remove_file(self.path(id, variant)).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::warn!(photo_id = id, error = %error, "Could not remove a meal photo file");
                }
            }
        }
    }

    /// A stored AVIF file, or `None` when `id` names no stored photo.
    pub async fn read(&self, id: &str, variant: Variant) -> Result<Option<Vec<u8>>, PhotoError> {
        // Only a UUID can name a file, so no path can leave the photo directory.
        let Ok(id) = Uuid::parse_str(id) else {
            return Ok(None);
        };
        match tokio::fs::read(self.path(&id.to_string(), variant)).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn path(&self, id: &str, variant: Variant) -> PathBuf {
        self.dir.join(match variant {
            Variant::Photo => format!("{id}.avif"),
            Variant::Thumbnail => format!("{id}.thumbnail.avif"),
        })
    }
}

/// Writes through a temporary name, so a crash never leaves a partial photo under its real name.
async fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let partial = path.with_extension("partial");
    tokio::fs::write(&partial, bytes).await?;
    tokio::fs::rename(&partial, path).await
}

fn decode(upload: &Upload) -> Result<DynamicImage, PhotoError> {
    let classify = |error: ImageError| match error {
        ImageError::Limits(_) => PhotoError::TooLarge,
        _ => PhotoError::Undecodable,
    };
    let mut reader = ImageReader::with_format(Cursor::new(&upload.bytes), upload.format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(classify)?;
    // Metadata is about to be dropped, so turn the pixels the way the camera meant them first.
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).map_err(classify)?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// `size` scaled down so its long side is at most `long_side`, never up.
fn fit(size: Size, long_side: u32) -> Size {
    let longest = size.width.max(size.height);
    if longest <= long_side {
        return size;
    }
    // Rounded integer scaling; the result is at most `long_side`, so it fits a u32.
    let scale = |side: u32| {
        let scaled =
            (u64::from(side) * u64::from(long_side) + u64::from(longest) / 2) / u64::from(longest);
        u32::try_from(scaled).unwrap_or(long_side).max(1)
    };
    Size {
        width: scale(size.width),
        height: scale(size.height),
    }
}

fn resized(image: &RgbImage, long_side: u32) -> (RgbImage, Size) {
    let original = Size {
        width: image.width(),
        height: image.height(),
    };
    let size = fit(original, long_side);
    if size == original {
        return (image.clone(), size);
    }
    let resized = image::imageops::resize(image, size.width, size.height, FilterType::CatmullRom);
    (resized, size)
}

fn avif(image: &RgbImage) -> Result<Vec<u8>, PhotoError> {
    let pixels = libavif::RgbPixels::new(image.width(), image.height(), image.as_raw())
        .map_err(|_| PhotoError::Encode)?;
    let mut encoder = libavif::Encoder::new();
    encoder
        .set_speed(SPEED)
        .set_max_threads(1)
        .set_quality(QUALITY);
    let data = encoder
        .encode(&pixels.to_image(libavif::YuvFormat::Yuv420))
        .map_err(|_| PhotoError::Encode)?;
    Ok(data.to_vec())
}

fn encode(upload: &Upload) -> Result<Encoded, PhotoError> {
    // Transparency has no meaning in a meal photo; RGB drops alpha along with the metadata.
    let image = decode(upload)?.into_rgb8();
    let (photo, size) = resized(&image, LONG_SIDE);
    drop(image);
    let (thumbnail, thumbnail_size) = resized(&photo, THUMBNAIL_LONG_SIDE);
    Ok(Encoded {
        photo: avif(&photo)?,
        thumbnail: avif(&thumbnail)?,
        size,
        thumbnail_size,
    })
}
