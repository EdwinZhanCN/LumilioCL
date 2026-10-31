//! What the content sources said about installed files, kept between visits
//! in its own SQLite file (plan content-recognition D2). A file's SHA-1 is
//! kept by its path, size and change time; what a source said is kept by
//! that SHA-1, which names the file's content.
//!
//! The expiries and the use of an expired answer follow Modrinth App's cache
//! (packages/app-lib/src/state/cache.rs, https://github.com/modrinth/code):
//! an expired answer is still shown while it is asked again, and stays when
//! asking fails.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::clock::now;
use crate::instances::Loader;

/// Which project version a file is: its content does not change.
pub(crate) const KNOWN_FOR: u64 = 30 * 24 * 60 * 60;
/// A source that did not know a file may know it once it is uploaded.
pub(crate) const UNKNOWN_FOR: u64 = 24 * 60 * 60;
/// Names, authors, icons and the newest versions change.
pub(crate) const DETAILS_FOR: u64 = 30 * 60;
/// An expired answer stays this long, for when no source can be asked.
const EXPIRED_KEPT: u64 = 7 * 24 * 60 * 60;
/// The hashes of a file not seen for this long are forgotten.
const UNSEEN_KEPT: u64 = 30 * 24 * 60 * 60;

/// How much of what the sources said before an answer may use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentLookup {
    /// Only what is kept, however old: no source is asked.
    Kept,
    /// What is kept and fresh; the rest is asked, and a source that cannot
    /// answer leaves the expired answer in place.
    Fresh,
    /// Every file is asked again (the person asked to identify again); a
    /// source that cannot answer leaves the kept answer in place.
    Again,
}

/// A kept answer, and whether it is still fresh.
pub(crate) struct Stored<T> {
    pub(crate) value: T,
    pub(crate) fresh: bool,
}

#[derive(Clone)]
pub(crate) struct RecognitionCache {
    path: PathBuf,
}

/// What one source said about one file.
pub(crate) fn identify_key(source: &str, sha1: &str) -> String {
    format!("identify\t{source}\t{sha1}")
}

/// The newest version a source has for one file and game.
pub(crate) fn latest_key(source: &str, sha1: &str, loader: Loader, game_version: &str) -> String {
    format!("latest\t{source}\t{sha1}\t{loader:?}\t{game_version}")
}

/// What one source says about one project.
pub(crate) fn project_key(source: &str, project: &str) -> String {
    format!("project\t{source}\t{project}")
}

fn changed_ns(path: &Path) -> Option<(i64, i64)> {
    let metadata = std::fs::metadata(path).ok()?;
    let changed = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((
        i64::try_from(metadata.len()).ok()?,
        i64::try_from(changed.as_nanos()).ok()?,
    ))
}

impl RecognitionCache {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn open(&self) -> rusqlite::Result<Connection> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| rusqlite::Error::InvalidPath(parent.to_owned()))?;
        }
        let db = Connection::open(&self.path)?;
        db.busy_timeout(Duration::from_secs(2))?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS hashes (path TEXT PRIMARY KEY, size INTEGER NOT NULL, \
             changed INTEGER NOT NULL, sha1 TEXT NOT NULL, seen INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS fingerprints (sha1 TEXT PRIMARY KEY, \
             murmur2 INTEGER NOT NULL, seen INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS answers (key TEXT PRIMARY KEY, value TEXT NOT NULL, \
             expires INTEGER NOT NULL);",
        )?;
        Ok(db)
    }

    /// Each file's SHA-1: the kept one while the file has the size and
    /// change time it was hashed at, else read again and kept. `None` for a
    /// file that cannot be read. Blocking.
    pub(crate) fn sha1s(&self, files: &[PathBuf], now: u64) -> Vec<Option<String>> {
        let db = self.open().ok();
        let mut found = Vec::with_capacity(files.len());
        let mut hashed = Vec::new();
        for path in files {
            let key = path.to_string_lossy();
            let stamp = changed_ns(path);
            let kept = match (&db, stamp) {
                (Some(db), Some((size, changed))) => db
                    .query_row(
                        "SELECT sha1 FROM hashes WHERE path=?1 AND size=?2 AND changed=?3",
                        params![key.as_ref(), size, changed],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .ok()
                    .flatten(),
                _ => None,
            };
            let sha1 = kept.or_else(|| crate::content::sha1_hex(path).ok());
            if let (Some(sha1), Some((size, changed))) = (&sha1, stamp) {
                hashed.push((key.into_owned(), size, changed, sha1.clone()));
            }
            found.push(sha1);
        }
        if let Some(mut db) = db {
            let _ = keep_hashes(&mut db, &hashed, now);
        }
        found
    }

    /// Each file's CurseForge fingerprint, kept by its SHA-1. Blocking.
    pub(crate) fn fingerprints(&self, files: &[(String, PathBuf)], now: u64) -> Vec<Option<u32>> {
        let db = self.open().ok();
        let mut found = Vec::with_capacity(files.len());
        let mut computed = Vec::new();
        for (sha1, path) in files {
            let kept = db.as_ref().and_then(|db| {
                db.query_row(
                    "SELECT murmur2 FROM fingerprints WHERE sha1=?1",
                    params![sha1],
                    |row| row.get::<_, u32>(0),
                )
                .optional()
                .ok()
                .flatten()
            });
            let murmur2 = kept.or_else(|| {
                let murmur2 = crate::content::fingerprint::of_file(path).ok()?;
                computed.push((sha1.clone(), murmur2));
                Some(murmur2)
            });
            found.push(murmur2);
        }
        if let Some(mut db) = db {
            let _ = keep_fingerprints(&mut db, files, &computed, now);
        }
        found
    }

    /// What is kept under each key that still decodes as `T`, expired or
    /// not; an entry that no longer decodes is a miss. Blocking.
    pub(crate) fn recall<T: DeserializeOwned>(
        &self,
        keys: &[String],
        now: u64,
    ) -> HashMap<String, Stored<T>> {
        let Ok(db) = self.open() else {
            return HashMap::new();
        };
        let Ok(mut query) = db.prepare("SELECT value, expires FROM answers WHERE key=?1") else {
            return HashMap::new();
        };
        let now = i64::try_from(now).unwrap_or(i64::MAX);
        keys.iter()
            .filter_map(|key| {
                let (value, expires) = query
                    .query_row(params![key], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                    })
                    .optional()
                    .ok()
                    .flatten()?;
                let value = serde_json::from_str(&value).ok()?;
                Some((
                    key.clone(),
                    Stored {
                        value,
                        fresh: expires > now,
                    },
                ))
            })
            .collect()
    }

    /// Keeps each answer for its own number of seconds, and forgets what
    /// expired long ago. Blocking.
    pub(crate) fn keep<T: Serialize>(&self, answers: &[(String, T, u64)], now: u64) {
        let Ok(mut db) = self.open() else {
            return;
        };
        let _ = keep_answers(&mut db, answers, now);
    }

    /// [`Self::recall`] off the async threads, now.
    pub(crate) async fn recall_now<T>(&self, keys: Vec<String>) -> HashMap<String, Stored<T>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let cache = self.clone();
        tokio::task::spawn_blocking(move || cache.recall(&keys, now()))
            .await
            .unwrap_or_default()
    }

    /// [`Self::keep`] off the async threads, now.
    pub(crate) async fn keep_now<T>(&self, answers: Vec<(String, T, u64)>)
    where
        T: Serialize + Send + 'static,
    {
        let cache = self.clone();
        let _ = tokio::task::spawn_blocking(move || cache.keep(&answers, now())).await;
    }
}

fn keep_hashes(
    db: &mut Connection,
    hashed: &[(String, i64, i64, String)],
    now: u64,
) -> rusqlite::Result<()> {
    let unseen = i64::try_from(now.saturating_sub(UNSEEN_KEPT)).unwrap_or(0);
    let now = i64::try_from(now).unwrap_or(i64::MAX);
    let transaction = db.transaction()?;
    for (path, size, changed, sha1) in hashed {
        transaction.execute(
            "INSERT INTO hashes (path, size, changed, sha1, seen) VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(path) DO UPDATE SET size=?2, changed=?3, sha1=?4, seen=?5",
            params![path, size, changed, sha1, now],
        )?;
    }
    transaction.execute("DELETE FROM hashes WHERE seen<?1", params![unseen])?;
    transaction.commit()
}

fn keep_fingerprints(
    db: &mut Connection,
    files: &[(String, PathBuf)],
    computed: &[(String, u32)],
    now: u64,
) -> rusqlite::Result<()> {
    let unseen = i64::try_from(now.saturating_sub(UNSEEN_KEPT)).unwrap_or(0);
    let now = i64::try_from(now).unwrap_or(i64::MAX);
    let transaction = db.transaction()?;
    for (sha1, murmur2) in computed {
        transaction.execute(
            "INSERT INTO fingerprints (sha1, murmur2, seen) VALUES (?1, ?2, ?3) \
             ON CONFLICT(sha1) DO UPDATE SET murmur2=?2, seen=?3",
            params![sha1, murmur2, now],
        )?;
    }
    for (sha1, _) in files {
        transaction.execute(
            "UPDATE fingerprints SET seen=?2 WHERE sha1=?1",
            params![sha1, now],
        )?;
    }
    transaction.execute("DELETE FROM fingerprints WHERE seen<?1", params![unseen])?;
    transaction.commit()
}

fn keep_answers<T: Serialize>(
    db: &mut Connection,
    answers: &[(String, T, u64)],
    now: u64,
) -> rusqlite::Result<()> {
    let transaction = db.transaction()?;
    for (key, value, fresh_for) in answers {
        let Ok(value) = serde_json::to_string(value) else {
            continue;
        };
        let expires = i64::try_from(now.saturating_add(*fresh_for)).unwrap_or(i64::MAX);
        transaction.execute(
            "INSERT INTO answers (key, value, expires) VALUES (?1, ?2, ?3) \
             ON CONFLICT(key) DO UPDATE SET value=?2, expires=?3",
            params![key, value, expires],
        )?;
    }
    let forgotten = i64::try_from(now.saturating_sub(EXPIRED_KEPT)).unwrap_or(0);
    transaction.execute("DELETE FROM answers WHERE expires<?1", params![forgotten])?;
    transaction.commit()
}

#[cfg(test)]
mod tests;
