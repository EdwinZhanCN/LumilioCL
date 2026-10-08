use crate::tr;
use lumilio_core::{ChangeKind, SessionOutcome};

pub fn change_label(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::ContentAdded => tr!("instance-change-content-added"),
        ChangeKind::ContentRemoved => tr!("instance-change-content-removed"),
        ChangeKind::ContentEnabled => tr!("instance-change-content-enabled"),
        ChangeKind::ContentDisabled => tr!("instance-change-content-disabled"),
        ChangeKind::ContentUpdated => tr!("instance-change-content-updated"),
        ChangeKind::SettingsChanged => tr!("instance-change-settings"),
        ChangeKind::GameVersionChanged => tr!("instance-change-game-version"),
        ChangeKind::Repaired => tr!("instance-change-repaired"),
        ChangeKind::WorldCopied => tr!("instance-change-world-copied"),
        ChangeKind::WorldDeleted => tr!("instance-change-world-deleted"),
        ChangeKind::WorldImported => tr!("instance-change-world-imported"),
        ChangeKind::SnapshotCreated => tr!("instance-change-snapshot-created"),
        ChangeKind::SnapshotRestored => tr!("instance-change-snapshot-restored"),
        ChangeKind::SnapshotDeleted => tr!("instance-change-snapshot-deleted"),
    }
}

pub fn outcome_label(outcome: SessionOutcome) -> &'static str {
    match outcome {
        SessionOutcome::Clean => tr!("instance-outcome-clean"),
        SessionOutcome::Crashed => tr!("instance-outcome-crashed"),
        SessionOutcome::FailedToStart => tr!("instance-outcome-failed-to-start"),
        SessionOutcome::Stopped => tr!("instance-outcome-stopped"),
        SessionOutcome::FailedToPrepare => tr!("instance-outcome-failed-to-prepare"),
        SessionOutcome::Cancelled => tr!("instance-outcome-cancelled"),
        SessionOutcome::Interrupted => tr!("instance-outcome-interrupted"),
    }
}

/// What a finished export tells, in words.
pub fn export_notice(report: &lumilio_core::ExportReport, path: &std::path::Path) -> String {
    if report.lookup_failed {
        tr!(
            "instance-export-offline",
            bundled = report.bundled,
            path = path.display().to_string()
        )
    } else {
        tr!(
            "instance-export-linked",
            path = path.display().to_string(),
            linked = report.linked,
            bundled = report.bundled
        )
    }
}

pub fn duration_label(seconds: u64) -> String {
    match seconds {
        0..=59 => tr!("activity-eta-under-minute").to_owned(),
        60..=3599 => tr!("instance-duration-minutes", count = seconds / 60),
        _ => tr!(
            "instance-duration-hours-minutes",
            hours = seconds / 3600,
            minutes = seconds % 3600 / 60
        ),
    }
}

pub fn size_label(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    match bytes {
        0 => "—".to_owned(),
        1..=1023 => format!("{bytes} B"),
        1024..=1_048_575 => format!("{} KB", bytes / 1024),
        _ if bytes < 1024 * MB => format!("{:.1} MB", bytes as f64 / MB as f64),
        _ => format!("{:.2} GB", bytes as f64 / (1024 * MB) as f64),
    }
}
