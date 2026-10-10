# 0044 — Themes, fonts, interface scale and the Home wallpaper

- Status: accepted
- Date: 2026-10-10

## Context

The launcher had two fixed looks (aluminium and night) and one font setup. People wanted other themes, their own fonts and size, and their own picture behind Home, without the launcher losing its instrument character or its legibility.

## Decision

- **One theme structure.** A theme is a JSON file that holds several `themes`, each with a `name`, an `appearance` (`light` or `dark`) and any of: the semantic colours of `Body`, two corner radii, font families, a `window_background`. Colours are `#rrggbb` or `#rrggbbaa`, so a theme may carry opacity. The shape follows Zed's theme files. What a theme leaves out is filled from the built-in body of the same appearance and reported. The built-in aluminium and night are written through the same structure.
- **A theme changes values, not controls.** Controls still come from `lumilio-ui` (`kit`, `controls`, `key`) and read colours through `Body::current` / `ShellColors::current`. Shapes, key travel and shadow construction are not themeable. A theme cannot turn off the focus ring or motion.
- **Only our defaults are gated.** The 4.5:1 and 3:1 contrast tests apply to aluminium and night. Other bundled themes keep their authors' palettes and local themes are not checked. A file is refused only when it cannot be parsed; one bad theme does not hide the others in its file.
- **Choice is by name.** `LookPreferences` stores a theme name per appearance, optional font families and a scale. A name that no longer resolves shows the built-in theme.
- **Fonts.** Interface, monospace and Chinese families can be chosen from the installed ones; a family that is not installed falls back to the launcher's own and the Appearance tab says so for the Chinese one. The Chinese family is applied as a text fallback for every window through `Theme::font_fallbacks`, a field added to the `forks/gpui-component` theme.
- **Interface scale.** 85–130 %, applied by scaling the theme's base font size, which the root turns into the rem size. Text and rem-based spacing scale together. Pixel text sizes were converted to `theme::font_px` (rems). Display digits keep their pixel size.
- **Wallpaper.** A separate preference, not part of a theme. Choosing copies the picture, scaled to at most 2560×1600 and re-encoded, into the data directory's `themes` folder under a content-derived name. Home shows it crisp under two shades and dissolves its bottom into the page with the same ordered dither as the world. The world's scenes and copy are not shown while a picture is set.
- **Window material.** `window_background` (`opaque`, `transparent`, `blurred`) is set on the window with each appearance sync. Only a theme can ask for it.
- **Bundled themes.** Catppuccin (Latte, Mocha) and Rosé Pine (Dawn, main), MIT, with source and licence in each theme's `source`.

## Consequences

- Positive: new looks are data. A person can add a theme by dropping a file in `themes/`.
- Negative / trade-offs: a third-party theme can be hard to read and the launcher will not stop it. The monospace family is held in process-wide state (`theme::mono_font`) because many call sites have no context. The scale also scales rem-based spacing, not text alone.
- Follow-ups: theme density (page spacing is written in rems and has no theme hook); Gruvbox and Tokyo Night; Windows 11 Mica; the `window_background` key name was not checked against Zed's documentation.

Shipped: `crates/lumilio-ui/src/theme/` (`spec.rs`, `look.rs`), `pages/settings/appearance.rs`, `hero/wallpaper.rs`, `crates/lumilio-core/src/wallpaper/`, `LookPreferences` in `tuning.rs`, bundled themes in `crates/lumilio-ui/themes/`.
