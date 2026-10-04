//! What Home shows: Continue, Recent, Needs Attention.
//!
//! Pure: it works from instance records and the problems already diagnosed for
//! each.

use std::collections::BTreeMap;

use crate::diagnostics::{Problem, Severity};
use crate::instance::InstanceRecord;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttentionItem {
    pub instance_id: String,
    /// The most severe problem (first reported among equals).
    pub headline: Problem,
    /// How many problems of warning level or above the instance has.
    pub total: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HomeSummary {
    /// The instance the Continue button resumes, if any.
    pub continue_with: Option<String>,
    /// Other recently played instances, newest first.
    pub recent: Vec<String>,
    pub needs_attention: Vec<AttentionItem>,
}

/// Builds the Home summary.
///
/// - *Continue* is the most recently played instance that has no error-level
///   problem; an instance that cannot launch is not offered as "continue".
/// - *Recent* lists played instances other than Continue's, newest first, up to
///   `recent_limit`. Never-played instances are not "recent".
/// - *Needs attention* lists instances with any warning or error problem,
///   errors first, then most recently played first; info-level problems
///   (such as "not installed yet") do not count.
#[must_use]
pub fn summarize(
    instances: &[InstanceRecord],
    problems: &BTreeMap<String, Vec<Problem>>,
    recent_limit: usize,
) -> HomeSummary {
    let has_error = |id: &str| {
        problems
            .get(id)
            .is_some_and(|list| list.iter().any(|p| p.severity == Severity::Error))
    };
    let mut played: Vec<&InstanceRecord> = instances
        .iter()
        .filter(|record| record.last_played.is_some())
        .collect();
    played.sort_by(|a, b| {
        b.last_played
            .cmp(&a.last_played)
            .then_with(|| a.id.cmp(&b.id))
    });

    let continue_with = played
        .iter()
        .find(|record| !has_error(&record.id))
        .map(|r| r.id.clone());
    let recent = played
        .iter()
        .filter(|record| Some(&record.id) != continue_with.as_ref())
        .take(recent_limit)
        .map(|record| record.id.clone())
        .collect();

    let mut needs_attention: Vec<(Option<u64>, AttentionItem)> = instances
        .iter()
        .filter_map(|record| {
            let serious: Vec<&Problem> = problems
                .get(&record.id)?
                .iter()
                .filter(|problem| problem.severity >= Severity::Warning)
                .collect();
            let top = serious.iter().map(|problem| problem.severity).max()?;
            let headline = (*serious.iter().find(|problem| problem.severity == top)?).clone();
            Some((
                record.last_played,
                AttentionItem {
                    instance_id: record.id.clone(),
                    headline,
                    total: serious.len(),
                },
            ))
        })
        .collect();
    needs_attention.sort_by(|(a_played, a), (b_played, b)| {
        b.headline
            .severity
            .cmp(&a.headline.severity)
            .then_with(|| b_played.cmp(a_played))
            .then_with(|| a.instance_id.cmp(&b.instance_id))
    });

    HomeSummary {
        continue_with,
        recent,
        needs_attention: needs_attention.into_iter().map(|(_, item)| item).collect(),
    }
}

#[cfg(test)]
mod tests {
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
        let summary = summarize(&[record("a", None)], &BTreeMap::new(), 5);
        assert_eq!(summary, HomeSummary::default());
    }

    #[test]
    fn continue_is_the_latest_launchable_and_recent_excludes_it() {
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
        let summary = summarize(&instances, &problems, 5);
        // b is newest but cannot launch, so Continue falls to c.
        assert_eq!(summary.continue_with.as_deref(), Some("c"));
        assert_eq!(summary.recent, ["b", "a"]);
    }

    #[test]
    fn recent_is_limited() {
        let instances: Vec<_> = (1..=6).map(|n| record(&format!("i{n}"), Some(n))).collect();
        let summary = summarize(&instances, &BTreeMap::new(), 2);
        assert_eq!(summary.continue_with.as_deref(), Some("i6"));
        assert_eq!(summary.recent, ["i5", "i4"]);
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
        let summary = summarize(&instances, &problems, 5);
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
        let summary = summarize(&instances, &problems, 5);
        assert_eq!(
            summary.needs_attention[0].headline.kind,
            ProblemKind::NoAccount
        );
    }
}
