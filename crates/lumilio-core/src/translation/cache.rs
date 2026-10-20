use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(super) const TTL: u64 = 7 * 24 * 60 * 60;
const MAX_ENTRIES: i64 = 10_000;

#[derive(Clone)]
pub(super) struct Cache {
    path: PathBuf,
    generation: Arc<Mutex<u64>>,
}

impl Cache {
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            path,
            generation: Arc::new(Mutex::new(0)),
        }
    }

    fn open(&self) -> rusqlite::Result<Connection> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| rusqlite::Error::InvalidPath(parent.to_owned()))?;
        }
        let db = Connection::open(&self.path)?;
        db.busy_timeout(std::time::Duration::from_secs(2))?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS translations (key TEXT PRIMARY KEY, value TEXT NOT NULL, expires INTEGER NOT NULL, written INTEGER NOT NULL);")?;
        Ok(db)
    }

    pub(super) fn generation(&self) -> u64 {
        *self
            .generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn invalidate(&self) {
        let mut generation = self
            .generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *generation = generation.wrapping_add(1);
    }

    pub(super) async fn get(&self, key: String, now: u64) -> Option<String> {
        let now = i64::try_from(now).ok()?;
        let cache = self.clone();
        tokio::task::spawn_blocking(move || {
            cache
                .open()?
                .query_row(
                    "SELECT value FROM translations WHERE key=?1 AND expires>?2 AND written<=?2",
                    params![key, now],
                    |row| row.get(0),
                )
                .optional()
        })
        .await
        .ok()?
        .ok()?
    }

    pub(super) async fn put(&self, key: String, value: String, now: u64, generation: u64) {
        self.put_for(key, value, now, generation, TTL).await;
    }

    pub(super) async fn put_for(
        &self,
        key: String,
        value: String,
        now: u64,
        generation: u64,
        ttl: u64,
    ) {
        let Ok(expires) = i64::try_from(now.saturating_add(ttl)) else {
            return;
        };
        let Ok(now) = i64::try_from(now) else { return };
        let cache = self.clone();
        let _ = tokio::task::spawn_blocking(move || -> rusqlite::Result<()> {
            let guard = cache.generation.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if *guard != generation { return Ok(()); }
            let mut db = cache.open()?;
            let tx = db.transaction()?;
            tx.execute("DELETE FROM translations WHERE expires<=?1", [now])?;
            tx.execute("INSERT OR REPLACE INTO translations VALUES (?1,?2,?3,?4)", params![key, value, expires, now])?;
            tx.execute("DELETE FROM translations WHERE key IN (SELECT key FROM translations ORDER BY written DESC, key DESC LIMIT -1 OFFSET ?1)", [MAX_ENTRIES])?;
            tx.execute("DELETE FROM translations WHERE key IN (SELECT key FROM (SELECT key, SUM(length(CAST(value AS BLOB))) OVER (ORDER BY written DESC, key DESC) AS bytes FROM translations) WHERE bytes > 67108864)", [])?;
            tx.commit()
        }).await;
    }

    pub(super) async fn clear(&self) -> Result<(), super::TranslationError> {
        let cache = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = cache
                .generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *guard = guard.wrapping_add(1);
            cache.open()?.execute("DELETE FROM translations", [])?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|_| super::TranslationError::Storage)?
        .map_err(|_| super::TranslationError::Storage)
    }
}
