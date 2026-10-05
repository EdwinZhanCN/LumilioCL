use super::{World, fake_java, publish_release, world};
use crate::diagnostics::ProblemKind;
use crate::history::{SessionOutcome, record_attempt};
use crate::instance::Loader;
use crate::plugins::PluginHost;
use lumilio_plugin_api::{
    API_VERSION, AnalysisInput, AnalysisSource, Analyzer, Finding, HostContext, Manifest, Plugin,
    PluginError, Severity,
};
use std::sync::{Arc, Mutex};

struct Recorder {
    inputs: Arc<Mutex<Vec<AnalysisInput>>>,
    caller: std::thread::ThreadId,
}

impl Plugin for Recorder {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "test.analysis".into(),
            name: "分析替身".into(),
            description: String::new(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: Vec::new(),
            settings: Vec::new(),
        }
    }
    fn analyzer(&self) -> Option<&dyn Analyzer> {
        assert_ne!(self.caller, std::thread::current().id());
        Some(self)
    }
}
impl Analyzer for Recorder {
    fn analyze(
        &self,
        _: &dyn HostContext,
        input: &AnalysisInput,
    ) -> Result<Vec<Finding>, PluginError> {
        assert_ne!(self.caller, std::thread::current().id());
        self.inputs.lock().unwrap().push(input.clone());
        Ok(vec![
            Finding {
                rule: "minor".into(),
                severity: Severity::Warning,
                title: "次要原因".into(),
                advice: "建议".into(),
                evidence: None,
            },
            Finding {
                rule: "major".into(),
                severity: Severity::Error,
                title: "主要原因".into(),
                advice: "建议".into(),
                evidence: Some(input.text.clone()),
            },
        ])
    }
}

fn analyzed_world() -> (World, Arc<Mutex<Vec<AnalysisInput>>>) {
    let mut world = world();
    let inputs = Arc::default();
    world.service.plugins = PluginHost::new(
        vec![Arc::new(Recorder {
            inputs: Arc::clone(&inputs),
            caller: std::thread::current().id(),
        })],
        Default::default(),
    );
    (world, inputs)
}

#[tokio::test]
async fn analysis_uses_latest_report_for_a_failed_session_and_home_picks_the_most_severe() {
    let (world, inputs) = analyzed_world();
    publish_release(&world);
    fake_java(&world, "exit 0");
    let record = world
        .service
        .create_instance("Analysis", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::create_dir_all(game.join("crash-reports")).unwrap();
    std::fs::write(game.join("logs/latest.log"), "latest log").unwrap();
    std::fs::write(game.join("crash-reports/crash.txt"), "report").unwrap();
    std::fs::write(game.join("crash-reports/old.txt"), "old report").unwrap();
    std::fs::File::open(game.join("crash-reports/old.txt"))
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH))
        .unwrap();
    // Clean/no session doesn't diagnose old crash files.
    world.service.problems(&record.id).await.unwrap();
    assert!(inputs.lock().unwrap().is_empty());
    record_attempt(
        world.service.layout.root(),
        &record.id,
        1,
        SessionOutcome::Crashed,
    )
    .unwrap();
    let problems = world.service.problems(&record.id).await.unwrap();
    assert_eq!(
        problems
            .iter()
            .filter(|p| matches!(p.kind, ProblemKind::Finding(_)))
            .count(),
        2
    );
    let input = inputs.lock().unwrap()[0].clone();
    assert_eq!(input.source, AnalysisSource::CrashReport);
    assert_eq!(input.text, "report");
    assert_eq!(
        (
            input.game.game_version.as_str(),
            input.game.loader.as_str(),
            input.game.java_major
        ),
        ("1.0", "vanilla", Some(21))
    );
    let (_, home) = world.service.home().await;
    assert!(
        matches!(&home.needs_attention[0].headline.kind, ProblemKind::Finding(result) if result.finding.rule == "major")
    );
    world
        .service
        .set_plugin_enabled("test.analysis", false)
        .await
        .unwrap();
    let count = inputs.lock().unwrap().len();
    assert!(
        world
            .service
            .crash_report(&record.id, "crash.txt")
            .await
            .unwrap()
            .1
            .is_empty()
    );
    let disabled = world.service.problems(&record.id).await.unwrap();
    assert!(
        !disabled
            .iter()
            .any(|p| matches!(p.kind, ProblemKind::Finding(_)))
    );
    assert!(disabled.iter().any(|p| matches!(
        p.kind,
        ProblemKind::LastSessionFailed(_) | ProblemKind::NotInstalled
    )));
    world.service.home().await;
    assert_eq!(
        inputs.lock().unwrap().len(),
        count,
        "disabled analyzer is never invoked"
    );
    world
        .service
        .set_plugin_enabled("test.analysis", true)
        .await
        .unwrap();
    assert_eq!(
        world
            .service
            .crash_report(&record.id, "crash.txt")
            .await
            .unwrap()
            .1
            .len(),
        2
    );
    record_attempt(
        world.service.layout.root(),
        &record.id,
        2,
        SessionOutcome::Clean,
    )
    .unwrap();
    let count = inputs.lock().unwrap().len();
    assert!(
        !world
            .service
            .problems(&record.id)
            .await
            .unwrap()
            .iter()
            .any(|p| matches!(p.kind, ProblemKind::Finding(_)))
    );
    assert_eq!(inputs.lock().unwrap().len(), count);
}

#[tokio::test]
async fn failed_start_uses_latest_log_and_missing_or_unreadable_logs_keep_builtin_checks() {
    let (world, inputs) = analyzed_world();
    let record = world
        .service
        .create_instance("Failed", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    record_attempt(
        world.service.layout.root(),
        &record.id,
        1,
        SessionOutcome::FailedToStart,
    )
    .unwrap();
    let empty = world.service.problems(&record.id).await.unwrap();
    assert!(inputs.lock().unwrap().is_empty());
    assert!(
        empty
            .iter()
            .any(|p| matches!(p.kind, ProblemKind::NoJava { .. }))
    );
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::write(game.join("logs/latest.log"), "failed start log").unwrap();
    world.service.problems(&record.id).await.unwrap();
    let input = inputs.lock().unwrap()[0].clone();
    assert_eq!(input.source, AnalysisSource::LatestLog);
    assert_eq!(input.text, "failed start log");
    std::fs::remove_file(game.join("logs/latest.log")).unwrap();
    std::fs::create_dir(game.join("logs/latest.log")).unwrap();
    let unreadable = world.service.problems(&record.id).await.unwrap();
    assert!(
        unreadable
            .iter()
            .any(|p| matches!(p.kind, ProblemKind::NoJava { .. }))
    );
    assert_eq!(inputs.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn game_facts_include_enabled_mods_and_disabling_analysis_keeps_duplicate_mod_checks() {
    use std::io::Write as _;
    let (world, inputs) = analyzed_world();
    let record = super::fabric_instance(&world, "1.21.1").await;
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("mods")).unwrap();
    for name in ["one.jar", "two.jar", "disabled.jar.disabled"] {
        let mut archive =
            zip::ZipWriter::new(std::fs::File::create(game.join("mods").join(name)).unwrap());
        archive
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(br#"{"schemaVersion":1,"id":"example","version":"2","name":"Example"}"#)
            .unwrap();
        archive.finish().unwrap();
    }
    std::fs::create_dir_all(game.join("logs")).unwrap();
    std::fs::write(game.join("logs/latest.log"), "failed log").unwrap();
    record_attempt(
        world.service.layout.root(),
        &record.id,
        1,
        SessionOutcome::FailedToPrepare,
    )
    .unwrap();
    world.service.problems(&record.id).await.unwrap();
    let game = inputs.lock().unwrap()[0].game.clone();
    assert_eq!(game.loader, "fabric");
    assert_eq!(game.loader_version.as_deref(), Some("0.16.0"));
    assert_eq!(game.mods.len(), 2);
    assert!(
        game.mods
            .iter()
            .all(|item| item.id == "example" && item.version.as_deref() == Some("2"))
    );
    assert!(game.mods.iter().any(|item| item.file == "one.jar"));
    assert!(game.mods.iter().any(|item| item.file == "two.jar"));
    world
        .service
        .set_plugin_enabled("test.analysis", false)
        .await
        .unwrap();
    let problems = world.service.problems(&record.id).await.unwrap();
    assert!(
        problems
            .iter()
            .any(|p| matches!(&p.kind, ProblemKind::DuplicateMods { ids } if ids == &["example"]))
    );
    assert!(
        problems
            .iter()
            .any(|p| matches!(p.kind, ProblemKind::NoJava { .. }))
    );
    assert!(
        !problems
            .iter()
            .any(|p| matches!(p.kind, ProblemKind::Finding(_)))
    );
}
