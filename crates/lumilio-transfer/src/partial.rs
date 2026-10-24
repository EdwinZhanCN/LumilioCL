//! Downloads left part-way. The bytes stay beside the destination, so the
//! finished file is still published by a rename; a note in one folder says
//! where they came from, so a later attempt continues instead of starting
//! over, and so leftovers can be found and removed.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};
use tokio::fs;

use super::files::encode_lower_hex;
use super::request::ContentDigest;

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
    /// `None`: the part file is a prefix of the content. `Some`: it has the
    /// content's full length and only some ranges are written, as left by a
    /// download over several connections.
    pub(super) written: Option<Written>,
}

/// The written ranges of a part file as long as the whole content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Written {
    pub(super) length: u64,
    pub(super) ranges: Vec<Range<u64>>,
}

impl Written {
    fn encode(&self) -> String {
        self.ranges
            .iter()
            .map(|range| format!("{}-{}", range.start, range.end))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Sorted ranges inside `0..length` that neither overlap nor touch, or
    /// `None`.
    fn decode(length: u64, text: &str) -> Option<Self> {
        let mut ranges: Vec<Range<u64>> = Vec::new();
        for range in text.split(',').filter(|range| !range.is_empty()) {
            let (start, end) = range.split_once('-')?;
            let (start, end) = (start.parse().ok()?, end.parse().ok()?);
            let after_previous = ranges.last().is_none_or(|last| last.end < start);
            if start >= end || end > length || !after_previous {
                return None;
            }
            ranges.push(start..end);
        }
        Some(Self { length, ranges })
    }
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
        let mut fields = vec![
            ("part", self.part.to_str()?.to_owned()),
            ("source", self.source.clone()),
        ];
        if let Some(validator) = &self.validator {
            fields.push(("validator", validator.clone()));
        }
        match &self.digest {
            Some(ContentDigest::Sha1(hex)) => fields.push(("sha1", hex.clone())),
            Some(ContentDigest::Sha256(hex)) => fields.push(("sha256", hex.clone())),
            None => {}
        }
        if let Some(written) = &self.written {
            fields.push(("length", written.length.to_string()));
            fields.push(("written", written.encode()));
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
        let mut length = None;
        let mut written = None;
        for line in text.lines() {
            let (key, value) = line.split_once('\t')?;
            match key {
                "part" => part = Some(PathBuf::from(value)),
                "source" => source = Some(value.to_owned()),
                "validator" => validator = Some(value.to_owned()),
                "sha1" => digest = ContentDigest::parse(value.to_owned(), false).ok(),
                "sha256" => digest = ContentDigest::parse(value.to_owned(), true).ok(),
                "length" => length = Some(value.parse().ok()?),
                "written" => written = Some(value.to_owned()),
                _ => {}
            }
        }
        let written = match (length, written) {
            (Some(length), Some(written)) => Some(Written::decode(length, &written)?),
            (None, None) => None,
            _ => return None,
        };
        Some(Self {
            part: part?,
            source: source?,
            validator,
            digest,
            written,
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
