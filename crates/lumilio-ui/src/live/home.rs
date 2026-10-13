use super::library::{instance_meta, relative_time, seed_of, world_of};
use crate::home::{HomePresentation, RecentEntry, Subject};
use gpui::{ImageSource, RenderImage};
use lumilio_core::{GameMode, HomeSummary, InstanceRecord, ServerEntry, WorldInfo};
use std::sync::Arc;

/// Worlds and servers listed under Home's world, at most.
pub const PLACE_WORLDS: usize = 3;
pub const PLACE_SERVERS: usize = 2;

/// What a place on Home enters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlaceTarget {
    /// A saved world, by its folder under `saves/`.
    World(String),
    /// A multiplayer server, by its address.
    Server(String),
}

/// One world or server Home can go straight into.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub target: PlaceTarget,
    pub name: String,
    /// A world's mode ([`world_mode`]).
    pub mode: Option<&'static str>,
    /// How long ago a world was last played ("3 天前").
    pub ago: Option<String>,
    /// The place's own picture: a world's icon or the server's cached one.
    pub picture: Option<PlacePicture>,
    /// Seeds the cover drawn when there is no picture.
    pub seed: u32,
}

/// A place's picture, read once when the places are.
#[derive(Clone)]
pub enum PlacePicture {
    File(std::path::PathBuf),
    Image(Arc<RenderImage>),
}

impl PlacePicture {
    #[must_use]
    pub fn source(&self) -> ImageSource {
        match self {
            Self::File(path) => path.clone().into(),
            Self::Image(image) => image.clone().into(),
        }
    }
}

// Two images are the same only when they are the same picture.
impl PartialEq for PlacePicture {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::File(a), Self::File(b)) => a == b,
            (Self::Image(a), Self::Image(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for PlacePicture {}

impl std::fmt::Debug for PlacePicture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(path) => f.debug_tuple("File").field(path).finish(),
            Self::Image(_) => f.write_str("Image"),
        }
    }
}

/// How a world is played, as its tag says: 极限 for a hardcore world (survival
/// with one life), otherwise its mode; nothing when `level.dat` does not say.
#[must_use]
pub fn world_mode(world: &WorldInfo) -> Option<&'static str> {
    if world.hardcore {
        return Some(crate::tr!("world-mode-hardcore"));
    }
    world.game_mode.map(|mode| match mode {
        GameMode::Survival => crate::tr!("world-mode-survival"),
        GameMode::Creative => crate::tr!("world-mode-creative"),
        GameMode::Adventure => crate::tr!("world-mode-adventure"),
        GameMode::Spectator => crate::tr!("world-mode-spectator"),
    })
}

/// Where the game Home continues left off, and its record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomePlaces {
    /// The game these belong to; shown only while Home is about it.
    pub instance: String,
    /// Empty when the game's version cannot go straight into a world.
    pub places: Vec<Place>,
    pub play_seconds: u64,
    pub worlds: usize,
    pub servers: usize,
}

/// The most recently played worlds that can be read, then the first servers
/// in the player's own order. `now` is in seconds since the Unix epoch.
pub fn home_places(
    record: &InstanceRecord,
    worlds: &[WorldInfo],
    servers: &[ServerEntry],
    now: u64,
) -> HomePlaces {
    let mut readable: Vec<&WorldInfo> = worlds.iter().filter(|world| !world.damaged).collect();
    readable.sort_by_key(|world| std::cmp::Reverse(world.last_played_ms.unwrap_or(0)));
    let worlds_shown = readable.into_iter().take(PLACE_WORLDS).map(|world| Place {
        target: PlaceTarget::World(world.folder.clone()),
        name: world.name.clone(),
        mode: world_mode(world),
        ago: world
            .last_played_ms
            .filter(|ms| *ms > 0)
            .map(|ms| relative_time(ms as u64 / 1000, now)),
        picture: world.icon.clone().map(PlacePicture::File),
        seed: seed_of(&world.folder),
    });
    let servers_shown = servers.iter().take(PLACE_SERVERS).map(|server| Place {
        target: PlaceTarget::Server(server.address.clone()),
        name: server.name.clone(),
        mode: None,
        ago: None,
        picture: server
            .icon
            .as_ref()
            .and_then(|icon| crate::pixels::image(icon.width, icon.height, &icon.rgba))
            .map(PlacePicture::Image),
        seed: seed_of(&server.address),
    });
    let places = if lumilio_core::quick_play_world_unsupported(&record.game_version) {
        Vec::new()
    } else {
        worlds_shown.chain(servers_shown).collect()
    };
    HomePlaces {
        instance: record.id.clone(),
        places,
        play_seconds: record.play_seconds,
        worlds: worlds.len(),
        servers: servers.len(),
    }
}

/// Home from the library and its summary.
///
/// Continue is the summary's pick, or — before anything was ever played — the
/// newest instance, so the first launch is one press away. No instances at all
/// is first use. Returns the instance Continue would launch alongside.
pub fn home_presentation(
    records: &[InstanceRecord],
    summary: &HomeSummary,
    prefer: Option<&str>,
    now: u64,
) -> (HomePresentation, Option<String>) {
    let find = |id: &str| records.iter().find(|record| record.id == id);
    let subject_of = |record: &InstanceRecord| Subject {
        id: Some(record.id.clone()),
        title: record.name.clone(),
        metadata: match record.last_played {
            Some(at) => crate::tr!(
                "home-meta-last-played",
                meta = instance_meta(record),
                when = relative_time(at, now)
            ),
            None => crate::tr!("home-meta-never-played", meta = instance_meta(record)),
        },
        world: world_of(&record.id),
    };
    // The current instance wins; without one, the latest played, then the newest.
    let chosen = prefer
        .and_then(find)
        .or_else(|| summary.continue_with.as_deref().and_then(find))
        .or_else(|| records.iter().max_by_key(|record| record.created_at));
    let Some(chosen) = chosen else {
        return (HomePresentation::FirstUse, None);
    };
    let recent = summary
        .recent
        .iter()
        .filter_map(|id| find(id))
        .filter(|record| record.id != chosen.id)
        .map(|record| RecentEntry {
            id: Some(record.id.clone()),
            title: record.name.clone(),
            metadata: format!(
                "{} · {}",
                instance_meta(record),
                record
                    .last_played
                    .map_or_else(String::new, |at| relative_time(at, now))
            ),
        })
        .collect();
    (
        HomePresentation::Continue {
            subject: subject_of(chosen),
            recent,
        },
        Some(chosen.id.clone()),
    )
}
