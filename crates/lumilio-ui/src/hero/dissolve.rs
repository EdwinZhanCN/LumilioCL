//! How pixel art meets the page it sits on: its edges break up texel by texel
//! into the page colour, so art and page meet without a seam (design
//! language §5, §7). Home's world and the Instance page's cover both dissolve
//! through [`PageDissolve`].
//!
//! On a translucent page the dissolved texels are left out instead of
//! painted, so the art breaks up into the glass behind it rather than into a
//! solid band of the page colour (plan translucent-themes, D6).

use gpui::Hsla;

use super::raster::{PixelGrid, Rect, Rgb};

/// The page pixel art dissolves into, and the edges of the art that do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageDissolve {
    /// The page colour without its opacity: what the band dissolves into.
    pub page: Rgb,
    /// The page lets the window's backdrop through.
    pub translucent: bool,
    /// Rows at the bottom that dissolve; the last one is the page.
    pub bottom_rows: usize,
    /// Columns at the left that dissolve; the first one is the page.
    pub left_columns: usize,
}

impl PageDissolve {
    /// Into an opaque page of this colour, no edge dissolving yet.
    pub const fn solid(page: Rgb) -> Self {
        Self {
            page,
            translucent: false,
            bottom_rows: 0,
            left_columns: 0,
        }
    }

    /// Into the live page colour, which may be translucent.
    pub fn of(page: Hsla) -> Self {
        let rgb = page.to_rgb();
        Self {
            translucent: page.a < 1.,
            ..Self::solid(Rgb::new(rgb.r, rgb.g, rgb.b))
        }
    }

    /// The last `rows` dissolve.
    pub const fn bottom(self, rows: usize) -> Self {
        Self {
            bottom_rows: rows,
            ..self
        }
    }

    /// The first `columns` dissolve as well.
    pub const fn left(self, columns: usize) -> Self {
        Self {
            left_columns: columns,
            ..self
        }
    }

    /// Dissolves the edges of `grid` into the page.
    pub fn apply(self, grid: &mut PixelGrid) {
        grid.fade_bottom(self.bottom_rows, self.page);
        grid.fade_left(self.left_columns, self.page);
    }

    /// Whether the texel at `x`, `y` of a grid `height` rows tall, packed as
    /// `color`, is left out: on a translucent page, a texel of a band that
    /// dissolved into the page itself.
    pub fn clears(self, x: usize, y: usize, height: usize, color: u32) -> bool {
        self.translucent
            && color == self.page.pack()
            && (y >= height.saturating_sub(self.bottom_rows) || x < self.left_columns)
    }

    /// The rects of `grid` to paint: all of them, less those whose first
    /// texel is left out.
    pub fn painted(self, grid: &PixelGrid) -> Vec<Rect> {
        let height = grid.height();
        grid.rects()
            .into_iter()
            .filter(|rect| !self.clears(rect.start, rect.row, height, rect.color))
            .collect()
    }
}

#[cfg(test)]
mod tests;
