//! The Library's actions: collections, favorites, games brought over from
//! another launcher or a backup, and how the page is ordered. [`Library`]
//! does the work; the shell owns it, gives it the collections and passes its
//! events on.

pub use library_actions::{Library, LibraryEvent};

mod collections;
mod game_picker;
mod imports;
mod library_actions;

#[cfg(test)]
mod tests;
