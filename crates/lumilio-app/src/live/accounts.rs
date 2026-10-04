use super::jobs::reload;
use super::{Reload, Wiring};
use gpui_kit::AppContext as _;
use gpui_kit::{App, WeakEntity, Window};
use lumilio_core::{CancellationToken, ServiceError, ThirdPartySignIn};
use lumilio_ui::accounts::{AccountForm, AccountRequest};
use lumilio_ui::auth_servers::{ServerIntent, ServerRow, ServersDialog};
use lumilio_ui::live::account_failure;
use lumilio_ui::microsoft_login::{SignInDialog, SignInIntent};
use lumilio_ui::platform;
use lumilio_ui::skin_dialog::{SkinDialog, SkinIntent};
use lumilio_ui::third_party_login::{ThirdPartyDialog, ThirdPartyIntent};
use lumilio_ui::toast::Toast;
use std::rc::Rc;

/// Opens the add-account dialog (IA `accounts.md`). With `then_launch` the
/// game the person asked to start begins as soon as the account is saved.
/// Deferred: this may run inside one of the shell's own updates.
pub(super) fn open_add_account(
    wiring: &Wiring,
    then_launch: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let handle = window.window_handle();
        let first = wiring
            .shell
            .update(cx, |shell, _| {
                shell.live().is_none_or(|model| model.accounts.is_empty())
            })
            .unwrap_or(true);
        let form = cx.new(|cx| {
            let form = cx.weak_entity();
            let wiring = wiring.clone();
            AccountForm::new(
                Rc::new(move |request, _, cx| {
                    add_account(&wiring, form.clone(), handle, request, then_launch, cx);
                }),
                first,
                window,
                cx,
            )
        });
        AccountForm::open(form, window, cx);
    });
}

/// Opens the Microsoft sign-in dialog. Nothing is asked of Microsoft until
/// the person presses the button in it.
pub(super) fn open_microsoft_sign_in(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let dialog = cx.new(|cx| {
            let weak: WeakEntity<SignInDialog> = cx.weak_entity();
            SignInDialog::new(Rc::new(move |intent, _, cx| match intent {
                SignInIntent::Start => start_microsoft_sign_in(&wiring, weak.clone(), cx),
                SignInIntent::Cancel => {
                    if let Some(cancel) = wiring.state.borrow_mut().sign_in_cancel.take() {
                        cancel.cancel();
                    }
                }
            }))
        });
        SignInDialog::open(dialog, window, cx);
    });
}

/// Asks for a code, shows it (and opens the page), waits for the browser, and
/// then adds the account. Failures are told in the dialog, in words.
pub(super) fn start_microsoft_sign_in(
    wiring: &Wiring,
    dialog: WeakEntity<SignInDialog>,
    cx: &mut App,
) {
    let cancel = CancellationToken::new();
    wiring.state.borrow_mut().sign_in_cancel = Some(cancel.clone());
    let (code_tx, mut code_rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();
    let service = wiring.backend.service.clone();
    let handle = wiring.backend.spawn(async move {
        service
            .microsoft_sign_in(
                move |code| {
                    let _ = code_tx.send((code.user_code.clone(), code.verification_uri.clone()));
                },
                cancel,
            )
            .await
    });
    let shown = dialog.clone();
    cx.spawn(async move |cx| {
        if let Some((code, address)) = code_rx.recv().await {
            let _ = shown.update(cx, |dialog, cx| dialog.code_arrived(&code, &address, cx));
            cx.update(|cx| platform::open_address(&address, cx));
        }
    })
    .detach();
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        wiring.state.borrow_mut().sign_in_cancel = None;
        match result {
            Ok((_, name)) => {
                let _ = dialog.update(cx, |dialog, cx| dialog.signed_in(cx));
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(Toast::success(format!("已登录 {name}")), cx)
                });
                cx.update(|cx| reload(&wiring, Reload::Lists, cx));
            }
            // The person cancelled: the dialog is already gone.
            Err(ServiceError::Auth(lumilio_core::AuthError::Cancelled)) => {}
            Err(error) => {
                let _ = dialog.update(cx, |dialog, cx| dialog.failed(account_failure(&error), cx));
            }
        }
    })
    .detach();
}

pub(super) fn add_account(
    wiring: &Wiring,
    form: WeakEntity<AccountForm>,
    window: gpui_kit::AnyWindowHandle,
    request: AccountRequest,
    then_launch: bool,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let wanted = request.clone();
    let handle = wiring.backend.spawn(async move {
        service
            .add_account(&wanted.name, wanted.uuid.as_deref())
            .await
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        if let Err(error) = result {
            let _ = form.update(cx, |form, cx| form.saved(Err(account_failure(&error)), cx));
            return;
        }
        let _ = form.update(cx, |form, cx| form.saved(Ok(()), cx));
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(Toast::success(format!("已添加账户 {}", request.name)), cx);
        });
        cx.update(|cx| reload(&wiring, Reload::Lists, cx));
        if then_launch {
            // The model learns of the account on the reload above; give it a beat.
            let _ = cx.update_window(window, |_, window, cx| {
                let wiring = wiring.clone();
                window.defer(cx, move |window, cx| {
                    let _ = wiring
                        .shell
                        .update(cx, |shell, cx| shell.request_continue(window, cx));
                });
            });
        }
    })
    .detach();
}

/// The authentication servers as the list shows them: LittleSkin, then the
/// added ones, each with how many accounts are signed in on it.
async fn server_rows(service: &crate::backend::Service) -> Vec<ServerRow> {
    let settings = service.settings().await;
    service
        .auth_servers()
        .await
        .into_iter()
        .map(|server| ServerRow {
            accounts: settings
                .accounts
                .iter()
                .filter(|entry| entry.server.as_deref() == Some(server.url.as_str()))
                .count(),
            builtin: server.url == lumilio_core::LITTLE_SKIN_URL,
            name: server.display_name().to_owned(),
            url: server.url,
        })
        .collect()
}

/// Opens the sign-in dialog for authentication servers. The servers are read
/// first (off the interface thread), then the dialog opens.
pub(super) fn open_third_party_sign_in(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let handle = window.window_handle();
    let service = wiring.backend.service.clone();
    let servers = wiring
        .backend
        .spawn(async move { service.auth_servers().await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Ok(servers) = servers.await else { return };
        let _ = cx.update_window(handle, |_, window, cx| {
            let dialog = cx.new(|cx| {
                let weak: WeakEntity<ThirdPartyDialog> = cx.weak_entity();
                ThirdPartyDialog::new(
                    Rc::new(move |intent, window, cx| {
                        third_party_intent(&wiring, weak.clone(), intent, window, cx)
                    }),
                    servers,
                    window,
                    cx,
                )
            });
            ThirdPartyDialog::open(dialog, window, cx);
        });
    })
    .detach();
}

fn third_party_intent(
    wiring: &Wiring,
    dialog: WeakEntity<ThirdPartyDialog>,
    intent: ThirdPartyIntent,
    window: &mut Window,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = match intent {
        ThirdPartyIntent::SignIn {
            server,
            login,
            password,
        } => wiring.backend.spawn(async move {
            service
                .third_party_sign_in(&server, &login, &password)
                .await
        }),
        ThirdPartyIntent::Choose { pending, character } => wiring.backend.spawn(async move {
            service
                .third_party_choose(pending, character)
                .await
                .map(|(key, name)| ThirdPartySignIn::Done { key, name })
        }),
        ThirdPartyIntent::Abandon(pending) => {
            service.third_party_abandon(pending);
            return;
        }
        ThirdPartyIntent::ManageServers => {
            open_auth_servers(wiring, window, cx);
            return;
        }
    };
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        match result {
            Ok(ThirdPartySignIn::Done { name, .. }) => {
                let _ = dialog.update(cx, |dialog, cx| dialog.signed_in(cx));
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(Toast::success(format!("已登录 {name}")), cx)
                });
                cx.update(|cx| reload(&wiring, Reload::Lists, cx));
            }
            Ok(ThirdPartySignIn::Choose {
                pending,
                characters,
            }) => {
                let _ = dialog.update(cx, |dialog, cx| dialog.choose(pending, characters, cx));
            }
            Err(error) => {
                let _ = dialog.update(cx, |dialog, cx| dialog.failed(account_failure(&error), cx));
            }
        }
    })
    .detach();
}

/// Opens the list of authentication servers.
pub(super) fn open_auth_servers(wiring: &Wiring, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let handle = window.window_handle();
        let service = wiring.backend.service.clone();
        let rows = wiring
            .backend
            .spawn(async move { server_rows(&service).await });
        cx.spawn(async move |cx| {
            let Ok(rows) = rows.await else { return };
            let _ = cx.update_window(handle, |_, window, cx| {
                let dialog = cx.new(|cx| {
                    let weak: WeakEntity<ServersDialog> = cx.weak_entity();
                    ServersDialog::new(
                        Rc::new(move |intent, window, cx| {
                            server_intent(&wiring, weak.clone(), intent, window, cx)
                        }),
                        rows,
                        window,
                        cx,
                    )
                });
                ServersDialog::open(dialog, window, cx);
            });
        })
        .detach();
    });
}

fn server_intent(
    wiring: &Wiring,
    dialog: WeakEntity<ServersDialog>,
    intent: ServerIntent,
    window: &mut Window,
    cx: &mut App,
) {
    let handle = window.window_handle();
    let service = wiring.backend.service.clone();
    let wiring = wiring.clone();
    match intent {
        ServerIntent::Locate(address) => {
            let work = wiring
                .backend
                .spawn(async move { service.locate_auth_server(&address).await });
            cx.spawn(async move |cx| {
                let result = match work.await {
                    Ok(result) => result,
                    Err(error) => Err(ServiceError::Remote(error.to_string())),
                };
                let _ = dialog.update(cx, |dialog, cx| {
                    dialog.located(result.map_err(|e| account_failure(&e)), cx)
                });
            })
            .detach();
        }
        ServerIntent::Add(server) => {
            let name = server.display_name().to_owned();
            let work = wiring.backend.spawn(async move {
                service.add_auth_server(&server).await?;
                Ok::<_, ServiceError>(server_rows(&service).await)
            });
            finish_server_change(
                &wiring,
                dialog,
                handle,
                work,
                format!("已添加认证服务器 {name}"),
                cx,
            );
        }
        ServerIntent::Remove(url) => {
            let work = wiring.backend.spawn(async move {
                service.remove_auth_server(&url).await?;
                Ok::<_, ServiceError>(server_rows(&service).await)
            });
            finish_server_change(
                &wiring,
                dialog,
                handle,
                work,
                "已移除认证服务器".to_owned(),
                cx,
            );
        }
    }
}

fn finish_server_change(
    wiring: &Wiring,
    dialog: WeakEntity<ServersDialog>,
    window: gpui_kit::AnyWindowHandle,
    work: tokio::task::JoinHandle<Result<Vec<ServerRow>, ServiceError>>,
    notice: String,
    cx: &mut App,
) {
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match work.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        match result {
            Ok(rows) => {
                let _ = cx.update_window(window, |_, window, cx| {
                    let _ = dialog.update(cx, |dialog, cx| dialog.listed(rows, window, cx));
                });
                let _ = wiring
                    .shell
                    .update(cx, |shell, cx| shell.toast(Toast::success(notice), cx));
                // Removing a server removes its accounts too.
                cx.update(|cx| reload(&wiring, Reload::Lists, cx));
            }
            Err(error) => {
                let _ = dialog.update(cx, |dialog, cx| dialog.failed(account_failure(&error), cx));
            }
        }
    })
    .detach();
}

/// Opens the skin dialog of an offline account.
pub(super) fn open_skin_dialog(wiring: &Wiring, key: String, window: &mut Window, cx: &mut App) {
    let wiring = wiring.clone();
    window.defer(cx, move |window, cx| {
        let Some(row) = wiring
            .shell
            .update(cx, |shell, _| {
                shell
                    .live()
                    .and_then(|model| model.accounts.iter().find(|row| row.key == key).cloned())
            })
            .ok()
            .flatten()
        else {
            return;
        };
        let dialog = cx.new(|cx| {
            let weak: WeakEntity<SkinDialog> = cx.weak_entity();
            let wiring = wiring.clone();
            SkinDialog::new(
                Rc::new(move |intent, _, cx| save_skin(&wiring, weak.clone(), intent, cx)),
                row.key.clone(),
                row.name.clone(),
                row.skin.as_ref(),
                window,
                cx,
            )
        });
        SkinDialog::open(dialog, window, cx);
    });
}

fn save_skin(wiring: &Wiring, dialog: WeakEntity<SkinDialog>, intent: SkinIntent, cx: &mut App) {
    let service = wiring.backend.service.clone();
    let work = wiring
        .backend
        .spawn(async move { service.set_account_skin(&intent.key, intent.choice).await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match work.await {
            Ok(result) => result,
            Err(error) => Err(ServiceError::Remote(error.to_string())),
        };
        match result {
            Ok(()) => {
                let _ = dialog.update(cx, |dialog, cx| dialog.saved(Ok(()), cx));
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.toast(Toast::success("皮肤已保存，下次启动时生效"), cx)
                });
                cx.update(|cx| reload(&wiring, Reload::Lists, cx));
            }
            Err(error) => {
                let _ = dialog.update(cx, |dialog, cx| {
                    dialog.saved(Err(account_failure(&error)), cx)
                });
            }
        }
    })
    .detach();
}
