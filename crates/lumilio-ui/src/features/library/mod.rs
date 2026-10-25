//! The Library page: the games as cards, their collections and favorites,
//! the game option presets, games brought over from another launcher or a
//! backup, and how the page is ordered. [`Library`] draws the page and does
//! the work; the shell owns it, gives it what it shows and passes its events
//! on.

pub use controls::LibraryControls;
pub use library_actions::{Library, LibraryEvent};
pub use order::{all_loaders, arranged, loader_code, present_loaders, sort_labels};
pub use page::{
    COLLECTIONS_TAB, LIBRARY_LOADER, LIBRARY_SORT, LibraryInputs, LibraryView, PRESET_GROUP,
    PRESETS_TAB,
};
pub use view::library_tabs;

mod collections;
mod controls;
mod game_picker;
mod imports;
mod library_actions;
mod order;
mod page;
mod presets;
mod presets_view;
mod view;

#[cfg(test)]
mod tests;
