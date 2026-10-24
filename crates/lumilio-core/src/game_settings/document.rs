use std::io::Read;
use std::path::Path;

use super::{GameSettingsError, Values, catalog};

const MAX_BYTES: usize = 1024 * 1024;
pub(super) struct Document {
    lines: Vec<(String, String)>,
    ending: String,
}

pub(super) fn read(path: &Path) -> Result<Option<String>, GameSettingsError> {
    if path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(GameSettingsError::Invalid(
            "options.txt must not be a symbolic link".into(),
        ));
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(GameSettingsError::Invalid(
            "options.txt is too large".into(),
        ));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| GameSettingsError::Invalid("options.txt is not lossless UTF-8".into()))
}

impl Document {
    pub(super) fn parse(text: &str) -> Result<Self, GameSettingsError> {
        if text.len() > MAX_BYTES || text.contains('\0') {
            return Err(GameSettingsError::Invalid("invalid options.txt".into()));
        }
        let lines = text
            .split_inclusive('\n')
            .map(|line| {
                let (raw, ending) = if let Some(raw) = line.strip_suffix("\r\n") {
                    (raw, "\r\n")
                } else if let Some(raw) = line.strip_suffix('\n') {
                    (raw, "\n")
                } else {
                    (line, "")
                };
                (raw.to_owned(), ending.to_owned())
            })
            .collect();
        Ok(Self {
            lines,
            ending: if text.contains("\r\n") { "\r\n" } else { "\n" }.into(),
        })
    }
    pub(super) fn values(&self, version: &str) -> Values {
        let mut result = Values::new();
        for (raw, _) in &self.lines {
            if let Some((key, value)) = raw.trim_start_matches('\u{feff}').split_once(':') {
                // Last duplicate wins, including a malformed last occurrence.
                result.remove(key);
                if let Some(value) = catalog::decode(key, value, version) {
                    result.insert(key.into(), value);
                }
            }
        }
        result
    }
    pub(super) fn set(&mut self, key: &str, value: &str) {
        // Update every occurrence: Minecraft's duplicate-key behavior must not affect the result.
        let mut found = false;
        for (raw, _) in &mut self.lines {
            let bom = raw.starts_with('\u{feff}');
            if raw
                .trim_start_matches('\u{feff}')
                .split_once(':')
                .is_some_and(|(k, _)| k == key)
            {
                *raw = format!("{}{key}:{value}", if bom { "\u{feff}" } else { "" });
                found = true;
            }
        }
        if !found {
            if let Some((_, ending)) = self.lines.last_mut()
                && ending.is_empty()
            {
                ending.clone_from(&self.ending);
            }
            self.lines
                .push((format!("{key}:{value}"), self.ending.clone()));
        }
    }
    pub(super) fn text(&self) -> String {
        self.lines
            .iter()
            .map(|(raw, end)| format!("{raw}{end}"))
            .collect()
    }
}

pub(super) fn publish(
    path: &Path,
    expected: Option<&str>,
    output: &str,
) -> Result<(), GameSettingsError> {
    if read(path)?.as_deref() != expected {
        return Err(GameSettingsError::Invalid(
            "options.txt changed during preparation".into(),
        ));
    }
    let mut nonce = [0_u8; 8];
    getrandom::fill(&mut nonce)
        .map_err(|e| GameSettingsError::Io(std::io::Error::other(e.to_string())))?;
    // Staging files left by a killed process cannot block the next publication.
    let temp = path.with_file_name(format!(
        ".lumilio-options-{:016x}.tmp",
        u64::from_le_bytes(nonce)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    use std::io::Write;
    let result = (|| {
        file.write_all(output.as_bytes())?;
        file.sync_all()?;
        if read(path)?.as_deref() != expected {
            return Err(GameSettingsError::Invalid(
                "options.txt changed before publication".into(),
            ));
        }
        std::fs::rename(&temp, path)?;
        #[cfg(unix)]
        if let Some(parent) = path.parent() {
            std::fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
