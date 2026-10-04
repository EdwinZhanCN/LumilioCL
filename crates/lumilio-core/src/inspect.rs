//! Gathers everything [`crate::diagnose`] needs for one instance.
//!
//! Behavior notes: `docs/behavior/diagnostics.md`. This is assembly, not
//! policy: the rules live in `diagnostics`.

use std::path::Path;

use crate::account::OfflineProfile;
use crate::content::{self, ContentItem, ModMetadata};
use crate::diagnostics::{Facts, Problem, diagnose};
use crate::discover::ProjectKind;
use crate::environment::HostProfile;
use crate::history::{HistoryEvent, HistoryLog, SessionOutcome};
use crate::install::InstallationPlan;
use crate::instance::InstanceRecord;
use crate::java::JavaRuntime;
use crate::launch::LaunchContext;
use crate::layout::Layout;
use crate::loader::LAUNCHABLE_LOADERS;
use crate::release::ReleaseManifest;
use crate::repair::InstallationVerifier;
use crate::settings::LauncherSettings;
use crate::transfer::{OfficialSource, SourceChain, SourceProvider};

/// Diagnoses one instance from what is on disk.
///
/// The installed release manifest, when present, supplies the required Java
/// and lets the installation be verified (a count of missing or damaged
/// files); without it neither is known and the instance simply reads as "not
/// installed". Mods are scanned with their embedded metadata. The last recorded
/// session and the effective memory limit (instance override, else launcher
/// default) are included.
pub async fn inspect_instance(
    layout: &Layout,
    settings: &LauncherSettings,
    runtimes: &[JavaRuntime],
    record: &InstanceRecord,
) -> Vec<Problem> {
    let directories = layout.launch_directories(record);

    let release = match record.release_id() {
        Some(id) => {
            let path = directories.versions().join(&id).join(format!("{id}.json"));
            tokio::fs::read_to_string(path)
                .await
                .ok()
                .and_then(|text| ReleaseManifest::decode_json(&text).ok())
        }
        None => None,
    };
    let required_java = release
        .as_ref()
        .and_then(|release| release.java_requirement())
        .map(|java| java.major());
    let damaged_files = match &release {
        Some(release) => damaged_count(release, &directories).await,
        None => None,
    };

    let mods = mods_with_metadata(directories.game());
    let last_session = HistoryLog::for_instance(layout.root(), &record.id)
        .sessions()
        .ok()
        .and_then(|sessions| {
            sessions.into_iter().find_map(|event| match event {
                HistoryEvent::Session { outcome, .. } => Some(outcome),
                HistoryEvent::Change { .. } => None,
            })
        });

    let facts = Facts {
        instance: record,
        launchable_loaders: &LAUNCHABLE_LOADERS,
        runtimes,
        required_java,
        has_account: settings.selected_account.as_deref().is_some_and(|key| {
            settings
                .accounts
                .iter()
                .any(|entry| entry.key() == key && entry.profile_id().is_ok())
        }),
        damaged_files,
        mods: &mods,
        max_memory_mb: record
            .settings
            .max_memory_mb
            .or(settings.default_max_memory_mb),
        last_session: last_session.filter(|outcome| *outcome != SessionOutcome::Clean),
    };
    diagnose(&facts)
}

fn mods_with_metadata(game_dir: &Path) -> Vec<(ContentItem, Option<ModMetadata>)> {
    content::scan(game_dir, ProjectKind::Mod)
        .unwrap_or_default()
        .into_iter()
        .map(|item| {
            let meta = if item.is_directory {
                None
            } else {
                content::read_mod_metadata(&game_dir.join("mods").join(&item.file_name))
            };
            (item, meta)
        })
        .collect()
}

/// How many installation files are missing or damaged; `None` if the plan
/// could not even be built (an unusable manifest).
async fn damaged_count(
    release: &ReleaseManifest,
    directories: &crate::launch::LaunchDirectories,
) -> Option<usize> {
    let context = OfflineProfile::new("Player")
        .expect("a fixed valid name")
        .session()
        .apply(LaunchContext::new(
            HostProfile::current(),
            directories.clone(),
        ))
        .with_value("version_type", "LumilioCL");
    let launch = release.build_launch_plan(&context).ok()?;
    let chain = SourceChain::new([
        std::sync::Arc::new(OfficialSource) as std::sync::Arc<dyn SourceProvider>
    ])
    .ok()?;
    let plan = InstallationPlan::build(release, &launch, directories.clone(), chain, None).ok()?;
    let repair = InstallationVerifier::scan(&plan, None).await.ok()?;
    Some(repair.findings().len())
}

#[cfg(test)]
mod tests;
