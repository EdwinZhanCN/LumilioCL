//! What Home shows: built from the library, the Java found, each recent game's
//! own problems and whether an account can play.

use std::collections::BTreeMap;

use super::LauncherService;
use crate::attention::{HomeChecks, HomeSummary, summarize};
use crate::diagnostics::{Problem, ProblemKind};
use crate::error::ServiceError;
use crate::instances::{InstanceRecord, Loader};
use crate::loader::LoaderVersion;
use crate::transfer::Transport;

/// Instances diagnosed for Home: verifying files is not free, so only the
/// ones a person is likely to care about right now.
const DIAGNOSED_LIMIT: usize = 8;

impl<T: Transport + Clone> LauncherService<T> {
    /// Saved records and recency, without Java discovery or disk diagnosis.
    /// The application uses this before supplementing Home with `home`.
    pub async fn home_snapshot(&self) -> (Vec<InstanceRecord>, HomeSummary) {
        let instances = self.instances.lock().await.instances().to_vec();
        let summary = summarize(&instances, &BTreeMap::new());
        (instances, summary)
    }

    /// Background Continue / Needs Attention checks for Home.
    /// Full content verification remains in explicit diagnostics and repair.
    pub async fn home(&self) -> (Vec<InstanceRecord>, HomeSummary) {
        let (instances, checks) = self.home_checks().await;
        let summary = self.home_from(&instances, &checks).await;
        (instances, summary)
    }

    /// The disk half of [`Self::home`]: each recently played game's own
    /// problems, without those that depend on the account.
    pub async fn home_checks(&self) -> (Vec<InstanceRecord>, HomeChecks) {
        let instances = self.instances.lock().await.instances().to_vec();
        let (settings, runtimes) = self.java.environment().await;
        let mut recent: Vec<&InstanceRecord> = instances.iter().collect();
        recent.sort_by_key(|record| std::cmp::Reverse(record.last_played));
        let mut checks: BTreeMap<String, Vec<Problem>> = BTreeMap::new();
        for record in recent.into_iter().take(DIAGNOSED_LIMIT) {
            let mut found = self
                .diagnostics
                .inspect_with_analysis(&settings, &runtimes, record, false)
                .await;
            found.retain(|problem| problem.kind != ProblemKind::NoAccount);
            checks.insert(record.id.clone(), found);
        }
        (instances, checks)
    }

    /// Home from checks already made and the account now; reads no disk.
    pub async fn home_from(
        &self,
        instances: &[InstanceRecord],
        checks: &HomeChecks,
    ) -> HomeSummary {
        let has_account = crate::inspect::account_usable(self.settings.lock().await.get());
        let problems = crate::attention::with_account(checks, has_account);
        summarize(instances, &problems)
    }

    /// Every game version the catalog lists, newest first, for choosing one
    /// (IA P-VERSION).
    pub async fn game_versions(
        &self,
    ) -> Result<Vec<lumilio_minecraft::CatalogEntry>, ServiceError> {
        crate::minecraft::game_versions(&self.network).await
    }

    /// The versions of `loader` available for `game_version`, newest first
    /// (IA P-LOADER-VERSION). Vanilla has none.
    pub async fn loader_versions(
        &self,
        loader: Loader,
        game_version: &str,
    ) -> Result<Vec<LoaderVersion>, ServiceError> {
        crate::minecraft::loader_versions(&self.network, loader, game_version).await
    }
}
