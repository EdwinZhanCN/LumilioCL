//! Launcher-wide settings and the accounts the user has added.
//!
//! Behavior notes: `docs/behavior/settings.md`. Per-instance overrides live on
//! the instance record; these are the defaults they fall back to.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::account::{OfflineProfile, ProfileError, ProfileId};
use crate::persist::{self, PersistError, Versioned};
use crate::transfer::{OfficialSource, PrefixMirror, SourceChain, SourceProvider};
use crate::tuning::{DOWNLOAD_CONCURRENCY, LaunchTuning, Preferences, TuningError};

const FILE: &str = "settings.json";
const SCHEMA_VERSION: u32 = 1;

/// Largest memory value accepted, in MB: a typo like an extra digit must not
/// produce a launch that cannot start.
pub const MAX_MEMORY_MB: u32 = 1024 * 1024;

/// Shared validation for stored defaults, instance overrides and resolved launches.
pub(crate) fn validate_memory(min: Option<u32>, max: Option<u32>) -> Result<(), SettingsError> {
    let in_range = |value: Option<u32>| value.is_none_or(|v| (1..=MAX_MEMORY_MB).contains(&v));
    if !in_range(min) || !in_range(max) || matches!((min, max), (Some(a), Some(b)) if a > b) {
        return Err(SettingsError::InvalidMemory);
    }
    Ok(())
}

/// Rewrites addresses under `official_prefix` to `mirror_prefix`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirrorRule {
    pub official_prefix: String,
    pub mirror_prefix: String,
}

/// What kind of identity an account is.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    #[default]
    Offline,
    /// Signed in with Microsoft; its secrets are in the system credential store.
    Microsoft,
}

impl AccountKind {
    const fn is_offline(&self) -> bool {
        matches!(self, Self::Offline)
    }
}

/// An account as `settings.json` keeps it: public facts only (ADR 0013).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AccountEntry {
    /// The profile name: an offline account's identity, and a Microsoft
    /// account's current display name.
    pub name: String,
    /// The profile id (32 lowercase hex digits). Absent on an offline account
    /// means "derived from the name"; always present on a Microsoft account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "AccountKind::is_offline")]
    pub kind: AccountKind,
    /// The stored sign-in can no longer be refreshed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub needs_sign_in: bool,
}

impl AccountEntry {
    /// What selection and the credential store call this account: the name
    /// for an offline account (as always), `msa:<profile id>` for Microsoft.
    #[must_use]
    pub fn key(&self) -> String {
        match (self.kind, &self.uuid) {
            (AccountKind::Microsoft, Some(uuid)) => format!("msa:{uuid}"),
            _ => self.name.clone(),
        }
    }

    /// The profile id the game sees, honoring a chosen or signed-in id.
    pub fn profile_id(&self) -> Result<ProfileId, ProfileError> {
        match self.uuid.as_deref() {
            Some(uuid) => ProfileId::parse(uuid),
            None => Ok(ProfileId::offline(&self.name)),
        }
    }

    /// The offline profile this entry stands for, honoring a custom id.
    pub fn profile(&self) -> Result<OfflineProfile, ProfileError> {
        let id = self.uuid.as_deref().map(ProfileId::parse).transpose()?;
        OfflineProfile::with_id(&self.name, id)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct LauncherSettings {
    schema: u32,
    pub default_max_memory_mb: Option<u32>,
    pub default_min_memory_mb: Option<u32>,
    /// Extra folders searched for Java installations.
    pub extra_java_roots: Vec<PathBuf>,
    pub mirrors: Vec<MirrorRule>,
    /// Try mirrors before the official address.
    pub prefer_mirrors: bool,
    pub accounts: Vec<AccountEntry>,
    pub selected_account: Option<String>,
    /// The instance the launcher plays and installs into by default.
    pub current_instance: Option<String>,
    pub preferences: Preferences,
    /// How many files download at once; `None` leaves it to the launcher.
    pub download_concurrency: Option<u32>,
    /// Java installations the user turned off; they are never chosen.
    pub disabled_java: Vec<PathBuf>,
    /// How the game starts, for every instance that does not override it.
    pub launch: LaunchTuning,
}

impl Versioned for LauncherSettings {
    fn schema(&self) -> u32 {
        self.schema
    }
    fn set_schema(&mut self, schema: u32) {
        self.schema = schema;
    }
}

#[derive(Debug)]
pub enum SettingsError {
    Persist(PersistError),
    Profile(ProfileError),
    DuplicateAccount(String),
    /// Another account already uses this profile id.
    DuplicateUuid(String),
    UnknownAccount(String),
    /// Memory must be 1..=[`MAX_MEMORY_MB`], and minimum ≤ maximum.
    InvalidMemory,
    InvalidMirror,
    Tuning(TuningError),
    /// Downloads at once must be 1..=32.
    InvalidConcurrency,
}

impl Display for SettingsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Persist(error) => write!(f, "settings {error}"),
            Self::Profile(error) => write!(f, "{error}"),
            Self::DuplicateAccount(name) => write!(f, "account {name:?} already exists"),
            Self::DuplicateUuid(uuid) => write!(f, "profile id {uuid} is already used"),
            Self::UnknownAccount(name) => write!(f, "no account {name:?}"),
            Self::InvalidMemory => f.write_str("memory values are out of range"),
            Self::InvalidMirror => f.write_str("mirror prefixes must not be empty"),
            Self::Tuning(error) => write!(f, "{error}"),
            Self::InvalidConcurrency => f.write_str("downloads at once must be 1 to 32"),
        }
    }
}

impl Error for SettingsError {}

impl From<PersistError> for SettingsError {
    fn from(error: PersistError) -> Self {
        Self::Persist(error)
    }
}

impl From<TuningError> for SettingsError {
    fn from(error: TuningError) -> Self {
        Self::Tuning(error)
    }
}

impl From<ProfileError> for SettingsError {
    fn from(error: ProfileError) -> Self {
        Self::Profile(error)
    }
}

pub struct SettingsStore {
    path: PathBuf,
    settings: LauncherSettings,
    recovered: bool,
    preserved: Option<PathBuf>,
}

impl SettingsStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let path = root.as_ref().join(FILE);
        let loaded = persist::load::<LauncherSettings>(&path, SCHEMA_VERSION)?;
        Ok(Self {
            path,
            settings: loaded.value,
            recovered: loaded.recovered,
            preserved: loaded.preserved,
        })
    }

    #[must_use]
    pub const fn recovered_from_damage(&self) -> bool {
        self.recovered
    }

    /// Where the unreadable settings file was kept, if it had to be set aside.
    #[must_use]
    pub fn preserved_original(&self) -> Option<&Path> {
        self.preserved.as_deref()
    }

    #[must_use]
    pub const fn get(&self) -> &LauncherSettings {
        &self.settings
    }

    fn save(&mut self, mut next: LauncherSettings) -> Result<(), SettingsError> {
        persist::save(&self.path, &mut next, SCHEMA_VERSION)?;
        self.settings = next;
        Ok(())
    }

    /// Sets the default memory limits. `None` clears a limit. Values must be
    /// positive and at most [`MAX_MEMORY_MB`], and the minimum may not exceed
    /// the maximum.
    pub fn set_memory(
        &mut self,
        min_mb: Option<u32>,
        max_mb: Option<u32>,
    ) -> Result<(), SettingsError> {
        validate_memory(min_mb, max_mb)?;
        let mut next = self.settings.clone();
        next.default_min_memory_mb = min_mb;
        next.default_max_memory_mb = max_mb;
        self.save(next)
    }

    pub fn set_preferences(&mut self, preferences: Preferences) -> Result<(), SettingsError> {
        let mut next = self.settings.clone();
        next.preferences = preferences;
        self.save(next)
    }

    /// Replaces the launch defaults after trimming blanks and checking them.
    pub fn set_launch_defaults(&mut self, launch: LaunchTuning) -> Result<(), SettingsError> {
        let launch = launch.normalized();
        launch.validate()?;
        let mut next = self.settings.clone();
        next.launch = launch;
        self.save(next)
    }

    /// `None` clears the choice.
    pub fn set_download_concurrency(&mut self, count: Option<u32>) -> Result<(), SettingsError> {
        if count.is_some_and(|count| !DOWNLOAD_CONCURRENCY.contains(&count)) {
            return Err(SettingsError::InvalidConcurrency);
        }
        let mut next = self.settings.clone();
        next.download_concurrency = count;
        self.save(next)
    }

    pub fn set_disabled_java(&mut self, homes: Vec<PathBuf>) -> Result<(), SettingsError> {
        let mut next = self.settings.clone();
        next.disabled_java = homes;
        self.save(next)
    }

    pub fn set_java_roots(&mut self, roots: Vec<PathBuf>) -> Result<(), SettingsError> {
        let mut next = self.settings.clone();
        next.extra_java_roots = roots;
        self.save(next)
    }

    pub fn set_mirrors(
        &mut self,
        mirrors: Vec<MirrorRule>,
        prefer: bool,
    ) -> Result<(), SettingsError> {
        if mirrors.iter().any(|rule| {
            rule.official_prefix.trim().is_empty() || rule.mirror_prefix.trim().is_empty()
        }) {
            return Err(SettingsError::InvalidMirror);
        }
        let mut next = self.settings.clone();
        next.mirrors = mirrors;
        next.prefer_mirrors = prefer;
        self.save(next)
    }

    /// Adds an offline account; the first account becomes the selected one.
    /// `uuid` overrides the name-derived profile id and is only settable here.
    pub fn add_offline_account(
        &mut self,
        name: &str,
        uuid: Option<&str>,
    ) -> Result<(), SettingsError> {
        let id = uuid
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ProfileId::parse)
            .transpose()?;
        let profile = OfflineProfile::with_id(name, id)?;
        if self.settings.accounts.iter().any(|entry| {
            entry.kind == AccountKind::Offline && entry.name.eq_ignore_ascii_case(profile.name())
        }) {
            return Err(SettingsError::DuplicateAccount(profile.name().to_owned()));
        }
        if self
            .settings
            .accounts
            .iter()
            .filter_map(|entry| entry.profile_id().ok())
            .any(|other| other == profile.id())
        {
            return Err(SettingsError::DuplicateUuid(profile.id().compact()));
        }
        let mut next = self.settings.clone();
        next.accounts.push(AccountEntry {
            name: profile.name().to_owned(),
            uuid: id.map(|id| id.compact()),
            ..AccountEntry::default()
        });
        if next.selected_account.is_none() {
            next.selected_account = Some(profile.name().to_owned());
        }
        self.save(next)
    }

    /// Records a Microsoft sign-in. The account is found by its profile id,
    /// so signing in again (or after a rename) updates it instead of adding a
    /// second one; it stops needing a sign-in. The first account becomes the
    /// selected one. Returns the account's key.
    pub fn sign_in_microsoft(
        &mut self,
        profile_id: ProfileId,
        name: &str,
    ) -> Result<String, SettingsError> {
        let compact = profile_id.compact();
        let mut next = self.settings.clone();
        let existing = next.accounts.iter_mut().find(|entry| {
            entry.kind == AccountKind::Microsoft && entry.uuid.as_deref() == Some(compact.as_str())
        });
        match existing {
            Some(entry) => {
                name.clone_into(&mut entry.name);
                entry.needs_sign_in = false;
            }
            None => {
                // The same id already used by an offline account would make
                // two identities indistinguishable to the game.
                if next
                    .accounts
                    .iter()
                    .filter_map(|entry| entry.profile_id().ok())
                    .any(|other| other == profile_id)
                {
                    return Err(SettingsError::DuplicateUuid(compact));
                }
                next.accounts.push(AccountEntry {
                    name: name.to_owned(),
                    uuid: Some(compact.clone()),
                    kind: AccountKind::Microsoft,
                    needs_sign_in: false,
                });
            }
        }
        let key = format!("msa:{compact}");
        if next.selected_account.is_none() {
            next.selected_account = Some(key.clone());
        }
        self.save(next)?;
        Ok(key)
    }

    /// Marks an account as needing (or no longer needing) a new sign-in.
    pub fn set_needs_sign_in(&mut self, key: &str, needed: bool) -> Result<(), SettingsError> {
        let mut next = self.settings.clone();
        let entry = next
            .accounts
            .iter_mut()
            .find(|entry| entry.key() == key)
            .ok_or_else(|| SettingsError::UnknownAccount(key.to_owned()))?;
        if entry.needs_sign_in == needed {
            return Ok(());
        }
        entry.needs_sign_in = needed;
        self.save(next)
    }

    /// Chooses the account later launches use, by key.
    pub fn select_account(&mut self, key: &str) -> Result<(), SettingsError> {
        if !self
            .settings
            .accounts
            .iter()
            .any(|entry| entry.key() == key)
        {
            return Err(SettingsError::UnknownAccount(key.to_owned()));
        }
        let mut next = self.settings.clone();
        next.selected_account = Some(key.to_owned());
        self.save(next)
    }

    /// Removes an account by key; if it was selected, the first remaining one is.
    pub fn remove_account(&mut self, key: &str) -> Result<(), SettingsError> {
        let mut next = self.settings.clone();
        let before = next.accounts.len();
        next.accounts.retain(|entry| entry.key() != key);
        if next.accounts.len() == before {
            return Err(SettingsError::UnknownAccount(key.to_owned()));
        }
        if next.selected_account.as_deref() == Some(key) {
            next.selected_account = next.accounts.first().map(AccountEntry::key);
        }
        self.save(next)
    }

    /// The selected account, if there is one.
    pub fn selected_entry(&self) -> Option<&AccountEntry> {
        let key = self.settings.selected_account.as_deref()?;
        self.settings
            .accounts
            .iter()
            .find(|entry| entry.key() == key)
    }

    /// The offline account to launch with, if the selected one is offline.
    pub fn selected_profile(&self) -> Option<OfflineProfile> {
        self.selected_entry()
            .filter(|entry| entry.kind == AccountKind::Offline)
            .and_then(|entry| entry.profile().ok())
    }

    /// Remembers the current instance; `None` clears it.
    pub fn set_current_instance(&mut self, id: Option<String>) -> Result<(), SettingsError> {
        if self.settings.current_instance == id {
            return Ok(());
        }
        let mut next = self.settings.clone();
        next.current_instance = id;
        self.save(next)
    }

    /// The download source order these settings describe: mirrors and the
    /// official address, in the configured preference.
    pub fn source_chain(&self) -> Result<SourceChain, SettingsError> {
        let mut mirrors: Vec<Arc<dyn SourceProvider>> = Vec::new();
        for rule in &self.settings.mirrors {
            let mirror = PrefixMirror::new(&rule.official_prefix, &rule.mirror_prefix, 16)
                .map_err(|_| SettingsError::InvalidMirror)?;
            mirrors.push(Arc::new(mirror));
        }
        let official: Arc<dyn SourceProvider> = Arc::new(OfficialSource);
        let providers = if self.settings.prefer_mirrors {
            mirrors.into_iter().chain([official]).collect::<Vec<_>>()
        } else {
            std::iter::once(official).chain(mirrors).collect()
        };
        SourceChain::new(providers).map_err(|_| SettingsError::InvalidMirror)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, SettingsStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::open(dir.path()).unwrap();
        (dir, store)
    }

    #[test]
    fn failed_settings_writes_preserve_saved_values_and_allow_retry() {
        for publication_failure in [false, true] {
            for operation in 0..6 {
                let (dir, mut store) = store();
                store.add_offline_account("Steve", None).unwrap();
                store.add_offline_account("Alex", None).unwrap();
                store.set_memory(Some(512), Some(2048)).unwrap();
                let before = store.get().clone();
                let saved = std::fs::read(&store.path).unwrap();
                let mutate = |store: &mut SettingsStore| -> Result<(), SettingsError> {
                    match operation {
                        0 => store.set_memory(Some(1024), Some(4096)),
                        1 => store.set_java_roots(vec!["/extra/jdk".into()]),
                        2 => store.set_mirrors(
                            vec![MirrorRule {
                                official_prefix: "https://official.example/".into(),
                                mirror_prefix: "https://mirror.example/".into(),
                            }],
                            true,
                        ),
                        3 => store.add_offline_account("New", None),
                        4 => store.select_account("Alex"),
                        _ => store.remove_account("Steve"),
                    }
                };
                let blocked = if publication_failure {
                    std::fs::rename(&store.path, dir.path().join("saved.json")).unwrap();
                    store.path.clone()
                } else {
                    dir.path().join("settings.json.tmp")
                };
                std::fs::create_dir(&blocked).unwrap();
                assert!(mutate(&mut store).is_err());
                assert_eq!(store.get(), &before, "operation {operation}");
                assert_eq!(store.selected_profile().unwrap().name(), "Steve");
                std::fs::remove_dir(&blocked).unwrap();
                if publication_failure {
                    std::fs::rename(dir.path().join("saved.json"), &store.path).unwrap();
                }
                assert_eq!(std::fs::read(&store.path).unwrap(), saved);
                assert_eq!(SettingsStore::open(dir.path()).unwrap().get(), &before);
                mutate(&mut store).unwrap();
                assert_eq!(SettingsStore::open(dir.path()).unwrap().get(), store.get());
            }
        }
    }

    #[test]
    fn settings_files_from_before_custom_ids_and_current_instance_still_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            r#"{"schema":1,"accounts":[{"name":"Steve"}],"selected_account":"Steve"}"#,
        )
        .unwrap();
        let store = SettingsStore::open(dir.path()).unwrap();
        assert!(!store.recovered_from_damage());
        assert_eq!(store.get().current_instance, None);
        let steve = store.selected_profile().unwrap();
        assert_eq!(steve.id(), ProfileId::offline("Steve"));
    }

    #[test]
    fn a_custom_id_wins_over_the_name_and_clashes_are_refused() {
        let (dir, mut store) = store();
        store
            .add_offline_account("Steve", Some("123e4567-e89b-12d3-a456-426614174000"))
            .unwrap();
        assert_eq!(
            store.selected_profile().unwrap().id().compact(),
            "123e4567e89b12d3a456426614174000"
        );
        assert!(matches!(
            store.add_offline_account("Alex", Some("123e4567e89b12d3a456426614174000")),
            Err(SettingsError::DuplicateUuid(_))
        ));
        for bad in ["xyz", "123e4567-e89b-12d3-a456", "0".repeat(32).as_str()] {
            assert!(matches!(
                store.add_offline_account("Alex", Some(bad)),
                Err(SettingsError::Profile(ProfileError::InvalidId))
            ));
        }
        // Blank means "derive from the name".
        store.add_offline_account("Alex", Some("  ")).unwrap();
        let reopened = SettingsStore::open(dir.path()).unwrap();
        let alex = &reopened.get().accounts[1];
        assert_eq!(alex.uuid, None);
        assert_eq!(alex.profile().unwrap().id(), ProfileId::offline("Alex"));
    }

    #[test]
    fn the_current_instance_persists() {
        let (dir, mut store) = store();
        store.set_current_instance(Some("abc".into())).unwrap();
        assert_eq!(
            SettingsStore::open(dir.path())
                .unwrap()
                .get()
                .current_instance
                .as_deref(),
            Some("abc")
        );
        store.set_current_instance(None).unwrap();
        assert_eq!(
            SettingsStore::open(dir.path())
                .unwrap()
                .get()
                .current_instance,
            None
        );
    }

    #[test]
    fn preferences_launch_defaults_and_concurrency_persist_and_validate() {
        use crate::tuning::{AfterLaunch, Appearance, EnvVar};
        let (dir, mut store) = store();
        let preferences = Preferences {
            appearance: Appearance::Dark,
            after_launch: AfterLaunch::Hide,
            foreground_on_exit: Some(false),
            ..Preferences::default()
        };
        store.set_preferences(preferences.clone()).unwrap();
        store
            .set_launch_defaults(LaunchTuning {
                window_width: Some(1280),
                window_height: Some(720),
                fullscreen: Some(false),
                game_arguments: vec![" --demo ".into(), "".into()],
                environment: vec![EnvVar {
                    name: "A".into(),
                    value: "1".into(),
                }],
                wrapper: Some("  ".into()),
                ..LaunchTuning::default()
            })
            .unwrap();
        store.set_download_concurrency(Some(8)).unwrap();
        store
            .set_disabled_java(vec!["/opt/old-jdk".into()])
            .unwrap();
        let reopened = SettingsStore::open(dir.path()).unwrap();
        let saved = reopened.get();
        assert_eq!(saved.preferences, preferences);
        assert_eq!(
            saved.launch.game_arguments,
            ["--demo"],
            "blank lines are dropped"
        );
        assert_eq!(saved.launch.wrapper, None);
        assert_eq!(saved.download_concurrency, Some(8));
        assert_eq!(saved.disabled_java, [PathBuf::from("/opt/old-jdk")]);

        // Rejections change nothing.
        let before = store.get().clone();
        assert!(matches!(
            store.set_launch_defaults(LaunchTuning {
                window_width: Some(800),
                ..LaunchTuning::default()
            }),
            Err(SettingsError::Tuning(_))
        ));
        for bad in [0, 33] {
            assert!(matches!(
                store.set_download_concurrency(Some(bad)),
                Err(SettingsError::InvalidConcurrency)
            ));
        }
        assert_eq!(store.get(), &before);
        store.set_download_concurrency(None).unwrap();
        assert_eq!(store.get().download_concurrency, None);
    }

    #[test]
    fn a_microsoft_account_is_found_by_its_profile_id_and_survives_a_rename() {
        let (dir, mut store) = store();
        let id = ProfileId::parse("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let key = store.sign_in_microsoft(id, "Edwin_Zhan").unwrap();
        assert_eq!(key, "msa:123e4567e89b12d3a456426614174000");
        assert_eq!(store.get().selected_account.as_deref(), Some(key.as_str()));
        assert_eq!(store.selected_entry().unwrap().kind, AccountKind::Microsoft);
        assert!(store.selected_profile().is_none(), "not an offline account");

        // Signing in again after a rename updates the one account.
        store.set_needs_sign_in(&key, true).unwrap();
        assert!(store.get().accounts[0].needs_sign_in);
        store.sign_in_microsoft(id, "New_Name").unwrap();
        let accounts = &store.get().accounts;
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].name, "New_Name");
        assert!(!accounts[0].needs_sign_in, "a new sign-in clears the flag");

        // An offline account may share the name; the keys differ.
        store.add_offline_account("New_Name", None).unwrap();
        assert_eq!(store.get().accounts.len(), 2);
        store.select_account("New_Name").unwrap();
        assert_eq!(store.selected_profile().unwrap().name(), "New_Name");
        store.select_account(&key).unwrap();

        // Public facts only reach the file, and it reads back the same.
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(
            text.contains("microsoft") && !text.to_lowercase().contains("token"),
            "{text}"
        );
        assert_eq!(SettingsStore::open(dir.path()).unwrap().get(), store.get());

        // Removing it by key reselects what remains.
        store.remove_account(&key).unwrap();
        assert_eq!(store.get().selected_account.as_deref(), Some("New_Name"));
        assert!(matches!(
            store.set_needs_sign_in(&key, true),
            Err(SettingsError::UnknownAccount(_))
        ));
    }

    #[test]
    fn a_microsoft_id_cannot_collide_with_an_offline_account() {
        let (_dir, mut store) = store();
        store
            .add_offline_account("Steve", Some("123e4567e89b12d3a456426614174000"))
            .unwrap();
        let same = ProfileId::parse("123e4567e89b12d3a456426614174000").unwrap();
        assert!(matches!(
            store.sign_in_microsoft(same, "Other"),
            Err(SettingsError::DuplicateUuid(_))
        ));
        assert_eq!(store.get().accounts.len(), 1);
    }

    #[test]
    fn defaults_are_empty_and_settings_persist() {
        let (dir, mut store) = store();
        assert_eq!(store.get().default_max_memory_mb, None);
        store.set_memory(Some(512), Some(4096)).unwrap();
        store.set_java_roots(vec!["/opt/jdks".into()]).unwrap();
        let reopened = SettingsStore::open(dir.path()).unwrap();
        assert_eq!(reopened.get().default_max_memory_mb, Some(4096));
        assert_eq!(
            reopened.get().extra_java_roots,
            [PathBuf::from("/opt/jdks")]
        );
    }

    #[test]
    fn memory_is_validated_and_a_bad_value_changes_nothing() {
        let (_dir, mut store) = store();
        store.set_memory(None, Some(2048)).unwrap();
        for (min, max) in [
            (Some(0), None),
            (None, Some(MAX_MEMORY_MB + 1)),
            (Some(4096), Some(1024)),
        ] {
            assert!(matches!(
                store.set_memory(min, max),
                Err(SettingsError::InvalidMemory)
            ));
        }
        assert_eq!(store.get().default_max_memory_mb, Some(2048));
    }

    #[test]
    fn accounts_first_becomes_selected_and_removal_reselects() {
        let (_dir, mut store) = store();
        assert!(store.selected_profile().is_none());
        store.add_offline_account("Steve", None).unwrap();
        store.add_offline_account("Alex", None).unwrap();
        assert_eq!(store.selected_profile().unwrap().name(), "Steve");
        assert!(matches!(
            store.add_offline_account("steve", None),
            Err(SettingsError::DuplicateAccount(_))
        ));
        assert!(matches!(
            store.add_offline_account("bad name", None),
            Err(SettingsError::Profile(_))
        ));
        store.select_account("Alex").unwrap();
        store.remove_account("Alex").unwrap();
        assert_eq!(store.selected_profile().unwrap().name(), "Steve");
        store.remove_account("Steve").unwrap();
        assert!(store.selected_profile().is_none());
        assert!(matches!(
            store.select_account("ghost"),
            Err(SettingsError::UnknownAccount(_))
        ));
    }

    #[test]
    fn mirrors_order_follows_the_preference() {
        let (_dir, mut store) = store();
        let rule = MirrorRule {
            official_prefix: "https://piston-data.mojang.com".into(),
            mirror_prefix: "https://mirror.example/data".into(),
        };
        let url = "https://piston-data.mojang.com/v1/x.jar";
        store.set_mirrors(vec![rule.clone()], false).unwrap();
        assert_eq!(
            store.source_chain().unwrap().candidates(url),
            [url, "https://mirror.example/data/v1/x.jar"]
        );
        store.set_mirrors(vec![rule], true).unwrap();
        assert_eq!(
            store.source_chain().unwrap().candidates(url),
            ["https://mirror.example/data/v1/x.jar", url]
        );
        // With no mirrors, only the official address remains.
        store.set_mirrors(vec![], true).unwrap();
        assert_eq!(store.source_chain().unwrap().candidates(url), [url]);
    }

    #[test]
    fn empty_mirror_prefixes_are_rejected() {
        let (_dir, mut store) = store();
        let bad = MirrorRule {
            official_prefix: " ".into(),
            mirror_prefix: "x".into(),
        };
        assert!(matches!(
            store.set_mirrors(vec![bad], false),
            Err(SettingsError::InvalidMirror)
        ));
        assert!(store.get().mirrors.is_empty());
    }

    #[test]
    fn damaged_and_newer_files_follow_the_shared_policy() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), "garbage").unwrap();
        let store = SettingsStore::open(dir.path()).unwrap();
        assert!(store.recovered_from_damage());
        std::fs::write(dir.path().join("settings.json"), r#"{"schema": 42}"#).unwrap();
        assert!(matches!(
            SettingsStore::open(dir.path()),
            Err(SettingsError::Persist(PersistError::NewerSchema(42)))
        ));
    }
}
