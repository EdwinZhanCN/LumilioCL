//! The 「存档位置」 layer: where a single-player save's `level.dat` says the
//! world spawn is, and where its player last stood.
use lumilio_nbt::Tag;
use lumilio_plugin_api::map::{
    Dimension, MapIcon, MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayRequest,
    WorldContext, WorldId,
};
use lumilio_plugin_api::{HostContext, PluginError};

pub(crate) const LAYER: &str = "save.positions";

pub(crate) fn is_layer(overlay: &str) -> bool {
    overlay == LAYER
}

pub(crate) fn catalog() -> Vec<OverlayInfo> {
    vec![OverlayInfo {
        id: LAYER.into(),
        kind_id: "map-save-positions".into(),
        dimensions: vec![Dimension::Overworld, Dimension::Nether, Dimension::End],
        icon: Some(MapIcon::Spawn),
        group_id: Some(crate::landmarks::GROUP.into()),
        approximate: false,
        max_scale: None,
        creatable: vec![],
    }]
}

/// Only a save has a `level.dat` to read.
pub(crate) fn catalog_for(context: &WorldContext) -> Vec<OverlayInfo> {
    if matches!(context.world, WorldId::Save { .. }) {
        catalog()
    } else {
        vec![]
    }
}

/// A place `level.dat` names, in block coordinates.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Place {
    pub dimension: Dimension,
    pub at: [f64; 3],
}

fn dimension_of(tag: &Tag) -> Option<Dimension> {
    if let Some(name) = tag.as_str() {
        return Some(match name.strip_prefix("minecraft:").unwrap_or(name) {
            "overworld" => Dimension::Overworld,
            "the_nether" => Dimension::Nether,
            "the_end" => Dimension::End,
            _ => Dimension::Custom(name.to_owned()),
        });
    }
    // Before 1.16 the dimension was a number.
    match tag.as_i64()? {
        0 => Some(Dimension::Overworld),
        -1 => Some(Dimension::Nether),
        1 => Some(Dimension::End),
        _ => None,
    }
}

/// The world spawn: `Data.spawn` (`pos` and `dimension`) from 26.x, the
/// Overworld's `Data.SpawnX/Y/Z` before.
pub(crate) fn spawn(level: &Tag) -> Option<Place> {
    if let Some(spawn) = level.at(&["Data", "spawn"]) {
        let Some(Tag::IntArray(pos)) = spawn.get("pos") else {
            return None;
        };
        let [x, y, z] = pos[..] else {
            return None;
        };
        let dimension = spawn
            .get("dimension")
            .and_then(dimension_of)
            .unwrap_or(Dimension::Overworld);
        return Some(Place {
            dimension,
            at: [x, y, z].map(f64::from),
        });
    }
    let axis = |name: &str| level.at(&["Data", name]).and_then(Tag::as_i64);
    Some(Place {
        dimension: Dimension::Overworld,
        at: [axis("SpawnX")?, axis("SpawnY")?, axis("SpawnZ")?].map(|value| value as f64),
    })
}

/// Where the single player last stood: `Data.Player`'s `Pos` and
/// `Dimension`. A server's `level.dat` has no player.
pub(crate) fn player(level: &Tag) -> Option<Place> {
    let player = level.at(&["Data", "Player"])?;
    let Some(Tag::List(pos)) = player.get("Pos") else {
        return None;
    };
    let pos: Vec<f64> = pos
        .iter()
        .map(|value| match value {
            Tag::Double(value) => Some(*value),
            Tag::Float(value) => Some(f64::from(*value)),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let [x, y, z] = pos[..] else {
        return None;
    };
    if ![x, y, z].iter().all(|value| value.is_finite()) {
        return None;
    }
    let dimension = player
        .get("Dimension")
        .and_then(dimension_of)
        .unwrap_or(Dimension::Overworld);
    Some(Place {
        dimension,
        at: [x, y, z],
    })
}

// ia[plugin.world-explorer]: 显示存档位置 | 图层浮层 ·「存档位置」开关 | 画出 level.dat 里的实际出生点和单人玩家最后的位置，只在所在维度显示；点选可看坐标并复制 | 默认打开；只对单人存档出现，此时种子估算的出生点不再显示
pub(crate) fn objects(
    ctx: &dyn HostContext,
    request: &OverlayRequest,
) -> Result<Vec<MapObject>, PluginError> {
    let WorldId::Save { folder, .. } = &request.context.world else {
        return Ok(vec![]);
    };
    let path = format!("saves/{folder}/level.dat");
    if ctx.file_stat(&path)?.is_none() {
        return Ok(vec![]);
    }
    let level = lumilio_nbt::parse_maybe_gzip(&ctx.read_file(&path)?)
        .map_err(|_| PluginError::Unavailable("map-save-unreadable".into()))?;
    let bounds = &request.bounds;
    let inside = |place: &Place| {
        place.dimension == request.context.dimension
            && place.at[0] >= bounds.min.x
            && place.at[0] < bounds.max.x
            && place.at[2] >= bounds.min.z
            && place.at[2] < bounds.max.z
    };
    let mut found = Vec::new();
    for (place, icon, kind, priority) in [
        (spawn(&level), MapIcon::Spawn, "spawn", 30),
        (player(&level), MapIcon::Player, "player", 40),
    ] {
        let Some(place) = place.filter(inside) else {
            continue;
        };
        let [x, y, z] = place.at;
        found.push(MapObject {
            id: format!("{LAYER}:{kind}"),
            raw_id: format!("level.dat:{kind}"),
            source: crate::ID.into(),
            world: request.context.world.clone(),
            dimension: request.context.dimension.clone(),
            kind: MapObjectKind::Icon {
                icon,
                at: MapPoint { x, z },
            },
            label: None,
            label_id: Some(format!("map-save-{kind}")),
            priority,
            approximate: false,
            color: (icon == MapIcon::Player).then_some([255, 255, 255]),
            note: Some(format!("Y {}", y.floor())),
            share: None,
            editable: vec![],
        });
    }
    Ok(found)
}
