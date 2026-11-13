//! Encode normalized sRGB pixels with matching CICP, set before RGB-to-YUV conversion.
//! libavif's safe wrapper does not expose these colour fields. Native resources remain local
//! and are released on every error path; no pointers escape this module.

use super::EncodeError;
use image::RgbImage;
use libavif_sys as avif;
use std::ptr::NonNull;

struct Image(NonNull<avif::avifImage>);
struct Encoder(NonNull<avif::avifEncoder>);
#[derive(Default)]
struct Data(avif::avifRWData);

impl Drop for Image {
    fn drop(&mut self) {
        // SAFETY: the pointer came from avifImageCreate, is uniquely owned and not yet freed.
        unsafe {
            avif::avifImageDestroy(self.0.as_ptr());
        }
    }
}
impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: the pointer came from avifEncoderCreate, is uniquely owned and not yet freed.
        unsafe {
            avif::avifEncoderDestroy(self.0.as_ptr());
        }
    }
}
impl Drop for Data {
    fn drop(&mut self) {
        // SAFETY: initialized empty or written by avifEncoderWrite, freed exactly once.
        unsafe {
            avif::avifRWDataFree(&raw mut self.0);
        }
    }
}

pub(super) fn encode(pixels: &RgbImage, speed: u8, quality: u8) -> Result<Vec<u8>, EncodeError> {
    // SAFETY: ImageBuffer guarantees contiguous width*height RGB bytes. Dimensions and row
    // bytes are checked, borrowed pixels outlive the synchronous conversion, and libavif only
    // reads RGB input. All native allocations are checked and wrapped before fallible calls.
    unsafe {
        let image = Image(
            NonNull::new(avif::avifImageCreate(
                pixels.width(),
                pixels.height(),
                8,
                avif::AVIF_PIXEL_FORMAT_YUV420,
            ))
            .ok_or(EncodeError::Encode)?,
        );
        let raw = &mut *image.0.as_ptr();
        raw.colorPrimaries = avif::AVIF_COLOR_PRIMARIES_BT709 as u16;
        raw.transferCharacteristics = avif::AVIF_TRANSFER_CHARACTERISTICS_SRGB as u16;
        raw.matrixCoefficients = avif::AVIF_MATRIX_COEFFICIENTS_BT601 as u16;
        raw.yuvRange = avif::AVIF_RANGE_FULL;
        let mut rgb = avif::avifRGBImage::default();
        avif::avifRGBImageSetDefaults(&raw mut rgb, raw);
        rgb.format = avif::AVIF_RGB_FORMAT_RGB;
        rgb.rowBytes = pixels.width().checked_mul(3).ok_or(EncodeError::Encode)?;
        // The C API takes a mutable pointer even for the read-only RGB-to-YUV input.
        rgb.pixels = pixels.as_raw().as_ptr().cast_mut();
        if avif::avifImageRGBToYUV(raw, &raw const rgb) != avif::AVIF_RESULT_OK {
            return Err(EncodeError::Encode);
        }
        let encoder = Encoder(NonNull::new(avif::avifEncoderCreate()).ok_or(EncodeError::Encode)?);
        let raw_encoder = &mut *encoder.0.as_ptr();
        raw_encoder.codecChoice = avif::AVIF_CODEC_CHOICE_AOM;
        raw_encoder.maxThreads = 1;
        raw_encoder.speed = i32::from(speed);
        raw_encoder.quality = i32::from(quality);
        let mut data = Data::default();
        if avif::avifEncoderWrite(raw_encoder, raw, &raw mut data.0) != avif::AVIF_RESULT_OK
            || data.0.data.is_null()
            || data.0.size == 0
        {
            return Err(EncodeError::Encode);
        }
        // The encoder owns a valid allocation of size bytes until Data is dropped.
        Ok(std::slice::from_raw_parts(data.0.data, data.0.size).to_vec())
    }
}
