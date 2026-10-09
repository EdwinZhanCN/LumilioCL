use super::context::io_error;
use lumilio_plugin_api::map::{TileReply, TileRequest};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

const MAX_ENTRY: u64 = 2 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct TileCache {
    root: PathBuf,
    limit: u64,
}
impl TileCache {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            limit: 1024 * 1024 * 1024,
        }
    }
    pub fn with_limit(root: PathBuf, limit: u64) -> Self {
        Self { root, limit }
    }
    fn path(&self, request: &TileRequest) -> io::Result<PathBuf> {
        // Save identities intentionally do not fragment the shared seed cache.
        let identity = (
            &request.key.provider,
            &request.key.base_map,
            request.context.seed,
            &request.context.version,
            request.context.data_version,
            &request.key.dimension,
            request.key.level,
            request.key.tx,
            request.key.tz,
            request.pixels,
            "cubiomes-e61f905-palette-1-y64-default",
        );
        let bytes = serde_json::to_vec(&identity).map_err(io_error)?;
        let digest: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Ok(self.root.join(format!("{digest}.tile")))
    }
    pub fn get(&self, request: &TileRequest) -> io::Result<Option<TileReply>> {
        let Some(bytes) = read_entry(&self.path(request)?, MAX_ENTRY)? else {
            return Ok(None);
        };
        Ok(serde_json::from_slice::<TileReply>(&bytes)
            .ok()
            .filter(TileReply::is_valid))
    }
    pub fn put(&self, request: &TileRequest, reply: &TileReply) -> io::Result<()> {
        if !reply.is_valid() {
            return Err(io_error("invalid cache tile"));
        }
        let bytes = serde_json::to_vec(reply).map_err(io_error)?;
        write_entry(
            &self.root,
            self.limit,
            &self.path(request)?,
            &bytes,
            MAX_ENTRY,
        )
    }
    pub fn clear(root: &Path) -> io::Result<()> {
        match fs::symlink_metadata(root) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
            Ok(meta) if meta.file_type().is_symlink() => Err(io_error("linked map cache")),
            Ok(_) => fs::remove_dir_all(root),
        }
    }
}

/// The bytes of a cache entry no larger than `max`, marking it used; `None`
/// when there is none.
pub(super) fn read_entry(path: &Path, max: u64) -> io::Result<Option<Vec<u8>>> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if !meta.is_file() || meta.len() > max {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    fs::File::open(path)?.set_modified(SystemTime::now())?;
    Ok(Some(bytes))
}

/// Writes an entry whole (a staging file renamed into place), then drops the
/// least recently used entries until `root` is within `limit`.
pub(super) fn write_entry(
    root: &Path,
    limit: u64,
    path: &Path,
    bytes: &[u8],
    max: u64,
) -> io::Result<()> {
    if bytes.len() as u64 > max {
        return Err(io_error("cache tile too large"));
    }
    fs::create_dir_all(root)?;
    let staging = root.join(format!(
        ".{}-{}.part",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&staging, path)?;
        evict(root, limit)
    })();
    if result.is_err() {
        let _ = fs::remove_file(staging);
    }
    result
}

fn evict(root: &Path, limit: u64) -> io::Result<()> {
    let mut entries = Vec::new();
    let mut size = 0;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.path().extension().is_none_or(|ext| ext != "tile") || !entry.file_type()?.is_file()
        {
            continue;
        }
        let meta = entry.metadata()?;
        size += meta.len();
        entries.push((meta.modified()?, entry.path(), meta.len()));
    }
    entries.sort();
    for (_, path, len) in entries {
        if size <= limit {
            break;
        }
        fs::remove_file(path)?;
        size -= len;
    }
    Ok(())
}
