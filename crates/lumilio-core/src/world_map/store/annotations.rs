//! Launcher-owned markers and routes. Their coordinates remain independent of
//! the provider object they may refer to, so losing that object loses no work.
use lumilio_plugin_api::map::{Dimension, MapPoint, WorldId};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AnnotationKind {
    Marker,
    Route,
}

impl AnnotationKind {
    fn database(self) -> &'static str {
        match self {
            Self::Marker => "marker",
            Self::Route => "route",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub id: i64,
    pub world: WorldId,
    pub dimension: Dimension,
    pub kind: AnnotationKind,
    pub name: String,
    pub color: [u8; 3],
    pub points: Vec<MapPoint>,
    pub linked_source: Option<String>,
    pub linked_raw_id: Option<String>,
}

#[derive(Debug)]
pub enum AnnotationError {
    Invalid(&'static str),
    Database(rusqlite::Error),
    Json(serde_json::Error),
}

impl From<rusqlite::Error> for AnnotationError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}
impl From<serde_json::Error> for AnnotationError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
impl std::fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => write!(f, "invalid map annotation: {reason}"),
            Self::Database(error) => error.fmt(f),
            Self::Json(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for AnnotationError {}

fn validate(annotation: &Annotation) -> Result<(), AnnotationError> {
    if annotation.name.trim().is_empty() || annotation.name.chars().count() > 64 {
        return Err(AnnotationError::Invalid("name"));
    }
    let count = annotation.points.len();
    if count == 0
        || count > 4096
        || (annotation.kind == AnnotationKind::Marker && count != 1)
        || (annotation.kind == AnnotationKind::Route && count < 2)
    {
        return Err(AnnotationError::Invalid("points"));
    }
    if annotation.points.iter().any(|point| {
        !point.x.is_finite()
            || !point.z.is_finite()
            || point.x.abs() > 30_000_000.
            || point.z.abs() > 30_000_000.
    }) {
        return Err(AnnotationError::Invalid("coordinates"));
    }
    if annotation.linked_source.is_some() != annotation.linked_raw_id.is_some() {
        return Err(AnnotationError::Invalid("link"));
    }
    Ok(())
}

fn belongs_to_instance(world: &WorldId, instance: &str) -> bool {
    match world {
        WorldId::Save {
            instance: owner, ..
        }
        | WorldId::Server {
            instance: owner, ..
        } => owner == instance,
        WorldId::Seed { .. } => true,
    }
}

/// Inserts a new annotation, or updates one that already belongs to this
/// instance. A caller cannot move another instance's record by guessing its id.
pub fn put(path: &Path, instance: &str, annotation: &Annotation) -> Result<i64, AnnotationError> {
    validate(annotation)?;
    if !belongs_to_instance(&annotation.world, instance) {
        return Err(AnnotationError::Invalid(
            "world belongs to another instance",
        ));
    }
    let db = Connection::open(path)?;
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM instances WHERE id = ?1)",
        [instance],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AnnotationError::Invalid("unknown instance"));
    }
    let world = serde_json::to_string(&annotation.world)?;
    let dimension = serde_json::to_string(&annotation.dimension)?;
    let points = serde_json::to_string(&annotation.points)?;
    let color = (i64::from(annotation.color[0]) << 16)
        | (i64::from(annotation.color[1]) << 8)
        | i64::from(annotation.color[2]);
    if annotation.id == 0 {
        db.execute(
            "INSERT INTO world_map_annotations (instance,world,dimension,kind,name,color,points,linked_source,linked_raw_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![instance, world, dimension, annotation.kind.database(), annotation.name, color, points, annotation.linked_source, annotation.linked_raw_id],
        )?;
        Ok(db.last_insert_rowid())
    } else {
        let changed = db.execute(
            "UPDATE world_map_annotations SET world=?3,dimension=?4,kind=?5,name=?6,color=?7,points=?8,linked_source=?9,linked_raw_id=?10 WHERE id=?1 AND instance=?2",
            params![annotation.id, instance, world, dimension, annotation.kind.database(), annotation.name, color, points, annotation.linked_source, annotation.linked_raw_id],
        )?;
        if changed == 0 {
            return Err(AnnotationError::Invalid(
                "annotation does not belong to instance",
            ));
        }
        Ok(annotation.id)
    }
}

pub fn remove(path: &Path, instance: &str, id: i64) -> Result<bool, AnnotationError> {
    if id <= 0 {
        return Err(AnnotationError::Invalid("id"));
    }
    let db = Connection::open(path)?;
    Ok(db.execute(
        "DELETE FROM world_map_annotations WHERE id=?1 AND instance=?2",
        params![id, instance],
    )? != 0)
}

pub fn list(
    path: &Path,
    instance: &str,
    world: &WorldId,
    dimension: &Dimension,
) -> Result<Vec<Annotation>, AnnotationError> {
    if !belongs_to_instance(world, instance) {
        return Err(AnnotationError::Invalid(
            "world belongs to another instance",
        ));
    }
    let db = Connection::open(path)?;
    let mut query = db.prepare(
        "SELECT id,kind,name,color,points,linked_source,linked_raw_id FROM world_map_annotations WHERE instance=?1 AND world=?2 AND dimension=?3 ORDER BY id",
    )?;
    let rows = query.query_map(
        params![
            instance,
            serde_json::to_string(world)?,
            serde_json::to_string(dimension)?
        ],
        |row| {
            let kind: String = row.get(1)?;
            let color: i64 = row.get(3)?;
            let points: String = row.get(4)?;
            Ok((
                row.get(0)?,
                kind,
                row.get(2)?,
                color,
                points,
                row.get(5)?,
                row.get(6)?,
            ))
        },
    )?;
    rows.map(|row| {
        let (id, kind, name, color, points, linked_source, linked_raw_id): (
            i64,
            String,
            String,
            i64,
            String,
            Option<String>,
            Option<String>,
        ) = row?;
        let kind = match kind.as_str() {
            "marker" => AnnotationKind::Marker,
            "route" => AnnotationKind::Route,
            _ => return Err(AnnotationError::Invalid("stored kind")),
        };
        Ok(Annotation {
            id,
            world: world.clone(),
            dimension: dimension.clone(),
            kind,
            name,
            color: [(color >> 16) as u8, (color >> 8) as u8, color as u8],
            points: serde_json::from_str(&points)?,
            linked_source,
            linked_raw_id,
        })
    })
    .collect()
}

/// Distance in blocks along a route, including every segment.
pub fn route_length(points: &[MapPoint]) -> f64 {
    points
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].z - pair[0].z))
        .sum()
}

/// Minecraft block coordinates for the corresponding Nether / Overworld
/// location. Negative values round down, as Minecraft's block positions do.
pub fn portal_coordinates(at: MapPoint, from: &Dimension) -> Option<(i64, i64)> {
    let ratio = match from {
        Dimension::Overworld => 1. / 8.,
        Dimension::Nether => 8.,
        _ => return None,
    };
    (at.x.is_finite() && at.z.is_finite())
        .then(|| ((at.x * ratio).floor() as i64, (at.z * ratio).floor() as i64))
}

#[cfg(test)]
mod tests;
