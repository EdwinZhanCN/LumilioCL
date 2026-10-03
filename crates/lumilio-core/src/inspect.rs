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
mod tests {
    use super::*;
    use crate::diagnostics::{ProblemKind, Severity};
    use crate::instance::{InstanceStore, Loader, NewInstance};
    use crate::java::JavaLocator;
    use std::fs;

    const MANIFEST: &str = r#"{
        "id": "1.0", "mainClass": "m",
        "javaVersion": {"component": "c", "majorVersion": 21},
        "downloads": {"client": {"url": "https://x/c.jar",
                                 "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size": 3}},
        "libraries": []
    }"#;

    fn store_with_instance() -> (tempfile::TempDir, InstanceStore, InstanceRecord) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = InstanceStore::open(dir.path().join("root")).unwrap();
        let record = store
            .create(
                NewInstance {
                    name: "T".into(),
                    game_version: "1.0".into(),
                    loader: Loader::Vanilla,
                    loader_version: None,
                },
                1,
            )
            .unwrap()
            .clone();
        (dir, store, record)
    }

    fn java_21(dir: &Path) -> Vec<JavaRuntime> {
        let home = dir.join("jdk");
        fs::create_dir_all(home.join("bin")).unwrap();
        fs::write(home.join("bin/java"), "").unwrap();
        fs::write(home.join("release"), "JAVA_VERSION=\"21.0.1\"").unwrap();
        JavaLocator::new([home]).discover()
    }

    fn kinds(problems: Vec<Problem>) -> Vec<ProblemKind> {
        problems.into_iter().map(|p| p.kind).collect()
    }

    #[tokio::test]
    async fn a_fresh_instance_reports_the_setup_gaps() {
        let (_dir, store, record) = store_with_instance();
        let kinds = kinds(
            inspect_instance(store.layout(), &LauncherSettings::default(), &[], &record).await,
        );
        assert!(kinds.contains(&ProblemKind::NoAccount));
        assert!(kinds.contains(&ProblemKind::NoJava { required: None }));
        assert!(kinds.contains(&ProblemKind::NotInstalled));
    }

    #[tokio::test]
    async fn an_installed_manifest_supplies_the_java_requirement_and_a_damage_count() {
        let (dir, mut store, record) = store_with_instance();
        let directories = store.directories(&record);
        let manifest_dir = directories.versions().join("1.0");
        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(manifest_dir.join("1.0.json"), MANIFEST).unwrap();
        store.mark_installed(&record.id, true).unwrap();
        let record = store.get(&record.id).unwrap().clone();
        let mut settings = LauncherSettings::default();
        settings.accounts.push(crate::settings::AccountEntry {
            name: "Steve".into(),
            ..Default::default()
        });
        settings.selected_account = Some("Steve".into());

        // Java 17 is too old for the manifest's Java 21; the client jar is missing.
        let home = dir.path().join("jdk17");
        fs::create_dir_all(home.join("bin")).unwrap();
        fs::write(home.join("bin/java"), "").unwrap();
        fs::write(home.join("release"), "JAVA_VERSION=\"17\"").unwrap();
        let old = JavaLocator::new([home]).discover();
        let found = kinds(inspect_instance(store.layout(), &settings, &old, &record).await);
        assert!(found.contains(&ProblemKind::NoJava { required: Some(21) }));
        assert!(found.contains(&ProblemKind::DamagedFiles { count: 1 }));

        let good = java_21(dir.path());
        let found = kinds(inspect_instance(store.layout(), &settings, &good, &record).await);
        assert!(
            !found
                .iter()
                .any(|k| matches!(k, ProblemKind::NoJava { .. }))
        );
        assert!(found.contains(&ProblemKind::DamagedFiles { count: 1 }));
    }

    #[tokio::test]
    async fn memory_defaults_and_instance_overrides_and_last_session_are_used() {
        let (dir, mut store, record) = store_with_instance();
        let mut settings = LauncherSettings::default();
        settings.accounts.push(crate::settings::AccountEntry {
            name: "Steve".into(),
            ..Default::default()
        });
        settings.selected_account = Some("Steve".into());
        settings.default_max_memory_mb = Some(512);
        let runtimes = java_21(dir.path());
        let found = kinds(inspect_instance(store.layout(), &settings, &runtimes, &record).await);
        assert!(found.contains(&ProblemKind::LowMemory { max_mb: 512 }));

        store
            .update_settings(
                &record.id,
                crate::instance::InstanceSettings {
                    max_memory_mb: Some(4096),
                    ..Default::default()
                },
            )
            .unwrap();
        let record = store.get(&record.id).unwrap().clone();
        let found = kinds(inspect_instance(store.layout(), &settings, &runtimes, &record).await);
        assert!(
            !found
                .iter()
                .any(|k| matches!(k, ProblemKind::LowMemory { .. }))
        );

        let log = HistoryLog::for_instance(store.root(), &record.id);
        log.append(&HistoryEvent::Session {
            started: 5,
            seconds: 1,
            exit_code: Some(1),
            outcome: SessionOutcome::Crashed,
        })
        .unwrap();
        let found = kinds(inspect_instance(store.layout(), &settings, &runtimes, &record).await);
        assert!(found.contains(&ProblemKind::LastSessionFailed(SessionOutcome::Crashed)));
        log.append(&HistoryEvent::Session {
            started: 9,
            seconds: 60,
            exit_code: Some(0),
            outcome: SessionOutcome::Clean,
        })
        .unwrap();
        let found = kinds(inspect_instance(store.layout(), &settings, &runtimes, &record).await);
        assert!(
            !found
                .iter()
                .any(|k| matches!(k, ProblemKind::LastSessionFailed(_)))
        );
    }

    #[tokio::test]
    async fn mods_are_read_with_their_metadata_for_mod_problems() {
        use std::io::Write;
        let (dir, store, record) = store_with_instance();
        // A Fabric instance holding a Forge-only mod.
        let mut fabric = record.clone();
        fabric.loader = Loader::Fabric;
        fabric.loader_version = Some("0.16.0".into());
        let game = store.directories(&record).game().to_path_buf();
        fs::create_dir_all(game.join("mods")).unwrap();
        let mut writer = zip::ZipWriter::new(fs::File::create(game.join("mods/jei.jar")).unwrap());
        writer
            .start_file(
                "META-INF/mods.toml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"[[mods]]\nmodId=\"jei\"\n").unwrap();
        writer.finish().unwrap();
        let mut settings = LauncherSettings::default();
        settings.accounts.push(crate::settings::AccountEntry {
            name: "Steve".into(),
            ..Default::default()
        });
        settings.selected_account = Some("Steve".into());
        let runtimes = java_21(dir.path());
        let problems = inspect_instance(store.layout(), &settings, &runtimes, &fabric).await;
        assert!(problems.iter().any(|p| p.severity == Severity::Warning
            && matches!(&p.kind, ProblemKind::WrongLoaderMods { names } if names == &["jei.jar"])));
    }
}
