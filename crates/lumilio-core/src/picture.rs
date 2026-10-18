//! Pictures decoded no larger than they are shown.
//!
//! A screenshot is often 4K or more: decoded whole it is tens of megabytes,
//! and the system allocator keeps a block that size for reuse long after the
//! picture was shrunk. A PNG here is read a row at a time and averaged straight
//! into the size it is shown at, so the full-size picture never exists.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{BufRead, Seek};

use image::RgbaImage;
use png::{BitDepth, ColorType, Decoder, Transformations};

/// The size a `width`×`height` picture is shown at: unchanged when its longest
/// edge fits `edge`, otherwise scaled down to it, keeping its shape.
#[must_use]
pub fn fit_within(width: u32, height: u32, edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= edge {
        return (width, height);
    }
    let scale = f64::from(edge) / f64::from(longest);
    let side = |length: u32| ((f64::from(length) * scale).round() as u32).max(1);
    (side(width), side(height))
}

/// A PNG that could not be read.
#[derive(Debug)]
pub struct PictureError(png::DecodingError);

impl Display for PictureError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for PictureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

/// A PNG decoded row by row straight into the size that fits `edge`, as RGBA.
///
/// `Ok(None)` when it is better decoded the usual way: it already fits, its
/// rows arrive out of order (interlaced), or it carries an orientation to
/// apply.
pub fn png_within(
    reader: impl BufRead + Seek,
    edge: u32,
) -> Result<Option<RgbaImage>, PictureError> {
    let mut decoder = Decoder::new(reader);
    decoder.set_transformations(Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(PictureError)?;
    let info = reader.info();
    let from = (info.width, info.height);
    let to = fit_within(from.0, from.1, edge);
    if to == from || info.interlaced || info.exif_metadata.is_some() {
        return Ok(None);
    }
    let channels = match reader.output_color_type() {
        (ColorType::Grayscale, BitDepth::Eight) => 1,
        (ColorType::GrayscaleAlpha, BitDepth::Eight) => 2,
        (ColorType::Rgb, BitDepth::Eight) => 3,
        (ColorType::Rgba, BitDepth::Eight) => 4,
        _ => return Ok(None),
    };
    let mut shrink = Shrink::new(from, to);
    while let Some(row) = reader.next_row().map_err(PictureError)? {
        shrink.row(row.data(), channels);
    }
    Ok(Some(shrink.finish()))
}

/// Area averaging from one size down to a smaller one, a source row at a time.
///
/// Along an axis, source pixel `x` spans `[x·to, (x+1)·to)` and output pixel
/// `i` spans `[i·from, (i+1)·from)`; the overlap is a whole number, so each
/// output pixel is an exact weighted mean. Colour is averaged premultiplied by
/// alpha, so a transparent pixel does not tint its neighbours.
struct Shrink {
    from: (u32, u32),
    to: (u32, u32),
    /// Per source column: the first output column it covers, its share of
    /// that column, and its share of the next.
    columns: Vec<(usize, u64, u64)>,
    /// The current source row, averaged across into output columns.
    across: Vec<[u64; 4]>,
    /// The output row being filled and the one after it.
    rows: [Vec<[u64; 4]>; 2],
    /// Which output row `rows[0]` is.
    row: u32,
    /// Source rows read so far.
    read: u32,
    out: RgbaImage,
}

/// Where source pixel `at` falls among output pixels: the first one it
/// covers, its share of that one, and its share of the next.
fn split(at: u32, from: u32, to: u32) -> (usize, u64, u64) {
    let (at, from, to) = (u64::from(at), u64::from(from), u64::from(to));
    let (start, end) = (at * to, (at + 1) * to);
    let first = start / from;
    let boundary = (first + 1) * from;
    if end <= boundary {
        (first as usize, to, 0)
    } else {
        (first as usize, boundary - start, end - boundary)
    }
}

impl Shrink {
    fn new(from: (u32, u32), to: (u32, u32)) -> Self {
        let width = to.0 as usize;
        Self {
            from,
            to,
            columns: (0..from.0).map(|x| split(x, from.0, to.0)).collect(),
            across: vec![[0; 4]; width],
            rows: [vec![[0; 4]; width], vec![[0; 4]; width]],
            row: 0,
            read: 0,
            out: RgbaImage::new(to.0, to.1),
        }
    }

    fn row(&mut self, data: &[u8], channels: usize) {
        if self.read >= self.from.1 {
            return;
        }
        self.across.fill([0; 4]);
        for (pixel, &(first, share, next)) in data.chunks_exact(channels).zip(&self.columns) {
            let [r, g, b, a] = match *pixel {
                [v] => [v, v, v, 255],
                [v, a] => [v, v, v, a],
                [r, g, b] => [r, g, b, 255],
                [r, g, b, a] => [r, g, b, a],
                _ => [0; 4],
            }
            .map(u64::from);
            let weighted = [r * a, g * a, b * a, a];
            add(&mut self.across[first], weighted, share);
            if next > 0 {
                add(&mut self.across[first + 1], weighted, next);
            }
        }
        let (first, share, next) = split(self.read, self.from.1, self.to.1);
        if first as u32 > self.row {
            self.emit();
        }
        for (target, &value) in self.rows[0].iter_mut().zip(&self.across) {
            add(target, value, share);
        }
        if next > 0 {
            for (target, &value) in self.rows[1].iter_mut().zip(&self.across) {
                add(target, value, next);
            }
        }
        self.read += 1;
    }

    /// Writes the finished output row and starts on the next.
    fn emit(&mut self) {
        if self.row < self.to.1 {
            let area = u64::from(self.from.0) * u64::from(self.from.1);
            let width = self.to.0 as usize;
            let start = self.row as usize * width * 4;
            let pixels: &mut [u8] = &mut self.out;
            let line = &mut pixels[start..start + width * 4];
            for (pixel, sum) in line.as_chunks_mut::<4>().0.iter_mut().zip(&self.rows[0]) {
                let alpha = sum[3];
                // Fully transparent: no colour to speak of.
                let colour = |value: u64| {
                    (value + alpha / 2)
                        .checked_div(alpha)
                        .map_or(0, |value| value.min(255) as u8)
                };
                pixel.copy_from_slice(&[
                    colour(sum[0]),
                    colour(sum[1]),
                    colour(sum[2]),
                    ((alpha + area / 2) / area).min(255) as u8,
                ]);
            }
        }
        self.rows.swap(0, 1);
        self.rows[1].fill([0; 4]);
        self.row += 1;
    }

    fn finish(mut self) -> RgbaImage {
        while self.row < self.to.1 {
            self.emit();
        }
        self.out
    }
}

fn add(target: &mut [u64; 4], value: [u64; 4], share: u64) {
    for (target, value) in target.iter_mut().zip(value) {
        *target += value * share;
    }
}

#[cfg(test)]
mod tests;
