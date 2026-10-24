//! The block assets a schematic is drawn with: which sources to use for an
//! instance (plan schematic-studio, D2, D3 and D23), where they are cached,
//! and loading them into one [`BakedPack`](crate::BakedPack).
//!
//! Only `lumilio-block-assets` reads Mojang's resource layout. Vanilla block
//! assets and entity geometry always come from the pre-baked bundle, which
//! [`BundleStore`] downloads and checks; an installed game is never baked
//! here (D23). This module picks sources by rule ([`resolve`]), lays the
//! instance's mods and resource packs that the studio folder switched on over
//! the bundle pack ([`load()`]), keeps those overlay bakes under
//! `Layout::model_cache()` ([`LocalCache`]) and says what every source holds,
//! on or off ([`provided()`]). The service turns a bundle download into an
//! Activity task.

mod added;
mod bundle;
mod cache;
mod data_version;
mod entities;
mod index;
mod instance;
mod load;
mod names;
mod order;
mod provided;
mod rules;
mod scan;
mod switches;

pub(crate) use added::{add as add_pack, read_added, remove as remove_pack};
pub use asset_plan::{
    AssetNotice, AssetPlan, BundleReach, InstalledClient, InstanceAssets, PlannedSource,
    SchematicAssetsError,
};
pub use bundle::{BUNDLE_RELEASE, BUNDLE_URL, BundleStore};
pub(crate) use cache::look;
pub use cache::{Baked, LocalCache};
pub use data_version::data_version;
pub use entities::{EntityGeometry, run_dumper};
pub use index::{BundleFile, BundleIndex};
pub(crate) use instance::instance_assets;
pub use load::load;
pub use names::installed_names;
pub(crate) use order::{read_order, swapped as swapped_sources, write_order};
pub use provided::{ProvidedSource, SourceRole, provided};
pub use rules::resolve;
pub use scan::Scan;
pub(crate) use switches::{forget as forget_switch, switch, switched_on};

#[cfg(test)]
mod tests;

mod asset_plan;
