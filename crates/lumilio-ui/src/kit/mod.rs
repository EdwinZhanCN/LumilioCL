//! Interface primitives shared by every page (design language §1, interface
//! register): theme tokens only, so light and dark are both correct.

mod controls;
mod faceplate;
mod frame;
mod item;
mod menu;
mod meter;
mod motd;
mod rich;
mod rows;

#[cfg(test)]
mod tests;

pub use self::controls::{
    Stand, TagKind, action, avatar, chip, filter_row, ghost, labeled_select, led_option, segments,
    switch, tabs, tag, tone_ok, tone_warn,
};
pub use self::faceplate::{
    FACEPLATE_FADE, FACEPLATE_WIDTH, display_window, faceplate, faceplate_cover, faceplate_head,
};
pub use self::frame::{
    SEARCH_WIDTH, entrance, fixed_page, header, keep_wheel, page, pane_body, scroll_body,
    search_field, toolbar,
};
pub use self::item::{Item, Tone, count_of, cover_tile, reading, signal, signal_bars, status};
pub use self::menu::{MenuAction, MenuEntry, PageActions, more_menu, small_more_menu, with_menu};
pub use self::meter::{Reading, lit_bars, progress, reading_digits, record};
pub use self::motd::{motd_lines, motd_text};
pub use self::rich::rich_text;
pub use self::rows::{
    empty, info, list, panel_list, row, row_with, section, section_at, section_head, section_label,
    section_with, setting, surface, technical, value_row,
};

use gpui::{App, Window};
use std::rc::Rc;

/// Something a page asked the shell to do. Pages stay stateless: they render
/// from the shell's view state and report clicks as intents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewIntent {
    LibraryTab(usize),
    ActivityTab(usize),
    /// Show or hide what this running task downloads.
    TaskDetail(u64),
    /// Choose one of a small set, keyed by (group, index).
    Choose(u8, usize),
}

pub type Emit = Rc<dyn Fn(ViewIntent, &mut Window, &mut App)>;
