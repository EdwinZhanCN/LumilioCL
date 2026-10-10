//! Turning core pixels into GPUI images.
//!
//! GPUI's sprite atlas wants BGRA bytes, while core hands out straight RGBA
//! (`lumilio_core::SkinPixels`). The one place that knows both orders lives
//! here, so callers do not repeat the swap.

use gpui::RenderImage;
use std::sync::Arc;

/// A GPUI image from straight RGBA pixels, or `None` when the byte count does
/// not fill `width × height`.
#[must_use]
pub fn image(width: u32, height: u32, rgba: &[u8]) -> Option<Arc<RenderImage>> {
    if width == 0 || height == 0 {
        return None;
    }
    let mut bytes = rgba.to_vec();
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let buffer = image::RgbaImage::from_raw(width, height, bytes)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}
