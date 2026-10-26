//! Interface primitives shared by every page (design language §1, interface
//! register): theme tokens only, so light and dark are both correct.

mod controls;
mod faceplate;
mod frame;
mod item;
mod menu;
mod meter;
mod motd;
mod project_icon;
mod rich;
mod rows;

#[cfg(test)]
mod tests;

pub use intent::{Emit, ViewIntent};

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
pub use self::meter::{
    ROW_METER_BARS, Reading, TransferDisplay, connection_cells, lit_bars, reading_digits, record,
    row_meter, speed_digits, transfer_display,
};
pub use self::motd::{motd_lines, motd_text};
pub use self::project_icon::project_icon;
pub use self::rich::rich_text;
pub use self::rows::{
    empty, info, line_row, list, middle_line, panel_list, path_row, row, section, section_at,
    section_head, section_label, section_with, setting, surface, technical, value_row,
};

mod intent;

/// What a failure looks like: one sentence, and the raw cause behind 技术详情.
pub type Failure = (String, String);
