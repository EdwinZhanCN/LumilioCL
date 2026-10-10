//! Semantic tokens: the instrument's two bodies (design language §12), brand
//! type (§13), and motion (§2). Themes, fonts and the interface scale that
//! replace them are described in ADR 0044.

mod look;
mod spec;
#[cfg(test)]
mod tests;

pub use look::{
    ALUMINIUM, Catalog, Choice, Entry, Look, NIGHT, Origin, Rejected, Wallpaper, scan_dir,
};
pub use spec::{Color, Fonts, Palette, Resolved, ThemeSpec, Tone, WindowBackground, parse_file};

use gpui::{BoxShadow, Hsla, Pixels, px};
use gpui_component::Theme;

pub const NAV_HEIGHT: Pixels = px(56.);
pub const NAV_OFFSET: Pixels = px(22.);
pub const BOTTOM_SAFE_AREA: Pixels = px(96.);
/// Height of the page-coloured veil under the capsule, so content passing
/// behind it fades out instead of colliding with the icons.
pub const NAV_VEIL: Pixels = px(120.);

/// One content column shared by hero copy, overlays, and page content, so
/// text over art and text on the page line up (design language §5).
pub const CONTENT_MAX_WIDTH: Pixels = px(1120.);
pub const CONTENT_PADDING_X: Pixels = px(40.);
/// Height of the dithered band where the Home world dissolves into the page.
pub const HERO_FADE: Pixels = px(88.);
/// The Home world's share of the window height, and its floor.
pub const HERO_SHARE: f32 = 0.56;
pub const HERO_MIN_HEIGHT: Pixels = px(330.);

/// Every enabled, clickable control shows the pointing hand; disabled ones
/// keep the default arrow (design language §9). gpui-component buttons default
/// to the arrow and apply instance styles last, so this overrides them.
pub fn clickable<E: gpui::Styled>(element: E, enabled: bool) -> E {
    if enabled {
        element.cursor_pointer()
    } else {
        element
    }
}

/// The shared content column (see [`CONTENT_MAX_WIDTH`]).
pub fn content_column() -> gpui::Div {
    use gpui::{Styled as _, div};
    div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(CONTENT_MAX_WIDTH)
        .px(CONTENT_PADDING_X)
}

/// The interface's Latin and figure face (design language §13).
pub const SANS_FONT: &str = "Space Grotesk";
/// Values: versions, sizes, counts, tags, section indices.
pub const MONO_FONT: &str = "JetBrains Mono";
/// Segment digits on a display, nowhere else.
pub const LCD_FONT: &str = "DSEG7 Classic";

/// The monospace family in force, for the many places that name it without a
/// context at hand. [`tune`] sets it with the rest of the look, so it is
/// process-wide state: one look is shown at a time.
static MONO_FAMILY: std::sync::RwLock<Option<gpui::SharedString>> = std::sync::RwLock::new(None);

/// The monospace family the launcher draws values and logs in now.
#[must_use]
pub fn mono_font() -> gpui::SharedString {
    MONO_FAMILY
        .read()
        .ok()
        .and_then(|family| family.clone())
        .unwrap_or_else(|| MONO_FONT.into())
}

/// A text size written in pixels at 100 %, as a rem, so it follows the
/// interface scale like the stock text sizes do. Display digits are not sized
/// with this: they keep their pixel size so they cannot outgrow the window.
#[must_use]
pub fn font_px(size: f32) -> gpui::Rems {
    gpui::rems(size / BASE_FONT_SIZE)
}

/// The interface's body text size at 100 %, which is also the rem.
pub const BASE_FONT_SIZE: f32 = 16.;
pub const BASE_MONO_FONT_SIZE: f32 = 13.;

fn hex(value: u32) -> Hsla {
    gpui::rgb(value).into()
}

/// The instrument's colours for one body (design language §12): a neutral
/// body, three key colours and one signal orange. Light is aluminium, dark is
/// night.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub dark: bool,
    pub page: Hsla,
    pub panel: Hsla,
    pub hairline: Hsla,
    pub ink: Hsla,
    pub muted: Hsla,
    pub key_white: Hsla,
    pub key_black: Hsla,
    pub key_grey: Hsla,
    pub on_white: Hsla,
    pub on_black: Hsla,
    /// Orange that fills: keys, the open tab, tags, lit LEDs.
    pub orange: Hsla,
    /// Orange read as text; on aluminium the fill is too bright for that.
    pub orange_text: Hsla,
    pub on_orange: Hsla,
    /// The destructive confirm key; white text on it is 5.6:1.
    pub danger: Hsla,
    /// Danger read as text or a failure LED.
    pub danger_text: Hsla,
    pub led_off: Hsla,
    /// A green LED: success.
    pub ok: Hsla,
    pub display: Hsla,
    pub display_ink: Hsla,
    pub display_dim: Hsla,
    pub display_label: Hsla,
}

impl Body {
    pub fn of(dark: bool) -> Self {
        if dark {
            Self {
                dark,
                page: hex(0x111111),
                panel: hex(0x1b1b1b),
                hairline: hex(0x353533),
                ink: hex(0xecebe7),
                muted: hex(0x8c8b86),
                key_white: hex(0x313130),
                key_black: hex(0x2a2a2a),
                key_grey: hex(0x262626),
                on_white: hex(0xecebe7),
                on_black: hex(0xecebe7),
                orange: hex(0xff5a1a),
                orange_text: hex(0xff5a1a),
                on_orange: hex(0xffffff),
                danger: hex(0xc8261b),
                danger_text: hex(0xff5a4f),
                led_off: hex(0x3a3a38),
                ok: hex(0x5fd068),
                display: hex(0x070707),
                display_ink: hex(0xff6a2a),
                display_dim: hex(0x3a2418),
                display_label: hex(0x948479),
            }
        } else {
            Self {
                dark,
                page: hex(0xe4e4e1),
                panel: hex(0xeeeeeb),
                hairline: hex(0xbcbcb7),
                ink: hex(0x121212),
                muted: hex(0x666662),
                key_white: hex(0xf9f9f7),
                key_black: hex(0x1d1d1d),
                key_grey: hex(0xd3d3cf),
                on_white: hex(0x121212),
                on_black: hex(0xf2f2ef),
                orange: hex(0xf0520f),
                orange_text: hex(0xb53c08),
                on_orange: hex(0xffffff),
                danger: hex(0xc8261b),
                danger_text: hex(0xb42318),
                led_off: hex(0xa3a39e),
                ok: hex(0x2f9a3c),
                display: hex(0x141414),
                display_ink: hex(0xff6a2a),
                display_dim: hex(0x3d2619),
                display_label: hex(0x948479),
            }
        }
    }

    /// The body the launcher is drawn with now: the chosen theme's colours,
    /// or the built-in body of the component theme's mode before a look has
    /// been applied.
    pub fn current(cx: &gpui::App) -> Self {
        match cx.try_global::<Look>() {
            Some(look) => look.theme.body,
            None => Self::of(Theme::global(cx).mode.is_dark()),
        }
    }
}

/// A slightly different shade for a hovered key: lighter, or darker on
/// near-white.
pub fn nudge(color: Hsla) -> Hsla {
    let l = if color.l > 0.9 {
        color.l - 0.03
    } else {
        (color.l + 0.05).min(1.)
    };
    Hsla { l, ..color }
}

/// How much a pressed key travels down.
pub const KEY_TRAVEL: Pixels = px(1.);

fn drop_shadow(color: Hsla, y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: gpui::point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}

fn inner_shadow(color: Hsla, y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        inset: true,
        ..drop_shadow(color, y, blur)
    }
}

/// A key at rest: a hard contact shadow, a soft cast shadow and a lit top edge.
pub fn key_shadow(body: Body) -> Vec<BoxShadow> {
    let black = |alpha| gpui::hsla(0., 0., 0., alpha);
    let white = |alpha| gpui::hsla(0., 0., 1., alpha);
    if body.dark {
        vec![
            drop_shadow(black(0.7), 1., 0.),
            drop_shadow(black(0.45), 3., 6.),
            inner_shadow(white(0.16), 1., 0.),
        ]
    } else {
        vec![
            drop_shadow(black(0.22), 1., 0.),
            drop_shadow(black(0.12), 3., 6.),
            inner_shadow(white(0.7), 1., 0.),
        ]
    }
}

/// A key held down: the cast shadow is gone and the face sits in shade.
pub fn pressed_shadow() -> Vec<BoxShadow> {
    vec![inner_shadow(gpui::hsla(0., 0., 0., 0.22), 1., 2.)]
}

/// A recessed socket or field: a faint shade along the top edge.
pub fn recess_shadow(alpha: f32) -> Vec<BoxShadow> {
    vec![inner_shadow(gpui::hsla(0., 0., 0., alpha), 1., 2.)]
}

/// A display window's inner shadow.
pub fn display_shadow() -> Vec<BoxShadow> {
    vec![inner_shadow(gpui::hsla(0., 0., 0., 0.6), 1., 3.)]
}

/// A faceplate's soft shadow.
pub fn panel_shadow(body: Body) -> Vec<BoxShadow> {
    vec![drop_shadow(
        gpui::hsla(0., 0., 0., if body.dark { 0.5 } else { 0.1 }),
        4.,
        12.,
    )]
}

/// The halo of a lit LED.
pub fn glow(color: Hsla) -> Vec<BoxShadow> {
    vec![BoxShadow {
        spread_radius: px(1.),
        ..drop_shadow(color.opacity(0.75), 0., 6.)
    }]
}

/// Adjusts the component theme after every appearance change: the instrument
/// colours (design language §12), brand type (§13), and the dialog overlay.
/// Stock dialogs, menus, popovers, toasts and inputs follow these tokens; only
/// their shapes stay stock.
pub fn tune(cx: &mut gpui::App) {
    let tone = Tone::of(Theme::global(cx).mode.is_dark());
    let look = Look::resolve(cx, tone);
    cx.set_global(look.clone());
    let body = look.theme.body;
    let theme = Theme::global_mut(cx);

    if let Ok(mut family) = MONO_FAMILY.write() {
        *family = Some(look.mono.clone().into());
    }
    theme.font_family = look.sans.into();
    theme.mono_font_family = look.mono.into();
    // A chosen Chinese face is tried before the system's, in every window.
    theme.font_fallbacks = look.cjk.into_iter().map(Into::into).collect();
    // The root sets the rem size from this, so every rem-based size follows.
    let scale = f32::from(look.scale_percent) / 100.;
    theme.font_size = px(BASE_FONT_SIZE * scale);
    theme.mono_font_size = px(BASE_MONO_FONT_SIZE * scale);
    theme.radius = look.theme.radius;
    theme.radius_lg = look.theme.radius_lg;
    // The stock dialog overlay (20 % black in dark) vanishes against our
    // near-black page, so a dialog would not read as in front (§10).
    theme.overlay = if body.dark {
        gpui::hsla(0., 0., 0., 0.62)
    } else {
        gpui::hsla(0., 0., 0., 0.28)
    };

    theme.background = body.page;
    theme.foreground = body.ink;
    theme.border = body.hairline;
    theme.input = body.hairline;
    theme.ring = body.orange;
    theme.caret = body.ink;
    theme.selection = body.orange.opacity(0.28);
    theme.link = body.orange_text;
    theme.link_hover = nudge(body.orange_text);
    theme.link_active = body.orange_text;

    theme.primary = body.orange;
    theme.primary_hover = nudge(body.orange);
    theme.primary_active = nudge(body.orange);
    theme.primary_foreground = body.on_orange;

    // Dialogs sit on the raised surface (see `dialog`).
    theme.secondary = body.panel;
    theme.secondary_hover = nudge(body.panel);
    theme.secondary_active = body.key_grey;
    theme.secondary_foreground = body.ink;

    theme.muted = body.key_grey;
    theme.muted_foreground = body.muted;
    theme.accent = body.key_grey;
    theme.accent_foreground = body.ink;
    theme.popover = body.panel;
    theme.popover_foreground = body.ink;

    theme.danger = body.danger;
    theme.danger_hover = nudge(body.danger);
    theme.danger_active = nudge(body.danger);
    theme.danger_foreground = body.on_orange;
    theme.success = body.ok;
    theme.progress_bar = body.orange;

    theme.list_hover = body.key_grey;
    theme.list_active = body.key_grey;
    theme.list_active_border = body.orange;
    theme.scrollbar_thumb = body.muted.opacity(0.5);
    theme.scrollbar_thumb_hover = body.muted;
}

/// Every dialog sits on the raised surface colour, not the page colour.
pub fn dialog(
    dialog: gpui_component::dialog::Dialog,
    cx: &gpui::App,
) -> gpui_component::dialog::Dialog {
    use gpui::Styled as _;
    use gpui_component::ActiveTheme as _;
    dialog.bg(cx.theme().secondary)
}

#[derive(Clone, Copy)]
pub struct ShellColors {
    pub body: Body,
    pub background: Hsla,
    pub surface: Hsla,
    pub surface_active: Hsla,
    pub surface_subtle: Hsla,
    pub foreground: Hsla,
    pub muted: Hsla,
    pub border: Hsla,
    pub primary: Hsla,
    pub primary_foreground: Hsla,
    pub focus: Hsla,
    /// Danger as text or a failure LED; the fill is `body.danger`.
    pub danger: Hsla,
}

impl ShellColors {
    pub fn current(cx: &gpui::App) -> Self {
        let body = Body::current(cx);
        Self {
            body,
            background: body.page,
            surface: body.panel,
            surface_active: body.key_grey,
            surface_subtle: body.key_grey,
            foreground: body.ink,
            muted: body.muted,
            border: body.hairline,
            primary: body.orange,
            primary_foreground: body.on_orange,
            focus: body.orange,
            danger: body.danger_text,
        }
    }
}

/// Motion tokens from `docs/design-language.md` §2. UI code uses these instead
/// of literal durations.
pub mod motion {
    use std::time::Duration;

    use gpui::{Pixels, px};

    /// Hover, press, and focus feedback.
    pub const INSTANT: Duration = Duration::from_millis(90);
    /// Route content fade and small state swaps.
    pub const QUICK: Duration = Duration::from_millis(160);
    /// Panels and morphs such as button → progress bar.
    pub const SETTLE: Duration = Duration::from_millis(280);
    /// Hero copy entrances and large narrative changes.
    pub const SCENE: Duration = Duration::from_millis(700);
    /// Frame interval of world-register animation: the game's 20 ticks/s.
    pub const WORLD_TICK: Duration = Duration::from_millis(50);
    /// Entrance travel distance.
    pub const LIFT: Pixels = px(8.);

    #[cfg(test)]
    mod tests {
        use super::{INSTANT, QUICK, SCENE, SETTLE};

        #[test]
        fn interface_durations_are_strictly_ordered() {
            assert!(INSTANT < QUICK && QUICK < SETTLE && SETTLE < SCENE);
        }
    }
}
