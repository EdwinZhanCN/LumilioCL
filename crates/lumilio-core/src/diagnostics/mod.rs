//! Why an instance might not work, what its logs say, and how much disk the
//! launcher uses.
//!
//! [`diagnose`] is a pure function over gathered [`Facts`]; the log and file
//! helpers do the I/O. [`Diagnostics`] reads them for one game, exports them
//! for a bug report and reclaims shared files no game uses.

pub use disk::{StorageUsage, clear_cache, directory_size, measure as measure_storage};
pub use problems::{
    CrashReport, Facts, FileEntry, LOW_MEMORY_MB, LogLevel, LogLine, Problem, ProblemKind,
    Severity, diagnose, filter_log, list_crash_reports, list_dir, log_level, log_lines,
    read_crash_report, read_latest_log,
};
pub(crate) use problems::{list_game_logs, read_complete_crash, read_game_log};
#[cfg(test)]
pub(crate) use reports::LOG_TAIL_BYTES;
pub(crate) use reports::{Diagnostics, game_facts};
pub use reports::{GameLogSource, GameLogs};

mod disk;
mod maintenance;
mod problems;
mod reports;
