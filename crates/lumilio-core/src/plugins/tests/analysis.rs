use super::*;
use lumilio_plugin_api::{AnalysisInput, AnalysisSource, Analyzer, Finding, GameFacts, Severity};

enum Behavior {
    Good,
    Panic,
    Error,
    Wait(Arc<Mutex<std::sync::mpsc::Receiver<()>>>),
}
struct AnalysisPlugin {
    id: &'static str,
    behavior: Behavior,
    caller: std::thread::ThreadId,
}
impl Plugin for AnalysisPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: self.id.into(),
            name: self.id.into(),
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
impl Analyzer for AnalysisPlugin {
    fn analyze(
        &self,
        _: &dyn HostContext,
        input: &AnalysisInput,
    ) -> Result<Vec<Finding>, PluginError> {
        assert_ne!(self.caller, std::thread::current().id());
        match &self.behavior {
            Behavior::Panic => panic!("analysis crashed"),
            Behavior::Error => return Err(PluginError::Unavailable("analysis failed".into())),
            Behavior::Wait(gate) => gate.lock().unwrap().recv().unwrap(),
            Behavior::Good => {}
        }
        Ok(vec![Finding {
            rule: "same-local-rule".into(),
            severity: Severity::Warning,
            title: self.id.into(),
            advice: "分析器建议".into(),
            evidence: Some(input.text.clone()),
        }])
    }
}

fn plugin(id: &'static str, behavior: Behavior) -> Arc<dyn Plugin> {
    Arc::new(AnalysisPlugin {
        id,
        behavior,
        caller: std::thread::current().id(),
    })
}
fn input() -> AnalysisInput {
    AnalysisInput {
        text: "log".into(),
        source: AnalysisSource::LatestLog,
        game: GameFacts::default(),
    }
}

#[tokio::test]
async fn findings_keep_plugin_identity_and_disabled_analyzers_disappear() {
    let host = PluginHost::new(
        vec![
            plugin("test.a", Behavior::Good),
            plugin("test.b", Behavior::Good),
        ],
        BTreeMap::new(),
    );
    let found = host.analyze(input()).await;
    assert_eq!(
        found
            .iter()
            .map(|finding| finding.plugin.as_str())
            .collect::<Vec<_>>(),
        ["test.a", "test.b"]
    );
    host.set_state(
        "test.a".into(),
        PluginState {
            enabled: Some(false),
            ..Default::default()
        },
    );
    assert_eq!(
        host.analyze(input())
            .await
            .iter()
            .map(|finding| finding.plugin.as_str())
            .collect::<Vec<_>>(),
        ["test.b"]
    );
    host.set_state("test.a".into(), PluginState::default());
    assert_eq!(host.analyze(input()).await.len(), 2);
}

#[tokio::test]
async fn analyzer_panics_and_errors_only_remove_their_own_findings() {
    for behavior in [Behavior::Panic, Behavior::Error] {
        let host = PluginHost::new(
            vec![
                plugin("test.bad", behavior),
                plugin("test.good", Behavior::Good),
            ],
            BTreeMap::new(),
        );
        let found = host.analyze(input()).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].plugin, "test.good");
        assert!(matches!(
            host.list().await[0].status,
            PluginStatus::Failed { .. }
        ));
        assert_eq!(host.analyze(input()).await.len(), 1);
        assert_eq!(
            host.call("test.good", |_, _| Ok("still available")).await,
            Some("still available")
        );
    }
}

#[tokio::test]
async fn analyzer_timeout_discards_the_late_result_and_other_analyzers_still_run() {
    let (release, gate) = std::sync::mpsc::channel();
    let mut host = PluginHost::new(
        vec![
            plugin("test.bad", Behavior::Wait(Arc::new(Mutex::new(gate)))),
            plugin("test.good", Behavior::Good),
        ],
        BTreeMap::new(),
    );
    host.list().await;
    host.timeout = Duration::from_millis(20);
    let found = host.analyze(input()).await;
    release.send(()).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].plugin, "test.good");
    assert!(matches!(
        host.list().await[0].status,
        PluginStatus::Failed { .. }
    ));
    assert_eq!(host.analyze(input()).await.len(), 1);
}
