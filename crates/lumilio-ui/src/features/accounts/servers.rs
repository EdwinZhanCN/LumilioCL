//! The list of authentication servers: what is there, adding one by its
//! address and removing one with the accounts signed in on it.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyWindowHandle, Context, WeakEntity, Window};
use lumilio_core::ServiceError;
use tokio::task::JoinHandle;

use super::accounts_actions::Accounts;
use crate::auth_servers::{ServerIntent, ServerRow, ServersDialog};
use crate::live::account_failure;
use crate::runtime::{self, Service};
use crate::toast::{self, Toast};
use crate::tr;

/// The authentication servers as the list shows them: LittleSkin, then the
/// added ones, each with how many accounts are signed in on it.
async fn server_rows(service: &Service) -> Vec<ServerRow> {
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

impl Accounts {
    /// Opens the list of authentication servers, once they are read.
    pub fn manage_servers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let shown_in = window.window_handle();
        let service = worker.service().clone();
        let rows = worker.spawn(async move { server_rows(&service).await });
        cx.spawn(async move |this, cx| {
            let Some(rows) = runtime::answer(rows, cx).await else {
                return;
            };
            let _ = cx.update_window(shown_in, |_, window, cx| {
                let dialog = cx.new(|cx| {
                    let dialog: WeakEntity<ServersDialog> = cx.weak_entity();
                    ServersDialog::new(
                        Rc::new(move |intent, window, cx| {
                            let dialog = dialog.clone();
                            let shown_in = window.window_handle();
                            let _ = this.update(cx, |accounts, cx| {
                                accounts.server_intent(dialog, shown_in, intent, cx);
                            });
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
    }

    fn server_intent(
        &mut self,
        dialog: WeakEntity<ServersDialog>,
        shown_in: AnyWindowHandle,
        intent: ServerIntent,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        match intent {
            ServerIntent::Locate(address) => {
                let work = worker.spawn(async move { service.locate_auth_server(&address).await });
                cx.spawn(async move |_, cx| {
                    let result = match work.await {
                        Ok(result) => result,
                        Err(error) => Err(error.into()),
                    };
                    let _ = dialog.update(cx, |dialog, cx| {
                        dialog.located(result.map_err(|e| account_failure(&e)), cx)
                    });
                })
                .detach();
            }
            ServerIntent::Add(server) => {
                let name = server.display_name().to_owned();
                let work = worker.spawn(async move {
                    service.add_auth_server(&server).await?;
                    Ok::<_, ServiceError>(server_rows(&service).await)
                });
                finish_server_change(
                    dialog,
                    shown_in,
                    work,
                    tr!("account-server-added", name = name),
                    cx,
                );
            }
            ServerIntent::Remove(url) => {
                let work = worker.spawn(async move {
                    service.remove_auth_server(&url).await?;
                    Ok::<_, ServiceError>(server_rows(&service).await)
                });
                finish_server_change(
                    dialog,
                    shown_in,
                    work,
                    tr!("account-server-removed").to_owned(),
                    cx,
                );
            }
        }
    }
}

fn finish_server_change(
    dialog: WeakEntity<ServersDialog>,
    shown_in: AnyWindowHandle,
    work: JoinHandle<Result<Vec<ServerRow>, ServiceError>>,
    notice: String,
    cx: &mut Context<Accounts>,
) {
    cx.spawn(async move |_, cx| {
        let result = match work.await {
            Ok(result) => result,
            Err(error) => Err(error.into()),
        };
        match result {
            Ok(rows) => {
                let _ = cx.update_window(shown_in, |_, window, cx| {
                    let _ = dialog.update(cx, |dialog, cx| dialog.listed(rows, window, cx));
                    toast::post(Toast::success(notice), cx);
                });
            }
            Err(error) => {
                let _ = dialog.update(cx, |dialog, cx| dialog.failed(account_failure(&error), cx));
            }
        }
    })
    .detach();
}
