use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use gpui::RenderImage;
use lumilio_core::ProjectKind;

use super::accounts::AccountRow;
use super::activity::{ActivityRow, ActivityState, RateSample, TrafficReading, next_sample};
use super::discover::{
    DiscoverQuery, FilterModel, Provided, SearchRow, SearchStatus, visible_kinds,
};
use super::home::HomePlaces;
use super::library::{CollectionRow, LibraryCard};
use super::settings::SettingsView;
use super::update::UpdateStatus;

/// Everything the live pages show.
#[derive(Clone, Debug)]
pub struct LiveModel {
    pub settings_presets: Vec<lumilio_core::SettingsPreset>,
    pub game_settings: Vec<lumilio_core::GameSettingsView>,
    pub presets_error: Option<String>,
    pub library: Vec<LibraryCard>,
    pub collections: Vec<CollectionRow>,
    /// Games with something wrong, for Home.
    pub attention: Vec<crate::home::AttentionRow>,
    /// The games are being checked again; `attention` is the last answer.
    pub attention_checking: bool,
    /// Counts changes to `attention`, so Home's section enters again.
    pub attention_revision: usize,
    /// Worlds and servers of the game Home continues, and its record.
    pub home_places: Option<HomePlaces>,
    pub library_loaded: bool,
    pub results: Vec<SearchRow>,
    pub search: SearchStatus,
    /// The content sources that are on, in the launcher's order.
    pub sources: Vec<lumilio_core::ContentSourceInfo>,
    pub query: DiscoverQuery,
    pub filters: FilterModel,
    /// The game Discover was opened from (its Content tab): the page then
    /// browses for that game, with its version and loader locked in. `None`
    /// is plain browsing. Either way installs go into the current game.
    pub browsing_for: Option<String>,
    /// What Discover remembers between visits.
    pub discover_prefs: lumilio_core::DiscoverPreferences,
    /// Projects (by slug) being installed now.
    pub installing: std::collections::BTreeSet<String>,
    /// What the target game already has of the searched kind, by project.
    pub installed: BTreeMap<String, lumilio_core::InstalledProject>,
    pub activity: Vec<ActivityRow>,
    /// The last reading of each running task, for its speed.
    pub(super) rates: BTreeMap<u64, RateSample>,
    /// How all downloads use their shared connections.
    pub traffic: TrafficReading,
    pub accounts: Vec<AccountRow>,
    pub accounts_loaded: bool,
    /// Each account's skin face, by key. `Some(None)` is "asked, no face",
    /// so an account that has none is not asked for again and again.
    pub faces: HashMap<String, Option<Arc<RenderImage>>>,
    pub settings: Option<SettingsView>,
    pub update_status: UpdateStatus,
}

impl Default for LiveModel {
    fn default() -> Self {
        Self {
            settings_presets: Vec::new(),
            game_settings: Vec::new(),
            presets_error: None,
            library: Vec::new(),
            collections: Vec::new(),
            attention: Vec::new(),
            attention_checking: false,
            attention_revision: 0,
            home_places: None,
            library_loaded: false,
            results: Vec::new(),
            search: SearchStatus::Idle,
            sources: Vec::new(),
            query: DiscoverQuery::new(ProjectKind::Modpack),
            filters: FilterModel::default(),
            browsing_for: None,
            discover_prefs: lumilio_core::DiscoverPreferences::default(),
            installing: std::collections::BTreeSet::new(),
            installed: BTreeMap::new(),
            activity: Vec::new(),
            rates: BTreeMap::new(),
            traffic: TrafficReading::default(),
            accounts: Vec::new(),
            accounts_loaded: false,
            faces: HashMap::new(),
            settings: None,
            update_status: UpdateStatus::default(),
        }
    }
}

impl LiveModel {
    /// Replaces the Activity rows, working out how fast each running task goes
    /// from how far it got since the last time, and naming each game.
    pub fn set_activity(&mut self, mut rows: Vec<ActivityRow>, now_ms: u64) {
        let mut next = BTreeMap::new();
        for row in &mut rows {
            match self
                .library
                .iter()
                .find(|card| card.id == row.detail)
                .map(|card| card.name.clone())
            {
                Some(name) => row.detail = name,
                // A game that is gone (or not made yet) has no name to show,
                // and its id is no name either.
                None => row.detail.clear(),
            }
            let (Some(task), Some((done, _))) = (row.task, row.amount) else {
                continue;
            };
            let sample = next_sample(self.rates.get(&task).copied(), done, now_ms);
            // A held task is not moving, whatever the smoothing remembers.
            row.rate = sample.per_sec.filter(|rate| *rate > 0. && !row.paused);
            next.insert(task, sample);
        }
        self.rates = next;
        self.activity = rows;
    }

    /// Takes a new reading of the shared connections at `now_ms`.
    pub fn set_traffic(&mut self, traffic: lumilio_core::Traffic, now_ms: u64) {
        self.traffic = self.traffic.next(traffic, now_ms);
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

    /// Replaces the account rows, keeping the faces of accounts that are still
    /// here so they are not fetched again on every refresh.
    pub fn set_accounts(&mut self, accounts: Vec<AccountRow>) {
        let keys: std::collections::HashSet<String> =
            accounts.iter().map(|row| row.key.clone()).collect();
        self.faces.retain(|key, _| keys.contains(key));
        self.accounts = accounts;
        self.accounts_loaded = true;
    }

    /// The face of an account, if one has been loaded.
    #[must_use]
    pub fn face_of(&self, key: &str) -> Option<Arc<RenderImage>> {
        self.faces.get(key).cloned().flatten()
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

    pub fn active_tasks(&self) -> u32 {
        u32::try_from(
            self.activity
                .iter()
                .filter(|row| row.state == ActivityState::Running)
                .count(),
        )
        .unwrap_or(u32::MAX)
    }

    /// Sets the library. Which game is current is core's to say.
    pub fn set_library(&mut self, library: Vec<LibraryCard>) {
        self.library = library;
        self.library_loaded = true;
    }
}
