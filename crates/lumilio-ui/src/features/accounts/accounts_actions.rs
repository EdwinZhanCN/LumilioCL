use gpui::{AnyWindowHandle, Context, EventEmitter};
use lumilio_core::{CancellationToken, ServiceError};

use super::look::{AccountViewer, Faces};
use super::page::{AccountsInputs, AccountsView};
use crate::live::{LiveHandler, account_failure};
use crate::runtime::{self, Worker};
use crate::toast::{self, Toast};
use crate::{platform, tr};

/// What Accounts tells the application and the shell.
#[derive(Clone, Debug, PartialEq)]
pub enum AccountsEvent {
    /// An account's look changed where the lists show it: they are read again.
    ListsChanged,
    /// A face arrived: the corner chip shows it too.
    FacesChanged,
    /// An account was added for a launch that waited for one: it goes on.
    Continue(AnyWindowHandle),
}

/// The Accounts page: the accounts it lists, which one the detail shows and
/// its look, and what can be done to an account.
pub struct Accounts {
    pub(super) worker: Option<Worker>,
    /// What the page shows of the rest of the launcher.
    pub(super) inputs: AccountsInputs,
    pub(super) view: AccountsView,
    /// Each account's skin face, by key.
    pub(super) faces: Faces,
    /// The look of the account the detail shows, while the page is shown.
    pub(super) viewer: Option<AccountViewer>,
    /// Counts the details made, so answers for an older one are dropped.
    pub(super) revision: u64,
    /// Stops the Microsoft sign-in that is waiting for the browser.
    pub(super) sign_in_cancel: Option<CancellationToken>,
    /// Where the page sends what other features answer.
    pub(super) handler: Option<LiveHandler>,
}

impl EventEmitter<AccountsEvent> for Accounts {}

impl Accounts {
    #[must_use]
    pub fn new(worker: Option<Worker>) -> Self {
        Self {
            worker,
            inputs: AccountsInputs::default(),
            view: AccountsView::default(),
            faces: Faces::default(),
            viewer: None,
            revision: 0,
            sign_in_cancel: None,
            handler: None,
        }
    }

    /// Gives the feature the launcher to ask (the application does, once).
    pub fn set_worker(&mut self, worker: Worker) {
        self.worker = Some(worker);
    }

    /// Signs a Microsoft or third-party account in again now.
    pub fn refresh(&mut self, key: String, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        runtime::job(
            worker,
            cx,
            None::<AccountsEvent>,
            None,
            async move { service.refresh_account(&key).await },
            |_, result, cx| {
                let toast = match result {
                    Ok(()) => Toast::success(tr!("account-refreshed")),
                    Err(error) => {
                        let (message, technical) = account_failure(&error);
                        Toast::error(message).technical(technical)
                    }
                };
                toast::post(toast, cx);
            },
        );
    }

    /// Forgets an account (by key); games and worlds stay. Core publishes
    /// the shorter list.
    pub fn remove(&mut self, key: String, cx: &mut Context<Self>) {
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        let removed = key.clone();
        runtime::job(
            worker,
            cx,
            None::<AccountsEvent>,
            None,
            async move { service.remove_account(&key).await },
            move |_, result, cx| {
                toast::post(
                    toast::outcome(
                        result,
                        |()| tr!("account-removed", name = removed.as_str()),
                        tr!("account-remove-failed").to_owned(),
                    ),
                    cx,
                );
            },
        );
    }

    /// Opens where a third-party account changes its look: the homepage its
    /// server publishes (LittleSkin's site as it is), never anything but a
    /// web address.
    pub fn open_skin_site(&mut self, address: String, cx: &mut Context<Self>) {
        if address == "https://littleskin.cn/" {
            platform::open_address(&address, cx);
            return;
        }
        let Some(worker) = &self.worker else {
            return;
        };
        let service = worker.service().clone();
        let work = worker.spawn(async move {
            let server = service.locate_auth_server(&address).await?;
            Ok::<_, ServiceError>(server.links.get("homepage").cloned().unwrap_or(address))
        });
        cx.spawn(async move |_, cx| {
            if let Some(Ok(address)) = runtime::answer(work, cx).await
                && (address.starts_with("https://") || address.starts_with("http://"))
            {
                cx.update(|cx| platform::open_address(&address, cx));
            }
        })
        .detach();
    }
}
