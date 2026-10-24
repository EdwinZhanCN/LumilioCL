use std::collections::{BTreeMap, BTreeSet};

use lumilio_core::{ContentSourceInfo, DiscoverPreferences, InstalledProject, ProjectKind};

use super::model::{DiscoverQuery, FilterModel, Provided, SearchRow, SearchStatus, visible_kinds};
use crate::live::LibraryCard;

/// What Discover shows and works from: its own search, and the library it
/// reads (never writes) to know the game it browses for and installs into.
#[derive(Clone, Debug)]
pub struct DiscoverState {
    pub results: Vec<SearchRow>,
    pub search: SearchStatus,
    /// The content sources that are on, in the launcher's order.
    pub sources: Vec<ContentSourceInfo>,
    pub query: DiscoverQuery,
    pub filters: FilterModel,
    /// The game Discover was opened from (its Content tab): the page then
    /// browses for that game, with its version and loader locked in. `None`
    /// is plain browsing. Either way installs go into the current game.
    pub browsing_for: Option<String>,
    /// What Discover remembers between visits.
    pub discover_prefs: DiscoverPreferences,
    /// Projects (by slug) being installed now.
    pub installing: BTreeSet<String>,
    /// What the target game already has of the searched kind, by project.
    pub installed: BTreeMap<String, InstalledProject>,
    /// The library, as the shell last gave it.
    pub library: Vec<LibraryCard>,
    /// The game installs go into: the current game, once the library shows it.
    pub target: Option<String>,
    /// Translation is on in Settings.
    pub translation: bool,
}

impl Default for DiscoverState {
    fn default() -> Self {
        Self {
            results: Vec::new(),
            search: SearchStatus::Idle,
            sources: Vec::new(),
            query: DiscoverQuery::new(ProjectKind::Modpack),
            filters: FilterModel::default(),
            browsing_for: None,
            discover_prefs: DiscoverPreferences::default(),
            installing: BTreeSet::new(),
            installed: BTreeMap::new(),
            library: Vec::new(),
            target: None,
            translation: false,
        }
    }
}

impl DiscoverState {
    /// How many pages the current results span.
    pub fn pages(&self) -> u32 {
        match self.search {
            SearchStatus::Done { total } => {
                u32::try_from(total.div_ceil(u64::from(self.query.page_size.max(1))))
                    .unwrap_or(u32::MAX)
            }
            _ => 0,
        }
    }

    /// The game Discover is browsing for, if it was opened from one.
    pub fn browsing_game(&self) -> Option<&LibraryCard> {
        let id = self.browsing_for.as_ref()?;
        self.library.iter().find(|card| &card.id == id)
    }

    /// The card of the game installs go into.
    pub fn target_game(&self) -> Option<&LibraryCard> {
        let id = self.target.as_ref()?;
        self.library.iter().find(|card| &card.id == id)
    }

    /// What the game provides to the current search (nothing outside a game).
    pub fn provided(&self) -> Provided {
        self.browsing_game().map_or_else(Provided::default, |game| {
            Provided::for_game(self.query.kind, &game.game_version, game.loader)
        })
    }

    /// The kinds the tabs offer right now.
    pub fn discover_kinds(&self) -> Vec<ProjectKind> {
        visible_kinds(self.browsing_game().map(|game| game.loader))
    }

    /// "Hide already installed" exists for packs (against the library) and
    /// inside a game (against that game).
    pub fn can_hide_installed(&self) -> bool {
        (self.query.kind == ProjectKind::Modpack || self.browsing_game().is_some())
            && self.filters.offers_hide_installed(self.query.kind)
    }

    /// Whether "hide already installed" is on for what the page shows: the
    /// remembered choice on the pack tab, the session's inside a game.
    pub fn hiding_installed(&self) -> bool {
        self.can_hide_installed()
            && if self.query.kind == ProjectKind::Modpack {
                self.discover_prefs.hide_installed_modpacks
            } else {
                self.query.hide_installed
            }
    }

    /// The projects "hide already installed" leaves out: the packs of the
    /// browsed source the library was made from, or what the browsed game
    /// already has of that source.
    pub fn installed_projects(&self) -> Vec<String> {
        if self.query.kind == ProjectKind::Modpack {
            return self
                .library
                .iter()
                .filter_map(|card| card.source_project.as_deref())
                .map(lumilio_core::ProjectOrigin::parse)
                .filter(|origin| origin.source == self.query.source)
                .map(|origin| origin.project)
                .collect();
        }
        self.installed.keys().cloned().collect()
    }

    /// A content source's name, by plugin ID; the ID when it is not on.
    pub fn source_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.sources
            .iter()
            .find(|source| source.id == id)
            .map_or(id, |source| source.name.as_str())
    }

    /// The name of the source Discover is browsing.
    pub fn browsed_source_name(&self) -> &str {
        self.source_name(&self.query.source)
    }
}

#[cfg(test)]
mod tests;
