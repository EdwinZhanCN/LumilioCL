//! Worlds and servers as list rows (design language §12 List rows). The
//! Worlds tab and Home's 接着玩 say the same things about a place, in the
//! same forms; only the picture and the keys are each list's own.

use crate::kit;
use crate::theme::ShellColors;
use crate::tr;
use gpui::RenderImage;
use gpui::prelude::*;
use lumilio_core::{ServerEntry, ServerStatus, WorldInfo};
use std::sync::Arc;

#[cfg(test)]
mod tests;

/// What is known of a server's state right now.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerState {
    Checking,
    Online(ServerStatus),
    Offline,
}

impl ServerState {
    /// The state an answer leaves: online with what it said, or offline.
    #[must_use]
    pub fn of<E>(answer: Result<ServerStatus, E>) -> Self {
        answer.map_or(Self::Offline, Self::Online)
    }
}

/// The picture a server sent with its answer; it replaces the one the game
/// cached.
#[must_use]
pub fn favicon(status: &ServerStatus) -> Option<Arc<RenderImage>> {
    let pixels = status.favicon.as_ref()?;
    crate::pixels::image(pixels.width, pixels.height, &pixels.rgba)
}

/// A world's state beside its name: unreadable before being played.
#[must_use]
pub fn world_state(world: &WorldInfo, playing: bool) -> Option<(&'static str, kit::Tone)> {
    if world.damaged {
        Some((tr!("instance-world-damaged"), kit::Tone::Failure))
    } else if playing {
        Some((tr!("instance-world-playing"), kit::Tone::Lit))
    } else {
        None
    }
}

/// The tags under a world's name: how it is played, whether cheats are on,
/// and the version that last saved it.
#[must_use]
pub fn world_tags(world: &WorldInfo) -> Vec<(String, kit::TagKind)> {
    let mut tags = Vec::new();
    if let Some(mode) = crate::live::world_mode(world) {
        tags.push((mode.to_owned(), kit::TagKind::Ink));
    }
    if world.cheats {
        tags.push((tr!("world-cheats").to_owned(), kit::TagKind::Plain));
    }
    if let Some(version) = &world.game_version {
        tags.push((version.clone(), kit::TagKind::Plain));
    }
    tags
}

/// A world row without its picture and keys. A copy keeps its name, so the
/// folder tells the two apart, as in the game's own list.
pub fn world_item(world: &WorldInfo, playing: bool, colors: ShellColors) -> kit::Item {
    let mut item = kit::Item::new(world.name.clone());
    if let Some((word, tone)) = world_state(world, playing) {
        item = item.state(word, tone);
    }
    for (text, kind) in world_tags(world) {
        item = item.tag(text, kind);
    }
    if world.folder != world.name {
        item = item.note(world.folder.clone());
    }
    if let Some(played) = world.last_played_ms.filter(|ms| *ms > 0) {
        item = item.reading(kit::reading(crate::live::ago(played as u64 / 1000), colors));
    }
    item
}

/// A server row without its picture and keys: its address under the name,
/// and how it is in the form each fact takes — the version a tag, the
/// players and the round trip readings, the message in its own colours
/// beside the name. Before an answer the readings are a state.
pub fn server_item(
    row: usize,
    server: &ServerEntry,
    state: Option<&ServerState>,
    colors: ShellColors,
) -> kit::Item {
    let item = kit::Item::new(server.name.clone()).note(server.address.clone());
    let quiet = |word: &'static str, selector: &'static str| {
        kit::status(word, kit::Tone::Off, colors)
            .debug_selector(move || format!("{selector}-{row}"))
    };
    match state {
        None | Some(ServerState::Checking) => {
            item.reading(quiet(tr!("instance-server-checking"), "server-checking"))
        }
        Some(ServerState::Offline) => {
            item.reading(quiet(tr!("instance-server-offline"), "server-offline"))
        }
        Some(ServerState::Online(status)) => {
            let mut item = item;
            if let Some(version) = &status.version {
                item = item.tag(version.clone(), kit::TagKind::Plain);
            }
            if let Some(online) = status.online {
                item = item.reading(
                    kit::count_of(u64::from(online), status.max.map(u64::from), colors)
                        .debug_selector(move || format!("server-players-{row}")),
                );
            }
            if let Some(ms) = status.latency_ms {
                item = item.reading(
                    kit::signal(("server-signal", row), ms, colors)
                        .debug_selector(move || format!("server-signal-{row}")),
                );
            }
            if status.online.is_none() && status.latency_ms.is_none() {
                item = item.reading(
                    kit::status(tr!("instance-server-online"), kit::Tone::Ok, colors)
                        .debug_selector(move || format!("server-online-{row}")),
                );
            }
            match kit::motd_text(&status.motd, colors) {
                Some(text) => item.aside(text.debug_selector(move || format!("server-motd-{row}"))),
                None => item,
            }
        }
    }
}
