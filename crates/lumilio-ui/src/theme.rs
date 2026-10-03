//! Semantic tokens: the instrument's two bodies (design language §12), brand
//! type (§13), and motion (§2).

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

    /// The body the component theme is currently showing.
    pub fn of_theme(theme: &Theme) -> Self {
        Self::of(theme.mode.is_dark())
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
    let theme = Theme::global_mut(cx);
    let body = Body::of_theme(theme);

    theme.font_family = SANS_FONT.into();
    theme.mono_font_family = MONO_FONT.into();
    theme.radius = px(4.);
    theme.radius_lg = px(6.);
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
    pub fn from_theme(theme: &Theme) -> Self {
        let body = Body::of_theme(theme);
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

#[cfg(test)]
mod tests {
    use gpui::{Hsla, Rgba, TestAppContext};
    use gpui_component::{Theme, ThemeMode};

    use super::{Body, ShellColors, tune};

    fn luminance(color: Hsla) -> f32 {
        let rgba = Rgba::from(color);
        let channel = |value: f32| {
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(rgba.r) + 0.7152 * channel(rgba.g) + 0.0722 * channel(rgba.b)
    }

    /// WCAG 2 contrast ratio.
    fn contrast(a: Hsla, b: Hsla) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    fn bodies() -> [(&'static str, Body); 2] {
        [("aluminium", Body::of(false)), ("night", Body::of(true))]
    }

    #[test]
    fn text_tokens_reach_4_5_to_1_on_what_they_sit_on() {
        for (name, b) in bodies() {
            let pairs = [
                ("ink on page", b.ink, b.page),
                ("ink on panel", b.ink, b.panel),
                ("muted on page", b.muted, b.page),
                ("muted on panel", b.muted, b.panel),
                ("on white key", b.on_white, b.key_white),
                ("on black key", b.on_black, b.key_black),
                ("ink on key grey (plain tag, port tab)", b.ink, b.key_grey),
                ("orange text on page", b.orange_text, b.page),
                ("orange text on panel", b.orange_text, b.panel),
                ("danger text on page", b.danger_text, b.page),
                ("danger text on panel", b.danger_text, b.panel),
                ("white on danger key", b.on_orange, b.danger),
                ("display ink on display", b.display_ink, b.display),
                ("display label on display", b.display_label, b.display),
                ("page on ink tag", b.page, b.ink),
            ];
            for (what, fg, bg) in pairs {
                let ratio = contrast(fg, bg);
                assert!(ratio >= 4.5, "{name}: {what} is {ratio:.2}:1");
            }
        }
    }

    /// The accepted exceptions (design language §9), listed so that they can
    /// neither widen nor sink further unnoticed.
    #[test]
    fn white_on_orange_stays_within_the_accepted_exception() {
        let (aluminium, night) = (Body::of(false), Body::of(true));
        let light = contrast(aluminium.on_orange, aluminium.orange);
        let dark = contrast(night.on_orange, night.orange);
        assert!((3.4..3.7).contains(&light), "aluminium: {light:.2}:1");
        assert!((3.0..3.3).contains(&dark), "night: {dark:.2}:1");
    }

    #[test]
    fn marks_that_stand_alone_reach_3_to_1() {
        for (name, b) in bodies() {
            // A lit LED is never the only sign of state (§9), which is why
            // orange on aluminium may sit at 2.8:1 on the page; held to 3:1
            // everywhere else. Keys are identified by their labels, which the
            // text test covers.
            let led = contrast(b.orange, b.page);
            let floor = if b.dark { 3. } else { 2.7 };
            assert!(led >= floor, "{name}: lit LED on page is {led:.2}:1");
            let on_panel = contrast(b.orange, b.panel);
            assert!(on_panel >= 3., "{name}: orange on panel is {on_panel:.2}:1");
            let led_off = contrast(b.led_off, b.page);
            assert!(
                led_off >= 1.5,
                "{name}: an unlit LED still shows ({led_off:.2}:1)"
            );
        }
    }

    #[gpui::test]
    fn tune_maps_the_body_onto_the_component_theme_in_both_appearances(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            cx.update(|cx| {
                Theme::change(mode, None, cx);
                tune(cx);
                let theme = Theme::global(cx);
                let body = Body::of(mode.is_dark());
                assert_eq!(theme.primary, body.orange, "{mode:?}");
                assert_eq!(theme.background, body.page, "{mode:?}");
                assert_eq!(theme.foreground, body.ink, "{mode:?}");
                assert_eq!(theme.border, body.hairline, "{mode:?}");
                assert_eq!(theme.font_family.as_ref(), super::SANS_FONT);
                assert_eq!(theme.mono_font_family.as_ref(), super::MONO_FONT);
                let colors = ShellColors::from_theme(theme);
                assert_eq!(colors.primary, theme.primary);
                assert_eq!(colors.background, theme.background);
            });
        }
    }
}
