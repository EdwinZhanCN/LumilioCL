//! Settings (`docs/ia/settings.md`): launcher preferences and the defaults
//! every game starts from. The page shows read-only values and quick
//! controls; anything typed is edited in a dialog (design language §10).

mod about;
mod appearance;
mod game_defaults;
mod general;
mod java;
mod plugins;
mod rows;
mod storage;
mod text;

#[cfg(test)]
mod tests;

use page::fullscreen_choices;
pub use page::{TAB_GROUP, render, tabs};

pub use self::text::{bytes_text, commands_text, list_text, memory_text, window_text};

mod page;
