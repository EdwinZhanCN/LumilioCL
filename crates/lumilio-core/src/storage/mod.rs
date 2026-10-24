//! Where and how the launcher keeps its files: the folder layout, the lock
//! that makes one launcher own a root, atomic JSON files and `launcher.db`.

pub(crate) use database::SCHEMA_VERSION;
pub use layout::Layout;
pub(crate) use root_lock::RootLock;

pub(crate) mod database;
mod layout;
pub(crate) mod persist;
mod root_lock;
