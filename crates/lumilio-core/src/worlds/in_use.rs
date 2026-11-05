//! Which saved worlds a running game has open (issue #128).
//!
//! Java Edition writes a world's `session.lock` when it opens the world, and
//! the file stays behind after the game leaves it, so the file being there
//! says nothing. When it was last touched does: a world whose lock was touched
//! after a game started was opened by that game.

use std::fs;
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

use super::WorldError;
use super::saves::{LOCK_FILE, saves_dir};

/// The worlds (by folder) that the running games of an instance may have
/// open, from when each world's lock was last touched and when each game
/// started (Unix milliseconds).
///
/// One game has at most one world open, the one it touched last. With more
/// than one game, a world one game left may have been opened by another, so
/// every world touched since the first of them started counts.
#[must_use]
pub fn worlds_in_use<'a>(
    touched: impl IntoIterator<Item = (&'a str, Option<i64>)>,
    started_ms: &[i64],
) -> Vec<&'a str> {
    let Some(first) = started_ms.iter().min() else {
        return Vec::new();
    };
    let mut since: Vec<(i64, &str)> = touched
        .into_iter()
        .filter_map(|(folder, at)| Some((at.filter(|at| at >= first)?, folder)))
        .collect();
    if started_ms.len() == 1 {
        since.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
        since.truncate(1);
    }
    since.into_iter().map(|(_, folder)| folder).collect()
}

/// When each world's lock under `game_dir` was last touched, without reading
/// the worlds themselves. A missing `saves/` has none.
pub(crate) fn lock_times(game_dir: &Path) -> Result<Vec<(String, Option<i64>)>, WorldError> {
    let entries = match fs::read_dir(saves_dir(game_dir)?) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut times = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Ok(folder) = entry.file_name().into_string() else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }
        times.push((folder, lock_touched_ms(&entry.path())));
    }
    Ok(times)
}

/// When the world at `world_dir` had its lock last touched.
pub(crate) fn lock_touched_ms(world_dir: &Path) -> Option<i64> {
    fs::metadata(world_dir.join(LOCK_FILE))
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
}

/// Refuses `folder` when a running game started at `started_ms` may have it
/// open.
pub(crate) fn refuse_if_open(
    game_dir: &Path,
    folder: &str,
    started_ms: &[i64],
) -> Result<(), crate::error::ServiceError> {
    if started_ms.is_empty() {
        return Ok(());
    }
    let times = lock_times(game_dir)?;
    let open = worlds_in_use(
        times.iter().map(|(name, at)| (name.as_str(), *at)),
        started_ms,
    );
    if open.contains(&folder) {
        return Err(crate::error::ServiceError::WorldInUse(folder.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
