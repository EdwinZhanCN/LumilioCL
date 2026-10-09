//! Validated Mojang textures are reused by normalized URL for a bounded time
//! on disk and in memory (ADR 0039). Concurrent reads share one download.
//! No account credentials enter.
use super::{AppearanceError, PICTURE_LIMIT, mojang::texture_url};
use crate::transfer::Transport;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
use tokio::{sync::Mutex, time::Instant};

#[derive(Default)]
struct Entry {
    bytes: Option<(Instant, Vec<u8>)>,
    failure: Option<(Instant, AppearanceError)>,
}

pub(crate) struct TextureCache {
    root: PathBuf,
    entries: Mutex<HashMap<String, Arc<Mutex<Entry>>>>,
}

impl TextureCache {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            entries: Mutex::default(),
        }
    }

    pub(super) async fn get<T: Transport + ?Sized>(
        &self,
        transport: &T,
        address: &str,
    ) -> Result<Vec<u8>, AppearanceError> {
        let url = texture_url(address)?;
        let key = Sha256::digest(url.as_str().as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let slot = {
            let mut entries = self.entries.lock().await;
            if entries.len() >= 128 {
                entries.retain(|_, entry| Arc::strong_count(entry) > 1);
            }
            entries.entry(key.clone()).or_default().clone()
        };
        let mut entry = slot.lock().await;
        if let Some((stored, bytes)) = &entry.bytes
            && stored.elapsed() < Duration::from_secs(60 * 60)
        {
            return Ok(bytes.clone());
        }
        entry.bytes = None;
        if let Some((until, error)) = &entry.failure
            && Instant::now() < *until
        {
            return Err(error.clone());
        }
        let path = self.root.join(format!("{key}.png"));
        let disk = path.clone();
        let cached = tokio::task::spawn_blocking(move || {
            let metadata = std::fs::metadata(&disk).ok()?;
            if metadata.len() > PICTURE_LIMIT
                || metadata.modified().ok()?.elapsed().ok()? > Duration::from_secs(24 * 60 * 60)
            {
                return None;
            }
            let bytes = std::fs::read(disk).ok()?;
            valid_png(&bytes).then_some(bytes)
        })
        .await
        .ok()
        .flatten();
        if let Some(bytes) = cached {
            entry.bytes = Some((Instant::now(), bytes.clone()));
            return Ok(bytes);
        }
        let result = super::MojangClient::new(transport)
            .texture_uncached(url.as_str())
            .await;
        match result {
            Ok(bytes) => {
                let valid = tokio::task::spawn_blocking({
                    let bytes = bytes.clone();
                    move || valid_png(&bytes)
                })
                .await
                .unwrap_or(false);
                if !valid {
                    let error = AppearanceError::Picture("invalid texture PNG".into());
                    entry.failure = Some((Instant::now() + error.retry_delay(), error.clone()));
                    return Err(error);
                }
                let saved = bytes.clone();
                // Cache writes are best effort: read-only/full disks don't hide a skin.
                let _ = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                    std::fs::create_dir_all(path.parent().expect("texture directory"))?;
                    let mut random = [0u8; 8];
                    getrandom::fill(&mut random)
                        .map_err(|error| std::io::Error::other(error.to_string()))?;
                    let temporary =
                        path.with_extension(format!("{}.tmp", u64::from_ne_bytes(random)));
                    let result = (|| {
                        std::fs::write(&temporary, saved)?;
                        std::fs::rename(&temporary, &path)?;
                        prune_disk(path.parent().expect("texture directory"));
                        Ok(())
                    })();
                    if result.is_err() {
                        let _ = std::fs::remove_file(temporary);
                    }
                    result
                })
                .await;
                entry.failure = None;
                entry.bytes = Some((Instant::now(), bytes.clone()));
                Ok(bytes)
            }
            Err(error) => {
                entry.failure = Some((Instant::now() + error.retry_delay(), error.clone()));
                Err(error)
            }
        }
    }
}

fn valid_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        && (super::skin_pixels(bytes).is_ok() || super::cape_pixels(bytes).is_ok())
}

fn prune_disk(root: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "tmp") {
                let stale = entry.metadata().ok()?.modified().ok()?.elapsed().ok()?
                    > Duration::from_secs(60 * 60);
                if stale {
                    let _ = std::fs::remove_file(path);
                }
                return None;
            }
            if path.extension().is_none_or(|ext| ext != "png") {
                return None;
            }
            Some((entry.metadata().ok()?.modified().ok()?, path))
        })
        .collect();
    if files.len() <= 128 {
        return;
    }
    files.sort_unstable_by_key(|(modified, _)| *modified);
    let excess = files.len() - 128;
    for (_, path) in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
}

impl AppearanceError {
    pub(crate) fn retry_delay(&self) -> Duration {
        Duration::from_secs(match self {
            Self::RateLimitedFor(seconds) => (*seconds).clamp(1, 24 * 60 * 60),
            Self::RateLimited | Self::SignInRequired => 60,
            _ => 10,
        })
    }
}

#[cfg(test)]
mod tests;
