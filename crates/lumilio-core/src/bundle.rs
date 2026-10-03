//! The diagnostics bundle: a zip of what a maintainer needs to understand a
//! problem, with names and paths of the person replaced.
//!
//! Behavior notes: `docs/behavior/diagnostics.md`.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;

/// Replaces private text with stand-ins, longest match first.
#[derive(Clone, Debug, Default)]
pub struct Redactor {
    pairs: Vec<(String, String)>,
}

impl Redactor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces `secret` with `stand_in`. Blank or one-character secrets are
    /// ignored: they would rewrite half of any text.
    pub fn hide(&mut self, secret: &str, stand_in: &str) {
        let secret = secret.trim();
        if secret.chars().count() < 2 || self.pairs.iter().any(|(known, _)| known == secret) {
            return;
        }
        self.pairs.push((secret.to_owned(), stand_in.to_owned()));
        self.pairs
            .sort_by_key(|(secret, _)| std::cmp::Reverse(secret.len()));
    }

    /// Hides a folder under both slash styles.
    pub fn hide_path(&mut self, path: &Path, stand_in: &str) {
        let text = path.to_string_lossy();
        self.hide(&text, stand_in);
        self.hide(&text.replace('\\', "/"), stand_in);
        self.hide(&text.replace('/', "\\\\"), stand_in);
    }

    #[must_use]
    pub fn apply(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for (secret, stand_in) in &self.pairs {
            text = text.replace(secret.as_str(), stand_in);
        }
        text
    }
}

/// Writes `files` (name, text) into a zip at `destination`, through a
/// temporary file so a failure leaves nothing half-written.
pub fn write_bundle(destination: &Path, files: &[(String, String)]) -> io::Result<()> {
    let temporary = destination.with_extension("zip.part");
    let result = (|| {
        let mut writer = zip::ZipWriter::new(fs::File::create(&temporary)?);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, text) in files {
            writer.start_file(name.as_str(), options)?;
            writer.write_all(text.as_bytes())?;
        }
        writer.finish()?;
        fs::rename(&temporary, destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn names_ids_and_folders_are_replaced_longest_first() {
        let mut redactor = Redactor::new();
        redactor.hide("Steve", "<player>");
        redactor.hide_path(Path::new("/Users/steve_home"), "~");
        redactor.hide_path(
            Path::new("/Users/steve_home/Library/LumilioCL"),
            "<launcher>",
        );
        redactor.hide("a", "<x>");
        redactor.hide("  ", "<y>");
        let text = "Steve at /Users/steve_home/Library/LumilioCL/profiles and /Users/steve_home/x";
        assert_eq!(
            redactor.apply(text),
            "<player> at <launcher>/profiles and ~/x",
            "the longer folder wins, and one-letter secrets are ignored"
        );
    }

    #[test]
    fn the_bundle_holds_the_files_and_leaves_nothing_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("diag.zip");
        write_bundle(
            &path,
            &[
                ("about.txt".to_owned(), "hello".to_owned()),
                ("logs/a.log".to_owned(), "line".to_owned()),
            ],
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
        let mut text = String::new();
        archive
            .by_name("logs/a.log")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "line");
        assert!(!dir.path().join("diag.zip.part").exists());

        let blocked = dir.path().join("no-such-folder/diag.zip");
        assert!(write_bundle(&blocked, &[]).is_err());
        assert!(!dir.path().join("no-such-folder").exists());
    }
}
