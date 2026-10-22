//! Everything the game printed during its last launch, kept in its own
//! `logs/` folder. A game that quits before it writes a log, or is killed,
//! still leaves a record that Diagnostics can open after the launcher has
//! closed. Modrinth App keeps `logs/launcher_log.txt` the same way
//! (packages/app-lib/src/state/process.rs in modrinth/code).

use super::LAUNCHER_NAME;
use super::types::LaunchRequest;
use crate::instance::Loader;
use crate::java::JavaRuntime;
use std::fs::File;
use std::io::{LineWriter, Write as _};
use std::path::Path;
use std::time::Duration;

/// The record's file name in the game's `logs/` folder.
pub const GAME_OUTPUT_LOG: &str = "lumilio-output.log";

/// Past this the rest is left to the game's own logs, so a game that floods
/// its console cannot fill the disk; Diagnostics reads files up to 64 MiB.
const LIMIT: u64 = 32 * 1024 * 1024;

pub(super) struct OutputFile {
    file: LineWriter<File>,
    written: u64,
    limit: u64,
    /// The limit was reached: the game's lines are no longer kept.
    full: bool,
    /// The disk refused a line: nothing more is written.
    failed: bool,
}

impl OutputFile {
    /// Starts this launch's record, replacing the last one. `about` heads it.
    pub(super) fn create(game_dir: &Path, about: &[String]) -> std::io::Result<Self> {
        let logs = game_dir.join("logs");
        std::fs::create_dir_all(&logs)?;
        let mut output = Self {
            file: LineWriter::new(File::create(logs.join(GAME_OUTPUT_LOG))?),
            written: 0,
            limit: LIMIT,
            full: false,
            failed: false,
        };
        for line in about {
            output.note(line);
        }
        Ok(output)
    }

    /// A line the launcher adds, marked as its own.
    pub(super) fn note(&mut self, text: &str) {
        self.write(&format!("# {text}"));
    }

    /// A line the game printed.
    pub(super) fn line(&mut self, text: &str) {
        if self.full {
            return;
        }
        if self.written + text.len() as u64 + 1 > self.limit {
            self.full = true;
            self.note("The output goes on past 32 MiB; the game's own logs have the rest.");
            return;
        }
        self.write(text);
    }

    /// How the game ended, said even after the limit.
    pub(super) fn ended(&mut self, code: Option<i32>, ran_for: Duration, killed: bool) {
        let seconds = ran_for.as_secs_f64();
        self.note(&match (killed, code) {
            (true, _) => format!("The launcher stopped the game after {seconds:.1} s."),
            (false, Some(code)) => {
                format!("The game ended with exit code {code} after {seconds:.1} s.")
            }
            (false, None) => format!("The game ended without an exit code after {seconds:.1} s."),
        });
    }

    fn write(&mut self, text: &str) {
        if self.failed {
            return;
        }
        // A disk that stops taking lines ends the record, never the game.
        match writeln!(self.file, "{text}") {
            Ok(()) => self.written += text.len() as u64 + 1,
            Err(_) => self.failed = true,
        }
    }
}

/// What heads the record: when and by what the game was started.
pub(super) fn heading(request: &LaunchRequest, java: &JavaRuntime) -> Vec<String> {
    let instance = &request.instance;
    let game = match (instance.loader, &instance.loader_version) {
        (Loader::Vanilla, _) | (_, None) => format!("Minecraft {}", instance.game_version),
        (loader, Some(version)) => {
            format!("Minecraft {} · {loader:?} {version}", instance.game_version)
        }
    };
    vec![
        format!(
            "{LAUNCHER_NAME} {} · the game's output from {}",
            env!("CARGO_PKG_VERSION"),
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S %:z")
        ),
        format!(
            "{game} · Java {} at {}",
            java.version(),
            java.executable().display()
        ),
    ]
}

#[cfg(test)]
mod tests;
