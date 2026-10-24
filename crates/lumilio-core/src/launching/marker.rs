use std::path::{Path, PathBuf};
use std::{fs, io};

use serde::{Deserialize, Serialize};

use crate::history::{SessionOutcome, record_attempt};
use crate::instances::InstanceStore;
use crate::recovery::RecoveryNote;
use crate::storage::Layout;

const MARKER_SCHEMA: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
struct MarkerFile {
    schema: u32,
    instance_id: String,
    started: u64,
}

/// Proof, on disk, that a launch was in progress. It exists from the start of
/// preparation until the launch returns or its future is dropped, so a marker
/// found at start-up means the launcher itself died in between.
pub(crate) struct SessionMarker {
    path: PathBuf,
}

impl SessionMarker {
    pub(crate) fn place(layout: &Layout, instance_id: &str, started: u64) -> io::Result<Self> {
        // Ids come from the library and are plain; refuse anything that could
        // name a file outside the sessions folder.
        if instance_id.is_empty()
            || !instance_id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "instance id is not a plain name",
            ));
        }
        fs::create_dir_all(layout.sessions())?;
        let path = layout.sessions().join(format!("{instance_id}.json"));
        let bytes = serde_json::to_vec(&MarkerFile {
            schema: MARKER_SCHEMA,
            instance_id: instance_id.to_owned(),
            started,
        })
        .map_err(io::Error::other)?;
        let mut temporary = path.as_os_str().to_owned();
        temporary.push(".tmp");
        fs::write(&temporary, bytes)?;
        fs::rename(&temporary, &path)?;
        Ok(Self { path })
    }
}

impl Drop for SessionMarker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Turns leftover markers into a history record and a note. The game is never
/// signalled or re-attached: a stored process id cannot prove it is still the
/// same process.
pub(crate) fn settle_sessions(layout: &Layout, store: &InstanceStore) -> Vec<RecoveryNote> {
    let Ok(entries) = fs::read_dir(layout.sessions()) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|path| settle_session(layout, store, path))
        .collect()
}

fn settle_session(layout: &Layout, store: &InstanceStore, path: &Path) -> Option<RecoveryNote> {
    let marker = fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<MarkerFile>(&bytes).ok());
    let Some(marker) = marker.filter(|marker| marker.schema <= MARKER_SCHEMA) else {
        // Unreadable or from a newer launcher: leave it, say so.
        return Some(RecoveryNote::JournalUnusable {
            path: path.to_path_buf(),
            reason: "session marker could not be read".to_owned(),
        });
    };
    if store.get(&marker.instance_id).is_some()
        && record_attempt(
            layout.root(),
            &marker.instance_id,
            marker.started,
            SessionOutcome::Interrupted,
        )
        .is_err()
    {
        // Not recorded yet: keep the marker so the next start tries again.
        return Some(RecoveryNote::SessionInterrupted {
            instance_id: marker.instance_id,
            started: marker.started,
        });
    }
    let _ = fs::remove_file(path);
    Some(RecoveryNote::SessionInterrupted {
        instance_id: marker.instance_id,
        started: marker.started,
    })
}
