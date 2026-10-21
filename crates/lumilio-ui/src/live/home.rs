use super::library::{instance_meta, relative_time, seed_of, world_of};
use crate::home::{HomePresentation, Subject};
use crate::places::ServerState;
use gpui::{ImageSource, RenderImage};
use lumilio_core::{GameMode, HomeSummary, InstanceRecord, ServerEntry, ServerStatus, WorldInfo};
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

/// What a place is: a world as the game saved it, or a server and what is
/// known of it now. Its row says what the Worlds tab says about it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlaceKind {
    World(WorldInfo),
    Server(ServerEntry, ServerState),
}

/// One world or server Home can go straight into.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub kind: PlaceKind,
    /// The place's own picture: a world's icon, or the server's (the one
    /// the game cached until the server sends its own).
    pub picture: Option<PlacePicture>,
    /// Seeds the cover drawn when there is no picture.
    pub seed: u32,
}

impl Place {
    /// What entering the place goes into.
    #[must_use]
    pub fn target(&self) -> PlaceTarget {
        match &self.kind {
            PlaceKind::World(world) => PlaceTarget::World(world.folder.clone()),
            PlaceKind::Server(server, _) => PlaceTarget::Server(server.address.clone()),
        }
    }
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

impl HomePlaces {
    /// The listed servers to ask how they are, each once.
    #[must_use]
    pub fn server_addresses(&self) -> Vec<String> {
        let mut addresses: Vec<String> = Vec::new();
        for place in &self.places {
            if let PlaceKind::Server(server, _) = &place.kind
                && !addresses.contains(&server.address)
            {
                addresses.push(server.address.clone());
            }
        }
        addresses
    }

    /// A server answered (or did not): its rows show how it is, and the
    /// picture it sent replaces the cached one.
    pub fn server_answered<E>(&mut self, address: &str, answer: Result<ServerStatus, E>) {
        let state = ServerState::of(answer);
        let sent = match &state {
            ServerState::Online(status) => crate::places::favicon(status),
            _ => None,
        };
        for place in &mut self.places {
            if let PlaceKind::Server(server, known) = &mut place.kind
                && server.address == address
            {
                *known = state.clone();
                if let Some(image) = &sent {
                    place.picture = Some(PlacePicture::Image(image.clone()));
                }
            }
        }
    }
}

/// The most recently played worlds that can be read, then the first servers
/// in the player's own order, still being asked about.
pub fn home_places(
    record: &InstanceRecord,
    worlds: &[WorldInfo],
    servers: &[ServerEntry],
) -> HomePlaces {
    let mut readable: Vec<&WorldInfo> = worlds.iter().filter(|world| !world.damaged).collect();
    readable.sort_by_key(|world| std::cmp::Reverse(world.last_played_ms.unwrap_or(0)));
    let worlds_shown = readable.into_iter().take(PLACE_WORLDS).map(|world| Place {
        kind: PlaceKind::World(world.clone()),
        picture: world.icon.clone().map(PlacePicture::File),
        seed: seed_of(&world.folder),
    });
    let servers_shown = servers.iter().take(PLACE_SERVERS).map(|server| Place {
        kind: PlaceKind::Server(server.clone(), ServerState::Checking),
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
    (
        HomePresentation::Continue {
            subject: subject_of(chosen),
        },
        Some(chosen.id.clone()),
    )
}
