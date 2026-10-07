use super::jobs::{job, outcome};
use super::{Reload, Wiring};
use gpui_kit::AppContext as _;
use gpui_kit::{App, Window};
use lumilio_core::CancellationToken;
use lumilio_ui::collections::{CollectionPicker, NamePrompt};
use lumilio_ui::game_picker::GamePicker;
use lumilio_ui::toast::Toast;
use lumilio_ui::tr;
use std::rc::Rc;

/// Makes `id` the current instance everywhere at once and remembers it for
/// the next start.
pub(super) fn choose_current(wiring: &Wiring, id: String, cx: &mut App) {
    let _ = wiring
        .shell
        .update(cx, |shell, cx| shell.set_install_target(id.clone(), cx));
    let service = wiring.backend.service.clone();
    job(
        wiring,
        cx,
        false,
        Reload::All,
        None,
        async move { service.set_current_instance(&id).await },
        |_, _, _| {},
    );
}

/// The names of the collections the library page shows.
pub(super) fn collection_names(wiring: &Wiring, cx: &mut App) -> Vec<String> {
    wiring
        .shell
        .update(cx, |shell, _| {
            shell.live().map_or_else(Vec::new, |model| {
                model.collections.iter().map(|c| c.name.clone()).collect()
            })
        })
        .unwrap_or_default()
}

/// Asks for a collection's name: a new one, or another for `original`.
pub(super) fn ask_collection_name(
    wiring: &Wiring,
    original: Option<String>,
    window: &mut Window,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let taken = collection_names(&wiring, cx);
        let renaming = original.is_some();
        let form = cx.new(|cx| {
            let (job_wiring, from) = (wiring.clone(), original.clone());
            NamePrompt::new(
                original,
                taken,
                Rc::new(move |name, _, cx| {
                    let service = job_wiring.backend.service.clone();
                    let from = from.clone();
                    job(
                        &job_wiring,
                        cx,
                        false,
                        Reload::Lists,
                        None,
                        async move {
                            match from {
                                Some(from) => service
                                    .rename_collection(&from, &name)
                                    .await
                                    .map(|()| tr!("collection-renamed", name = name.as_str())),
                                None => service
                                    .create_collection(&name)
                                    .await
                                    .map(|()| tr!("collection-created", name = name.as_str())),
                            }
                        },
                        |shell, result, cx| {
                            shell.toast(
                                outcome(
                                    result,
                                    |text| text,
                                    tr!("collection-save-failed").to_owned(),
                                ),
                                cx,
                            )
                        },
                    );
                }),
                window,
                cx,
            )
        });
        if renaming {
            NamePrompt::open(
                form,
                tr!("collection-rename-title"),
                tr!("collection-rename-confirm"),
                window,
                cx,
            );
        } else {
            NamePrompt::open(
                form,
                tr!("library-collection-new"),
                tr!("collection-create-confirm"),
                window,
                cx,
            );
        }
    });
}

/// Asks which collections a game belongs to, then files it there.
pub(super) fn ask_collections_of(wiring: &Wiring, id: String, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let rows = wiring
            .shell
            .update(cx, |shell, _| {
                shell
                    .live()
                    .map_or_else(Vec::new, |model| model.memberships_of(&id))
            })
            .unwrap_or_default();
        let form = cx.new(|cx| {
            let job_wiring = wiring.clone();
            CollectionPicker::new(
                rows,
                Rc::new(move |membership, _, cx| {
                    let service = job_wiring.backend.service.clone();
                    let id = id.clone();
                    job(
                        &job_wiring,
                        cx,
                        false,
                        Reload::Lists,
                        None,
                        async move {
                            let mut collections = membership.collections;
                            if let Some(new) = membership.new {
                                service.create_collection(&new).await?;
                                collections.push(new);
                            }
                            service.set_game_collections(&id, &collections).await
                        },
                        |shell, result, cx| {
                            shell.toast(
                                outcome(
                                    result,
                                    |()| tr!("collection-updated").to_owned(),
                                    tr!("collection-update-failed").to_owned(),
                                ),
                                cx,
                            )
                        },
                    );
                }),
                window,
                cx,
            )
        });
        CollectionPicker::open(form, window, cx);
    });
}

/// Asks for a folder made by another launcher and brings its game over;
/// when it holds several, asks which.
pub(super) fn import_game(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(tr!("library-import-game-prompt").into()),
    });
    let wiring = wiring.clone();
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = picked.await else {
            return;
        };
        let Some(folder) = paths.into_iter().next() else {
            return;
        };
        let service = wiring.backend.service.clone();
        let Ok(found) = wiring
            .backend
            .spawn(async move { service.detect_games(&folder).await })
            .await
        else {
            return;
        };
        let games = match found {
            Ok(games) => games,
            Err(error) => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(
                        Toast::error(tr!("library-import-game-none")).technical(error.to_string()),
                        cx,
                    )
                });
                return;
            }
        };
        let start = {
            let wiring = wiring.clone();
            move |games: Vec<lumilio_core::FoundGame>, cx: &mut App| {
                for game in games {
                    let service = wiring.backend.service.clone();
                    let name = game.name.clone();
                    job(
                        &wiring,
                        cx,
                        true,
                        Reload::All,
                        Some(Toast::info(tr!(
                            "library-import-game-started",
                            name = name.as_str()
                        ))),
                        async move { service.import_game(game, CancellationToken::new()).await },
                        move |shell, result, cx| {
                            shell.toast(
                                outcome(
                                    result,
                                    |record| tr!("library-import-game-done", name = record.name),
                                    tr!("library-import-game-failed", name = name.as_str()),
                                ),
                                cx,
                            )
                        },
                    );
                }
            }
        };
        if let [only] = games.as_slice() {
            let only = only.clone();
            cx.update(|cx| start(vec![only], cx));
            return;
        }
        let _ = cx.update_window(handle, |_, window, cx| {
            let form =
                cx.new(|_| GamePicker::new(games, Rc::new(move |chosen, _, cx| start(chosen, cx))));
            GamePicker::open(form, window, cx);
        });
    })
    .detach();
}

/// Asks for a backup file and makes a new game of it.
pub(super) fn restore_backup(wiring: &Wiring, cx: &mut App) {
    let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(tr!("library-restore-prompt").into()),
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = picked.await else {
            return;
        };
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        cx.update(|cx| {
            let service = wiring.backend.service.clone();
            job(
                &wiring,
                cx,
                true,
                Reload::All,
                Some(Toast::info(tr!("library-restore-started"))),
                async move {
                    service
                        .restore_backup(&path, CancellationToken::new())
                        .await
                },
                |shell, result, cx| {
                    shell.toast(
                        outcome(
                            result,
                            |record| tr!("library-restore-done", name = record.name),
                            tr!("library-restore-failed").to_owned(),
                        ),
                        cx,
                    )
                },
            );
        });
    })
    .detach();
}
