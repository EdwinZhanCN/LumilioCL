//! The launcher's own log: its warnings and errors, and what it noted while
//! starting, in `<root>/logs/launcher.log`. A released build has no console,
//! so this file is where a problem the person reports can be read back. When
//! the file passes [`LIMIT`] it becomes `launcher.1.log` (replacing the one
//! before) and a new one starts. Lines also go to stderr for development.

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// How large one log file grows before the next one starts.
pub(crate) const LIMIT: u64 = 1024 * 1024;

/// Writes the log under `root` for the rest of the run. Without a writable
/// folder the lines still go to stderr.
pub(crate) fn install(root: &Path) {
    let layout = lumilio_core::Layout::new(root);
    let log = FileLog::new(layout.launcher_log(), layout.earlier_launcher_log(), LIMIT);
    if log::set_boxed_logger(Box::new(log)).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

pub(crate) struct FileLog {
    current: PathBuf,
    earlier: PathBuf,
    limit: u64,
    /// The open file and how much it holds; `None` until the first line, or
    /// after the folder could not be written.
    file: Mutex<Option<(File, u64)>>,
}

impl FileLog {
    pub(crate) fn new(current: PathBuf, earlier: PathBuf, limit: u64) -> Self {
        Self {
            current,
            earlier,
            limit,
            file: Mutex::new(None),
        }
    }

    fn open(&self) -> Option<(File, u64)> {
        std::fs::create_dir_all(self.current.parent()?).ok()?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.current)
            .ok()?;
        let size = file.metadata().map_or(0, |meta| meta.len());
        Some((file, size))
    }

    fn write(&self, line: &str) {
        let Ok(mut slot) = self.file.lock() else {
            return;
        };
        if slot.as_ref().is_some_and(|(_, size)| *size >= self.limit) {
            *slot = None;
            let _ = std::fs::rename(&self.current, &self.earlier);
        }
        if slot.is_none() {
            *slot = self.open();
        }
        if let Some((file, size)) = slot.as_mut()
            && file.write_all(line.as_bytes()).is_ok()
        {
            *size += line.len() as u64;
        }
    }
}

impl Log for FileLog {
    /// The launcher's own notes from `info` up; libraries (GPUI, the HTTP
    /// stack) only when something went wrong.
    fn enabled(&self, metadata: &Metadata) -> bool {
        let floor = if metadata.target().starts_with("lumilio") {
            Level::Info
        } else {
            Level::Warn
        };
        metadata.level() <= floor
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {:<5} {}: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z"),
            record.level(),
            record.target(),
            record.args()
        );
        eprint!("{line}");
        self.write(&line);
    }

    fn flush(&self) {
        if let Ok(mut slot) = self.file.lock()
            && let Some((file, _)) = slot.as_mut()
        {
            let _ = file.flush();
        }
    }
}

#[cfg(test)]
mod tests;
