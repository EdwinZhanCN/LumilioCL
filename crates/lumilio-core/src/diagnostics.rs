//! Why an instance might not work, and what its logs say.
//!
//! Behavior notes: `docs/behavior/diagnostics.md`. [`diagnose`] is a pure
//! function over gathered [`Facts`]; the log and file helpers do the I/O.

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
mod tests {
    use super::*;
    use crate::instance::InstanceSettings;

    #[test]
    fn levels_come_from_the_bracket_groups_and_continuations_inherit() {
        assert_eq!(
            log_level("[21:04:11] [main/INFO]: Loading"),
            Some(LogLevel::Info)
        );
        assert_eq!(
            log_level("[21:04:11] [Render thread/WARN] [net.minecraft/]: x"),
            Some(LogLevel::Warn)
        );
        assert_eq!(
            log_level("[21:04:11] [Server thread/FATAL]: x"),
            Some(LogLevel::Error)
        );
        assert_eq!(log_level("[21:04:11] [ERROR]: x"), Some(LogLevel::Error));
        assert_eq!(
            log_level("[21:04:11] [main/TRACE]: x"),
            Some(LogLevel::Debug)
        );
        assert_eq!(log_level("\tat net.minecraft.Foo(Foo.java:1)"), None);
        assert_eq!(log_level("[not a level] plain"), None);
        assert_eq!(log_level("a [main/ERROR] mid-line mention"), None);

        let text = "boot line\n[1] [main/INFO]: fine\n[2] [main/ERROR]: boom\n\tat a.B(B.java)\n[3] [main/INFO]: ok";
        let lines = log_lines(text);
        let levels: Vec<_> = lines.iter().map(|line| line.level).collect();
        assert_eq!(
            levels,
            [
                LogLevel::Info,
                LogLevel::Info,
                LogLevel::Error,
                LogLevel::Error,
                LogLevel::Info
            ]
        );
    }

    #[test]
    fn filtering_keeps_the_level_and_the_search_together() {
        let text = "[1] [main/INFO]: loaded sodium\n[2] [main/WARN]: Sodium is old\n[3] [main/ERROR]: crash\n\tat sodium.Thing";
        let lines = log_lines(text);
        assert_eq!(filter_log(&lines, None, "").len(), 4);
        assert_eq!(filter_log(&lines, Some(LogLevel::Warn), "").len(), 3);
        assert_eq!(filter_log(&lines, Some(LogLevel::Error), "").len(), 2);
        let found = filter_log(&lines, Some(LogLevel::Warn), "SODIUM");
        assert_eq!(
            found.len(),
            2,
            "case-insensitive, trace line inherits error"
        );
        assert!(filter_log(&lines, None, "nothing like this").is_empty());
    }

    fn record(loader: Loader, installed: bool) -> InstanceRecord {
        InstanceRecord {
            id: "a".into(),
            name: "A".into(),
            game_version: "1.21.1".into(),
            loader,
            loader_version: None,
            favorite: false,
            created_at: 0,
            last_played: None,
            play_seconds: 0,
            installed,
            settings: InstanceSettings::default(),
        }
    }

    fn java(major: u32) -> JavaRuntime {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("j");
        fs::create_dir_all(home.join("bin")).unwrap();
        fs::write(home.join("bin/java"), "").unwrap();
        fs::write(home.join("release"), format!("JAVA_VERSION=\"{major}\"")).unwrap();
        // Only the parsed values are used after the temp dir goes away.
        JavaRuntime::inspect(&home).unwrap()
    }

    fn item(name: &str, enabled: bool) -> ContentItem {
        ContentItem {
            file_name: name.into(),
            display_name: name.into(),
            enabled,
            size: 1,
            modified: 0,
            is_directory: false,
        }
    }

    fn meta(id: &str, loader: &'static str) -> Option<ModMetadata> {
        Some(ModMetadata {
            id: id.into(),
            name: None,
            version: None,
            loader,
        })
    }

    fn healthy<'a>(instance: &'a InstanceRecord, runtimes: &'a [JavaRuntime]) -> Facts<'a> {
        Facts {
            instance,
            launchable_loaders: &[Loader::Vanilla, Loader::Fabric],
            runtimes,
            required_java: Some(21),
            has_account: true,
            damaged_files: Some(0),
            mods: &[],
            max_memory_mb: Some(4096),
            last_session: Some(SessionOutcome::Clean),
        }
    }

    #[test]
    fn a_healthy_installed_instance_has_no_problems() {
        let instance = record(Loader::Vanilla, true);
        let runtimes = [java(21)];
        assert!(diagnose(&healthy(&instance, &runtimes)).is_empty());
    }

    #[test]
    fn blocking_problems_are_errors_and_sort_first() {
        let instance = record(Loader::Forge, false);
        let mut facts = healthy(&instance, &[]);
        facts.has_account = false;
        facts.damaged_files = Some(3);
        let problems = diagnose(&facts);
        let kinds: Vec<_> = problems
            .iter()
            .map(|p| (p.severity, p.kind.clone()))
            .collect();
        assert_eq!(kinds[0].0, Severity::Error);
        assert!(kinds.contains(&(
            Severity::Error,
            ProblemKind::LoaderUnsupported(Loader::Forge)
        )));
        assert!(kinds.contains(&(Severity::Error, ProblemKind::NoAccount)));
        assert!(kinds.contains(&(Severity::Error, ProblemKind::NoJava { required: Some(21) })));
        assert!(kinds.contains(&(Severity::Info, ProblemKind::NotInstalled)));
        assert!(kinds.contains(&(Severity::Warning, ProblemKind::DamagedFiles { count: 3 })));
        assert!(problems.windows(2).all(|w| w[0].severity >= w[1].severity));
    }

    #[test]
    fn an_older_java_is_not_enough_but_a_newer_one_is() {
        let instance = record(Loader::Vanilla, true);
        let old = [java(17)];
        assert!(
            diagnose(&healthy(&instance, &old))
                .iter()
                .any(|p| matches!(p.kind, ProblemKind::NoJava { .. }))
        );
        let newer = [java(25)];
        assert!(diagnose(&healthy(&instance, &newer)).is_empty());
    }

    #[test]
    fn mod_problems_ignore_disabled_files_and_unreadable_metadata() {
        let instance = record(Loader::Fabric, true);
        let runtimes = [java(21)];
        let mods = vec![
            (item("a.jar", true), meta("sodium", "fabric")),
            (item("a-old.jar", true), meta("sodium", "fabric")),
            (item("forge-only.jar", true), meta("jei", "forge")),
            (item("off.jar", false), meta("jei", "forge")),
            (item("mystery.jar", true), None),
        ];
        let mut facts = healthy(&instance, &runtimes);
        facts.mods = &mods;
        let kinds: Vec<_> = diagnose(&facts).into_iter().map(|p| p.kind).collect();
        assert!(kinds.contains(&ProblemKind::DuplicateMods {
            ids: vec!["sodium".into()]
        }));
        assert!(kinds.contains(&ProblemKind::WrongLoaderMods {
            names: vec!["forge-only.jar".into()]
        }));
    }

    #[test]
    fn memory_and_last_session_warnings() {
        let instance = record(Loader::Vanilla, true);
        let runtimes = [java(21)];
        let mut facts = healthy(&instance, &runtimes);
        facts.max_memory_mb = Some(512);
        facts.last_session = Some(SessionOutcome::Crashed);
        let kinds: Vec<_> = diagnose(&facts).into_iter().map(|p| p.kind).collect();
        assert!(kinds.contains(&ProblemKind::LowMemory { max_mb: 512 }));
        assert!(kinds.contains(&ProblemKind::LastSessionFailed(SessionOutcome::Crashed)));
        facts.max_memory_mb = None;
        facts.last_session = Some(SessionOutcome::Stopped);
        assert!(
            diagnose(&facts).is_empty(),
            "no limit set and a user-stopped run are not problems"
        );
    }

    #[test]
    fn the_log_tail_starts_on_a_line_boundary() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_latest_log(dir.path(), 100).unwrap(), None);
        fs::create_dir_all(dir.path().join("logs")).unwrap();
        fs::write(
            dir.path().join("logs/latest.log"),
            "line one\nline two\nline three\n",
        )
        .unwrap();
        assert_eq!(
            read_latest_log(dir.path(), 1000).unwrap().unwrap(),
            "line one\nline two\nline three\n"
        );
        // 15 bytes back lands mid "line two"; the partial line is dropped.
        assert_eq!(
            read_latest_log(dir.path(), 15).unwrap().unwrap(),
            "line three\n"
        );
    }

    #[test]
    fn crash_reports_list_newest_first_and_read_by_name_only() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list_crash_reports(dir.path()).unwrap().is_empty());
        let folder = dir.path().join("crash-reports");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("crash-b.txt"), "second").unwrap();
        fs::write(folder.join("crash-a.txt"), "first").unwrap();
        fs::write(folder.join("notes.md"), "x").unwrap();
        let reports = list_crash_reports(dir.path()).unwrap();
        assert_eq!(reports.len(), 2);
        assert_eq!(
            read_crash_report(dir.path(), "crash-a.txt", 100).unwrap(),
            "first"
        );
        assert_eq!(
            read_crash_report(dir.path(), "crash-a.txt", 3).unwrap(),
            "fir"
        );
        assert!(read_crash_report(dir.path(), "../x", 10).is_err());
    }

    #[test]
    fn known_failures_are_recognized_once_each_in_order() {
        let text = "Exception in thread main java.lang.OutOfMemoryError: Java heap space\n\
                    java.lang.UnsupportedClassVersionError: bad\n\
                    Mixin apply failed x\nMixin apply failed y\n";
        assert_eq!(
            analyze(text),
            [
                CrashHint::OutOfMemory,
                CrashHint::JavaTooOld,
                CrashHint::ModConflict
            ]
        );
        assert!(analyze("all good").is_empty());
        assert_eq!(
            analyze("Unrecognized option: -XX:Foo"),
            [CrashHint::BadJvmOption]
        );
    }

    #[test]
    fn listing_files_is_confined_to_the_game_directory() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("config")).unwrap();
        fs::write(game.join("options.txt"), "12345").unwrap();
        fs::write(game.join("Alpha.txt"), "1").unwrap();
        fs::write(dir.path().join("secret.txt"), "no").unwrap();
        let root = list_dir(&game, "").unwrap();
        let names: Vec<_> = root.iter().map(|e| (e.name.as_str(), e.is_dir)).collect();
        assert_eq!(
            names,
            [
                ("config", true),
                ("Alpha.txt", false),
                ("options.txt", false)
            ]
        );
        assert_eq!(root[2].size, 5);
        assert!(list_dir(&game, "config/").unwrap().is_empty());
        for bad in ["..", "config/../..", "/etc", "a\\b", "C:"] {
            assert!(list_dir(&game, bad).is_err(), "{bad}");
        }
    }
}
