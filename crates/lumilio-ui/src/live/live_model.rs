use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use gpui::RenderImage;

use super::accounts::AccountRow;
use super::activity::{ActivityRow, ActivityState, RateSample, TrafficReading, next_sample};
use super::home::HomePlaces;
use super::library::{CollectionRow, LibraryCard};
use super::update::UpdateStatus;

/// Everything the live pages show.
#[derive(Clone, Debug, Default)]
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
    pub update_status: UpdateStatus,
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
