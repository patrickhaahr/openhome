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

pub const MAX_SIDE: u32 = 12_000;
const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub speed: u8,
    pub quality: u8,
}

pub struct Encoded {
    pub bytes: Vec<u8>,
    pub size: Size,
}

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

fn fit(size: Size, long_side: u32) -> Size {
    let longest = size.width.max(size.height);
    if longest <= long_side {
        return size;
    }
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
