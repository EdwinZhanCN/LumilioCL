use super::error::ServiceError;
use super::support::blocking;
use super::types::now;
use super::{DIAGNOSED_LIMIT, LauncherService, RECENT_LIMIT};
use crate::attention::{HomeSummary, summarize};
use crate::catalog::VersionCatalog;
use crate::deletion::Deletion;
use crate::diagnostics::Problem;
use crate::fetch::fetch_document;
use crate::instance::{InstanceRecord, Loader, NewInstance};
use crate::launcher::LaunchServiceError;
use crate::loader::LAUNCHABLE_LOADERS;
use crate::transfer::Transport;
use crate::{forge_meta, loader};
use std::collections::BTreeMap;

impl<T: Transport + Clone> LauncherService<T> {
    /// Deletes an instance so that a failure or crash at any step can be
    /// undone or finished: journal, move the profile aside, commit the library,
    /// then remove the moved files. `meta/` is untouched because other
    /// instances may share it. If only the final cleanup fails the deletion
    /// still stands and the leftovers are removed at the next start.
    pub async fn delete_instance(&self, id: &str) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let record = self
            .store
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))?;
        let layout = self.layout.clone();
        let deletion = {
            let record = record.clone();
            blocking(move || {
                let mut deletion = Deletion::begin(&layout, &record, now())?;
                match deletion.quarantine() {
                    Ok(()) => Ok(deletion),
                    Err(error) => {
                        deletion.discard();
                        Err(error)
                    }
                }
            })
            .await?
        };
        let committed = self.store.lock().await.remove(id);
        if let Err(error) = committed {
            let restored = blocking(move || deletion.rollback()).await;
            // A failed restore keeps the journal; the library error stays the cause.
            drop(restored);
            return Err(error.into());
        }
        // The deletion is committed; leftovers are retried by recovery.
        let _ = blocking(move || deletion.finish()).await;
        Ok(())
    }

    /// Continue / Recent / Needs Attention for Home.
    pub async fn home(&self) -> (Vec<InstanceRecord>, HomeSummary) {
        let instances = self.store.lock().await.instances().to_vec();
        let (settings, runtimes) = self.environment().await;
        let mut recent: Vec<&InstanceRecord> = instances.iter().collect();
        recent.sort_by_key(|record| std::cmp::Reverse(record.last_played));
        let mut problems: BTreeMap<String, Vec<Problem>> = BTreeMap::new();
        for record in recent.into_iter().take(DIAGNOSED_LIMIT) {
            let found = self
                .inspect_with_plugins(&settings, &runtimes, record)
                .await;
            problems.insert(record.id.clone(), found);
        }
        let summary = summarize(&instances, &problems, RECENT_LIMIT);
        (instances, summary)
    }

    /// Creates an instance. A missing game version means the newest release;
    /// a missing loader version means the recommended one for that game.
    pub async fn create_instance(
        &self,
        name: &str,
        game_version: Option<&str>,
        loader: Loader,
        loader_version: Option<&str>,
    ) -> Result<InstanceRecord, ServiceError> {
        if !LAUNCHABLE_LOADERS.contains(&loader) {
            return Err(ServiceError::Launch(LaunchServiceError::LoaderUnsupported(
                loader,
            )));
        }
        let chain = self.chain().await?;
        let game_version = match game_version {
            Some(version) => version.to_owned(),
            None => VersionCatalog::fetch(&self.transport, &chain)
                .await
                .map_err(|error| ServiceError::Remote(error.to_string()))?
                .latest_release()
                .map(|entry| entry.id().to_owned())
                .ok_or_else(|| ServiceError::Remote("the catalog lists no release".to_owned()))?,
        };
        let loader_version = match (loader, loader_version) {
            (Loader::Vanilla, _) => None,
            (_, Some(version)) => Some(version.to_owned()),
            (_, None) => {
                let url = loader::versions_url(loader, &game_version)
                    .ok_or_else(|| ServiceError::Remote("no loader source".to_owned()))?;
                let sources = chain.candidates(&url);
                let versions = loader::fetch_versions(&self.transport, &sources)
                    .await
                    .map_err(|error| ServiceError::Remote(error.to_string()))?;
                let chosen = loader::recommended(&versions).ok_or_else(|| {
                    ServiceError::Remote(format!("{game_version} has no {loader:?} version"))
                })?;
                Some(chosen.version.clone())
            }
        };
        let mut store = self.store.lock().await;
        let record = store.create(
            NewInstance {
                name: name.to_owned(),
                game_version,
                loader,
                loader_version,
            },
            now(),
        )?;
        Ok(record.clone())
    }

    /// Every game version the catalog lists, newest first, for choosing one
    /// (IA P-VERSION). Read fresh each time; a failure is reported, never
    /// replaced by a guess.
    pub async fn game_versions(&self) -> Result<Vec<crate::catalog::CatalogEntry>, ServiceError> {
        let chain = self.chain().await?;
        let catalog = VersionCatalog::fetch(&self.transport, &chain)
            .await
            .map_err(|error| ServiceError::Remote(error.to_string()))?;
        Ok(catalog.entries().to_vec())
    }

    /// The versions of `loader` available for `game_version`, newest first
    /// (IA P-LOADER-VERSION). Vanilla has none.
    pub async fn loader_versions(
        &self,
        loader: Loader,
        game_version: &str,
    ) -> Result<Vec<loader::LoaderVersion>, ServiceError> {
        let chain = self.chain().await?;
        let remote = |error: crate::fetch::FetchError| ServiceError::Remote(error.to_string());
        let decode = |error: loader::LoaderError| ServiceError::Remote(error.to_string());
        match loader {
            Loader::Vanilla => Ok(Vec::new()),
            Loader::Fabric | Loader::Quilt => {
                let url = loader::versions_url(loader, game_version)
                    .ok_or_else(|| ServiceError::Remote("no loader source".to_owned()))?;
                loader::fetch_versions(&self.transport, &chain.candidates(&url))
                    .await
                    .map_err(decode)
            }
            Loader::Forge => {
                let metadata = fetch_document(
                    &self.transport,
                    &chain.candidates(forge_meta::FORGE_METADATA_URL),
                )
                .await
                .map_err(remote)?;
                // Promotions only mark the recommended build; the list stands without them.
                let promotions = fetch_document(
                    &self.transport,
                    &chain.candidates(forge_meta::FORGE_PROMOTIONS_URL),
                )
                .await
                .ok();
                forge_meta::forge_versions(&metadata, promotions.as_deref(), game_version)
                    .map_err(decode)
            }
            Loader::NeoForge => {
                let modern = fetch_document(
                    &self.transport,
                    &chain.candidates(forge_meta::NEOFORGE_VERSIONS_URL),
                )
                .await
                .map_err(remote)?;
                let legacy = if game_version == forge_meta::NEOFORGE_LEGACY_GAME {
                    Some(
                        fetch_document(
                            &self.transport,
                            &chain.candidates(forge_meta::NEOFORGE_LEGACY_VERSIONS_URL),
                        )
                        .await
                        .map_err(remote)?,
                    )
                } else {
                    None
                };
                forge_meta::neoforge_versions(&modern, legacy.as_deref(), game_version)
                    .map_err(decode)
            }
        }
    }
}
