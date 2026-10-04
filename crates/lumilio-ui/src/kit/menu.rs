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
    let (mut safe, danger): (Vec<_>, Vec<_>) = entries.into_iter().partition(|entry| !entry.danger);
    let split = safe.len();
    safe.extend(danger);
    let entries = Rc::new(safe);
    Key::new(id)
        .icon(Icon::new(UiIcon::More))
        .black()
        .size(size)
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
