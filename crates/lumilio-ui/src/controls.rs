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
    labels: Vec<SharedString>,
    active: usize,
    on_select: Select,
}

impl Segments {
    pub fn new<S: Clone + Into<SharedString>>(
        id: &'static str,
        labels: &[S],
        active: usize,
        on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id,
            labels: labels.iter().cloned().map(Into::into).collect(),
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
    labels: Vec<SharedString>,
    active: usize,
    on_select: Select,
}

impl PortTabs {
    pub fn new<S: Clone + Into<SharedString>>(
        id: &'static str,
        labels: &[S],
        active: usize,
        on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id,
            labels: labels.iter().cloned().map(Into::into).collect(),
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
mod tests;
