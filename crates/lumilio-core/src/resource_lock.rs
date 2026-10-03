//! Shared publication keys for cooperating domain workers in this process.
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

type Registry = BTreeMap<PathBuf, Weak<AsyncMutex<()>>>;
static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

async fn key(path: &Path) -> io::Result<PathBuf> {
    let mut ancestor = std::path::absolute(path)?;
    let mut suffix = Vec::new();
    loop {
        match tokio::fs::canonicalize(&ancestor).await {
            Ok(mut resolved) => {
                for name in suffix.into_iter().rev() {
                    resolved.push(name);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let name = ancestor
                    .file_name()
                    .ok_or_else(|| io::Error::other("resource has no existing ancestor"))?
                    .to_owned();
                suffix.push(name);
                ancestor.pop();
            }
            Err(error) => return Err(error),
        }
    }
}

pub(crate) async fn acquire(paths: &[PathBuf]) -> io::Result<Vec<OwnedMutexGuard<()>>> {
    let mut keys = Vec::new();
    for path in paths {
        keys.push(key(path).await?);
    }
    keys.sort();
    keys.dedup();
    let locks = {
        let mut registry = REGISTRY
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry.retain(|_, lock| lock.strong_count() > 0);
        keys.into_iter()
            .map(|key| {
                if let Some(lock) = registry.get(&key).and_then(Weak::upgrade) {
                    lock
                } else {
                    let lock = Arc::new(AsyncMutex::new(()));
                    registry.insert(key, Arc::downgrade(&lock));
                    lock
                }
            })
            .collect::<Vec<_>>()
    };
    let mut guards = Vec::new();
    for lock in locks {
        guards.push(lock.lock_owned().await);
    }
    Ok(guards)
}
