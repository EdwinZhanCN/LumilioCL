//! New games: the dialog that makes one, and imported packs. [`NewGame`]
//! asks the launcher for version lists, creates the game, downloads its
//! files and imports packs; the shell owns it and passes its events on.

pub use create::{NewGame, NewGameEvent, answer_versions};
pub use form::{NewGameForm, NewGameIntent, NewGameRequest, RuntimeChange};

mod create;
mod form;

#[cfg(test)]
mod tests;
