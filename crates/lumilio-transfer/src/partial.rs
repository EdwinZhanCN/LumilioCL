//! Downloads left part-way. The bytes stay beside the destination, so the
//! finished file is still published by a rename; a note in one folder says
//! where they came from, so a later attempt continues instead of starting
//! over, and so leftovers can be found and removed.

use super::files::encode_lower_hex;
use super::request::ContentDigest;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::fs;

/// Where a download keeps its bytes until they are verified.
pub(super) fn part_path(destination: &Path) -> PathBuf {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");
    parent.join(format!(".{name}.lumilio.part"))
}

/// What the bytes in a part file are, so a later attempt knows whether it
/// may continue them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Note {
    pub(super) part: PathBuf,
    pub(super) source: String,
    /// The response's `ETag` or `Last-Modified`, sent back as `If-Range`.
    pub(super) validator: Option<String>,
    pub(super) digest: Option<ContentDigest>,
}

impl Note {
    /// Whether the bytes may continue from `source` for a file that must
    /// hash to `digest`. A known digest checks the result from any source;
    /// without one only the same unchanged source can be trusted.
    pub(super) fn continues(&self, source: &str, digest: Option<&ContentDigest>) -> bool {
        match digest {
            Some(digest) => self.digest.as_ref() == Some(digest),
            None => self.source == source && self.validator.is_some(),
        }
    }

    /// One `key<TAB>value` line per field; `None` when a value would break
    /// that shape.
    fn encode(&self) -> Option<String> {
        let mut fields = vec![("part", self.part.to_str()?), ("source", &self.source)];
        if let Some(validator) = &self.validator {
            fields.push(("validator", validator));
        }
        match &self.digest {
            Some(ContentDigest::Sha1(hex)) => fields.push(("sha1", hex)),
            Some(ContentDigest::Sha256(hex)) => fields.push(("sha256", hex)),
            None => {}
        }
        if fields
            .iter()
            .any(|(_, value)| value.contains(['\n', '\r', '\t']))
        {
            return None;
        }
        Some(
            fields
                .iter()
                .map(|(key, value)| format!("{key}\t{value}\n"))
                .collect(),
        )
    }

    fn decode(text: &str) -> Option<Self> {
        let mut part = None;
        let mut source = None;
        let mut validator = None;
        let mut digest = None;
        for line in text.lines() {
            let (key, value) = line.split_once('\t')?;
            match key {
                "part" => part = Some(PathBuf::from(value)),
                "source" => source = Some(value.to_owned()),
                "validator" => validator = Some(value.to_owned()),
                "sha1" => digest = ContentDigest::parse(value.to_owned(), false).ok(),
                "sha256" => digest = ContentDigest::parse(value.to_owned(), true).ok(),
                _ => {}
            }
        }
        Some(Self {
            part: part?,
            source: source?,
            validator,
            digest,
        })
    }
}

/// The folder of notes for downloads kept part-way.
#[derive(Clone, Debug)]
pub(super) struct Partials {
    pub(super) folder: PathBuf,
}

impl Partials {
    fn note_path(&self, destination: &Path) -> PathBuf {
        let key = Sha256::digest(destination.as_os_str().as_encoded_bytes());
        self.folder.join(format!("{}.txt", encode_lower_hex(&key)))
    }

    pub(super) async fn read(&self, destination: &Path) -> Option<Note> {
        let text = fs::read_to_string(self.note_path(destination)).await.ok()?;
        Note::decode(&text).filter(|note| note.part == part_path(destination))
    }

    /// Best effort: without a note the bytes are only kept for this attempt.
    pub(super) async fn write(&self, destination: &Path, note: &Note) {
        let Some(text) = note.encode() else {
            return;
        };
        if fs::create_dir_all(&self.folder).await.is_ok() {
            let _ = fs::write(self.note_path(destination), text).await;
        }
    }

    pub(super) async fn forget(&self, destination: &Path) {
        let _ = fs::remove_file(self.note_path(destination)).await;
    }
}

/// Removes downloads kept part-way for longer than `age`, with their notes,
/// and notes whose bytes are gone. Returns how many downloads were removed.
pub fn sweep_partials(folder: &Path, age: Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return 0;
    };
    let now = SystemTime::now();
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "txt") {
            continue;
        }
        let note = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| Note::decode(&text));
        let stale = |file: &Path| {
            std::fs::metadata(file)
                .and_then(|metadata| metadata.modified())
                .map_or(true, |modified| {
                    now.duration_since(modified).unwrap_or_default() >= age
                })
        };
        match note {
            Some(note) if note.part.exists() => {
                if stale(&note.part) && std::fs::remove_file(&note.part).is_ok() {
                    removed += 1;
                    let _ = std::fs::remove_file(&path);
                }
            }
            _ => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    removed
}
