//! A shared AVIF encoder: JPEG or PNG bytes in, AVIF bytes and dimensions out, for features that
//! store images (`meal_photos` today). It owns the decoding, colour handling and encoding, never
//! the storage, sizes, quality or error wording of its callers.
//!
//! A source is decoded within [`MAX_SIDE`] and [`MAX_DECODE_BYTES`], turned upright from its EXIF
//! orientation and, when it carries an RGB or gray ICC profile, converted to sRGB (untagged
//! sources are treated as sRGB). Re-encoding from pixels drops every piece of metadata and the
//! alpha channel. Encoding uses libavif's aom encoder on one thread, on a blocking worker outside
//! the async runtime, and one encode at a time across the whole process, so the other services
//! on the host keep their CPU.

use std::io::Cursor;
use std::sync::{Arc, LazyLock};

use image::{
    DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits, RgbImage,
    imageops::FilterType, metadata::Orientation,
};
use moxcms::{ColorProfile, DataColorSpace, Layout, TransformOptions};
use serde::Serialize;
use tokio::sync::Semaphore;

#[expect(
    unsafe_code,
    reason = "libavif's safe wrapper cannot set colour information; this module owns and checks native allocations"
)]
mod native;

/// The longest side a source may have before it is decoded, against decompression bombs.
pub const MAX_SIDE: u32 = 12_000;
/// The most memory one decode may allocate: a 48-megapixel image as RGBA, with headroom.
const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;

/// One permit for the whole process: a Raspberry Pi has no CPU to spare for parallel encodes.
static ENCODES: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(1)));

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("the image must be a JPEG or PNG")]
    Undecodable,
    #[error("the image exceeds the decode limits")]
    TooLarge,
    #[error("the image could not be encoded")]
    Encode,
}

/// JPEG or PNG bytes, recognised by their signature rather than a declared type, not yet decoded.
pub struct Source {
    bytes: Vec<u8>,
    format: ImageFormat,
}

impl Source {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, EncodeError> {
        let format = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            ImageFormat::Jpeg
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            ImageFormat::Png
        } else {
            return Err(EncodeError::Undecodable);
        };
        Ok(Self { bytes, format })
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Pixel dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// The caller's choice of encoder effort and fidelity.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// aom speed, 0 (slowest) to 10.
    pub speed: u8,
    /// libavif quality, 0 to 100.
    pub quality: u8,
}

/// An AVIF image and its dimensions.
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub size: Size,
}

/// Decodes `source` once, then encodes it at each of `long_sides` in turn, each at most that many
/// pixels on its long side and never scaled up. Every size is resized from the one before it,
/// so list them from largest to smallest. Waits for any encode already running. The permit moves
/// into the blocking worker, so an abandoned caller still holds it until its encode ends.
pub async fn encode<const N: usize>(
    source: Source,
    settings: Settings,
    long_sides: [u32; N],
) -> Result<[Encoded; N], EncodeError> {
    let permit = Arc::clone(&ENCODES)
        .acquire_owned()
        .await
        .map_err(|_| EncodeError::Encode)?;
    tokio::task::spawn_blocking(move || {
        let started = std::time::Instant::now();
        let encoded = encode_blocking(&source, settings, long_sides);
        drop(permit);
        if let Ok(encoded) = &encoded {
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis(),
                bytes = encoded.iter().map(|image| image.bytes.len()).sum::<usize>(),
                "Encoded an AVIF image"
            );
        }
        encoded
    })
    .await
    .map_err(|_| EncodeError::Encode)?
}

fn encode_blocking<const N: usize>(
    source: &Source,
    settings: Settings,
    long_sides: [u32; N],
) -> Result<[Encoded; N], EncodeError> {
    let mut image = decode(source)?;
    let mut encoded = Vec::with_capacity(N);
    for long_side in long_sides {
        let (resized, size) = resized(&image, long_side);
        image = resized;
        encoded.push(Encoded {
            bytes: native::encode(&image, settings.speed, settings.quality)?,
            size,
        });
    }
    encoded.try_into().map_err(|_| EncodeError::Encode)
}

fn decode(source: &Source) -> Result<RgbImage, EncodeError> {
    let classify = |error: ImageError| match error {
        ImageError::Limits(_) => EncodeError::TooLarge,
        _ => EncodeError::Undecodable,
    };
    let mut reader = ImageReader::with_format(Cursor::new(&source.bytes), source.format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(classify)?;
    // Metadata is about to be dropped, so turn the pixels the way the camera meant them first.
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let profile = decoder.icc_profile().map_err(classify)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(classify)?;
    image.apply_orientation(orientation);
    let (width, height) = (image.width(), image.height());
    let Some(profile) = profile else {
        return Ok(image.into_rgb8());
    };
    let profile = ColorProfile::new_from_slice(&profile).map_err(|_| EncodeError::Undecodable)?;
    let (pixels, layout) = match profile.color_space {
        DataColorSpace::Rgb => (image.into_rgb8().into_raw(), Layout::Rgb),
        DataColorSpace::Gray => (image.into_luma8().into_raw(), Layout::Gray),
        // The JPEG decoder has already converted CMYK pixels to RGB; applying the original
        // CMYK profile to them would be wrong. Refuse it rather than mislabel its colours.
        _ => return Err(EncodeError::Undecodable),
    };
    let transform = profile
        .create_transform_8bit(
            layout,
            &ColorProfile::new_srgb(),
            Layout::Rgb,
            TransformOptions::default(),
        )
        .map_err(|_| EncodeError::Undecodable)?;
    let mut converted = vec![0; pixels.len() / layout.channels() * 3];
    transform
        .transform(&pixels, &mut converted)
        .map_err(|_| EncodeError::Undecodable)?;
    RgbImage::from_raw(width, height, converted).ok_or(EncodeError::Undecodable)
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
