use crate::assets::UiIcon;
use crate::key::Key;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, SharedString, Window, div, px};
use gpui_component::menu::DropdownMenu as _;
use gpui_component::menu::PopupMenuItem;
use gpui_component::{Icon, h_flex};
use std::rc::Rc;

/// What a menu entry does when chosen.
pub type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// One entry of a page's More menu (design language §7).
#[derive(Clone)]
pub struct MenuEntry {
    pub(super) label: SharedString,
    pub(super) danger: bool,
    pub(super) disabled: bool,
    /// A separator goes before it: it starts a group of its own.
    pub(super) separated: bool,
    /// Why a disabled entry cannot be chosen, shown as its tooltip.
    pub(super) hint: Option<SharedString>,
    pub(super) on_click: MenuAction,
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
            separated: false,
            hint: None,
            on_click: Rc::new(on_click),
        }
    }

    /// Starts a group of its own, after a separator.
    pub fn separated(mut self) -> Self {
        self.separated = true;
        self
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

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn is_danger(&self) -> bool {
        self.danger
    }

    /// Why a disabled entry cannot be chosen, if it says.
    pub fn hint(&self) -> Option<&SharedString> {
        self.hint.as_ref()
    }

    /// Runs the entry as choosing it in the menu does.
    pub fn choose(&self, window: &mut Window, cx: &mut App) {
        (self.on_click)(window, cx);
    }

    /// Disables the entry and says why in its tooltip ("游戏运行中不能改动").
    pub fn unavailable(mut self, why: impl Into<SharedString>) -> Self {
        self.disabled = true;
        self.hint = Some(why.into());
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
    more_key(id, entries, colors, crate::key::KeySize::Regular)
}

/// The ⋯ button at the height of a small key, for a row of small keys.
pub fn small_more_menu(
    id: impl Into<gpui::ElementId>,
    entries: Vec<MenuEntry>,
    colors: ShellColors,
) -> impl IntoElement {
    more_key(id, entries, colors, crate::key::KeySize::Small)
}

pub(super) fn more_key(
    id: impl Into<gpui::ElementId>,
    entries: Vec<MenuEntry>,
    colors: ShellColors,
    size: crate::key::KeySize,
) -> impl IntoElement {
    with_menu(
        Key::new(id)
            .icon(Icon::new(UiIcon::More))
            .black()
            .size(size)
            .tooltip(crate::tr!("common-more")),
        entries,
        gpui::Anchor::TopRight,
        colors,
    )
}

/// `key` opening `entries` as a menu anchored at `anchor`: the More key, or
/// a labelled key for a few related commands ("添加…": a zip, a folder or a
/// typed path). Destructive entries are sorted last, behind one separator.
pub fn with_menu(
    key: Key,
    entries: Vec<MenuEntry>,
    anchor: gpui::Anchor,
    colors: ShellColors,
) -> impl IntoElement {
    let (mut safe, danger): (Vec<_>, Vec<_>) = entries.into_iter().partition(|entry| !entry.danger);
    let split = safe.len();
    safe.extend(danger);
    let entries = Rc::new(safe);
    key.dropdown_menu_with_anchor(anchor, move |menu, _, _| {
        let mut menu = menu.min_w(px(180.));
        for (index, entry) in entries.iter().enumerate() {
            if index > 0 && (index == split || entry.separated) {
                menu = menu.separator();
            }
            let run = entry.on_click.clone();
            let item = if let Some(hint) = entry.hint.clone() {
                let (label, danger, index) = (entry.label.clone(), entry.danger, index);
                PopupMenuItem::element(move |_, _| {
                    let hint = hint.clone();
                    div()
                        .id(("menu-hint", index))
                        .debug_selector(move || format!("menu-entry-{index}"))
                        .when(danger, |item| item.text_color(colors.danger))
                        .tooltip(move |window, cx| {
                            gpui_component::tooltip::Tooltip::new(hint.clone()).build(window, cx)
                        })
                        .child(label.clone())
                })
            } else {
                // Every entry carries the same selector, enabled or not.
                let (label, danger) = (entry.label.clone(), entry.danger);
                PopupMenuItem::element(move |_, _| {
                    div()
                        .debug_selector(move || format!("menu-entry-{index}"))
                        .when(danger, |item| item.text_color(colors.danger))
                        .child(label.clone())
                })
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
