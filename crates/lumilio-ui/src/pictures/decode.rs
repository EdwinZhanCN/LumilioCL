//! Reading and decoding one picture for [`super::PictureCache`].
//!
//! Adapted from GPUI's `ImageAssetLoader` and `decode_static_image`
//! (`crates/gpui/src/elements/img.rs`, `crates/gpui/src/platform.rs`,
//! <https://github.com/zed-industries/zed>, Apache-2.0): the same sources,
//! formats and orientation handling, but a picture is decoded no larger than
//! the cache's edge, and an animation past its byte limit keeps only its first
//! frame.

use std::io::Cursor;
use std::sync::Arc;

use futures_util::AsyncReadExt as _;
use gpui::http_client::{AsyncBody, HttpClient, anyhow};
use gpui::{AssetSource, ImageCacheError, RenderImage, Resource, SvgRenderer};
use image::codecs::gif::GifDecoder;
use image::codecs::webp::WebPDecoder;
use image::imageops::FilterType;
use image::{AnimationDecoder, DynamicImage, Frame, ImageDecoder, ImageFormat, RgbaImage};

/// How large a decoded picture may be.
#[derive(Clone, Copy, Debug)]
pub(super) struct Limits {
    /// Longest edge, in pixels, a picture is decoded at.
    pub edge: u32,
    /// Decoded bytes an animation may hold; past them only its first frame stays.
    pub animation: usize,
}

/// The bytes behind `resource`, from disk, the network or the binary.
pub(super) async fn read(
    resource: Resource,
    client: Arc<dyn HttpClient>,
    assets: Arc<dyn AssetSource>,
) -> Result<Vec<u8>, ImageCacheError> {
    match resource {
        Resource::Path(path) => Ok(std::fs::read(&*path)?),
        Resource::Uri(uri) => {
            let mut response = client
                .get(uri.as_ref(), AsyncBody::empty(), true)
                .await
                .map_err(|error| error.context(format!("loading picture from {uri:?}")))?;
            let mut body = Vec::new();
            response.body_mut().read_to_end(&mut body).await?;
            if !response.status().is_success() {
                let text = String::from_utf8_lossy(&body);
                let first_line = text.lines().next().unwrap_or("").trim_end().to_owned();
                return Err(ImageCacheError::BadStatus {
                    uri,
                    status: response.status(),
                    body: first_line,
                });
            }
            Ok(body)
        }
        Resource::Embedded(path) => match assets.load(&path).ok().flatten() {
            Some(data) => Ok(data.to_vec()),
            None => Err(ImageCacheError::Asset(
                format!("Embedded resource not found: {path}").into(),
            )),
        },
    }
}

/// `bytes` as a picture GPUI can draw: BGRA, no larger than `limits` allow.
/// Bytes no raster decoder recognises are tried as SVG, as GPUI does.
pub(super) fn decode(
    bytes: &[u8],
    limits: Limits,
    svg: &SvgRenderer,
) -> Result<Arc<RenderImage>, ImageCacheError> {
    let Ok(format) = image::guess_format(bytes) else {
        return Ok(svg.render_single_frame(bytes, 1.)?);
    };
    let frames = match format {
        ImageFormat::Gif => animation(GifDecoder::new(Cursor::new(bytes))?, limits)?,
        ImageFormat::WebP => {
            let mut decoder = WebPDecoder::new(Cursor::new(bytes))?;
            if decoder.has_animation() {
                let _ = decoder.set_background_color(image::Rgba([0, 0, 0, 0]));
                animation(decoder, limits)?
            } else {
                vec![still(decoder, limits)?]
            }
        }
        _ => {
            // A large PNG (a screenshot) is shrunk as it is read, so the
            // full-size picture is never held.
            let streamed = if format == ImageFormat::Png {
                lumilio_core::png_within(Cursor::new(bytes), limits.edge)
                    .map_err(|error| anyhow!("{error}"))?
            } else {
                None
            };
            match streamed {
                Some(small) => vec![Frame::new(bgra(small))],
                None => {
                    let decoder = image::ImageReader::with_format(Cursor::new(bytes), format)
                        .into_decoder()?;
                    vec![still(decoder, limits)?]
                }
            }
        }
    };
    Ok(Arc::new(RenderImage::new(frames)))
}

fn still(mut decoder: impl ImageDecoder, limits: Limits) -> Result<Frame, ImageCacheError> {
    let orientation = decoder.orientation()?;
    let mut picture = DynamicImage::from_decoder(decoder)?;
    picture.apply_orientation(orientation);
    Ok(Frame::new(bgra(fit(picture, limits.edge))))
}

/// The frames of an animation, each fitted to the edge. A frame that fails
/// to decode is skipped, as GPUI does.
fn animation<'a>(
    decoder: impl AnimationDecoder<'a>,
    limits: Limits,
) -> Result<Vec<Frame>, ImageCacheError> {
    let (mut frames, mut held) = (Vec::new(), 0);
    for frame in decoder.into_frames().flatten() {
        let delay = frame.delay();
        let buffer = bgra(fit(
            DynamicImage::ImageRgba8(frame.into_buffer()),
            limits.edge,
        ));
        held += buffer.len();
        frames.push(Frame::from_parts(buffer, 0, 0, delay));
        if held > limits.animation {
            frames.truncate(1);
            break;
        }
    }
    if frames.is_empty() {
        return Err(anyhow!("no frame of the animation could be decoded").into());
    }
    Ok(frames)
}

fn fit(picture: DynamicImage, edge: u32) -> RgbaImage {
    let (width, height) = lumilio_core::fit_within(picture.width(), picture.height(), edge);
    if (width, height) == (picture.width(), picture.height()) {
        return picture.into_rgba8();
    }
    picture
        .resize_exact(width, height, FilterType::Triangle)
        .into_rgba8()
}

/// GPUI's atlas takes blue first.
fn bgra(mut picture: RgbaImage) -> RgbaImage {
    for pixel in picture.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    picture
}
