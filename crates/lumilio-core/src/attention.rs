//! What Home shows: Continue, Recent, Needs Attention.
//!
//! Pure: it works from instance records and the problems already diagnosed for
//! each.

use std::collections::BTreeMap;

use crate::diagnostics::{Problem, ProblemKind, Severity};
use crate::instance::InstanceRecord;

/// Each diagnosed game's own problems, by id, without those that depend on
/// the account: those are added when Home is summarized, so a change of
/// account needs no new look at the disk (plan global-context, D7).
pub type HomeChecks = BTreeMap<String, Vec<Problem>>;

/// The checks with the account applied: without a usable account, every
/// diagnosed game also cannot start for that reason.
#[must_use]
pub fn with_account(checks: &HomeChecks, has_account: bool) -> BTreeMap<String, Vec<Problem>> {
    checks
        .iter()
        .map(|(id, problems)| {
            let mut problems = problems.clone();
            if !has_account {
                // Where diagnosis reports it: right after an unsupported loader.
                let at = problems
                    .iter()
                    .take_while(|problem| matches!(problem.kind, ProblemKind::LoaderUnsupported(_)))
                    .count();
                problems.insert(
                    at,
                    Problem {
                        severity: Severity::Error,
                        kind: ProblemKind::NoAccount,
                    },
                );
            }
            (id.clone(), problems)
        })
        .collect()
}

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
    pub needs_attention: Vec<AttentionItem>,
}

/// Builds the Home summary.
///
/// - *Continue* is the most recently played instance that has no error-level
///   problem; an instance that cannot launch is not offered as "continue".
/// - *Needs attention* lists instances with any warning or error problem,
///   errors first, then most recently played first; info-level problems
///   (such as "not installed yet") do not count.
#[must_use]
pub fn summarize(
    instances: &[InstanceRecord],
    problems: &BTreeMap<String, Vec<Problem>>,
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
        needs_attention: needs_attention.into_iter().map(|(_, item)| item).collect(),
    }
}

#[cfg(test)]
mod tests;
