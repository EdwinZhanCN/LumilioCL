//! Account-scoped profile reads (ADR 0039). Cache lifetimes and authentication
//! backoff follow Modrinth `packages/app-lib/src/state/minecraft_auth.rs`
//! (Copyright Modrinth contributors, GPL-3.0-only; ADR 0022).
//! Unlike a map-only cache, each account also serializes misses and writes.
use super::{LauncherService, ServiceError};
use crate::{
    microsoft::Secret,
    skin::{AppearanceError, MojangClient, MojangProfile},
    transfer::Transport,
};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{sync::Mutex, time::Instant};

#[derive(Default)]
pub(super) struct ProfileState {
    pub(super) profile: Option<(Instant, MojangProfile)>,
    /// A completed GET, distinct from a write response stored in `profile`.
    read_at: Option<Instant>,
    /// How many GETs have completed in this slot. A refresh shares a GET that
    /// finished while it waited for the slot, and still fetches after a write
    /// reply, which does not increment this.
    reads: u64,
    pub(super) failure: Option<(Instant, AppearanceError)>,
    token: Vec<u8>,
}

#[derive(Default)]
pub(super) struct Profiles(Mutex<HashMap<String, Arc<Mutex<ProfileState>>>>);

pub struct ProfileSnapshot {
    pub profile: MojangProfile,
    /// A recoverable failed refresh left this older profile available.
    pub warning: Option<AppearanceError>,
}

impl Profiles {
    pub(super) async fn slot(&self, key: &str) -> Arc<Mutex<ProfileState>> {
        self.0
            .lock()
            .await
            .entry(key.to_owned())
            .or_default()
            .clone()
    }
    pub(super) async fn remove(&self, key: &str) {
        self.0.lock().await.remove(key);
    }

    pub(super) async fn is_current(&self, key: &str, slot: &Arc<Mutex<ProfileState>>) -> bool {
        self.0
            .lock()
            .await
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, slot))
    }
}

impl ProfileState {
    pub(super) fn token(&mut self, token: &Secret) {
        let fingerprint = Sha256::digest(token.expose().as_bytes()).to_vec();
        if self.token != fingerprint {
            self.token = fingerprint;
            self.profile = None;
            self.read_at = None;
            if self
                .failure
                .as_ref()
                .is_some_and(|(_, error)| matches!(error, AppearanceError::SignInRequired))
            {
                self.failure = None;
            }
        }
    }
    pub(super) fn blocked(&self) -> Option<AppearanceError> {
        self.failure
            .as_ref()
            .filter(|(until, _)| Instant::now() < *until)
            .map(|(_, error)| error.clone())
    }
    pub(super) fn failed_read(&mut self, error: &AppearanceError) {
        self.failure = Some((Instant::now() + error.retry_delay(), error.clone()));
    }
    pub(super) fn failed_write(&mut self, error: &AppearanceError) {
        self.failure = match error {
            AppearanceError::SignInRequired
            | AppearanceError::RateLimited
            | AppearanceError::RateLimitedFor(_)
            | AppearanceError::Network(_)
            | AppearanceError::Refused(500..=599) => {
                Some((Instant::now() + error.retry_delay(), error.clone()))
            }
            _ => None,
        };
    }
}

impl<T: Transport + Clone> LauncherService<T> {
    /// Reuses a recent snapshot and coalesces concurrent misses. Failed reads
    /// retain the previous appearance; explicit confirmation never uses stale data.
    pub async fn account_profile(&self, key: &str) -> Result<MojangProfile, ServiceError> {
        self.read_appearance_profile(key, false).await
    }

    pub async fn account_profile_snapshot(
        &self,
        key: &str,
    ) -> Result<ProfileSnapshot, ServiceError> {
        let profile = self.account_profile(key).await?;
        let slot = self.profiles.slot(key).await;
        let state = slot.lock().await;
        let warning = state.blocked().filter(recoverable);
        Ok(ProfileSnapshot { profile, warning })
    }

    /// Bypasses freshness, but still respects server/authentication cooldowns.
    pub async fn refresh_account_profile(&self, key: &str) -> Result<MojangProfile, ServiceError> {
        self.read_appearance_profile(key, true).await
    }

    async fn read_appearance_profile(
        &self,
        key: &str,
        force: bool,
    ) -> Result<MojangProfile, ServiceError> {
        let slot = self.profiles.slot(key).await;
        let observed = slot.lock().await.reads;
        let mut state = slot.lock().await;
        let (entry, token) = self.appearance_token(key).await?;
        state.token(&token);
        if let Some(error) = state.blocked() {
            if !force
                && recoverable(&error)
                && let Some((_, profile)) = &state.profile
            {
                return Ok(profile.clone());
            }
            return Err(error.into());
        }
        // A GET that finished while this call waited is this call's read.
        // A write reply does not count, so confirmation still fetches.
        if state.reads > observed
            && state.read_at.is_some()
            && let Some((_, profile)) = &state.profile
        {
            return Ok(profile.clone());
        }
        if let Some((fetched, profile)) = &state.profile
            && !force
            && fetched.elapsed() < Duration::from_secs(60)
        {
            return Ok(profile.clone());
        }
        let result = MojangClient::new(&self.transport)
            .profile(&token)
            .await
            .and_then(|profile| {
                validate_profile(&entry, &profile)?;
                Ok(profile)
            });
        if !self.profiles.is_current(key, &slot).await {
            return Err(ServiceError::Cancelled);
        }
        match result {
            Ok(profile) => {
                state.failure = None;
                let now = Instant::now();
                state.reads += 1;
                state.read_at = Some(now);
                state.profile = Some((now, profile.clone()));
                Ok(profile)
            }
            Err(error) => {
                state.failed_read(&error);
                if !force
                    && recoverable(&error)
                    && let Some((_, profile)) = &state.profile
                {
                    return Ok(profile.clone());
                }
                self.appearance_answer(key, Err(error)).await
            }
        }
    }
}

fn recoverable(error: &AppearanceError) -> bool {
    matches!(
        error,
        AppearanceError::Network(_)
            | AppearanceError::RateLimited
            | AppearanceError::RateLimitedFor(_)
            | AppearanceError::Refused(500..=599)
    )
}

pub(super) fn validate_profile(
    entry: &crate::settings::AccountEntry,
    profile: &MojangProfile,
) -> Result<(), AppearanceError> {
    let expected = entry
        .profile_id()
        .map_err(|_| AppearanceError::Protocol("invalid account id".into()))?;
    let actual = crate::account::ProfileId::parse(&profile.id)
        .map_err(|_| AppearanceError::Protocol("invalid profile id".into()))?;
    if expected != actual {
        return Err(AppearanceError::Protocol(
            "profile belongs to another account".into(),
        ));
    }
    Ok(())
}
