//! Which feature answers what a page asks: the shell's own features first,
//! the application for the rest.

use std::rc::Rc;

use gpui::{Entity, WeakEntity};

use crate::features::new_game::NewGame;
use crate::live::{LiveHandler, LiveIntent};

/// Sends the intents a feature the shell owns answers to that feature, and
/// every other one to `app`.
pub(super) fn route(new_game: &Entity<NewGame>, app: LiveHandler) -> LiveHandler {
    let new_game: WeakEntity<NewGame> = new_game.downgrade();
    Rc::new(move |intent, window, cx| match intent {
        LiveIntent::NewInstance => {
            let _ = new_game.update(cx, |new_game, cx| new_game.open(window, cx));
        }
        LiveIntent::ImportPack => {
            let _ = new_game.update(cx, |new_game, cx| new_game.import_pack(cx));
        }
        LiveIntent::DropFiles(paths) => {
            let _ = new_game.update(cx, |new_game, cx| new_game.import_files(paths, cx));
        }
        other => app(other, window, cx),
    })
}
