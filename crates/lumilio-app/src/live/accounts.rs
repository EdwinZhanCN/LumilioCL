use super::jobs::reload;
use super::{Reload, Wiring};
use gpui_kit::AppContext as _;
use gpui_kit::{App, WeakEntity, Window};
use lumilio_core::{CancellationToken, ServiceError};
use lumilio_ui::accounts::{AccountForm, AccountRequest};
use lumilio_ui::live::account_failure;
use lumilio_ui::microsoft_login::{SignInDialog, SignInIntent};
use lumilio_ui::platform;
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
