//! Host-owned map viewport. Background data and rendering enter through mailboxes.
mod annotations;
mod camera;
mod canvas;
mod edit;
mod layers;
mod notices;
mod objects;
mod seed;
mod select;
mod stats;
mod toolbar;
mod window;
mod xaero;

#[cfg(test)]
use std::rc::Rc;
#[cfg(test)]
use std::sync::Arc;

use gpui::canvas;
#[cfg(test)]
use gpui::prelude::*;
#[cfg(test)]
use gpui::{Context, MouseButton, Window, px};
#[cfg(test)]
use lumilio_core::world_map::WorldMapContext;
#[cfg(test)]
use lumilio_core::{MapFailure, MapProviders};
#[cfg(test)]
use lumilio_map_render::Grid;
#[cfg(test)]
use lumilio_plugin_api::map::{
    MapObject, MapObjectKind, OverlayInfo, TileKey, TileReply, WorldContext,
};
#[cfg(test)]
use map_view::prepare;
use map_view::{Backend, ICON, IN_FLIGHT, Loaded, MAX_FILLS, MAX_ICONS, STAND_INS, TILE_CAP};
pub use map_view::{Command, Connection, Event, Load, MapView};
pub use objects::ObjectKey;
pub use seed::VERSIONS as MANUAL_VERSIONS;
pub use window::{MapHandoff, MapWindow, PopOut};

#[cfg(test)]
use crate::tr;

#[cfg(test)]
mod perf;
#[cfg(test)]
mod tests;
mod worker;

mod render;

mod tiles;

mod map_view;
