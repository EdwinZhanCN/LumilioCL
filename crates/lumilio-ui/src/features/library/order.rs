//! How the Library orders and filters its cards, and the numbers the choices
//! are remembered by.

use lumilio_core::Loader;

use crate::live::LibraryCard;
use crate::tr_all;

#[must_use]
pub fn sort_labels() -> &'static [&'static str] {
    tr_all![
        "library-sort-recent",
        "library-sort-name",
        "library-sort-created"
    ]
}

/// The text of the "no loader filter" entry.
#[must_use]
pub fn all_loaders() -> &'static str {
    crate::tr!("library-all-loaders")
}

/// The number a loader is remembered by: 1 vanilla, 2 Fabric, 3 Forge,
/// 4 NeoForge, 5 Quilt (0 is every loader).
#[must_use]
pub const fn loader_code(loader: Loader) -> usize {
    match loader {
        Loader::Vanilla => 1,
        Loader::Fabric => 2,
        Loader::Forge => 3,
        Loader::NeoForge => 4,
        Loader::Quilt => 5,
    }
}

/// The loaders the library has, in a steady order, for the filter.
#[must_use]
pub fn present_loaders(cards: &[LibraryCard]) -> Vec<Loader> {
    const ORDER: [Loader; 5] = [
        Loader::Vanilla,
        Loader::Fabric,
        Loader::Forge,
        Loader::NeoForge,
        Loader::Quilt,
    ];
    ORDER
        .into_iter()
        .filter(|loader| cards.iter().any(|card| card.loader == *loader))
        .collect()
}

/// The cards in the chosen order (0 as given — most recently played, favourites
/// first; 1 by name; 2 newest first) and only those with the loader, if any.
#[must_use]
pub fn arranged(
    mut cards: Vec<&LibraryCard>,
    sort: usize,
    loader: Option<Loader>,
) -> Vec<&LibraryCard> {
    cards.retain(|card| loader.is_none_or(|loader| card.loader == loader));
    match sort {
        1 => cards.sort_by_key(|card| card.name.to_lowercase()),
        2 => cards.sort_by_key(|card| std::cmp::Reverse(card.created)),
        _ => {}
    }
    cards
}
