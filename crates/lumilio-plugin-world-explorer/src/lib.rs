//! Local seed-map provider. The host owns the world picker and map viewport.

#[cfg(test)]
use lumilio_plugin_api::map::{
    BaseMapProvider, Dimension, OverlayProvider, OverlayRequest, TileRequest,
};
#[cfg(test)]
use lumilio_plugin_api::{HostContext, Plugin, PluginError};
pub use world_explorer::{ID, SUPPORTED_VERSIONS, WorldExplorer};

mod landmarks;
mod save;
mod structures;
mod text;
mod underground;
mod xaero;

#[cfg(test)]
mod tests;

mod world_explorer;
