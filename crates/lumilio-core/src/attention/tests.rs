use super::*;
use crate::diagnostics::ProblemKind;
use crate::instance::{InstanceSettings, Loader};

fn record(id: &str, last_played: Option<u64>) -> InstanceRecord {
    InstanceRecord {
        id: id.into(),
        name: id.into(),
        game_version: "1.21.1".into(),
        loader: Loader::Vanilla,
        loader_version: None,
        favorite: false,
        created_at: 0,
        last_played,
        play_seconds: 0,
        installed: true,
        settings: InstanceSettings::default(),
        source_project: None,
    }
}

fn problem(severity: Severity, kind: ProblemKind) -> Problem {
    Problem { severity, kind }
}

#[test]
fn nothing_played_means_nothing_to_continue() {
    let summary = summarize(&[record("a", None)], &BTreeMap::new());
    assert_eq!(summary, HomeSummary::default());
}

#[test]
fn continue_is_the_latest_launchable() {
    let instances = [
        record("a", Some(10)),
        record("b", Some(30)),
        record("c", Some(20)),
        record("d", None),
    ];
    let mut problems = BTreeMap::new();
    problems.insert(
        "b".to_owned(),
        vec![problem(Severity::Error, ProblemKind::NoAccount)],
    );
    let summary = summarize(&instances, &problems);
    // b is newest but cannot launch, so Continue falls to c.
    assert_eq!(summary.continue_with.as_deref(), Some("c"));
}

#[test]
fn attention_orders_errors_first_then_recency_and_ignores_info() {
    let instances = [
        record("warn-old", Some(1)),
        record("warn-new", Some(9)),
        record("err", Some(5)),
        record("info", Some(7)),
        record("clean", Some(8)),
    ];
    let mut problems = BTreeMap::new();
    let low = problem(Severity::Warning, ProblemKind::LowMemory { max_mb: 512 });
    problems.insert("warn-old".to_owned(), vec![low.clone()]);
    problems.insert("warn-new".to_owned(), vec![low.clone()]);
    problems.insert(
        "err".to_owned(),
        vec![
            low.clone(),
            problem(Severity::Error, ProblemKind::NoAccount),
        ],
    );
    problems.insert(
        "info".to_owned(),
        vec![problem(Severity::Info, ProblemKind::NotInstalled)],
    );
    let summary = summarize(&instances, &problems);
    let order: Vec<_> = summary
        .needs_attention
        .iter()
        .map(|i| i.instance_id.as_str())
        .collect();
    assert_eq!(order, ["err", "warn-new", "warn-old"]);
    assert_eq!(
        summary.needs_attention[0].headline.kind,
        ProblemKind::NoAccount
    );
    assert_eq!(summary.needs_attention[0].total, 2);
}

#[test]
fn the_headline_is_the_first_of_the_most_severe() {
    let instances = [record("a", Some(1))];
    let mut problems = BTreeMap::new();
    problems.insert(
        "a".to_owned(),
        vec![
            problem(Severity::Warning, ProblemKind::LowMemory { max_mb: 1 }),
            problem(Severity::Error, ProblemKind::NoAccount),
            problem(Severity::Error, ProblemKind::NoJava { required: None }),
        ],
    );
    let summary = summarize(&instances, &problems);
    assert_eq!(
        summary.needs_attention[0].headline.kind,
        ProblemKind::NoAccount
    );
}
