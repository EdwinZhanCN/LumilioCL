//! Semantic tokens: the instrument's two bodies (design language §12), brand
//! type (§13), and motion (§2). Themes, fonts and the interface scale can replace them.

mod look;
mod spec;
#[cfg(test)]
mod tests;

pub use look::{
    ALUMINIUM, BIRCH, Catalog, Choice, DEEPSLATE, Entry, GLASS, Look, NIGHT, OWN, Origin, Rejected,
    TINTED_GLASS, Wallpaper, scan_dir,
};
pub use spec::{
    Color, Fonts, Palette, Resolved, ThemeSpec, Tone, WindowBackground, can_blur, parse_file,
};
pub use tokens::{
    BASE_FONT_SIZE, BASE_MONO_FONT_SIZE, BOTTOM_SAFE_AREA, Body, CONTENT_MAX_WIDTH,
    CONTENT_PADDING_X, HERO_FADE, HERO_MIN_HEIGHT, HERO_SHARE, KEY_TRAVEL, LCD_FONT, MONO_FONT,
    NAV_HEIGHT, NAV_OFFSET, NAV_VEIL, PIXEL_FONT, SANS_FONT, ShellColors, clickable,
    content_column, contrast, dialog, display_shadow, font_px, glow, key_shadow, legible,
    mono_font, nudge, panel_shadow, pressed_shadow, recess_shadow, tune, world_height,
};

/// Motion tokens from `docs/design-language.md` §2. UI code uses these instead
/// of literal durations.
pub mod motion;

mod tokens;
