use super::library::relative_time;
use lumilio_core::{
    ActiveTask, ActivityView, FinishedTask, RecoveryNote, TaskCategory, TaskOutcome,
};

use crate::tr;

#[derive(Clone, Debug, PartialEq)]
pub struct ActivityRow {
    /// The running task's id (running rows only).
    pub task: Option<u64>,
    pub category: TaskCategory,
    pub title: String,
    pub detail: String,
    /// `None` while running with unknown progress.
    pub fraction: Option<f32>,
    pub state: ActivityState,
    /// The task id to cancel with, while the row is running and stoppable.
    pub cancel: Option<u64>,
    /// Done and total in `unit`, while running and known.
    pub amount: Option<(u64, u64)>,
    pub unit: lumilio_core::ProgressUnit,
    /// How fast it is going, in `unit` per second, once two readings exist.
    pub rate: Option<f64>,
    /// The game it concerns, for opening it.
    pub instance: Option<String>,
    /// How to run it again, if it can be.
    pub retry: Option<lumilio_core::RetryAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivityState {
    Running,
    Done,
    Failed(String),
    Cancelled,
}

/// What was last seen of a running task.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RateSample {
    pub(super) done: u64,
    pub(super) at_ms: u64,
    pub(super) per_sec: Option<f64>,
}

/// The shortest gap between two readings that is worth a speed.
pub(super) const RATE_GAP_MS: u64 = 200;

/// The next reading's speed: the change since the last reading, smoothed
/// with the earlier speed so a burst does not jump the number around.
pub(super) fn next_sample(previous: Option<RateSample>, done: u64, now_ms: u64) -> RateSample {
    let Some(previous) = previous else {
        return RateSample {
            done,
            at_ms: now_ms,
            per_sec: None,
        };
    };
    let gap = now_ms.saturating_sub(previous.at_ms);
    if gap < RATE_GAP_MS || done < previous.done {
        return previous;
    }
    let instant = (done - previous.done) as f64 / (gap as f64 / 1000.);
    RateSample {
        done,
        at_ms: now_ms,
        per_sec: Some(
            previous
                .per_sec
                .map_or(instant, |old| old * 0.6 + instant * 0.4),
        ),
    }
}

/// `3.2 MB`, `850 KB`.
pub(super) fn bytes_text(bytes: f64) -> String {
    const KB: f64 = 1024.;
    if bytes >= KB * KB * KB {
        format!("{:.1} GB", bytes / (KB * KB * KB))
    } else if bytes >= KB * KB {
        format!("{:.1} MB", bytes / (KB * KB))
    } else {
        format!("{:.0} KB", (bytes / KB).max(1.))
    }
}

/// How fast a task goes, in words.
#[must_use]
pub fn rate_text(unit: lumilio_core::ProgressUnit, per_sec: f64) -> String {
    match unit {
        lumilio_core::ProgressUnit::Bytes => {
            tr!("activity-rate-bytes", rate = bytes_text(per_sec))
        }
        lumilio_core::ProgressUnit::Items if per_sec >= 10. => {
            tr!("activity-rate-files", rate = format!("{per_sec:.0}"))
        }
        lumilio_core::ProgressUnit::Items => {
            tr!("activity-rate-files", rate = format!("{per_sec:.1}"))
        }
    }
}

/// How long is left at this speed, in words; `None` when it is not moving.
#[must_use]
pub fn eta_text(remaining: u64, per_sec: f64) -> Option<String> {
    if per_sec <= 0. || !per_sec.is_finite() {
        return None;
    }
    let seconds = (remaining as f64 / per_sec).ceil() as u64;
    Some(match seconds {
        0..=59 => tr!("activity-eta-under-minute").to_owned(),
        60..=3599 => tr!("activity-eta-minutes", count = seconds.div_ceil(60)),
        _ => tr!(
            "activity-eta-hours-minutes",
            hours = seconds / 3600,
            minutes = seconds % 3600 / 60
        ),
    })
}

/// The loaders worth offering as filters.
/// One calm sentence about what start-up recovery did, worst news first.
/// `None` when it had nothing to report. Paths and reasons stay out of it; the
/// full notes are in the log.
pub fn recovery_message(notes: &[RecoveryNote]) -> Option<String> {
    use RecoveryNote::*;
    let rank = |note: &RecoveryNote| match note {
        LibraryRecovered { .. } => 0,
        SettingsRecovered { .. } => 1,
        DeleteStuck { .. }
        | PublishStuck { .. }
        | RestoreStuck { .. }
        | DeleteConflict { .. }
        | JournalUnusable { .. } => 2,
        RestoreRolledBack { .. } => 3,
        SessionInterrupted { .. } => 4,
        ProfileMissing { .. } => 5,
        DeleteRolledBack { .. }
        | DeleteCompleted { .. }
        | PublishCompleted { .. }
        | PublishDiscarded { .. } => 6,
        ActivityLogSkipped { .. } => 7,
    };
    let first = notes.iter().min_by_key(|note| rank(note))?;
    let headline = match first {
        LibraryRecovered { candidates, .. } if candidates.is_empty() => {
            tr!("activity-recovery-library").to_owned()
        }
        LibraryRecovered { candidates, .. } => tr!(
            "activity-recovery-library-candidates",
            count = candidates.len()
        ),
        SettingsRecovered { .. } => tr!("activity-recovery-settings").to_owned(),
        DeleteStuck { .. } | PublishStuck { .. } | RestoreStuck { .. } => {
            tr!("activity-recovery-stuck").to_owned()
        }
        DeleteConflict { .. } | JournalUnusable { .. } => {
            tr!("activity-recovery-conflict").to_owned()
        }
        RestoreRolledBack { .. } => tr!("activity-recovery-restore-rolled-back").to_owned(),
        SessionInterrupted { .. } => tr!("activity-recovery-session-interrupted").to_owned(),
        ProfileMissing { .. } => tr!("activity-recovery-profile-missing").to_owned(),
        DeleteRolledBack { .. } => tr!("activity-recovery-delete-rolled-back").to_owned(),
        DeleteCompleted { .. } => tr!("activity-recovery-delete-completed").to_owned(),
        PublishCompleted { .. } => tr!("activity-recovery-publish-completed").to_owned(),
        PublishDiscarded { .. } => tr!("activity-recovery-publish-discarded").to_owned(),
        ActivityLogSkipped { count } => {
            tr!("activity-recovery-log-skipped", count = *count)
        }
    };
    Some(match notes.len() {
        1 => headline,
        more => tr!(
            "activity-recovery-more",
            headline = headline,
            more = more - 1
        ),
    })
}

pub(super) fn active_row(task: &ActiveTask, cancellable: bool) -> ActivityRow {
    ActivityRow {
        task: Some(task.id),
        amount: task.progress.filter(|(_, total)| *total > 0),
        unit: task.unit,
        rate: None,
        instance: task.instance_id.clone(),
        retry: None,
        category: task.category,
        title: task.label.clone(),
        detail: task.instance_id.clone().unwrap_or_default(),
        fraction: task
            .progress
            .filter(|(_, total)| *total > 0)
            .map(|(done, total)| (done as f64 / total as f64).clamp(0., 1.) as f32),
        state: ActivityState::Running,
        cancel: cancellable.then_some(task.id),
    }
}

pub(super) fn finished_row(task: &FinishedTask, now: u64) -> ActivityRow {
    let state = match &task.outcome {
        TaskOutcome::Succeeded => ActivityState::Done,
        TaskOutcome::Failed(message) => ActivityState::Failed(message.clone()),
        TaskOutcome::Cancelled => ActivityState::Cancelled,
    };
    ActivityRow {
        task: None,
        amount: None,
        unit: lumilio_core::ProgressUnit::Items,
        rate: None,
        instance: task.instance_id.clone(),
        retry: task.retry.clone(),
        category: task.category,
        title: task.label.clone(),
        detail: relative_time(task.finished, now),
        fraction: None,
        state,
        cancel: None,
    }
}

/// Running tasks first, then finished ones newest first.
pub fn activity_rows(view: &ActivityView, now: u64) -> Vec<ActivityRow> {
    view.active
        .iter()
        .map(|task| active_row(task, view.cancellable.contains(&task.id)))
        .chain(view.finished.iter().map(|task| finished_row(task, now)))
        .collect()
}

/// The Activity tabs: everything, then one tab per task category.
pub const ACTIVITY_CATEGORIES: [Option<TaskCategory>; 5] = [
    None,
    Some(TaskCategory::Download),
    Some(TaskCategory::Install),
    Some(TaskCategory::Update),
    Some(TaskCategory::Repair),
];

/// The rows of one Activity tab, in their given order.
pub fn activity_in_tab(rows: &[ActivityRow], tab: usize) -> Vec<&ActivityRow> {
    let wanted = ACTIVITY_CATEGORIES.get(tab).copied().flatten();
    rows.iter()
        .filter(|row| wanted.is_none_or(|category| row.category == category))
        .collect()
}
