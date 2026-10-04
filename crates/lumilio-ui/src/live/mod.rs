//! The presentation model for live data (plan 0007, T2).
//!
//! Everything here is owned, window-free and built from `lumilio-core`
//! values by pure functions, so it is unit-tested without a GPUI context. The
//! shell renders it and reports [`LiveIntent`]s; the application decides what
//! they mean.

mod accounts;
mod activity;
mod discover;
mod home;
mod intent;
mod library;
mod settings;

#[cfg(test)]
mod tests;

pub use self::accounts::{AccountRow, account_failure, account_rows, auth_message};
pub use self::activity::{
    ACTIVITY_CATEGORIES, ActivityRow, ActivityState, activity_in_tab, activity_rows, eta_text,
    rate_text, recovery_message,
};
pub use self::discover::{
    AdvancedOption, CardTag, CardTags, DateKind, DiscoverChange, DiscoverQuery, EPILEPSY_TRIGGERS,
    FilterModel, Lock, PAGE_SIZES, PageItem, PickGroup, Provided, SORTS, SearchRow, SearchStatus,
    Section, Side, advanced_options, card_tags, count_label, default_loaders, environment_label,
    is_type_exclusion, page_items, parse_rfc3339, search_row, search_rows, section_title, sections,
    sort_label, tag_label, visible_kinds,
};
pub use self::home::home_presentation;
pub use self::intent::{LiveHandler, LiveIntent};
pub use self::library::{
    CollectionRow, LibraryCard, attention_rows, cover_loader, instance_meta, library_card,
    library_cards, loader_label, relative_time, seed_of, world_of,
};
pub use self::settings::{JavaRow, SettingsView, settings_view};

use self::activity::{RateSample, next_sample};
use lumilio_core::ProjectKind;
use std::collections::BTreeMap;

/// Everything the live pages show.
#[derive(Clone, Debug)]
pub struct LiveModel {
    pub library: Vec<LibraryCard>,
    pub collections: Vec<CollectionRow>,
    /// Games with something wrong, for Home.
    pub attention: Vec<crate::home::AttentionRow>,
    pub library_loaded: bool,
    pub results: Vec<SearchRow>,
    pub search: SearchStatus,
    pub query: DiscoverQuery,
    pub filters: FilterModel,
    /// Which instance Discover installs into.
    pub install_target: Option<String>,
    /// The game Discover was opened from (its Content tab): the page then
    /// browses for that game, with its version and loader locked in. `None`
    /// is plain browsing, installing into `install_target`.
    pub browsing_for: Option<String>,
    /// What Discover remembers between visits.
    pub discover_prefs: lumilio_core::DiscoverPreferences,
    /// Projects (by slug) being installed now.
    pub installing: std::collections::BTreeSet<String>,
    /// What the target game already has of the searched kind, by project.
    pub installed: BTreeMap<String, lumilio_core::InstalledProject>,
    pub activity: Vec<ActivityRow>,
    /// The last reading of each running task, for its speed.
    rates: BTreeMap<u64, RateSample>,
    pub accounts: Vec<AccountRow>,
    pub accounts_loaded: bool,
    pub settings: Option<SettingsView>,
}

impl Default for LiveModel {
    fn default() -> Self {
        Self {
            library: Vec::new(),
            collections: Vec::new(),
            attention: Vec::new(),
            library_loaded: false,
            results: Vec::new(),
            search: SearchStatus::Idle,
            query: DiscoverQuery::new(ProjectKind::Modpack),
            filters: FilterModel::default(),
            install_target: None,
            browsing_for: None,
            discover_prefs: lumilio_core::DiscoverPreferences::default(),
            installing: std::collections::BTreeSet::new(),
            installed: BTreeMap::new(),
            activity: Vec::new(),
            rates: BTreeMap::new(),
            accounts: Vec::new(),
            accounts_loaded: false,
            settings: None,
        }
    }
}

impl LiveModel {
    /// Replaces the Activity rows, working out how fast each running task goes
    /// from how far it got since the last time, and naming each game.
    pub fn set_activity(&mut self, mut rows: Vec<ActivityRow>, now_ms: u64) {
        let mut next = BTreeMap::new();
        for row in &mut rows {
            if let Some(name) = self
                .library
                .iter()
                .find(|card| card.id == row.detail)
                .map(|card| card.name.clone())
            {
                row.detail = name;
            }
            let (Some(task), Some((done, _))) = (row.task, row.amount) else {
                continue;
            };
            let sample = next_sample(self.rates.get(&task).copied(), done, now_ms);
            row.rate = sample.per_sec.filter(|rate| *rate > 0.);
            next.insert(task, sample);
        }
        self.rates = next;
        self.activity = rows;
    }

    /// The games of one collection that are still in the library.
    pub fn collection_cards(&self, collection: &CollectionRow) -> Vec<&LibraryCard> {
        collection
            .members
            .iter()
            .filter_map(|id| self.library.iter().find(|card| &card.id == id))
            .collect()
    }

    /// Each collection with whether this game is in it, for the picker.
    pub fn memberships_of(&self, id: &str) -> Vec<(String, bool)> {
        self.collections
            .iter()
            .map(|collection| {
                (
                    collection.name.clone(),
                    collection.members.iter().any(|member| member == id),
                )
            })
            .collect()
    }

    /// The account later launches use, if one is chosen.
    pub fn selected_account(&self) -> Option<&AccountRow> {
        self.accounts.iter().find(|row| row.selected)
    }

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
        self.query.kind == ProjectKind::Modpack || self.browsing_game().is_some()
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

    /// The projects "hide already installed" leaves out: the packs the
    /// library was made from, or what the browsed game already has.
    pub fn installed_projects(&self) -> Vec<String> {
        if self.query.kind == ProjectKind::Modpack {
            return self
                .library
                .iter()
                .filter_map(|card| card.source_project.clone())
                .collect();
        }
        self.installed.keys().cloned().collect()
    }

    pub fn active_tasks(&self) -> u32 {
        u32::try_from(
            self.activity
                .iter()
                .filter(|row| row.state == ActivityState::Running)
                .count(),
        )
        .unwrap_or(u32::MAX)
    }

    /// Sets the library and keeps the install target valid: the previous
    /// choice if it still exists, else the most recently played instance.
    pub fn set_library(&mut self, library: Vec<LibraryCard>, target_hint: Option<String>) {
        let exists = |id: &String| library.iter().any(|card| &card.id == id);
        let keep = self.install_target.clone().filter(exists);
        self.install_target = keep
            .or(target_hint.filter(exists))
            .or_else(|| library.first().map(|card| card.id.clone()));
        self.library = library;
        self.library_loaded = true;
    }
}
