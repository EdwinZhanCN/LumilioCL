use super::library::{instance_meta, relative_time, world_of};
use crate::home::{HomePresentation, RecentEntry, Subject};
use lumilio_core::{HomeSummary, InstanceRecord, ServerEntry, WorldInfo};

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
    pub detail: String,
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
    let worlds_shown = readable.into_iter().take(PLACE_WORLDS).map(|world| {
        let mut detail = vec!["世界".to_owned()];
        if let Some(ms) = world.last_played_ms.filter(|ms| *ms > 0) {
            detail.push(format!("上次游玩 {}", relative_time(ms as u64 / 1000, now)));
        }
        if world.hardcore {
            detail.push("极限模式".to_owned());
        }
        Place {
            target: PlaceTarget::World(world.folder.clone()),
            name: world.name.clone(),
            detail: detail.join(" · "),
        }
    });
    let servers_shown = servers.iter().take(PLACE_SERVERS).map(|server| Place {
        target: PlaceTarget::Server(server.address.clone()),
        name: server.name.clone(),
        detail: format!("服务器 · {}", server.address),
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
            Some(at) => format!(
                "{} · 上次游玩于 {}",
                instance_meta(record),
                relative_time(at, now)
            ),
            None => format!("{} · 还没玩过", instance_meta(record)),
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
