use std::rc::Rc;

use gpui::{AppContext as _, Context, Window};
use lumilio_core::{CancellationToken, FoundGame};

use super::game_picker::GamePicker;
use super::library_actions::{Library, LibraryEvent};
use crate::runtime::{self, answer};
use crate::toast::{self, Toast, outcome};
use crate::tr;

impl Library {
    /// Asks for a folder made by another launcher and brings its game over;
    /// when it holds several, asks which.
    pub fn import_game(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("library-import-game-prompt").into()),
        });
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            let service = worker.service().clone();
            let found = worker.spawn(async move { service.detect_games(&folder).await });
            let Some(found) = answer(found, cx).await else {
                return;
            };
            let games = match found {
                Ok(games) => games,
                Err(error) => {
                    let toast =
                        Toast::error(tr!("library-import-game-none")).technical(error.to_string());
                    cx.update(|cx| toast::post(toast, cx));
                    return;
                }
            };
            if let [only] = games.as_slice() {
                let only = only.clone();
                let _ = this.update(cx, |this, cx| this.bring_over(vec![only], cx));
                return;
            }
            let _ = cx.update_window(window, |_, window, cx| {
                let form = cx.new(|_| {
                    GamePicker::new(
                        games,
                        Rc::new(move |chosen, _, cx| {
                            let _ = this.update(cx, |this, cx| this.bring_over(chosen, cx));
                        }),
                    )
                });
                GamePicker::open(form, window, cx);
            });
        })
        .detach();
    }

    /// Brings each game over as a task of its own in Activity.
    fn bring_over(&mut self, games: Vec<FoundGame>, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        for game in games {
            let service = worker.service().clone();
            let name = game.name.clone();
            runtime::job(
                &worker,
                cx,
                Some(LibraryEvent::ListsChanged),
                Some(Toast::info(tr!(
                    "library-import-game-started",
                    name = name.as_str()
                ))),
                async move { service.import_game(game, CancellationToken::new()).await },
                move |_, result, cx| {
                    let toast = outcome(
                        result,
                        |record| tr!("library-import-game-done", name = record.name),
                        tr!("library-import-game-failed", name = name.as_str()),
                    );
                    toast::post(toast, cx);
                },
            );
        }
    }

    /// Asks for a backup file and makes a new game of it.
    pub fn restore_backup(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("library-restore-prompt").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| this.restore(path, cx));
        })
        .detach();
    }

    fn restore(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        runtime::job(
            worker,
            cx,
            Some(LibraryEvent::ListsChanged),
            Some(Toast::info(tr!("library-restore-started"))),
            async move {
                service
                    .restore_backup(&path, CancellationToken::new())
                    .await
            },
            |_, result, cx| {
                let toast = outcome(
                    result,
                    |record| tr!("library-restore-done", name = record.name),
                    tr!("library-restore-failed").to_owned(),
                );
                toast::post(toast, cx);
            },
        );
    }
}
