//! Which feature answers what a page asks: the shell's own features first,
//! the application for the rest.

use std::rc::Rc;

use gpui::WeakEntity;

use super::LauncherShell;
use crate::features::library::Library;
use crate::features::new_game::NewGame;
use crate::live::{LiveHandler, LiveIntent};

/// Sends the intents the shell or a feature it owns answers there, and every
/// other one to `app`.
pub(super) fn route(shell: &LauncherShell, app: LiveHandler) -> LiveHandler {
    let this = shell.this.clone();
    let new_game: WeakEntity<NewGame> = shell.new_game.downgrade();
    let library: WeakEntity<Library> = shell.library.downgrade();
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
        LiveIntent::InstallTarget(id) => {
            // Pages may ask while the shell is updating.
            let this = this.clone();
            window.defer(cx, move |_, cx| {
                let _ = this.update(cx, |shell, cx| shell.choose_current(id, cx));
            });
        }
        LiveIntent::ToggleFavorite(id) => {
            let _ = library.update(cx, |library, cx| library.toggle_favorite(id, cx));
        }
        LiveIntent::NewCollection => {
            let _ = library.update(cx, |library, cx| {
                library.ask_collection_name(None, window, cx);
            });
        }
        LiveIntent::RenameCollection(name) => {
            let _ = library.update(cx, |library, cx| {
                library.ask_collection_name(Some(name), window, cx);
            });
        }
        LiveIntent::DeleteCollection(name) => {
            let _ = library.update(cx, |library, cx| library.delete_collection(name, cx));
        }
        LiveIntent::EditCollections(id) => {
            let _ = library.update(cx, |library, cx| {
                library.ask_collections_of(id, window, cx);
            });
        }
        LiveIntent::ImportGame => {
            let _ = library.update(cx, |library, cx| library.import_game(window, cx));
        }
        LiveIntent::RestoreBackup => {
            let _ = library.update(cx, |library, cx| library.restore_backup(cx));
        }
        LiveIntent::RevealGame(id) => {
            let _ = library.read_with(cx, |library, cx| library.reveal_game(&id, cx));
        }
        LiveIntent::OpenGamesFolder => {
            let _ = library.read_with(cx, |library, cx| library.open_games_folder(cx));
        }
        other => app(other, window, cx),
    })
}
