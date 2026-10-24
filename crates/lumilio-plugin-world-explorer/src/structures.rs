//! Structure and decoration layers from the cubiomes fork, one overlay per
//! kind so each has its own switch. Positions are exact for the seed;
//! viability is the fork's biome check, an estimate where terrain decides.

use std::sync::Mutex;

use lumilio_cubiomes::{Bastion, Decoration, Dimension as Cube, Structure, Variant, Version};
use lumilio_plugin_api::map::{
    Dimension, MapBounds, MapIcon, MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayRequest,
    WorldContext,
};
use lumilio_plugin_api::{HostContext, PluginError};

pub(crate) const GROUP: &str = "map-group-structures";
/// Features the decoration random places in a chunk: fossils, lava lakes.
pub(crate) const FEATURE_GROUP: &str = "map-group-features";
const SOURCE: &str = crate::ID;

/// What a layer shows.
#[derive(Clone, Copy)]
enum Kind {
    Structure(Structure),
    /// Not a region grid: a world's 128, computed once.
    Strongholds,
    Decoration(Decoration),
    /// Desert pyramids whose chests hold an enchanted golden apple.
    PyramidApples,
}

/// One switchable layer. Structures are named `structure.<name>` with the
/// catalog label `map-structure-<name>`, features (decorations and chest
/// loot) `feature.<name>` and `map-feature-<name>`.
struct Layer {
    name: &'static str,
    kind: Kind,
    icon: MapIcon,
}

const LAYERS: [Layer; 29] = [
    structure("village", Structure::Village, MapIcon::Village),
    structure(
        "desert-pyramid",
        Structure::DesertPyramid,
        MapIcon::DesertPyramid,
    ),
    structure(
        "jungle-temple",
        Structure::JungleTemple,
        MapIcon::JungleTemple,
    ),
    structure("swamp-hut", Structure::SwampHut, MapIcon::SwampHut),
    structure("igloo", Structure::Igloo, MapIcon::Igloo),
    structure("outpost", Structure::Outpost, MapIcon::Outpost),
    structure("monument", Structure::Monument, MapIcon::Monument),
    structure("mansion", Structure::Mansion, MapIcon::Mansion),
    structure("ocean-ruin", Structure::OceanRuin, MapIcon::OceanRuin),
    structure("shipwreck", Structure::Shipwreck, MapIcon::Shipwreck),
    structure(
        "ruined-portal",
        Structure::RuinedPortal,
        MapIcon::RuinedPortal,
    ),
    structure("ancient-city", Structure::AncientCity, MapIcon::AncientCity),
    structure("trail-ruins", Structure::TrailRuins, MapIcon::TrailRuins),
    structure(
        "trial-chambers",
        Structure::TrialChambers,
        MapIcon::TrialChambers,
    ),
    Layer {
        name: "stronghold",
        kind: Kind::Strongholds,
        icon: MapIcon::Stronghold,
    },
    structure("fortress", Structure::Fortress, MapIcon::Fortress),
    structure("bastion", Structure::Bastion, MapIcon::Bastion),
    structure(
        "nether-fossil",
        Structure::NetherFossil,
        MapIcon::NetherFossil,
    ),
    structure("end-city", Structure::EndCity, MapIcon::EndCity),
    structure(
        "abandoned-camp",
        Structure::AbandonedCamp,
        MapIcon::AbandonedCamp,
    ),
    structure(
        "buried-treasure",
        Structure::Treasure,
        MapIcon::BuriedTreasure,
    ),
    structure("mineshaft", Structure::Mineshaft, MapIcon::Mineshaft),
    structure("desert-well", Structure::DesertWell, MapIcon::DesertWell),
    structure("geode", Structure::Geode, MapIcon::Geode),
    structure("end-gateway", Structure::EndGateway, MapIcon::EndGateway),
    decoration("fossil", Decoration::Fossil, MapIcon::Fossil),
    decoration(
        "fossil-diamond",
        Decoration::DiamondFossil,
        MapIcon::DiamondFossil,
    ),
    decoration("lava-lake", Decoration::LavaLake, MapIcon::LavaLake),
    Layer {
        name: "enchanted-golden-apple",
        kind: Kind::PyramidApples,
        icon: MapIcon::EnchantedGoldenApple,
    },
];

const fn structure(name: &'static str, kind: Structure, icon: MapIcon) -> Layer {
    Layer {
        name,
        kind: Kind::Structure(kind),
        icon,
    }
}

const fn decoration(name: &'static str, kind: Decoration, icon: MapIcon) -> Layer {
    Layer {
        name,
        kind: Kind::Decoration(kind),
        icon,
    }
}

impl Layer {
    fn decoration(&self) -> bool {
        matches!(self.kind, Kind::Decoration(_) | Kind::PyramidApples)
    }
    fn id(&self) -> String {
        let prefix = if self.decoration() {
            "feature"
        } else {
            "structure"
        };
        format!("{prefix}.{}", self.name)
    }
    fn label_id(&self) -> String {
        let prefix = if self.decoration() {
            "map-feature"
        } else {
            "map-structure"
        };
        format!("{prefix}-{}", self.name)
    }
    fn group(&self) -> &'static str {
        if self.decoration() {
            FEATURE_GROUP
        } else {
            GROUP
        }
    }
    fn structure(&self) -> Option<Structure> {
        match self.kind {
            Kind::Structure(kind) => Some(kind),
            _ => None,
        }
    }
    /// Whether this version's positions are estimates, not certainties.
    fn estimated(&self, version: Version) -> bool {
        match self.kind {
            Kind::Structure(kind) => kind.estimated(version),
            Kind::Strongholds => false,
            // Each passes a terrain check (heights, empty corners) the fork
            // does not evaluate.
            Kind::Decoration(_) => true,
            // The loot is exact; whether the pyramid stands is estimated.
            Kind::PyramidApples => Structure::DesertPyramid.estimated(version),
        }
    }
    /// The layer's icon in a dimension: Nether ruined portals have their own.
    fn icon_in(&self, dimension: Cube) -> MapIcon {
        match (self.icon, dimension) {
            (MapIcon::RuinedPortal, Cube::Nether) => MapIcon::RuinedPortalNether,
            (icon, _) => icon,
        }
    }
    /// The icon and catalog label of one found structure: its start variant's
    /// where the kind has one, else the layer's own.
    fn appearance(&self, variant: Option<Variant>, dimension: Cube) -> (MapIcon, String) {
        let (icon, suffix) = match variant {
            Some(Variant::Bastion(start)) => match start {
                Bastion::Housing => (MapIcon::BastionHousing, "-housing"),
                Bastion::HoglinStable => (MapIcon::BastionHoglinStable, "-hoglin-stable"),
                Bastion::Treasure => (MapIcon::BastionTreasure, "-treasure"),
                Bastion::Bridge => (MapIcon::BastionBridge, "-bridge"),
            },
            Some(Variant::Igloo { basement: true }) => (MapIcon::IglooBasement, "-basement"),
            Some(Variant::Igloo { basement: false }) => (MapIcon::IglooNoBasement, ""),
            Some(Variant::EndCity { ship: true }) => (self.icon, "-ship"),
            Some(Variant::EndCity { ship: false }) | None => (self.icon_in(dimension), ""),
        };
        (icon, format!("{}{suffix}", self.label_id()))
    }
}

/// Version and dimension of a world, as the tile provider reads them.
pub(crate) fn world_version(context: &WorldContext) -> Result<Version, PluginError> {
    match context.version.as_deref() {
        Some(name) => Version::from_name(name),
        None => context
            .data_version
            .ok_or(lumilio_cubiomes::Error::Unsupported)
            .and_then(Version::from_data_version),
    }
    .map_err(|_| PluginError::Unavailable("map-version-unsupported".into()))
}

fn cube(dimension: &Dimension) -> Option<Cube> {
    match dimension {
        Dimension::Overworld => Some(Cube::Overworld),
        Dimension::Nether => Some(Cube::Nether),
        Dimension::End => Some(Cube::End),
        Dimension::Custom(_) => None,
    }
}

fn offered(layer: &Layer, version: Version, dimension: Cube) -> bool {
    match layer.kind {
        Kind::Structure(kind) => kind.available(version, dimension),
        Kind::Strongholds => dimension == Cube::Overworld,
        Kind::Decoration(kind) => kind.available(version, dimension),
        Kind::PyramidApples => {
            Structure::DesertPyramid.available(version, dimension)
                && lumilio_cubiomes::desert_pyramid_loot_available(version)
        }
    }
}

pub(crate) fn catalog() -> Vec<OverlayInfo> {
    LAYERS
        .iter()
        .map(|layer| OverlayInfo {
            id: layer.id(),
            kind_id: layer.label_id(),
            dimensions: vec![Dimension::Overworld, Dimension::Nether, Dimension::End],
            icon: Some(layer.icon),
            group_id: Some(layer.group().into()),
            approximate: false,
            max_scale: None,
            creatable: vec![],
        })
        .collect()
}

/// The layers this world's version and dimension can show. Without a seed or
/// with a version cubiomes lacks there are none.
pub(crate) fn catalog_for(context: &WorldContext) -> Vec<OverlayInfo> {
    let (Ok(version), Some(dimension), Some(_)) = (
        world_version(context),
        cube(&context.dimension),
        context.seed,
    ) else {
        return vec![];
    };
    LAYERS
        .iter()
        .filter(|layer| offered(layer, version, dimension))
        .map(|layer| OverlayInfo {
            id: layer.id(),
            kind_id: layer.label_id(),
            dimensions: vec![context.dimension.clone()],
            icon: Some(layer.icon_in(dimension)),
            group_id: Some(layer.group().into()),
            approximate: layer.estimated(version),
            max_scale: None,
            creatable: vec![],
        })
        .collect()
}

/// Strongholds depend on biomes and cost a search each, so the 128 of a world
/// are computed once and kept for the few worlds being looked at.
type Worlds = Vec<((i64, i32), Vec<[i32; 2]>)>;
static STRONGHOLDS: Mutex<Worlds> = Mutex::new(Vec::new());
const STRONGHOLD_WORLDS: usize = 4;

fn strongholds(
    version: Version,
    seed: i64,
    cancelled: impl FnMut() -> bool,
) -> Result<Vec<[i32; 2]>, lumilio_cubiomes::Error> {
    let key = (seed, version.index());
    {
        let cache = STRONGHOLDS
            .lock()
            .expect("structure cache lock was poisoned");
        if let Some((_, found)) = cache.iter().find(|(known, _)| *known == key) {
            return Ok(found.clone());
        }
    }
    let found = lumilio_cubiomes::strongholds(version, seed, 128, cancelled)?;
    let mut cache = STRONGHOLDS
        .lock()
        .expect("structure cache lock was poisoned");
    cache.retain(|(known, _)| *known != key);
    cache.push((key, found.clone()));
    if cache.len() > STRONGHOLD_WORLDS {
        cache.remove(0);
    }
    Ok(found)
}

fn area(bounds: &MapBounds) -> Result<[i32; 4], PluginError> {
    let edges = [bounds.min.x, bounds.min.z, bounds.max.x, bounds.max.z];
    if edges
        .iter()
        .any(|edge| !edge.is_finite() || edge.abs() > 30_000_000.)
        || bounds.min.x >= bounds.max.x
        || bounds.min.z >= bounds.max.z
    {
        return Err(PluginError::InvalidInput("invalid structure bounds".into()));
    }
    Ok([
        bounds.min.x.floor() as i32,
        bounds.min.z.floor() as i32,
        bounds.max.x.ceil() as i32,
        bounds.max.z.ceil() as i32,
    ])
}

pub(crate) fn objects(
    ctx: &dyn HostContext,
    request: &OverlayRequest,
) -> Result<Vec<MapObject>, PluginError> {
    let layer = LAYERS
        .iter()
        .find(|layer| layer.id() == request.overlay)
        .ok_or_else(|| PluginError::InvalidInput("unknown structure layer".into()))?;
    let context = &request.context;
    let version = world_version(context)?;
    let (Some(seed), Some(dimension)) = (context.seed, cube(&context.dimension)) else {
        return Ok(vec![]);
    };
    if !offered(layer, version, dimension) {
        return Ok(vec![]);
    }
    let [x0, z0, x1, z1] = area(&request.bounds)?;
    let cancelled = || ctx.cancelled();
    let found = match layer.kind {
        Kind::Structure(kind) => lumilio_cubiomes::structures(
            version,
            seed,
            dimension,
            kind,
            [x0, z0, x1, z1],
            cancelled,
        ),
        Kind::Strongholds => strongholds(version, seed, cancelled).map(|all| {
            all.into_iter()
                .filter(|[x, z]| (x0..x1).contains(x) && (z0..z1).contains(z))
                .collect()
        }),
        // A candidate after a placement in its chunk is not known; the modelled
        // layers have one candidate per chunk, so none is dropped in practice.
        Kind::Decoration(kind) => lumilio_cubiomes::decorations(
            version,
            seed,
            dimension,
            kind,
            [x0, z0, x1, z1],
            cancelled,
        )
        .map(|found| {
            found
                .into_iter()
                .filter(|placed| !placed.after)
                .map(|placed| [placed.at[0], placed.at[2]])
                .collect()
        }),
        // Pyramids in the area whose chests hold an apple, marked at the
        // middle of their four chests.
        Kind::PyramidApples => lumilio_cubiomes::structures(
            version,
            seed,
            dimension,
            Structure::DesertPyramid,
            [x0, z0, x1, z1],
            cancelled,
        )
        .map(|pyramids| {
            pyramids
                .into_iter()
                .filter(|at| {
                    lumilio_cubiomes::desert_pyramid_chests(version, seed, *at)
                        .iter()
                        .any(|chest| chest.enchanted_golden_apples > 0)
                })
                .map(|[x, z]| [x + 10, z + 10])
                .collect()
        }),
    }
    .map_err(|error| match error {
        lumilio_cubiomes::Error::Cancelled => PluginError::Transient("map-cancelled".into()),
        lumilio_cubiomes::Error::InvalidRange => {
            PluginError::InvalidInput("structure area too large".into())
        }
        _ => PluginError::InvalidInput("structure search failed".into()),
    })?;
    let approximate = layer.estimated(version);
    Ok(found
        .into_iter()
        .map(|[x, z]| {
            let variant = layer
                .structure()
                .and_then(|kind| kind.variant(version, seed, [x, z]));
            let (icon, label_id) = layer.appearance(variant, dimension);
            object(request, layer, [x, z], icon, label_id, approximate)
        })
        .collect())
}

fn object(
    request: &OverlayRequest,
    layer: &Layer,
    [x, z]: [i32; 2],
    icon: MapIcon,
    label_id: String,
    approximate: bool,
) -> MapObject {
    let context = &request.context;
    MapObject {
        id: format!("{}:{x}:{z}", request.overlay),
        raw_id: format!("{x},{z}"),
        source: SOURCE.into(),
        world: context.world.clone(),
        dimension: context.dimension.clone(),
        kind: MapObjectKind::Icon {
            icon,
            at: MapPoint {
                x: f64::from(x),
                z: f64::from(z),
            },
        },
        label: None,
        label_id: Some(label_id),
        priority: if matches!(layer.kind, Kind::Strongholds) {
            10
        } else {
            0
        },
        approximate,
        color: None,
        note: None,
        share: None,
        editable: vec![],
    }
}
