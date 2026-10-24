//! Remote Minecraft metadata: the parts of `lumilio-minecraft` that need the
//! network.

pub use catalog::fetch_catalog;
pub(crate) use versions::{game_versions, loader_versions};

mod catalog;
mod versions;
