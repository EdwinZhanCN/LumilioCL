//! Data-only world-map contracts. The host owns rendering and interaction.
use crate::{HostContext, ImageData, PluginError};
use serde::{Deserialize, Serialize};

pub const TILE_PIXELS: u32 = 256;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum WorldId {
    Save { instance: String, folder: String },
    Server { instance: String, address: String },
    Seed { seed: i64, version: String },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Dimension {
    Overworld,
    Nether,
    End,
    Custom(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SourceLink {
    Save(String),
    XaeroMinimap(String),
    XaeroWorldMap(String),
    Seed(SeedSource),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SeedSource {
    LevelDat,
    WorldGenSettings,
    Manual,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorldContext {
    pub world: WorldId,
    pub version: Option<String>,
    pub data_version: Option<i32>,
    pub seed: Option<i64>,
    pub dimension: Dimension,
    pub sources: Vec<SourceLink>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TileKey {
    pub provider: String,
    pub base_map: String,
    pub world: WorldId,
    pub dimension: Dimension,
    pub level: u8,
    pub tx: i32,
    pub tz: i32,
}

impl TileKey {
    pub fn blocks_per_pixel(&self) -> Option<i32> {
        [1, 4, 16, 64, 256].get(usize::from(self.level)).copied()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TileRequest {
    pub context: WorldContext,
    pub key: TileKey,
    pub pixels: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TileReply {
    Image(ImageData),
    /// One byte per pixel, zero for missing data and 255 for complete data.
    Partial {
        image: ImageData,
        coverage: Vec<u8>,
    },
    Empty,
}

impl TileReply {
    pub fn is_valid(&self) -> bool {
        let valid = |image: &ImageData| {
            image.width == TILE_PIXELS && image.height == TILE_PIXELS && image.is_valid()
        };
        match self {
            Self::Empty => true,
            Self::Image(image) => valid(image),
            Self::Partial { image, coverage } => {
                valid(image) && coverage.len() == (TILE_PIXELS * TILE_PIXELS) as usize
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MapPoint {
    pub x: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct MapBounds {
    pub min: MapPoint,
    pub max: MapPoint,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OverlayRequest {
    pub context: WorldContext,
    pub overlay: String,
    pub bounds: MapBounds,
    pub level: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MapIcon {
    Village,
    Temple,
    Stronghold,
    Monument,
    Mansion,
    Fortress,
    Bastion,
    EndCity,
    Spawn,
    Waypoint,
    Marker,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum MapObjectKind {
    Icon {
        icon: MapIcon,
        at: MapPoint,
    },
    Point(MapPoint),
    Polyline(Vec<MapPoint>),
    Area(Vec<MapPoint>),
    Text {
        at: MapPoint,
        text: String,
    },
    Heat {
        cell: f64,
        values: Vec<(MapPoint, f32)>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MapObject {
    pub id: String,
    pub raw_id: String,
    pub source: String,
    pub world: WorldId,
    pub dimension: Dimension,
    pub kind: MapObjectKind,
    pub label: Option<String>,
    pub label_id: Option<String>,
    pub priority: i32,
    pub approximate: bool,
    pub color: Option<[u8; 3]>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BaseMapInfo {
    pub id: String,
    pub kind_id: String,
    pub dimensions: Vec<Dimension>,
    pub levels: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OverlayInfo {
    pub id: String,
    pub kind_id: String,
    pub dimensions: Vec<Dimension>,
}

pub trait BaseMapProvider: Send + Sync {
    fn base_maps(&self) -> Vec<BaseMapInfo>;
    fn tile(&self, ctx: &dyn HostContext, request: &TileRequest) -> Result<TileReply, PluginError>;
}

pub trait OverlayProvider: Send + Sync {
    fn overlays(&self) -> Vec<OverlayInfo>;
    fn objects(
        &self,
        ctx: &dyn HostContext,
        request: &OverlayRequest,
    ) -> Result<Vec<MapObject>, PluginError>;
}

#[cfg(test)]
mod tests;
