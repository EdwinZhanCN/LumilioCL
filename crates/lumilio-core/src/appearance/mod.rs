//! How a player looks in game: skins, capes and the local skin library, and
//! the [`Appearance`] capability that reads and changes them.

pub(crate) use profiles::Appearance;
pub use profiles::ProfileSnapshot;
pub use wardrobe::AppearanceApplyResult;

mod profiles;
pub(crate) mod skin;
mod skins;
mod wardrobe;
