//! Adding an account: an offline one by name, or signing in with Microsoft
//! or an authentication server. Each opens its dialog; what the launcher
//! answers is told in the dialog, and a new account in a toast.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyWindowHandle, Context, WeakEntity, Window};
use lumilio_core::{ServiceError, ThirdPartySignIn};

use super::accounts_actions::{Accounts, AccountsEvent};
use crate::accounts::{AccountForm, AccountRequest};
use crate::live::account_failure;
use crate::microsoft_login::{SignInDialog, SignInIntent};
use crate::third_party_login::{ThirdPartyDialog, ThirdPartyIntent};
use crate::toast::{self, Toast};
use crate::{platform, runtime, tr};

impl Accounts {
    /// Opens the add-account dialog (IA `accounts.md`). With `then_launch`
    /// the game the person asked to start begins as soon as the account is
    /// saved. Deferred: the person may have asked from inside another view's
    /// update.
    pub fn add_offline(&mut self, then_launch: bool, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.inputs.accounts.is_empty();
        let this = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let shown_in = window.window_handle();
            let form = cx.new(|cx| {
                let form = cx.weak_entity();
                AccountForm::new(
                    Rc::new(move |request, _, cx| {
                        let form = form.clone();
                        let _ = this.update(cx, |accounts, cx| {
                            accounts.add_account(form, shown_in, request, then_launch, cx);
                        });
                    }),
                    first,
                    window,
                    cx,
                )
            });
            AccountForm::open(form, window, cx);
        });
    }

    fn add_account(
        &mut self,
        form: WeakEntity<AccountForm>,
        shown_in: AnyWindowHandle,
        request: AccountRequest,
        then_launch: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        let wanted = request.clone();
        let handle = worker.spawn(async move {
            service
                .add_account(&wanted.name, wanted.uuid.as_deref())
                .await
        });
        cx.spawn(async move |this, cx| {
            let result = match handle.await {
                Ok(result) => result,
                Err(error) => Err(error.into()),
            };
            if let Err(error) = result {
                let _ = form.update(cx, |form, cx| form.saved(Err(account_failure(&error)), cx));
                return;
            }
            let _ = form.update(cx, |form, cx| form.saved(Ok(()), cx));
            cx.update(|cx| {
                toast::post(
                    Toast::success(tr!("account-added", name = request.name)),
                    cx,
                );
            });
            if then_launch {
                let _ = this.update(cx, |_, cx| cx.emit(AccountsEvent::Continue(shown_in)));
            }
        })
        .detach();
    }

    /// Opens the Microsoft sign-in dialog. Nothing is asked of Microsoft until
    /// the person presses the button in it.
    pub fn sign_in_microsoft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let dialog = cx.new(|cx| {
                let dialog: WeakEntity<SignInDialog> = cx.weak_entity();
                SignInDialog::new(Rc::new(move |intent, _, cx| {
                    let dialog = dialog.clone();
                    let _ = this.update(cx, |accounts, cx| match intent {
                        SignInIntent::Start => accounts.start_microsoft_sign_in(dialog, cx),
                        SignInIntent::Cancel => {
                            if let Some(cancel) = accounts.sign_in_cancel.take() {
                                cancel.cancel();
                            }
                        }
                    });
                }))
            });
            SignInDialog::open(dialog, window, cx);
        });
    }

    /// Asks for a code, shows it (and opens the page), waits for the browser,
    /// and then adds the account. Failures are told in the dialog, in words.
    fn start_microsoft_sign_in(
        &mut self,
        dialog: WeakEntity<SignInDialog>,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = &self.worker else {
            return;
        };
        let cancel = lumilio_core::CancellationToken::new();
        self.sign_in_cancel = Some(cancel.clone());
        let (code_tx, mut code_rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();
        let service = worker.service().clone();
        let handle = worker.spawn(async move {
            service
                .microsoft_sign_in(
                    move |code| {
                        let _ =
                            code_tx.send((code.user_code.clone(), code.verification_uri.clone()));
                    },
                    cancel,
                )
                .await
        });
        let shown = dialog.clone();
        cx.spawn(async move |_, cx| {
            if let Some((code, address)) = code_rx.recv().await
                && shown
                    .update(cx, |dialog, cx| dialog.code_arrived(&code, &address, cx))
                    .unwrap_or(false)
            {
                cx.update(|cx| platform::open_address(&address, cx));
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let result = match handle.await {
                Ok(result) => result,
                Err(error) => Err(error.into()),
            };
            let _ = this.update(cx, |accounts, _| accounts.sign_in_cancel = None);
            match result {
                Ok((_, name)) => {
                    let _ = dialog.update(cx, |dialog, cx| dialog.signed_in(cx));
                    cx.update(|cx| {
                        toast::post(Toast::success(tr!("account-signed-in", name = name)), cx);
                    });
                }
                // The person cancelled: the dialog is already gone.
                Err(ServiceError::Auth(lumilio_core::AuthError::Cancelled)) => {}
                Err(error) => {
                    let _ =
                        dialog.update(cx, |dialog, cx| dialog.failed(account_failure(&error), cx));
                }
            }
        })
        .detach();
    }

    /// Opens the sign-in dialog for authentication servers. The servers are
    /// read first (off the interface thread), then the dialog opens.
    pub fn sign_in_third_party(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let shown_in = window.window_handle();
        let service = worker.service().clone();
        let servers = worker.spawn(async move { service.auth_servers().await });
        cx.spawn(async move |this, cx| {
            let Some(servers) = runtime::answer(servers, cx).await else {
                return;
            };
            let _ = cx.update_window(shown_in, |_, window, cx| {
                let dialog = cx.new(|cx| {
                    let dialog: WeakEntity<ThirdPartyDialog> = cx.weak_entity();
                    ThirdPartyDialog::new(
                        Rc::new(move |intent, window, cx| {
                            let dialog = dialog.clone();
                            let _ = this.update(cx, |accounts, cx| {
                                accounts.third_party_intent(dialog, intent, window, cx);
                            });
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
        &mut self,
        dialog: WeakEntity<ThirdPartyDialog>,
        intent: ThirdPartyIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        let handle = match intent {
            ThirdPartyIntent::SignIn {
                server,
                login,
                password,
            } => worker.spawn(async move {
                service
                    .third_party_sign_in(&server, &login, &password)
                    .await
            }),
            ThirdPartyIntent::Choose { pending, character } => worker.spawn(async move {
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
                self.manage_servers(window, cx);
                return;
            }
        };
        cx.spawn(async move |_, cx| {
            let result = match handle.await {
                Ok(result) => result,
                Err(error) => Err(error.into()),
            };
            match result {
                Ok(ThirdPartySignIn::Done { name, .. }) => {
                    let _ = dialog.update(cx, |dialog, cx| dialog.signed_in(cx));
                    cx.update(|cx| {
                        toast::post(Toast::success(tr!("account-signed-in", name = name)), cx);
                    });
                }
                Ok(ThirdPartySignIn::Choose {
                    pending,
                    characters,
                }) => {
                    let _ = dialog.update(cx, |dialog, cx| dialog.choose(pending, characters, cx));
                }
                Err(error) => {
                    let _ =
                        dialog.update(cx, |dialog, cx| dialog.failed(account_failure(&error), cx));
                }
            }
        })
        .detach();
    }
}
