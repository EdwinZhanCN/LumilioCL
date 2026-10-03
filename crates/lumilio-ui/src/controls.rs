//! State controls (design language §12): LEDs, fader switches, check keys,
//! segment keys, LED radios and port-label tabs. Every one shows its state
//! twice: an LED, and a second sign that does not depend on colour (§9).

use std::rc::Rc;

use gpui::{
    App, BoxShadow, ClickEvent, Div, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, div, point, prelude::*, px,
};
use gpui_component::{ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _};

use crate::key::{ClickHandler, Key};
use crate::theme::{self, Body};

/// A 6 px LED: orange with a soft glow when lit, LED-off grey when not.
pub fn led(on: bool, body: Body) -> Div {
    div()
        .flex_none()
        .size(px(6.))
        .rounded_full()
        .bg(if on { body.orange } else { body.led_off })
        .when(on, |led| led.shadow(theme::glow(body.orange)))
}

/// A 6 px LED in an arbitrary colour (success green, failure red); lit.
pub fn tone_led(color: gpui::Hsla) -> Div {
    div()
        .flex_none()
        .size(px(6.))
        .rounded_full()
        .bg(color)
        .shadow(theme::glow(color))
}

/// A 10 px muted silkscreen legend.
pub fn silk(text: impl Into<SharedString>, body: Body) -> Div {
    div()
        .text_size(px(10.))
        .text_color(body.muted)
        .child(text.into())
}

fn focus_ring(body: Body) -> BoxShadow {
    BoxShadow {
        color: body.orange.opacity(0.7),
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(2.),
        inset: false,
    }
}

/// Makes a control reachable by Tab and ringed while it has keyboard focus.
fn focusable(
    element: Stateful<Div>,
    id: &ElementId,
    enabled: bool,
    body: Body,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let focus = window
        .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone();
    element
        .track_focus(&focus)
        .tab_stop(enabled)
        .focus_visible(move |style| style.shadow(vec![focus_ring(body)]))
}

type Toggle = Rc<dyn Fn(&mut Window, &mut App)>;

/// The fader switch: a black slot, a white round cap and an LED. On means the
/// cap sits at the trailing end and the LED is lit.
#[derive(IntoElement)]
pub struct Fader {
    id: ElementId,
    on: bool,
    label: &'static str,
    disabled: bool,
    on_toggle: Toggle,
}

impl Fader {
    pub fn new(
        id: impl Into<ElementId>,
        on: bool,
        label: &'static str,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            on,
            label,
            disabled: false,
            on_toggle: Rc::new(on_toggle),
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Fader {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let body = Body::of_theme(cx.theme());
        let on = self.on;
        let toggle = self.on_toggle;
        let slot = if body.dark {
            gpui::rgb(0x000000).into()
        } else {
            body.key_black
        };
        let cap_id = format!("{}-cap", self.id);
        let led_id = format!("{}-led", self.id);
        let control = div()
            .id(self.id.clone())
            .rounded(px(4.))
            .flex()
            .items_center()
            .gap(px(8.))
            .aria_label(self.label)
            .child(
                div()
                    .relative()
                    .flex_none()
                    .w(px(40.))
                    .h(px(18.))
                    .child(
                        div()
                            .absolute()
                            .left(px(2.))
                            .right(px(2.))
                            .top(px(7.))
                            .h(px(4.))
                            .rounded_full()
                            .bg(slot)
                            .when(body.dark, |slot| {
                                slot.border_1().border_color(body.hairline)
                            }),
                    )
                    .child(
                        div()
                            .debug_selector(move || cap_id)
                            .absolute()
                            .top(px(1.))
                            .left(px(if on { 24. } else { 0. }))
                            .size(px(16.))
                            .rounded_full()
                            .bg(body.key_white)
                            .shadow(theme::key_shadow(body)),
                    ),
            )
            .child(led(on, body).debug_selector(move || led_id));
        focusable(control, &self.id, !self.disabled, body, window, cx).map(|control| {
            if self.disabled {
                control.opacity(0.5)
            } else {
                control
                    .cursor_pointer()
                    .on_click(move |_, window, cx| toggle(window, cx))
            }
        })
    }
}

/// A check key: 18 px, white when clear, orange with a white check when set.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    disabled: bool,
    label: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            disabled: false,
            label: None,
            on_click: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let body = Body::of_theme(cx.theme());
        let box_id = format!("{}-box", self.id);
        let checked = self.checked;
        let control = div()
            .id(self.id.clone())
            .rounded(px(4.))
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .debug_selector(move || box_id)
                    .flex_none()
                    .size(px(18.))
                    .rounded(px(3.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .shadow(theme::key_shadow(body))
                    .map(|key| {
                        if checked {
                            key.bg(body.orange).child(
                                Icon::new(IconName::Check)
                                    .size(px(13.))
                                    .text_color(body.on_orange),
                            )
                        } else {
                            key.bg(body.key_white)
                        }
                    }),
            )
            .children(
                self.label
                    .map(|label| div().text_sm().text_color(body.ink).child(label)),
            );
        let on_click = self.on_click;
        focusable(control, &self.id, !self.disabled, body, window, cx).map(|control| {
            if self.disabled {
                control.opacity(0.5)
            } else {
                control
                    .cursor_pointer()
                    .when_some(on_click, |control, handler| {
                        control.on_click(move |event, window, cx| handler(event, window, cx))
                    })
            }
        })
    }
}

/// A radio: an LED in a 14 px recessed socket, lit for the one chosen. The
/// chosen option's label stays ink; the others are muted (§9's second sign).
pub fn radio(chosen: bool, body: Body) -> Div {
    div()
        .flex_none()
        .size(px(14.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(if body.dark {
            gpui::rgb(0x050505).into()
        } else {
            body.key_grey
        })
        .when(body.dark, |socket| {
            socket.border_1().border_color(body.hairline)
        })
        .shadow(theme::recess_shadow(0.3))
        .child(
            div()
                .size(px(8.))
                .rounded_full()
                .bg(if chosen { body.orange } else { body.led_off })
                .when(chosen, |dot| dot.shadow(theme::glow(body.orange))),
        )
}

pub type Select = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Refinement choices: a row of small white keys, each with an LED above it.
/// The chosen key's LED is lit, the key sits pressed and its label is
/// semibold.
#[derive(IntoElement)]
pub struct Segments {
    id: &'static str,
    labels: Vec<&'static str>,
    active: usize,
    on_select: Select,
}

impl Segments {
    pub fn new(
        id: &'static str,
        labels: &[&'static str],
        active: usize,
        on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id,
            labels: labels.to_vec(),
            active,
            on_select: Rc::new(on_select),
        }
    }
}

impl RenderOnce for Segments {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let body = Body::of_theme(cx.theme());
        let (id, active) = (self.id, self.active);
        div()
            .flex()
            .items_start()
            .gap(px(8.))
            .children(self.labels.into_iter().enumerate().map(|(index, label)| {
                let chosen = index == active;
                let on_select = self.on_select.clone();
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(7.))
                    .child(led(chosen, body).debug_selector(move || format!("{id}-led-{index}")))
                    .child(
                        Key::new((id, index))
                            .label(label)
                            .white()
                            .small()
                            .min_w(px(64.))
                            .bold(chosen)
                            .selected(chosen)
                            .debug_selector(move || format!("{id}-key-{index}"))
                            .on_click(move |_, window, cx| on_select(index, window, cx)),
                    )
            }))
    }
}

/// View tabs as port labels: flush 30 px blocks with 1 px gaps along the
/// toolbar's leading edge. Resting blocks are key grey; the open one is
/// orange with a white medium label.
#[derive(IntoElement)]
pub struct PortTabs {
    id: &'static str,
    labels: Vec<&'static str>,
    active: usize,
    on_select: Select,
}

impl PortTabs {
    pub fn new(
        id: &'static str,
        labels: &[&'static str],
        active: usize,
        on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id,
            labels: labels.to_vec(),
            active,
            on_select: Rc::new(on_select),
        }
    }
}

impl RenderOnce for PortTabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let body = Body::of_theme(cx.theme());
        let (id, active) = (self.id, self.active);
        let tabs: Vec<Stateful<Div>> = self
            .labels
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                let open = index == active;
                let rest = body.key_grey;
                let on_select = self.on_select.clone();
                let tab = div()
                    .id((id, index))
                    .debug_selector(move || format!("{id}-tab-{index}"))
                    .h(px(30.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .text_size(px(12.))
                    .cursor_pointer()
                    .bg(if open { body.orange } else { rest })
                    .text_color(if open { body.on_orange } else { body.ink })
                    .when(open, |tab| tab.font_weight(FontWeight::MEDIUM))
                    .when(!open, |tab| {
                        tab.hover(move |style| style.bg(theme::nudge(rest)))
                    })
                    .on_click(move |_, window, cx| on_select(index, window, cx))
                    .child(label);
                focusable(tab, &ElementId::from((id, index)), true, body, window, cx)
            })
            .collect();
        div().flex().gap(px(1.)).children(tabs)
    }
}

/// A radio row's control and its label colour: chosen options read in ink,
/// the rest muted.
pub fn radio_label(chosen: bool, body: Body) -> gpui::Hsla {
    if chosen { body.ink } else { body.muted }
}

#[cfg(test)]
mod tests {
    use gpui::{
        Context, Hsla, IntoElement, Modifiers, ParentElement as _, Render, Styled as _,
        TestAppContext, VisualTestContext, Window, div,
    };
    use gpui_component::{Theme, ThemeMode};

    use super::{Checkbox, Fader, PortTabs, Segments};
    use crate::theme::Body;

    /// A board that owns the state each control toggles, as a page would.
    struct Board {
        fader: bool,
        check: bool,
        segment: usize,
        tab: usize,
    }

    impl Render for Board {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let entity = cx.entity();
            let (a, b, c, d) = (entity.clone(), entity.clone(), entity.clone(), entity);
            div()
                .flex()
                .flex_col()
                .gap_4()
                .p_4()
                .child(Fader::new("fader", self.fader, "示例", move |_, cx| {
                    a.update(cx, |board, cx| {
                        board.fader = !board.fader;
                        cx.notify();
                    })
                }))
                .child(
                    Checkbox::new("check")
                        .checked(self.check)
                        .label("同意")
                        .on_click(move |_, _, cx| {
                            b.update(cx, |board, cx| {
                                board.check = !board.check;
                                cx.notify();
                            })
                        }),
                )
                .child(Segments::new(
                    "seg",
                    &["Mod", "资源包", "光影"],
                    self.segment,
                    move |index, _, cx| {
                        c.update(cx, |board, cx| {
                            board.segment = index;
                            cx.notify();
                        })
                    },
                ))
                .child(PortTabs::new(
                    "ports",
                    &["概览", "内容", "世界"],
                    self.tab,
                    move |index, _, cx| {
                        d.update(cx, |board, cx| {
                            board.tab = index;
                            cx.notify();
                        })
                    },
                ))
        }
    }

    fn board(
        cx: &mut TestAppContext,
        mode: ThemeMode,
    ) -> (gpui::Entity<Board>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            Theme::change(mode, None, cx);
            crate::theme::tune(cx);
        });
        let (board, cx) = cx.add_window_view(|_, _| Board {
            fader: false,
            check: false,
            segment: 0,
            tab: 0,
        });
        cx.run_until_parked();
        (board, cx)
    }

    fn fill_of(cx: &mut VisualTestContext, selector: &'static str) -> Hsla {
        let bounds = cx.debug_bounds(selector).expect("drawn");
        cx.update(|window, _| {
            let scale = window.scale_factor();
            let near = |a: f32, b: gpui::Pixels| (a / scale - f32::from(b)).abs() < 1.5;
            window
                .painted_quads()
                .into_iter()
                .filter(|quad| {
                    near(quad.bounds.origin.x.0, bounds.origin.x)
                        && near(quad.bounds.origin.y.0, bounds.origin.y)
                        && near(quad.bounds.size.width.0, bounds.size.width)
                        && near(quad.bounds.size.height.0, bounds.size.height)
                })
                .filter_map(|quad| quad.background.as_solid())
                .find(|color| color.a > 0.)
                .expect("a fill is painted")
        })
    }

    fn click(cx: &mut VisualTestContext, selector: &'static str) {
        let center = cx.debug_bounds(selector).expect("drawn").center();
        cx.simulate_click(center, Modifiers::none());
        cx.run_until_parked();
    }

    fn left(cx: &mut VisualTestContext, selector: &'static str) -> f32 {
        f32::from(cx.debug_bounds(selector).unwrap().origin.x)
    }

    fn top(cx: &mut VisualTestContext, selector: &'static str) -> f32 {
        f32::from(cx.debug_bounds(selector).unwrap().origin.y)
    }

    #[gpui::test]
    fn the_fader_lights_its_led_and_moves_its_cap_together(cx: &mut TestAppContext) {
        let body = Body::of(false);
        let (board, cx) = board(cx, ThemeMode::Light);
        let (led_off, cap_off) = (fill_of(cx, "fader-led"), left(cx, "fader-cap"));
        assert_eq!(led_off, body.led_off);

        click(cx, "fader-cap");
        assert!(board.read_with(cx, |board, _| board.fader));
        let (led_on, cap_on) = (fill_of(cx, "fader-led"), left(cx, "fader-cap"));
        assert_eq!(led_on, body.orange, "the LED lights");
        assert!(
            cap_on - cap_off > 20.,
            "the cap travels to the trailing end"
        );
    }

    #[gpui::test]
    fn the_check_key_turns_orange_when_set(cx: &mut TestAppContext) {
        let body = Body::of(false);
        let (board, cx) = board(cx, ThemeMode::Light);
        assert_eq!(fill_of(cx, "check-box"), body.key_white);
        click(cx, "check-box");
        assert!(board.read_with(cx, |board, _| board.check));
        assert_eq!(fill_of(cx, "check-box"), body.orange);
    }

    #[gpui::test]
    fn the_chosen_segment_lights_its_led_and_sits_pressed(cx: &mut TestAppContext) {
        let body = Body::of(false);
        let (board, cx) = board(cx, ThemeMode::Light);
        assert_eq!(fill_of(cx, "seg-led-0"), body.orange);
        assert_eq!(fill_of(cx, "seg-led-1"), body.led_off);
        let (pressed, resting) = (top(cx, "seg-key-0"), top(cx, "seg-key-1"));
        assert!(pressed > resting, "the chosen key sits lower");

        click(cx, "seg-key-1");
        assert_eq!(board.read_with(cx, |board, _| board.segment), 1);
        assert_eq!(fill_of(cx, "seg-led-0"), body.led_off);
        assert_eq!(fill_of(cx, "seg-led-1"), body.orange);
        assert!(top(cx, "seg-key-1") > top(cx, "seg-key-0"));
    }

    #[gpui::test]
    fn the_open_port_tab_is_orange_the_rest_key_grey_and_one_pixel_apart(cx: &mut TestAppContext) {
        let body = Body::of(false);
        let (board, cx) = board(cx, ThemeMode::Light);
        assert_eq!(fill_of(cx, "ports-tab-0"), body.orange);
        assert_eq!(fill_of(cx, "ports-tab-1"), body.key_grey);
        let first = cx.debug_bounds("ports-tab-0").unwrap();
        let second = cx.debug_bounds("ports-tab-1").unwrap();
        assert!(
            (f32::from(second.origin.x) - f32::from(first.origin.x + first.size.width) - 1.).abs()
                < 0.01,
            "tabs are flush with 1 px between them"
        );

        click(cx, "ports-tab-2");
        assert_eq!(board.read_with(cx, |board, _| board.tab), 2);
        assert_eq!(fill_of(cx, "ports-tab-0"), body.key_grey);
        assert_eq!(fill_of(cx, "ports-tab-2"), body.orange);
    }

    #[gpui::test]
    fn night_keeps_the_same_rules(cx: &mut TestAppContext) {
        let body = Body::of(true);
        let (_, cx) = board(cx, ThemeMode::Dark);
        assert_eq!(fill_of(cx, "ports-tab-0"), body.orange);
        assert_eq!(fill_of(cx, "ports-tab-1"), body.key_grey);
        assert_eq!(fill_of(cx, "seg-led-0"), body.orange);
    }
}
