//! Launcher-wide settings and the accounts the user has added.
//!
//! Per-instance overrides live on the instance record; these are the defaults
//! they fall back to.

use std::collections::BTreeMap;
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

/// Candidate order for downloads. MCIM is a fallback in both priority modes.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadSourcePreference {
    OfficialOnly,
    #[default]
    OfficialFirst,
    MirrorFirst,
}

/// What kind of identity an account is.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    #[default]
    Offline,
    /// Signed in with Microsoft; its secrets are in the system credential store.
    Microsoft,
    /// Signed in on an authlib-injector server (`server`); its secrets are in
    /// the system credential store too.
    ThirdParty,
}

impl AccountKind {
    const fn is_offline(&self) -> bool {
        matches!(self, Self::Offline)
    }
}

/// An account as `settings.json` keeps it: public facts only (ADR 0020).
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
    /// A third-party account's authentication server: its API root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// A third-party account's login name (an email or a user name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    /// How an offline account looks in the game; none is the game's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin: Option<crate::skin::SkinChoice>,
}

/// An authentication server the person added (public facts only; LittleSkin
/// is built in and never listed here).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AuthServerEntry {
    /// The API root, ending with `/`.
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub non_email_login: bool,
}

impl AccountEntry {
    /// What selection and the credential store call this account: the name
    /// for an offline account (as always), `msa:<profile id>` for Microsoft.
    #[must_use]
    pub fn key(&self) -> String {
        match (self.kind, &self.uuid, &self.server) {
            (AccountKind::Microsoft, Some(uuid), _) => format!("msa:{uuid}"),
            (AccountKind::ThirdParty, Some(uuid), Some(server)) => format!("ali:{uuid}@{server}"),
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct LauncherSettings {
    schema: u32,
    #[serde(default)]
    pub plugins: BTreeMap<String, lumilio_plugin_api::PluginState>,
    pub default_max_memory_mb: Option<u32>,
    pub default_min_memory_mb: Option<u32>,
    /// Extra folders searched for Java installations.
    pub extra_java_roots: Vec<PathBuf>,
    pub mirrors: Vec<MirrorRule>,
    /// Legacy preference, retained for older settings and callers.
    pub prefer_mirrors: bool,
    /// Absent in older files; their `prefer_mirrors` supplies the preference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_source: Option<DownloadSourcePreference>,
    pub accounts: Vec<AccountEntry>,
    /// Authentication servers added besides the built-in LittleSkin.
    pub auth_servers: Vec<AuthServerEntry>,
    pub selected_account: Option<String>,
    /// The instance the launcher plays and installs into by default.
    pub current_instance: Option<String>,
    pub preferences: Preferences,
    /// How many files download at once; `None` leaves it to the launcher.
    pub download_concurrency: Option<u32>,
    /// Java installations the user turned off; they are never chosen.
    pub disabled_java: Vec<PathBuf>,
    /// Whether the launcher checks for and downloads stable launcher updates.
    #[serde(default = "auto_update_default")]
    pub auto_update: bool,
    /// How the game starts, for every instance that does not override it.
    pub launch: LaunchTuning,
}

fn auto_update_default() -> bool {
    true
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            schema: 0,
            plugins: BTreeMap::new(),
            default_max_memory_mb: None,
            default_min_memory_mb: None,
            extra_java_roots: Vec::new(),
            mirrors: Vec::new(),
            prefer_mirrors: false,
            download_source: None,
            accounts: Vec::new(),
            auth_servers: Vec::new(),
            selected_account: None,
            current_instance: None,
            preferences: Preferences::default(),
            download_concurrency: None,
            disabled_java: Vec::new(),
            auto_update: true,
            launch: LaunchTuning::default(),
        }
    }
}

impl Versioned for LauncherSettings {
    fn schema(&self) -> u32 {
        self.schema
    }
    fn set_schema(&mut self, schema: u32) {
        self.schema = schema;
    }
}

impl LauncherSettings {
    #[must_use]
    pub fn download_source_preference(&self) -> DownloadSourcePreference {
        self.download_source.unwrap_or(if self.prefer_mirrors {
            DownloadSourcePreference::MirrorFirst
        } else {
            DownloadSourcePreference::OfficialFirst
        })
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
    UnknownServer(String),
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
            Self::UnknownServer(url) => write!(f, "no authentication server {url:?}"),
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
    pub fn set_plugin(
        &mut self,
        id: String,
        state: lumilio_plugin_api::PluginState,
    ) -> Result<(), SettingsError> {
        let mut next = self.settings.clone();
        next.plugins.insert(id, state);
        self.save(next)
    }

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

    pub fn set_auto_update(&mut self, enabled: bool) -> Result<(), SettingsError> {
        let mut next = self.settings.clone();
        next.auto_update = enabled;
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
        next.download_source = Some(if prefer {
            DownloadSourcePreference::MirrorFirst
        } else {
            DownloadSourcePreference::OfficialFirst
        });
        self.save(next)
    }

    /// Replace rules while preserving the current download source preference.
    pub fn set_mirror_rules(&mut self, mirrors: Vec<MirrorRule>) -> Result<(), SettingsError> {
        if mirrors.iter().any(|rule| {
            rule.official_prefix.trim().is_empty() || rule.mirror_prefix.trim().is_empty()
        }) {
            return Err(SettingsError::InvalidMirror);
        }
        let mut next = self.settings.clone();
        next.mirrors = mirrors;
        self.save(next)
    }

    pub fn set_download_source(
        &mut self,
        preference: DownloadSourcePreference,
    ) -> Result<(), SettingsError> {
        let mut next = self.settings.clone();
        next.download_source = Some(preference);
        next.prefer_mirrors = preference == DownloadSourcePreference::MirrorFirst;
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
                    ..AccountEntry::default()
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

    /// Remembers an authentication server (or updates what is known of it).
    /// LittleSkin is built in and needs no entry.
    pub fn add_auth_server(&mut self, server: AuthServerEntry) -> Result<(), SettingsError> {
        if server.url == crate::yggdrasil::LITTLE_SKIN_URL {
            return Ok(());
        }
        let mut next = self.settings.clone();
        match next
            .auth_servers
            .iter_mut()
            .find(|known| known.url == server.url)
        {
            Some(known) => *known = server,
            None => next.auth_servers.push(server),
        }
        self.save(next)
    }

    /// Forgets an authentication server and the accounts signed in on it;
    /// returns the keys of the removed accounts, whose secrets the caller
    /// deletes.
    pub fn remove_auth_server(&mut self, url: &str) -> Result<Vec<String>, SettingsError> {
        let mut next = self.settings.clone();
        let before = next.auth_servers.len();
        next.auth_servers.retain(|server| server.url != url);
        if next.auth_servers.len() == before {
            return Err(SettingsError::UnknownServer(url.to_owned()));
        }
        let gone: Vec<String> = next
            .accounts
            .iter()
            .filter(|entry| {
                entry.kind == AccountKind::ThirdParty && entry.server.as_deref() == Some(url)
            })
            .map(AccountEntry::key)
            .collect();
        next.accounts.retain(|entry| {
            !(entry.kind == AccountKind::ThirdParty && entry.server.as_deref() == Some(url))
        });
        if next
            .selected_account
            .as_ref()
            .is_some_and(|selected| gone.contains(selected))
        {
            next.selected_account = next.accounts.first().map(AccountEntry::key);
        }
        self.save(next)?;
        Ok(gone)
    }

    /// Records a sign-in on an authentication server. The account is found by
    /// server and profile id, so signing in again updates it. The first
    /// account becomes the selected one. Returns the account's key.
    pub fn sign_in_third_party(
        &mut self,
        server: &str,
        profile_id: ProfileId,
        name: &str,
        login: &str,
    ) -> Result<String, SettingsError> {
        let compact = profile_id.compact();
        let mut next = self.settings.clone();
        let existing = next.accounts.iter_mut().find(|entry| {
            entry.kind == AccountKind::ThirdParty
                && entry.uuid.as_deref() == Some(compact.as_str())
                && entry.server.as_deref() == Some(server)
        });
        match existing {
            Some(entry) => {
                name.clone_into(&mut entry.name);
                entry.login = Some(login.to_owned());
                entry.needs_sign_in = false;
            }
            None => {
                // Another account with this id would be indistinguishable to the game.
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
                    kind: AccountKind::ThirdParty,
                    server: Some(server.to_owned()),
                    login: Some(login.to_owned()),
                    ..AccountEntry::default()
                });
            }
        }
        let key = format!("ali:{compact}@{server}");
        if next.selected_account.is_none() {
            next.selected_account = Some(key.clone());
        }
        self.save(next)?;
        Ok(key)
    }

    /// Sets how an offline account looks; `None` is the game's default skin.
    /// Only offline accounts have a skin of their own: a signed-in account
    /// gets its skin from its server.
    pub fn set_account_skin(
        &mut self,
        key: &str,
        skin: Option<crate::skin::SkinChoice>,
    ) -> Result<(), SettingsError> {
        let mut next = self.settings.clone();
        let entry = next
            .accounts
            .iter_mut()
            .find(|entry| entry.key() == key && entry.kind == AccountKind::Offline)
            .ok_or_else(|| SettingsError::UnknownAccount(key.to_owned()))?;
        if entry.skin == skin {
            return Ok(());
        }
        entry.skin = skin;
        self.save(next)
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
        let preference = self.settings.download_source_preference();
        if preference == DownloadSourcePreference::OfficialOnly {
            return SourceChain::new([Arc::new(OfficialSource) as Arc<dyn SourceProvider>])
                .map_err(|_| SettingsError::InvalidMirror);
        }
        let mut mirrors: Vec<Arc<dyn SourceProvider>> = Vec::new();
        let mut fallbacks: Vec<Arc<dyn SourceProvider>> = Vec::new();
        for rule in &self.settings.mirrors {
            let mirror = PrefixMirror::new(&rule.official_prefix, &rule.mirror_prefix, 16)
                .map_err(|_| SettingsError::InvalidMirror)?;
            if crate::mirror_presets::is_fallback_rule(rule) {
                fallbacks.push(Arc::new(mirror));
            } else {
                mirrors.push(Arc::new(mirror));
            }
        }
        let official: Arc<dyn SourceProvider> = Arc::new(OfficialSource);
        let mut providers = if preference == DownloadSourcePreference::MirrorFirst {
            mirrors.into_iter().chain([official]).collect::<Vec<_>>()
        } else {
            std::iter::once(official).chain(mirrors).collect()
        };
        providers.extend(fallbacks);
        SourceChain::new(providers).map_err(|_| SettingsError::InvalidMirror)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod download_source_tests;
