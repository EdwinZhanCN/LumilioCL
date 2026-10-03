//! Interface primitives shared by every page (design language §1, interface
//! register): theme tokens only, so light and dark are both correct.

use std::rc::Rc;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, ClickEvent, Hsla, IntoElement, SharedString,
    Window, div, ease_out_quint, prelude::*, px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{Icon, Sizable as _, StyledExt as _, TITLE_BAR_HEIGHT, h_flex, v_flex};

use crate::assets::UiIcon;
use crate::controls::{PortTabs, Segments};
use crate::key::Key;
use crate::theme::{self, ShellColors, motion};

/// Something a page asked the shell to do. Pages stay stateless: they render
/// from the shell's view state and report clicks as intents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewIntent {
    LibraryTab(usize),
    ActivityTab(usize),
    /// Choose one of a small set, keyed by (group, index).
    Choose(u8, usize),
}

pub type Emit = Rc<dyn Fn(ViewIntent, &mut Window, &mut App)>;

/// The scrolling page frame: clears the title bar and the floating capsule and
/// keeps content on the shared column.
pub fn page(id: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .id(id)
        .size_full()
        // The scroll container stays a plain block: a flex row here would
        // stretch the column to the viewport and nothing would ever scroll.
        .overflow_y_scroll()
        .child(
            theme::content_column()
                .mx_auto()
                .pt(TITLE_BAR_HEIGHT + px(12.))
                .pb(theme::BOTTOM_SAFE_AREA)
                .gap_6()
                .child(content),
        )
}

/// Keeps the wheel inside a list that scrolls within a page. Gpui gives a
/// wheel event to every scrollable under the pointer, so without this the
/// page moves along with the list. A list that fits has nothing to scroll and
/// lets the page have the wheel.
pub fn keep_wheel<E: gpui::InteractiveElement>(element: E, scroll: &gpui::ScrollHandle) -> E {
    let scroll = scroll.clone();
    element.on_scroll_wheel(move |_, _, cx| {
        if scroll.max_offset().y > px(0.) {
            cx.stop_propagation();
        }
    })
}

/// Route content enters with a short fade and lift, keyed by what it shows so
/// a new subject restarts the entrance and an unchanged one does not.
pub fn entrance(element: impl IntoElement, key: (&'static str, usize)) -> AnyElement {
    div()
        .w_full()
        .child(element)
        .with_animation(
            key,
            Animation::new(motion::QUICK).with_easing(ease_out_quint()),
            |element, delta| element.opacity(delta).mt(motion::LIFT * (1. - delta)),
        )
        .into_any_element()
}

pub fn header(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    trailing: Option<AnyElement>,
    colors: ShellColors,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_size(px(36.))
                        .line_height(px(42.))
                        .font_weight(gpui::FontWeight::LIGHT)
                        .text_color(colors.foreground)
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child(subtitle.into()),
                ),
        )
        .children(trailing)
}

/// What a menu entry does when chosen.
pub type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// One entry of a page's More menu (design language §7).
#[derive(Clone)]
pub struct MenuEntry {
    label: SharedString,
    danger: bool,
    disabled: bool,
    on_click: MenuAction,
}

impl MenuEntry {
    pub fn new(
        label: impl Into<SharedString>,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            danger: false,
            disabled: false,
            on_click: Rc::new(on_click),
        }
    }

    /// A destructive entry: drawn in the danger colour, after a separator.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// The icon-only ⋯ button that holds every action beyond the first two.
/// Destructive entries are sorted last, behind one separator.
pub fn more_menu(
    id: impl Into<gpui::ElementId>,
    entries: Vec<MenuEntry>,
    colors: ShellColors,
) -> impl IntoElement {
    let (mut safe, danger): (Vec<_>, Vec<_>) = entries.into_iter().partition(|entry| !entry.danger);
    let split = safe.len();
    safe.extend(danger);
    let entries = Rc::new(safe);
    Key::new(id)
        .icon(Icon::new(UiIcon::More))
        .black()
        .tooltip("更多")
        .dropdown_menu_with_anchor(gpui::Anchor::TopRight, move |menu, _, _| {
            let mut menu = menu.min_w(px(180.));
            for (index, entry) in entries.iter().enumerate() {
                if index == split && index > 0 {
                    menu = menu.separator();
                }
                let run = entry.on_click.clone();
                let item = if entry.danger {
                    let label = entry.label.clone();
                    PopupMenuItem::element(move |_, _| {
                        div().text_color(colors.danger).child(label.clone())
                    })
                } else {
                    PopupMenuItem::new(entry.label.clone())
                };
                menu = menu.item(
                    item.disabled(entry.disabled)
                        .on_click(move |_, window, cx| run(window, cx)),
                );
            }
            menu
        })
}

/// The page actions on the header's trailing edge, always in the order
/// secondary · primary · more (design language §7). `None` when empty.
pub struct PageActions {
    pub id: &'static str,
    pub secondary: Option<Key>,
    pub primary: Option<Key>,
    pub more: Vec<MenuEntry>,
}

impl PageActions {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            secondary: None,
            primary: None,
            more: Vec::new(),
        }
    }

    pub fn secondary(mut self, button: Key) -> Self {
        self.secondary = Some(button);
        self
    }

    pub fn primary(mut self, button: Key) -> Self {
        self.primary = Some(button);
        self
    }

    pub fn more(mut self, entry: MenuEntry) -> Self {
        self.more.push(entry);
        self
    }

    pub fn render(self, colors: ShellColors) -> Option<AnyElement> {
        if self.secondary.is_none() && self.primary.is_none() && self.more.is_empty() {
            return None;
        }
        let id = self.id;
        let more = (!self.more.is_empty())
            .then(|| more_menu((id, 0usize), self.more, colors).into_any_element());
        Some(
            h_flex()
                .debug_selector(move || id.into())
                .gap_2()
                .flex_none()
                .children(self.secondary)
                .children(self.primary)
                .children(more)
                .into_any_element(),
        )
    }
}

/// The toolbar row (design language §7): view tabs on the leading side,
/// search on the trailing edge, even when there are no tabs.
pub fn toolbar(tabs: Option<AnyElement>, search: Option<AnyElement>) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap_4()
        .child(div().min_w_0().children(tabs))
        .children(search.map(|search| div().flex_none().child(search)))
}

/// The page search field: fixed width, magnifier prefix.
pub fn search_field(state: &gpui::Entity<InputState>, colors: ShellColors) -> AnyElement {
    div()
        .w(SEARCH_WIDTH)
        .child(
            Input::new(state).small().prefix(
                Icon::new(UiIcon::Search)
                    .size(px(14.))
                    .text_color(colors.muted),
            ),
        )
        .into_any_element()
}

pub const SEARCH_WIDTH: gpui::Pixels = px(240.);

/// A quiet 技术详情 that opens the raw cause in a dialog (§8, §11).
pub fn technical(id: impl Into<gpui::ElementId>, detail: impl Into<SharedString>) -> Key {
    let detail: SharedString = detail.into();
    Key::new(id)
        .label("技术详情")
        .ghost()
        .small()
        .on_click(move |_: &ClickEvent, window, cx| {
            crate::toast::technical_dialog(detail.clone(), window, cx)
        })
}

/// A muted ⓘ after a label that shows its help as a tooltip (§10).
pub fn info(id: impl Into<gpui::ElementId>, help: impl Into<SharedString>) -> impl IntoElement {
    Key::new(id)
        .icon(Icon::new(UiIcon::Info).size(px(14.)))
        .ghost()
        .compact()
        .tooltip(help.into())
}

/// A settings row (§10): label and optional help on the leading side, the
/// read-only value and an optional quick control on the trailing side.
pub fn value_row(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    help: Option<SharedString>,
    value: impl Into<SharedString>,
    control: Option<AnyElement>,
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    h_flex()
        .id(id)
        .w_full()
        .items_center()
        .gap_3()
        .py(px(11.))
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(colors.foreground)
                        .child(label.into()),
                )
                // The row's own id scopes this one.
                .children(help.map(|help| info("info", help))),
        )
        .child(
            div()
                .font_family(theme::MONO_FONT)
                .text_xs()
                .text_color(colors.muted)
                .child(value.into()),
        )
        .children(control)
}

/// Silkscreen for a titled group (design language §12): an optional orange
/// mono index, the ink title, then a hairline running to the column's edge and
/// ending in a bracket tick.
pub fn section_label(text: impl Into<SharedString>, colors: ShellColors) -> impl IntoElement {
    silk_label(None, text, colors)
}

fn silk_label(
    index: Option<usize>,
    text: impl Into<SharedString>,
    colors: ShellColors,
) -> gpui::Div {
    h_flex()
        .w_full()
        .items_center()
        .gap(px(10.))
        .children(index.map(|index| {
            div()
                .font_family(theme::MONO_FONT)
                .text_xs()
                .text_color(colors.body.orange_text)
                .child(format!("{index:02}"))
        }))
        .child(
            div()
                .text_xs()
                .text_color(colors.foreground)
                .child(text.into()),
        )
        .child(div().flex_1().h(px(1.)).bg(colors.border))
        .child(div().w(px(1.)).h(px(7.)).mt(px(6.)).bg(colors.border))
}

pub fn section(
    label: impl Into<SharedString>,
    colors: ShellColors,
    body: impl IntoElement,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_3()
        .child(section_label(label, colors))
        .child(body)
}

/// [`section`] with a numbered silkscreen index, for pages with several
/// groups.
pub fn section_at(
    index: usize,
    label: impl Into<SharedString>,
    colors: ShellColors,
    body: impl IntoElement,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_3()
        .child(silk_label(Some(index), label, colors))
        .child(body)
}

/// View tabs as port labels (design language §12).
pub fn tabs(
    id: &'static str,
    labels: &[&'static str],
    active: usize,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    PortTabs::new(id, labels, active, on_select)
}

/// Refinement choices, for the second level under a section: segment keys
/// with an LED over the one in use.
pub fn segments(
    id: &'static str,
    labels: &[&'static str],
    active: usize,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    Segments::new(id, labels, active, on_select)
}

/// One option of a filter list: an LED and a label on a hairline row. The
/// chosen option lights its LED and reads in ink, semibold; the rest are muted.
pub fn led_option(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    chosen: bool,
    colors: ShellColors,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let body = colors.body;
    h_flex()
        .id(id)
        .w_full()
        .h(px(32.))
        .gap(px(10.))
        .items_center()
        .border_b_1()
        .border_color(colors.border)
        .cursor_pointer()
        .hover(move |row| row.bg(body.panel))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(crate::controls::led(chosen, body))
        .child(
            div()
                .text_sm()
                .when(chosen, |label| label.font_semibold())
                .text_color(if chosen {
                    colors.foreground
                } else {
                    colors.muted
                })
                .child(label.into()),
        )
}

/// The base surface of a card or list block.
pub fn surface(colors: ShellColors) -> gpui::Div {
    div()
        .rounded(px(6.))
        .border_1()
        .border_color(colors.border)
        .bg(colors.surface)
}

/// A round identity mark: the name's first letter on a colour taken from the
/// name, so the same account always looks the same. Stands in for the skin
/// head until skins exist.
pub fn avatar(name: &str, size: f32) -> gpui::Div {
    let hue = (crate::live::seed_of(name) % 360) as f32 / 360.;
    let initial = name
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().to_string())
        .unwrap_or_default();
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_full()
        .bg(gpui::hsla(hue, 0.42, 0.42, 1.))
        .text_color(gpui::white())
        .text_size(px(size * 0.45))
        .font_semibold()
        .child(initial)
}

/// The kinds of tag (design language §12).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TagKind {
    /// A proper noun such as the loader.
    Ink,
    /// Versions and plain facts.
    Plain,
    /// At most one emphasis tag per region.
    Orange,
}

/// A 20 px mono tag.
pub fn tag(text: impl Into<SharedString>, kind: TagKind, colors: ShellColors) -> gpui::Div {
    let body = colors.body;
    let (bg, fg) = match kind {
        TagKind::Orange => (body.orange, body.on_orange),
        TagKind::Ink => (body.ink, body.page),
        TagKind::Plain => (body.key_grey, body.ink),
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .h(px(20.))
        .px(px(6.))
        .rounded(px(2.))
        .bg(bg)
        .text_color(fg)
        .font_family(theme::MONO_FONT)
        .text_size(px(11.))
        .child(text.into())
}

/// A status is not a tag: it is an LED and a word. Without a `tone` it is a
/// plain tag.
pub fn chip(
    text: impl Into<SharedString>,
    tone: Option<Hsla>,
    colors: ShellColors,
) -> impl IntoElement {
    match tone {
        Some(tone) => h_flex()
            .flex_none()
            .gap(px(6.))
            .items_center()
            .text_xs()
            .text_color(colors.foreground)
            .child(crate::controls::tone_led(tone))
            .child(text.into())
            .into_any_element(),
        None => tag(text, TagKind::Plain, colors).into_any_element(),
    }
}

pub fn tone_ok() -> Hsla {
    gpui::hsla(0.38, 0.55, 0.5, 1.)
}

pub fn tone_warn() -> Hsla {
    gpui::hsla(0.11, 0.85, 0.55, 1.)
}

/// A key on the page: orange when primary, white otherwise (design language
/// §12).
pub fn action(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    icon: Option<UiIcon>,
    primary: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> Key {
    let mut key = Key::new(id).label(label);
    if let Some(icon) = icon {
        key = key.icon(Icon::new(icon));
    }
    let key = if primary { key.primary() } else { key.white() };
    key.on_click(move |_: &ClickEvent, window, cx| on_click(window, cx))
}

/// A quiet text key for row-level actions.
pub fn ghost(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> Key {
    Key::new(id)
        .label(label)
        .ghost()
        .small()
        .on_click(move |_: &ClickEvent, window, cx| on_click(window, cx))
}

/// Width of a faceplate.
pub const FACEPLATE_WIDTH: gpui::Pixels = px(232.);

/// An instance card's frame (design language §12): a panel with a hairline
/// border, a soft shadow and a screw mark in each corner. Hover turns the
/// border ink.
pub fn faceplate(id: impl Into<gpui::ElementId>, colors: ShellColors) -> gpui::Stateful<gpui::Div> {
    let body = colors.body;
    let screw = |top: bool, left: bool| {
        let screw = div()
            .absolute()
            .size(px(7.))
            .rounded_full()
            .border_1()
            .border_color(body.hairline)
            .bg(body.page);
        let screw = if top {
            screw.top(px(7.))
        } else {
            screw.bottom(px(7.))
        };
        if left {
            screw.left(px(7.))
        } else {
            screw.right(px(7.))
        }
    };
    v_flex()
        .id(id)
        .relative()
        .w(FACEPLATE_WIDTH)
        .gap(px(12.))
        .p(px(16.))
        .rounded(px(6.))
        .bg(body.panel)
        .border_1()
        .border_color(body.hairline)
        .shadow(theme::panel_shadow(body))
        .cursor_pointer()
        .hover(move |card| card.border_color(body.ink))
        .child(screw(true, true))
        .child(screw(true, false))
        .child(screw(false, true))
        .child(screw(false, false))
}

/// A faceplate's top: the name in light type over the loader's orange label.
pub fn faceplate_head(
    name: impl Into<SharedString>,
    loader: impl Into<SharedString>,
    colors: ShellColors,
) -> impl IntoElement {
    v_flex()
        .gap(px(2.))
        .pt(px(4.))
        .child(
            div()
                .text_size(px(20.))
                .line_height(px(26.))
                .font_weight(gpui::FontWeight::LIGHT)
                .text_color(colors.foreground)
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(name.into()),
        )
        .child(
            div()
                .font_family(theme::MONO_FONT)
                .text_xs()
                .text_color(colors.body.orange_text)
                .child(SharedString::from(loader.into().to_uppercase())),
        )
}

/// The display window a cover sits in: the one place the world is framed.
/// The cover does not dissolve here.
pub fn display_window(cover: impl IntoElement, colors: ShellColors) -> gpui::Div {
    div()
        .relative()
        .h(px(96.))
        .w_full()
        .rounded(px(3.))
        .overflow_hidden()
        .bg(colors.body.display)
        .child(cover)
}

/// The cover of a faceplate's display: no dissolve, 3 px corners.
pub fn faceplate_cover(
    seed: u32,
    loader: crate::cover::Loader,
    world: crate::home::WorldHint,
    colors: ShellColors,
) -> impl IntoElement {
    crate::cover::element(
        seed,
        loader,
        world,
        colors.body.display,
        FACEPLATE_FADE,
        px(3.),
    )
}

/// A faceplate's cover has no dissolve into the page: it sits in a window.
pub const FACEPLATE_FADE: gpui::Pixels = px(0.);

/// One row of a list block. `lead` and `trail` are optional slots.
pub fn row(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    lead: Option<AnyElement>,
    trail: Option<AnyElement>,
    colors: ShellColors,
) -> gpui::Div {
    h_flex()
        .w_full()
        .items_center()
        .gap_3()
        .py(px(11.))
        .children(lead)
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(colors.foreground)
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child(detail.into()),
                ),
        )
        .children(trail)
}

/// Stacks rows into a hairline table with no surface (design language §12):
/// an ink rule on top and a hairline under each row.
pub fn list<E: IntoElement>(rows: Vec<E>, colors: ShellColors) -> impl IntoElement {
    div()
        .w_full()
        .border_t_1()
        .border_color(colors.foreground)
        .children(rows.into_iter().map(move |row| {
            div()
                .w_full()
                .border_b_1()
                .border_color(colors.border)
                .child(row)
        }))
}

/// A collection of things (mods, worlds, downloads) as a panel of rows with
/// hairline separators.
pub fn panel_list<E: IntoElement>(rows: Vec<E>, colors: ShellColors) -> impl IntoElement {
    let last = rows.len().saturating_sub(1);
    surface(colors)
        .overflow_hidden()
        .children(rows.into_iter().enumerate().map(move |(index, row)| {
            div()
                .w_full()
                .px_4()
                .when(index < last, |wrap| {
                    wrap.border_b_1().border_color(colors.border)
                })
                .child(row)
        }))
}

/// Bars on a display's meter.
const METER_BARS: usize = 24;

/// How many whole bars light for a fraction: monotonic in `fraction`, so a
/// display never steps back while progress only moves forward (§3).
pub fn lit_bars(fraction: f32, bars: usize) -> usize {
    let fraction = if fraction.is_nan() {
        0.
    } else {
        fraction.clamp(0., 1.)
    };
    ((fraction * bars as f32).floor() as usize).min(bars)
}

/// A live number on a black display (design language §12): segment digits
/// over unlit eights, and a meter that lights one whole bar at a time.
pub fn progress(
    id: impl Into<gpui::ElementId>,
    fraction: f32,
    colors: ShellColors,
) -> impl IntoElement {
    let body = colors.body;
    let lit = lit_bars(fraction, METER_BARS);
    let percent = (fraction.clamp(0., 1.) * 100.).floor() as u32;
    let id: gpui::ElementId = id.into();
    v_flex()
        .id(id)
        .flex_none()
        .w(px(168.))
        .gap(px(6.))
        .p(px(8.))
        .rounded(px(6.))
        .bg(body.display)
        .shadow(theme::display_shadow())
        .child(
            h_flex()
                .justify_end()
                .items_baseline()
                .gap(px(3.))
                .child(
                    div()
                        .relative()
                        .font_family(theme::LCD_FONT)
                        .text_size(px(18.))
                        .line_height(px(18.))
                        // Unlit segments sit behind the lit ones, as on a real
                        // display.
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .text_color(body.display_dim)
                                .child("888"),
                        )
                        .child(
                            div()
                                .relative()
                                .text_color(body.display_ink)
                                .child(format!("{percent:03}")),
                        ),
                )
                .child(
                    div()
                        .font_family(theme::MONO_FONT)
                        .text_size(px(10.))
                        .text_color(body.display_label)
                        .child("%"),
                ),
        )
        .child(
            h_flex()
                .h(px(8.))
                .gap(px(1.))
                .children((0..METER_BARS).map(|bar| {
                    div().flex_1().h_full().bg(if bar < lit {
                        body.display_ink
                    } else {
                        body.display_dim
                    })
                })),
        )
}

/// One small vignette, one sentence (design language §7).
pub fn empty(
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    colors: ShellColors,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .items_center()
        .gap_1()
        .py_10()
        .child(
            div()
                .text_base()
                .font_semibold()
                .text_color(colors.foreground)
                .child(title.into()),
        )
        .child(div().text_sm().text_color(colors.muted).child(body.into()))
}

/// A labelled on/off preference: a fader with an LED.
pub fn switch(
    id: impl Into<gpui::ElementId>,
    checked: bool,
    label: &'static str,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    crate::controls::Fader::new(id, checked, label, on_toggle)
}

/// A settings row: title and explanation on the left, the control on the right.
pub fn setting(
    title: &'static str,
    detail: impl Into<SharedString>,
    control: impl IntoElement,
    colors: ShellColors,
) -> gpui::Div {
    row(
        title,
        detail,
        None,
        Some(control.into_any_element()),
        colors,
    )
}

#[cfg(test)]
mod tests {
    use gpui::{
        InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
        TestAppContext, Window, div,
    };
    use gpui_component::{ActiveTheme as _, Theme, ThemeMode};

    use super::{TagKind, list, lit_bars, panel_list, tag};
    use crate::theme::{self, ShellColors};

    struct Sheet;

    impl Render for Sheet {
        fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let colors = ShellColors::from_theme(cx.theme());
            div()
                .w(gpui::px(300.))
                .child(
                    div()
                        .debug_selector(|| "table".into())
                        .child(list(vec![div().child("内存"), div().child("窗口")], colors)),
                )
                .child(
                    div()
                        .debug_selector(|| "panel".into())
                        .child(panel_list(vec![div().child("a"), div().child("b")], colors)),
                )
                .child(div().debug_selector(|| "tag".into()).child(tag(
                    "FABRIC",
                    TagKind::Orange,
                    colors,
                )))
        }
    }

    fn quads(cx: &mut gpui::VisualTestContext, selector: &'static str) -> Vec<gpui::Quad> {
        let bounds = cx.debug_bounds(selector).expect("drawn");
        cx.update(|window, _| {
            let scale = window.scale_factor();
            window
                .painted_quads()
                .into_iter()
                .filter(|quad| {
                    let x = quad.bounds.origin.x.0 / scale;
                    let y = quad.bounds.origin.y.0 / scale;
                    x >= f32::from(bounds.origin.x) - 1.5
                        && y >= f32::from(bounds.origin.y) - 1.5
                        && y <= f32::from(bounds.origin.y + bounds.size.height)
                })
                .collect()
        })
    }

    #[gpui::test]
    fn a_hairline_table_has_an_ink_rule_on_top_and_hairlines_beneath(cx: &mut TestAppContext) {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            cx.update(|cx| {
                gpui_component::init(cx);
                Theme::change(mode, None, cx);
                theme::tune(cx);
            });
            let (_, cx) = cx.add_window_view(|_, _| Sheet);
            cx.run_until_parked();
            let body = theme::Body::of(mode.is_dark());
            let edges: Vec<_> = quads(cx, "table")
                .into_iter()
                .filter(|quad| quad.border_widths.top.0 > 0. || quad.border_widths.bottom.0 > 0.)
                .collect();
            assert!(
                edges
                    .iter()
                    .any(|quad| quad.border_widths.top.0 > 0. && quad.border_color == body.ink),
                "{mode:?}: the top rule is ink"
            );
            assert!(
                edges
                    .iter()
                    .any(|quad| quad.border_widths.bottom.0 > 0.
                        && quad.border_color == body.hairline),
                "{mode:?}: rows end in hairlines"
            );
            let panel = quads(cx, "panel");
            assert!(
                panel
                    .iter()
                    .any(|quad| quad.background.as_solid() == Some(body.panel)),
                "{mode:?}: a collection sits on a panel"
            );
            let orange = quads(cx, "tag");
            assert!(
                orange
                    .iter()
                    .any(|quad| quad.background.as_solid() == Some(body.orange)),
                "{mode:?}: the orange tag fills with the signal orange"
            );
        }
    }

    #[test]
    fn a_faceplate_cover_does_not_dissolve_into_the_page() {
        assert_eq!(super::FACEPLATE_FADE, gpui::px(0.));
    }

    #[test]
    fn the_meter_lights_whole_bars_and_never_steps_back() {
        let mut last = 0;
        for step in 0..=1000 {
            let lit = lit_bars(step as f32 / 1000., 24);
            assert!(lit >= last, "bars only increase");
            last = lit;
        }
        assert_eq!(lit_bars(0., 24), 0);
        assert_eq!(lit_bars(1., 24), 24);
        assert_eq!(lit_bars(0.5, 24), 12);
        assert_eq!(lit_bars(-1., 24), 0, "below range clamps");
        assert_eq!(lit_bars(7., 24), 24, "above range clamps");
        assert_eq!(lit_bars(f32::NAN, 24), 0);
    }
}
