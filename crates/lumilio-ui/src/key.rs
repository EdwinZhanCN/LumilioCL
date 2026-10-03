//! Keys: the interface's buttons (design language §12). A key is a physical
//! object: white, black or orange, with a contact shadow at rest and 1 px of
//! travel when pressed. Drawn here rather than on gpui-component's `Button`,
//! whose variants rest at a fraction of their colour (postmortem 0001) and
//! cannot carry a stack of shadows.

use std::rc::Rc;

use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Div, ElementId, FontWeight, Hsla, InteractiveElement,
    Interactivity, IntoElement, ParentElement, RenderOnce, SharedString, Stateful,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, hsla, point, prelude::*,
    px,
};
use gpui_component::spinner::Spinner;
use gpui_component::{ActiveTheme as _, Icon, Sizable};

use crate::theme::{self, Body};

/// Which key it is. The kind decides the face, not the position on the page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyKind {
    /// Secondary actions.
    White,
    /// The More key; secondary actions drawn over the world.
    Black,
    /// The primary action: at most one per region.
    Orange,
    /// A text key for row actions, with a key-grey hover and no shadow.
    Ghost,
    /// The destructive confirm: only in a delete dialog, always naming the
    /// destruction.
    Danger,
}

/// How tall a key is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeySize {
    /// 34 px: page and dialog actions.
    Regular,
    /// 44 px: a key that carries a thumbnail, in the navigation.
    Large,
    /// 28 px: row actions and inline keys.
    Small,
    /// 24 px: quiet text keys inside copy.
    Compact,
}

impl KeySize {
    pub const fn height(self) -> f32 {
        match self {
            Self::Large => 44.,
            Self::Regular => 34.,
            Self::Small => 28.,
            Self::Compact => 24.,
        }
    }
}

/// The colours of a key in one state.
#[derive(Clone, Copy, Debug)]
pub struct KeyFace {
    pub face: Hsla,
    pub text: Hsla,
    pub hover: Hsla,
    /// Whether the key stands off the page (shadow, travel). Ghost and
    /// disabled keys lie flat.
    pub raised: bool,
}

/// The face a key shows: the one place that maps kind and state to colour.
pub fn face(kind: KeyKind, disabled: bool, body: Body) -> KeyFace {
    if disabled {
        return KeyFace {
            // A flat text key stays flat when it cannot act.
            face: if kind == KeyKind::Ghost {
                hsla(0., 0., 0., 0.)
            } else {
                body.key_grey
            },
            text: body.muted,
            hover: body.key_grey,
            raised: false,
        };
    }
    let (face, text) = match kind {
        KeyKind::White => (body.key_white, body.on_white),
        KeyKind::Black => (body.key_black, body.on_black),
        KeyKind::Orange => (body.orange, body.on_orange),
        KeyKind::Danger => (body.danger, body.on_orange),
        KeyKind::Ghost => (hsla(0., 0., 0., 0.), body.ink),
    };
    let hover = if kind == KeyKind::Ghost {
        body.key_grey
    } else {
        theme::nudge(face)
    };
    KeyFace {
        face,
        text,
        hover,
        raised: kind != KeyKind::Ghost,
    }
}

/// The ring a focused key wears: orange, outside the face.
fn focus_ring(body: Body) -> BoxShadow {
    BoxShadow {
        color: body.orange.opacity(0.7),
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(2.),
        inset: false,
    }
}

/// What a key does when clicked.
pub type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Key {
    id: ElementId,
    base: Stateful<Div>,
    kind: KeyKind,
    size: KeySize,
    label: Option<SharedString>,
    icon: Option<Icon>,
    /// Overrides the icon's colour, for a key whose icon shows a state.
    icon_tint: Option<Hsla>,
    children: Vec<AnyElement>,
    disabled: bool,
    loading: bool,
    /// Held down: a menu key whose menu is open stays pressed.
    held: bool,
    bold: bool,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl Key {
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: div().id(id.clone()),
            id,
            kind: KeyKind::White,
            size: KeySize::Regular,
            label: None,
            icon: None,
            icon_tint: None,
            children: Vec::new(),
            disabled: false,
            loading: false,
            held: false,
            bold: false,
            tooltip: None,
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Draws the icon in this colour instead of the key's text colour.
    pub fn icon_color(mut self, color: Hsla) -> Self {
        self.icon_tint = Some(color);
        self
    }

    pub fn kind(mut self, kind: KeyKind) -> Self {
        self.kind = kind;
        self
    }

    /// The orange primary key.
    pub fn primary(self) -> Self {
        self.kind(KeyKind::Orange)
    }

    /// The white secondary key.
    pub fn white(self) -> Self {
        self.kind(KeyKind::White)
    }

    pub fn black(self) -> Self {
        self.kind(KeyKind::Black)
    }

    pub fn ghost(self) -> Self {
        self.kind(KeyKind::Ghost)
    }

    pub fn danger(self) -> Self {
        self.kind(KeyKind::Danger)
    }

    pub fn size(mut self, size: KeySize) -> Self {
        self.size = size;
        self
    }

    pub fn large(self) -> Self {
        self.size(KeySize::Large)
    }

    pub fn compact(self) -> Self {
        self.size(KeySize::Compact)
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// A loading key keeps its colour but is as inert as a disabled one.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// A semibold label: the chosen key of a segment row.
    pub fn bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    /// Icon-only keys name themselves in a tooltip (§9).
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Whether the key can act, and so shows the pointing hand.
    pub fn shows_pointer(&self) -> bool {
        !(self.disabled || self.loading)
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Sizable for Key {
    fn with_size(self, size: impl Into<gpui_component::Size>) -> Self {
        use gpui_component::Size;
        self.size(match size.into() {
            Size::XSmall => KeySize::Compact,
            Size::Small => KeySize::Small,
            Size::Large => KeySize::Large,
            _ => KeySize::Regular,
        })
    }
}

impl gpui_component::menu::DropdownMenu for Key {}

impl gpui_component::Selectable for Key {
    fn selected(mut self, selected: bool) -> Self {
        self.held = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.held
    }

    fn open(mut self, open: bool) -> Self {
        self.held = open;
        self
    }

    fn is_open(&self) -> bool {
        self.held
    }
}

impl Styled for Key {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for Key {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

impl InteractiveElement for Key {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl RenderOnce for Key {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let body = Body::of_theme(cx.theme());
        let inert = !self.shows_pointer();
        let self_pointer = !inert;
        let KeyFace {
            face,
            text,
            hover,
            raised,
        } = face(self.kind, self.disabled, body);
        let square = self.label.is_none() && self.children.is_empty();
        let height = self.size.height();
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let tall = matches!(self.size, KeySize::Regular | KeySize::Large);
        let icon_size = if tall { px(14.) } else { px(13.) };
        let resting = if raised {
            theme::key_shadow(body)
        } else {
            Vec::new()
        };
        let mut focused = resting.clone();
        focused.push(focus_ring(body));

        let tooltip = self.tooltip;
        let on_click = self.on_click;
        let loading = self.loading;
        let spinner = loading.then(|| Spinner::new().xsmall().color(text));
        let icon = self.icon.filter(|_| !loading).map(|icon| {
            icon.size(icon_size)
                .text_color(self.icon_tint.unwrap_or(text))
        });

        self.base
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .h(px(height))
            .map(|key| {
                if square {
                    key.w(px(height))
                } else {
                    key.px(px(if tall { 16. } else { 12. }))
                }
            })
            .rounded(px(4.))
            .bg(face)
            .text_color(text)
            .when(tall, |key| key.text_sm())
            .when(!tall, |key| key.text_xs())
            .font_weight(if self.bold {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::MEDIUM
            })
            .shadow(resting)
            .when(raised && self.held, |key| {
                key.top(theme::KEY_TRAVEL).shadow(theme::pressed_shadow())
            })
            .track_focus(&focus)
            .tab_stop(!inert)
            .focus_visible(move |style| style.shadow(focused))
            .when(self_pointer, |key| {
                key.cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .when(raised, |key| {
                        key.active(|style| {
                            style.top(theme::KEY_TRAVEL).shadow(theme::pressed_shadow())
                        })
                    })
                    .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                        // Pressing a key must not steal focus from a field.
                        window.prevent_default();
                    })
            })
            .when(loading, |key| key.opacity(0.8))
            .children(spinner)
            .children(icon)
            .children(self.label)
            .children(self.children)
            .when_some(on_click.filter(|_| !inert), |key, on_click| {
                key.on_click(move |event, window, cx| on_click(event, window, cx))
            })
            .when_some(tooltip, |key, tooltip| {
                key.tooltip(move |window, cx| {
                    gpui_component::tooltip::Tooltip::new(tooltip.clone()).build(window, cx)
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Hsla, Modifiers, TestAppContext, VisualTestContext};
    use gpui_component::{Theme, ThemeMode};

    use super::{Key, KeyKind, face};
    use crate::theme::{Body, KEY_TRAVEL};

    struct Board {
        disabled: bool,
    }

    impl gpui::Render for Board {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            use gpui::{InteractiveElement as _, ParentElement as _, Styled as _};
            gpui::div()
                .flex()
                .gap_4()
                .p_4()
                .child(
                    Key::new("white")
                        .label("导入")
                        .debug_selector(|| "white".into()),
                )
                .child(
                    Key::new("black")
                        .label("更多")
                        .black()
                        .debug_selector(|| "black".into()),
                )
                .child(
                    Key::new("orange")
                        .label("新建")
                        .primary()
                        .debug_selector(|| "orange".into()),
                )
                .child(
                    Key::new("ghost")
                        .label("编辑")
                        .ghost()
                        .debug_selector(|| "ghost".into()),
                )
                .child(
                    Key::new("off")
                        .label("已在运行")
                        .primary()
                        .disabled(self.disabled)
                        .debug_selector(|| "off".into()),
                )
        }
    }

    fn board(cx: &mut TestAppContext, mode: ThemeMode) -> &mut VisualTestContext {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(mode, None, cx);
            crate::theme::tune(cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Board { disabled: true });
        cx.run_until_parked();
        cx
    }

    /// The painted quads over a key's bounds, plus the bounds.
    fn quads(cx: &mut VisualTestContext, selector: &'static str) -> Vec<gpui::Quad> {
        let bounds = cx.debug_bounds(selector).expect("the key is drawn");
        cx.update(|window, _| {
            let scale = window.scale_factor();
            let near = |a: f32, b: gpui::Pixels| (a / scale - f32::from(b)).abs() < 1.5;
            window
                .painted_quads()
                .into_iter()
                .filter(|quad| {
                    near(quad.bounds.origin.x.0, bounds.origin.x)
                        && near(quad.bounds.size.width.0, bounds.size.width)
                        && near(quad.bounds.size.height.0, bounds.size.height)
                })
                .collect()
        })
    }

    fn face_of(cx: &mut VisualTestContext, selector: &'static str) -> Hsla {
        quads(cx, selector)
            .iter()
            .filter_map(|quad| quad.background.as_solid())
            .find(|color| color.a > 0.)
            .expect("the key paints a face")
    }

    fn top_of(cx: &mut VisualTestContext, selector: &'static str) -> f32 {
        f32::from(cx.debug_bounds(selector).unwrap().origin.y)
    }

    #[gpui::test]
    fn each_kind_paints_its_face_in_both_bodies(cx: &mut TestAppContext) {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let body = Body::of(mode.is_dark());
            let cx = board(cx, mode);
            assert_eq!(face_of(cx, "white"), body.key_white, "{mode:?} white");
            assert_eq!(face_of(cx, "black"), body.key_black, "{mode:?} black");
            assert_eq!(face_of(cx, "orange"), body.orange, "{mode:?} orange");
            assert_eq!(face_of(cx, "off"), body.key_grey, "{mode:?} disabled");
        }
    }

    #[gpui::test]
    fn pressing_a_raised_key_moves_it_down_by_the_travel(cx: &mut TestAppContext) {
        let cx = board(cx, ThemeMode::Light);
        let rest = top_of(cx, "orange");
        let center = cx.debug_bounds("orange").unwrap().center();
        cx.simulate_mouse_move(center, None, Modifiers::none());
        cx.simulate_event(gpui::MouseDownEvent {
            position: center,
            modifiers: Modifiers::none(),
            button: gpui::MouseButton::Left,
            click_count: 1,
            first_mouse: false,
        });
        cx.run_until_parked();
        let pressed = top_of(cx, "orange");
        assert!(
            (pressed - rest - f32::from(KEY_TRAVEL)).abs() < 0.01,
            "pressed key travels {} px, moved {}",
            f32::from(KEY_TRAVEL),
            pressed - rest
        );
        cx.simulate_event(gpui::MouseUpEvent {
            position: center,
            modifiers: Modifiers::none(),
            button: gpui::MouseButton::Left,
            click_count: 1,
        });
    }

    #[gpui::test]
    fn a_disabled_key_has_no_shadow_hover_or_travel(cx: &mut TestAppContext) {
        let cx = board(cx, ThemeMode::Light);
        let body = Body::of(false);
        let quads = quads(cx, "off");
        assert!(
            quads
                .iter()
                .all(|quad| quad.background.as_solid() != Some(body.orange)),
            "a disabled primary is not orange"
        );
        let rest = top_of(cx, "off");
        let center = cx.debug_bounds("off").unwrap().center();
        cx.simulate_mouse_move(center, None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(face_of(cx, "off"), body.key_grey, "no hover shade");
        assert_eq!(top_of(cx, "off"), rest);
        assert!(!face(KeyKind::Orange, true, body).raised);
    }

    #[gpui::test]
    fn hovering_a_key_shifts_its_face_and_a_ghost_takes_key_grey(cx: &mut TestAppContext) {
        let cx = board(cx, ThemeMode::Light);
        let body = Body::of(false);
        let rest = face_of(cx, "white");
        let center = cx.debug_bounds("white").unwrap().center();
        cx.simulate_mouse_move(center, None, Modifiers::none());
        cx.run_until_parked();
        assert_ne!(face_of(cx, "white"), rest, "hover shifts the face");

        let center = cx.debug_bounds("ghost").unwrap().center();
        cx.simulate_mouse_move(center, None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(face_of(cx, "ghost"), body.key_grey);
    }
}
