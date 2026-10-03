//! Cooperative single-writer ownership of one launcher data root.
use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

use crate::service::ServiceError;

pub(crate) struct RootLock {
    // Never unlink a held lock: a replacement inode would admit another writer.
    _file: File,
}

impl RootLock {
    pub(crate) fn acquire(root: &Path) -> Result<Self, ServiceError> {
        let directory = root.join("locks");
        std::fs::create_dir_all(&directory)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("writer.lock"))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(ServiceError::RootBusy(root.to_path_buf())),
            Err(TryLockError::Error(error)) => Err(ServiceError::Io(error)),
        }
    }
}

impl Drop for RootLock {
    fn drop(&mut self) {
        // Release ownership explicitly: a concurrently spawned child can briefly
        // inherit the open-file description before close-on-exec takes effect.
        let _ = self._file.unlock();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn dropping_ownership_unlocks_even_if_an_open_file_description_is_retained() {
        let dir = tempfile::tempdir().unwrap();
        let owner = RootLock::acquire(dir.path()).unwrap();
        let retained = owner._file.try_clone().unwrap();
        drop(owner);
        let replacement = RootLock::acquire(dir.path()).unwrap();
        drop(retained);
        assert!(matches!(
            RootLock::acquire(dir.path()),
            Err(ServiceError::RootBusy(_))
        ));
        drop(replacement);
        RootLock::acquire(dir.path()).unwrap();
    }
}
