use super::library::relative_time;
use lumilio_core::{
    ActiveTask, ActivityView, FinishedTask, RecoveryNote, TaskCategory, TaskOutcome,
};

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
        lumilio_core::ProgressUnit::Bytes => format!("{}/秒", bytes_text(per_sec)),
        lumilio_core::ProgressUnit::Items if per_sec >= 10. => format!("{per_sec:.0} 个文件/秒"),
        lumilio_core::ProgressUnit::Items => format!("{per_sec:.1} 个文件/秒"),
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
        0..=59 => "不到 1 分钟".to_owned(),
        60..=3599 => format!("约 {} 分钟", seconds.div_ceil(60)),
        _ => format!("约 {} 小时 {} 分钟", seconds / 3600, seconds % 3600 / 60),
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
            "游戏库文件无法读取，已保留原文件并重新开始".to_owned()
        }
        LibraryRecovered { candidates, .. } => format!(
            "游戏库文件无法读取，已保留原文件并重新开始；磁盘上找到 {} 个可能的游戏目录",
            candidates.len()
        ),
        SettingsRecovered { .. } => "设置文件无法读取，已保留原文件并使用默认设置".to_owned(),
        DeleteStuck { .. } | PublishStuck { .. } | RestoreStuck { .. } => {
            "上次中断的操作还没能收尾，文件已保留，下次启动会再试".to_owned()
        }
        DeleteConflict { .. } | JournalUnusable { .. } => {
            "发现无法自动处理的中断记录，相关文件没有被改动".to_owned()
        }
        RestoreRolledBack { .. } => "上次快照恢复被中断，已回到恢复前的样子".to_owned(),
        SessionInterrupted { .. } => "启动器上次在游戏运行时退出，那次游玩的结果未知".to_owned(),
        ProfileMissing { .. } => "有游戏的目录不见了，可以在诊断里查看".to_owned(),
        DeleteRolledBack { .. } => "上次删除被中断，游戏已原样保留".to_owned(),
        DeleteCompleted { .. } => "上次中断的删除已经完成".to_owned(),
        PublishCompleted { .. } => "上次中断的导入或复制已经完成".to_owned(),
        PublishDiscarded { .. } => "上次中断的导入或复制没有完成，已清理，可以重新开始".to_owned(),
        ActivityLogSkipped { count } => format!("动态记录里有 {count} 行无法读取，已跳过"),
    };
    Some(match notes.len() {
        1 => headline,
        more => format!("{headline}（另有 {} 项恢复记录）", more - 1),
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
