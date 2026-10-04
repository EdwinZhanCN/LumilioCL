use super::instance::{load_instance, open_instance};
use super::jobs::{job, outcome, reload};
use super::library::choose_current;
use super::{Reload, Wiring};
use gpui_kit::AppContext as _;
use gpui_kit::{App, WeakEntity, Window};
use lumilio_core::{CancellationToken, LaunchServiceError, ServiceError};
use lumilio_ui::new_game::{NewGameForm, NewGameIntent, NewGameRequest};
use lumilio_ui::toast::Toast;
use std::rc::Rc;

/// Opens the new-game dialog (IA `library.md#新建游戏`). Deferred: this may
/// run inside one of the shell's own updates.
pub(super) fn open_new_game(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let handle = window.window_handle();
        let form = cx.new(|cx| {
            let form = cx.weak_entity();
            let wiring = wiring.clone();
            NewGameForm::new(
                Rc::new(move |intent, _, cx| {
                    new_game_intent(&wiring, form.clone(), handle, intent, cx);
                }),
                window,
                cx,
            )
        });
        NewGameForm::open(form, window, cx);
    });
}

pub(super) fn new_game_intent(
    wiring: &Wiring,
    form: WeakEntity<NewGameForm>,
    window: gpui_kit::AnyWindowHandle,
    intent: NewGameIntent,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    match intent {
        NewGameIntent::LoadGameVersions => {
            let handle = wiring
                .backend
                .spawn(async move { service.game_versions().await });
            cx.spawn(async move |cx| {
                let result = match handle.await {
                    Ok(Ok(entries)) => Ok(entries),
                    Ok(Err(error)) => Err(("读不到版本列表".to_owned(), error.to_string())),
                    Err(error) => Err(("读不到版本列表".to_owned(), error.to_string())),
                };
                let _ = form.update(cx, |form, cx| form.game_versions_arrived(result, cx));
            })
            .detach();
        }
        NewGameIntent::LoadLoaderVersions {
            loader,
            game_version,
        } => {
            let wanted = game_version.clone();
            let handle = wiring
                .backend
                .spawn(async move { service.loader_versions(loader, &wanted).await });
            cx.spawn(async move |cx| {
                let failed = format!("读不到 {} 的版本", lumilio_ui::live::loader_label(loader));
                let result = match handle.await {
                    Ok(Ok(versions)) => Ok(versions),
                    Ok(Err(error)) => Err((failed, error.to_string())),
                    Err(error) => Err((failed, error.to_string())),
                };
                let _ = form.update(cx, |form, cx| {
                    form.loader_versions_arrived(loader, &game_version, result, cx)
                });
            })
            .detach();
        }
        NewGameIntent::Create(request) => create_game(wiring, form, window, request, cx),
    }
}

/// Creates the game, then (if asked) downloads its files in the background,
/// shows it and makes it the current game.
pub(super) fn create_game(
    wiring: &Wiring,
    form: WeakEntity<NewGameForm>,
    window: gpui_kit::AnyWindowHandle,
    request: NewGameRequest,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let wanted = request.clone();
    let handle = wiring.backend.spawn(async move {
        service
            .create_instance(
                &wanted.name,
                Some(&wanted.game_version),
                wanted.loader,
                wanted.loader_version.as_deref(),
            )
            .await
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        let record = match result {
            Ok(record) => record,
            Err(error) => {
                let message = match &error {
                    ServiceError::Launch(LaunchServiceError::LoaderUnsupported(loader)) => format!(
                        "{} 还不能安装，支持正在路上",
                        lumilio_ui::live::loader_label(*loader)
                    ),
                    _ => "没有创建成功，可以再试一次".to_owned(),
                };
                let _ = form.update(cx, |form, cx| {
                    form.created(Err((message, error.to_string())), cx)
                });
                return;
            }
        };
        let _ = form.update(cx, |form, cx| form.created(Ok(()), cx));
        let id = record.id.clone();
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                Toast::success(if request.install {
                    format!("已创建 {}，正在下载游戏文件", record.name)
                } else {
                    format!("已创建 {}", record.name)
                }),
                cx,
            );
        });
        cx.update(|cx| {
            reload(&wiring, Reload::All, cx);
            if request.install {
                install_new_game(&wiring, id.clone(), cx);
            }
        });
        let _ = cx.update_window(window, |_, window, cx| {
            open_instance(&wiring, id.clone(), None, window, cx);
            choose_current(&wiring, id, cx);
        });
    })
    .detach();
}

/// Downloads a new game's files; progress lives in Activity.
pub(super) fn install_new_game(wiring: &Wiring, id: String, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let target = id.clone();
    let after = wiring.clone();
    job(
        wiring,
        cx,
        true,
        Reload::Lists,
        None,
        async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(async move { while rx.recv().await.is_some() {} });
            service
                .install_instance(&id, tx, CancellationToken::new())
                .await
        },
        move |shell, result, cx| {
            if let Err(error) = result {
                shell.toast(
                    Toast::error("游戏文件没有下载完，开始游戏时会再试")
                        .technical(error.to_string()),
                    cx,
                );
            }
            // An open page for this game shows the new install state.
            if let Some(view) = shell
                .live_instance()
                .filter(|view| view.read(cx).id() == target)
                .map(|view| view.downgrade())
            {
                load_instance(&after, target, view, cx);
            }
        },
    );
}

/// Asks for a local `.mrpack` and imports it as a new instance. Cancelling the
/// picker does nothing.
pub(super) fn import_pack(wiring: &Wiring, cx: &mut App) {
    let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("选择整合包（.mrpack，或 MultiMC / Prism 的 .zip）".into()),
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = picked.await else {
            return;
        };
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        cx.update(|cx| import_pack_file(&wiring, path, cx));
    })
    .detach();
}

/// Imports a chosen or dropped `.mrpack` as a new game.
pub(super) fn import_pack_file(wiring: &Wiring, path: std::path::PathBuf, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    job(
        wiring,
        cx,
        true,
        Reload::All,
        Some(Toast::info(format!("开始导入 {name}，进度在动态里"))),
        async move {
            service
                .import_modpack_file(&path, CancellationToken::new())
                .await
        },
        move |shell, result, cx| {
            let toast = outcome(
                result,
                |record| format!("已导入整合包 {}", record.name),
                format!("没有导入 {name}"),
            );
            shell.toast(toast, cx);
        },
    );
}
