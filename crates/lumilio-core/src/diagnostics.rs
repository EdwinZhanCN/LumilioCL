//! Why an instance might not work, and what its logs say.
//!
//! [`diagnose`] is a pure function over gathered [`Facts`]; the log and file
//! helpers do the I/O.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use regex::Regex;

use crate::content::{ContentItem, ModMetadata};
use crate::discover::is_safe_file_name;
use crate::history::SessionOutcome;
use crate::instance::{InstanceRecord, Loader};
use crate::java::JavaRuntime;

/// Below this maximum heap (MB) the game is likely to run out of memory.
pub const LOW_MEMORY_MB: u32 = 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProblemKind {
    /// Nothing installed yet; the first launch will install.
    NotInstalled,
    LoaderUnsupported(Loader),
    NoJava {
        required: Option<u32>,
    },
    NoAccount,
    /// Files of the game installation are missing or damaged.
    DamagedFiles {
        count: usize,
    },
    /// Enabled mods built for another loader.
    WrongLoaderMods {
        names: Vec<String>,
    },
    /// The same mod id in more than one enabled file.
    DuplicateMods {
        ids: Vec<String>,
    },
    LowMemory {
        max_mb: u32,
    },
    LastSessionFailed(SessionOutcome),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Problem {
    pub severity: Severity,
    pub kind: ProblemKind,
}

/// Everything [`diagnose`] needs, gathered by the caller.
pub struct Facts<'a> {
    pub instance: &'a InstanceRecord,
    /// Loaders the launcher can currently start.
    pub launchable_loaders: &'a [Loader],
    pub runtimes: &'a [JavaRuntime],
    /// The Java major the game needs, if known (once its release is known).
    pub required_java: Option<u32>,
    pub has_account: bool,
    /// Missing/damaged installation files, if a scan was done.
    pub damaged_files: Option<usize>,
    pub mods: &'a [(ContentItem, Option<ModMetadata>)],
    /// The heap limit that would apply (instance override or launcher default).
    pub max_memory_mb: Option<u32>,
    pub last_session: Option<SessionOutcome>,
}

fn loader_name(loader: Loader) -> &'static str {
    match loader {
        Loader::Vanilla => "vanilla",
        Loader::Fabric => "fabric",
        Loader::Forge => "forge",
        Loader::NeoForge => "neoforge",
        Loader::Quilt => "quilt",
    }
}

/// Problems, most severe first (stable within a severity).
#[must_use]
pub fn diagnose(facts: &Facts<'_>) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut add = |severity, kind| problems.push(Problem { severity, kind });

    if !facts.launchable_loaders.contains(&facts.instance.loader) {
        add(
            Severity::Error,
            ProblemKind::LoaderUnsupported(facts.instance.loader),
        );
    }
    if !facts.has_account {
        add(Severity::Error, ProblemKind::NoAccount);
    }
    let java_ok = facts.runtimes.iter().any(|runtime| {
        facts
            .required_java
            .is_none_or(|required| runtime.major() >= required)
    });
    if !java_ok {
        add(
            Severity::Error,
            ProblemKind::NoJava {
                required: facts.required_java,
            },
        );
    }
    if !facts.instance.installed {
        add(Severity::Info, ProblemKind::NotInstalled);
    }
    if let Some(count) = facts.damaged_files.filter(|count| *count > 0) {
        add(Severity::Warning, ProblemKind::DamagedFiles { count });
    }

    let enabled: Vec<_> = facts.mods.iter().filter(|(item, _)| item.enabled).collect();
    let expected = loader_name(facts.instance.loader);
    if facts.instance.loader != Loader::Vanilla {
        let names: Vec<String> = enabled
            .iter()
            .filter(|(_, meta)| meta.as_ref().is_some_and(|meta| meta.loader != expected))
            .map(|(item, _)| item.display_name.clone())
            .collect();
        if !names.is_empty() {
            add(Severity::Warning, ProblemKind::WrongLoaderMods { names });
        }
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, meta) in &enabled {
        if let Some(meta) = meta {
            *seen.entry(meta.id.as_str()).or_default() += 1;
        }
    }
    let ids: Vec<String> = seen
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(id, _)| id.to_owned())
        .collect();
    if !ids.is_empty() {
        add(Severity::Warning, ProblemKind::DuplicateMods { ids });
    }
    if let Some(max_mb) = facts.max_memory_mb.filter(|max| *max < LOW_MEMORY_MB) {
        add(Severity::Warning, ProblemKind::LowMemory { max_mb });
    }
    if let Some(
        outcome @ (SessionOutcome::Crashed
        | SessionOutcome::FailedToStart
        | SessionOutcome::FailedToPrepare),
    ) = facts.last_session
    {
        add(Severity::Warning, ProblemKind::LastSessionFailed(outcome));
    }
    problems.sort_by_key(|problem| std::cmp::Reverse(problem.severity));
    problems
}

// ---- logs ----------------------------------------------------------------

/// The last `max_bytes` of the game's `logs/latest.log`, starting on a line
/// boundary when the file was cut. A missing log is `None`.
pub fn read_latest_log(game_dir: &Path, max_bytes: u64) -> io::Result<Option<String>> {
    let mut file = match fs::File::open(game_dir.join("logs").join("latest.log")) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let length = file.metadata()?.len();
    let start = length.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(max_bytes).read_to_end(&mut bytes)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if start > 0
        && let Some(newline) = text.find('\n')
    {
        text.drain(..=newline);
    }
    Ok(Some(text))
}

/// How serious a log line is. Fatal counts as an error, trace as debug.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// One line of a game log with the level it belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogLine<'a> {
    pub level: LogLevel,
    pub text: &'a str,
}

fn level_named(name: &str) -> Option<LogLevel> {
    match name {
        "TRACE" | "DEBUG" => Some(LogLevel::Debug),
        "INFO" => Some(LogLevel::Info),
        "WARN" => Some(LogLevel::Warn),
        "ERROR" | "FATAL" => Some(LogLevel::Error),
        _ => None,
    }
}

/// The level a line states about itself, from its leading bracket groups:
/// `[21:04:11] [main/INFO]: …` or `[21:04:11] [ERROR]: …`. Stack-trace lines
/// and other continuations state none.
#[must_use]
pub fn log_level(line: &str) -> Option<LogLevel> {
    let mut rest = line.trim_start();
    for _ in 0..3 {
        let inner = rest.strip_prefix('[')?;
        let end = inner.find(']')?;
        let group = &inner[..end];
        let name = group.rsplit('/').next().unwrap_or(group);
        if let Some(level) = level_named(name) {
            return Some(level);
        }
        rest = inner[end + 1..].trim_start();
    }
    None
}

/// Splits a log into lines. A line that states no level (a stack trace, a
/// wrapped message) belongs to the line before it; a log that starts without
/// one reads as information.
#[must_use]
pub fn log_lines(text: &str) -> Vec<LogLine<'_>> {
    let mut level = LogLevel::Info;
    text.lines()
        .map(|text| {
            if let Some(stated) = log_level(text) {
                level = stated;
            }
            LogLine { level, text }
        })
        .collect()
}

/// The lines at or above `least` (all when `None`) that contain `needle`
/// ignoring case (all when empty).
#[must_use]
pub fn filter_log<'a>(
    lines: &[LogLine<'a>],
    least: Option<LogLevel>,
    needle: &str,
) -> Vec<LogLine<'a>> {
    let needle = needle.trim().to_lowercase();
    lines
        .iter()
        .filter(|line| least.is_none_or(|least| line.level >= least))
        .filter(|line| needle.is_empty() || line.text.to_lowercase().contains(&needle))
        .copied()
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrashReport {
    pub file_name: String,
    pub modified: u64,
}

/// Crash reports, newest first.
pub fn list_crash_reports(game_dir: &Path) -> io::Result<Vec<CrashReport>> {
    let entries = match fs::read_dir(game_dir.join("crash-reports")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut reports = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };
        if !file_name.ends_with(".txt") || !entry.file_type()?.is_file() {
            continue;
        }
        let modified = entry
            .metadata()?
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |elapsed| elapsed.as_secs());
        reports.push(CrashReport {
            file_name,
            modified,
        });
    }
    reports.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.file_name.cmp(&a.file_name))
    });
    Ok(reports)
}

/// Reads one crash report by file name (never a path), capped at `max_bytes`.
pub fn read_crash_report(game_dir: &Path, file_name: &str, max_bytes: u64) -> io::Result<String> {
    if !is_safe_file_name(file_name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsafe file name",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(game_dir.join("crash-reports").join(file_name))?
        .take(max_bytes)
        .read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrashHint {
    /// The JVM ran out of heap.
    OutOfMemory,
    /// Classes were built for a newer Java than the one used.
    JavaTooOld,
    /// A JVM option was not recognized by this Java.
    BadJvmOption,
    /// Mods disagree with each other or with the loader.
    ModConflict,
    /// The graphics driver or OpenGL failed.
    GraphicsFailure,
}

/// Recognizes well-known failures in a log or crash report. Hints come in a
/// fixed order and each appears at most once.
#[must_use]
pub fn analyze(text: &str) -> Vec<CrashHint> {
    let rules: [(CrashHint, &str); 5] = [
        (
            CrashHint::OutOfMemory,
            r"java\.lang\.OutOfMemoryError|There is insufficient memory",
        ),
        (
            CrashHint::JavaTooOld,
            r"UnsupportedClassVersionError|class file version \d+\.\d+|requires running the game with Java \d+",
        ),
        (
            CrashHint::BadJvmOption,
            r"Unrecognized (VM )?option|Could not create the Java Virtual Machine",
        ),
        (
            CrashHint::ModConflict,
            r"Mixin apply failed|Incompatible mod set|Mod resolution failed|Duplicate mod",
        ),
        (
            CrashHint::GraphicsFailure,
            r"GLFW error|OpenGL error|Pixel format not accelerated",
        ),
    ];
    rules
        .into_iter()
        .filter(|(_, pattern)| Regex::new(pattern).is_ok_and(|regex| regex.is_match(text)))
        .map(|(hint, _)| hint)
        .collect()
}

// ---- files ---------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}

/// Lists a folder inside the game directory. `relative` may be empty (the game
/// directory itself) or a `/`-separated path of plain names; `..`, absolute
/// paths and prefixes are refused. Folders first, then names ignoring case.
pub fn list_dir(game_dir: &Path, relative: &str) -> io::Result<Vec<FileEntry>> {
    let mut path = PathBuf::from(game_dir);
    for part in relative.split('/').filter(|part| !part.is_empty()) {
        let component = Path::new(part).components().next();
        if part.contains(['\\', ':', '\0'])
            || !matches!(component, Some(Component::Normal(_)))
            || Path::new(part).components().count() != 1
        {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "unsafe path"));
        }
        path.push(part);
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let kind = entry.file_type()?;
        // Links are shown as files of size 0, never followed.
        let metadata = entry.metadata()?;
        entries.push(FileEntry {
            name,
            is_dir: kind.is_dir(),
            size: if kind.is_file() { metadata.len() } else { 0 },
            modified: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |elapsed| elapsed.as_secs()),
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

#[cfg(test)]
mod tests;
