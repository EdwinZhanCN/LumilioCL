//! Remote Minecraft metadata: the parts of `lumilio-minecraft` that need the
//! network.

pub use catalog::fetch_catalog;
pub(crate) use versions::{game_versions, loader_versions};

mod catalog;
pub(crate) mod forge_install;
pub(crate) mod forge_meta;
pub(crate) mod install;
pub(crate) mod loader;
pub(crate) mod repair;
mod versions;
