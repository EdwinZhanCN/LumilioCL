//! The diagnostics bundle: a zip of what a maintainer needs to understand a
//! problem, with names and paths of the person replaced.

use std::io::Write;
use std::path::Path;
use std::{fs, io};

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
mod tests;
