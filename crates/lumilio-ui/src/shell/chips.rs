use super::LauncherShell;
use crate::live::LiveIntent;
use crate::navigation;
use crate::navigation::{AccountChoice, CurrentAccount, CurrentInstance, InstanceChoice};
use crate::route::Route;
use gpui::Context;
use std::rc::Rc;

impl LauncherShell {
    /// The trailing zone: the live install target.
    pub(super) fn current_instance(&self, cx: &mut Context<Self>) -> Option<CurrentInstance> {
        let empty = cx.listener(|shell, _: &(), window, cx| {
            shell.select_route(Route::Library, window, cx);
        });
        let on_empty: navigation::CloseHandler = Rc::new(move |window, cx| empty(&(), window, cx));
        let model = self.live.as_ref()?;
        let handler = self.live_handler.clone()?;
        let choices: Vec<InstanceChoice> = model
            .library
            .iter()
            .map(|card| InstanceChoice {
                id: card.id.clone(),
                name: card.name.clone().into(),
                meta: card.meta.clone().into(),
                seed: card.seed,
                loader: crate::live::cover_loader(card.loader),
                world: card.world,
            })
            .collect();
        let current = model
            .install_target
            .as_ref()
            .and_then(|id| choices.iter().find(|choice| &choice.id == id))
            .cloned();
        Some(CurrentInstance {
            current,
            choices,
            on_choose: Rc::new(move |id, window, cx| {
                handler(LiveIntent::InstallTarget(id.to_owned()), window, cx)
            }),
            on_empty,
        })
    }
    /// The trailing zone's account chip.
    pub(super) fn current_account(&self, cx: &mut Context<Self>) -> Option<CurrentAccount> {
        let model = self.live.as_ref().filter(|model| model.accounts_loaded)?;
        let handler = self.live_handler.clone()?;
        let manage = cx.listener(|shell, _: &(), window, cx| {
            if let Some(ix) = shell
                .live
                .as_ref()
                .and_then(|model| model.accounts.iter().position(|row| row.selected))
            {
                shell.view.apply(crate::kit::ViewIntent::Choose(
                    crate::pages::live::ACCOUNT_GROUP,
                    ix,
                ));
            }
            shell.select_route(Route::Accounts, window, cx);
        });
        let choices = model
            .accounts
            .iter()
            .map(|row| AccountChoice {
                key: row.key.clone(),
                name: row.name.clone().into(),
                detail: if row.needs_sign_in {
                    crate::tr!("nav-account-needs-sign-in", kind = row.kind_label())
                } else {
                    format!(
                        "{} · {}",
                        row.kind_label(),
                        row.uuid.split('-').next().unwrap_or_default()
                    )
                }
                .into(),
                selected: row.selected,
            })
            .collect();
        let choose = handler.clone();
        Some(CurrentAccount {
            choices,
            on_choose: Rc::new(move |name, window, cx| {
                choose(LiveIntent::SelectAccount(name.to_owned()), window, cx)
            }),
            on_manage: Rc::new(move |window, cx| manage(&(), window, cx)),
            on_add: Rc::new(move |window, cx| handler(LiveIntent::NewAccount, window, cx)),
        })
    }

    /// A launch that cannot start (no account yet) leaves the Home launch
    /// moment as if cancelled, so the dialog that follows has the page.
    pub fn abort_launch(&mut self, cx: &mut Context<Self>) {
        let home = std::mem::take(&mut self.home).cancel_launch();
        self.set_home(home, cx);
    }
}
