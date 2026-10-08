use crate::tr_all;
use lumilio_core::FileEntry;

pub fn diagnostic_labels() -> &'static [&'static str] {
    tr_all![
        "instance-diagnostic-problems",
        "instance-diagnostic-logs",
        "instance-diagnostic-files",
    ]
}

/// The entries whose name contains `query` (ignoring case); all when empty.
pub fn files_matching<'a>(entries: &'a [FileEntry], query: &str) -> Vec<&'a FileEntry> {
    let query = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|entry| query.is_empty() || entry.name.to_lowercase().contains(&query))
        .collect()
}

/// How long after a session's end a crash report still belongs to it.
pub(super) const CRASH_GRACE: u64 = 120;

/// The crash report written during a session: the one modified closest to
/// its end, between its start and shortly after.
pub fn crash_for_session(
    reports: &[lumilio_core::CrashReport],
    started: u64,
    seconds: u64,
) -> Option<&lumilio_core::CrashReport> {
    let end = started.saturating_add(seconds);
    reports
        .iter()
        .filter(|report| report.modified >= started && report.modified <= end + CRASH_GRACE)
        .min_by_key(|report| report.modified.abs_diff(end))
}

/// `folder` and `name` as one path below the game directory.
pub fn join_path(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_owned()
    } else {
        format!("{folder}/{name}")
    }
}

/// The folder one level up (`""` at the top).
pub fn parent_path(folder: &str) -> String {
    folder
        .rsplit_once('/')
        .map_or_else(String::new, |(parent, _)| parent.to_owned())
}
