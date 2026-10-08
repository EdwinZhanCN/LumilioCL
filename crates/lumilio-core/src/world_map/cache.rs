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
        let path = self.path(request)?;
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        if !meta.is_file() || meta.len() > MAX_ENTRY {
            return Ok(None);
        }
        let reply: TileReply = match serde_json::from_slice(&fs::read(&path)?) {
            Ok(reply) => reply,
            Err(_) => return Ok(None),
        };
        if !reply.is_valid() {
            return Ok(None);
        }
        fs::File::open(&path)?.set_modified(SystemTime::now())?;
        Ok(Some(reply))
    }
    pub fn put(&self, request: &TileRequest, reply: &TileReply) -> io::Result<()> {
        if !reply.is_valid() {
            return Err(io_error("invalid cache tile"));
        }
        fs::create_dir_all(&self.root)?;
        let bytes = serde_json::to_vec(reply).map_err(io_error)?;
        if bytes.len() as u64 > MAX_ENTRY {
            return Err(io_error("cache tile too large"));
        }
        let staging = self.root.join(format!(
            ".{}-{}.part",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staging)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&staging, self.path(request)?)?;
            self.evict()
        })();
        if result.is_err() {
            let _ = fs::remove_file(staging);
        }
        result
    }
    fn evict(&self) -> io::Result<()> {
        let mut entries = Vec::new();
        let mut size = 0;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.path().extension().is_none_or(|ext| ext != "tile")
                || !entry.file_type()?.is_file()
            {
                continue;
            }
            let meta = entry.metadata()?;
            size += meta.len();
            entries.push((meta.modified()?, entry.path(), meta.len()));
        }
        entries.sort();
        for (_, path, len) in entries {
            if size <= self.limit {
                break;
            }
            fs::remove_file(path)?;
            size -= len;
        }
        Ok(())
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
