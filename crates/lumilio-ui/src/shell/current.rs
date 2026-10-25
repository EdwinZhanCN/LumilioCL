//! Choosing the current game: from the Library, the corner chip, a game's
//! page, a launch or a new game; and the current account, from the corner
//! chip or the Accounts page. Core saves the choice and publishes it, and
//! everything showing it follows (see `mirror`).

use gpui::Context;
use lumilio_core::ServiceError;
use tokio::sync::{mpsc, oneshot};

use super::LauncherShell;
use crate::runtime::{self, Worker};
use crate::toast::{self, Toast};
use crate::tr;

/// The answer to one choice of the current game.
pub type Saved = oneshot::Receiver<Result<(), ServiceError>>;

/// A game to make current, and where to tell how that went.
type Choice = (String, oneshot::Sender<Result<(), ServiceError>>);

/// Saves choices one after another, in the order they were made, so the
/// last choice is the one that stays.
#[derive(Default)]
pub(super) struct CurrentChoices {
    pub(super) worker: Option<Worker>,
    queue: Option<mpsc::UnboundedSender<Choice>>,
    /// Every game chosen, in order (tests read it).
    #[cfg(test)]
    pub(super) chosen: Vec<String>,
    /// Every account chosen, in order (tests read it).
    #[cfg(test)]
    pub(super) accounts_chosen: Vec<String>,
    /// Every game played, and where to, in order (tests read it).
    #[cfg(test)]
    pub(super) played: Vec<(String, Option<crate::live::PlaceTarget>)>,
}

impl CurrentChoices {
    fn queue(&mut self) -> Option<&mpsc::UnboundedSender<Choice>> {
        if self.queue.is_none() {
            let worker = self.worker.as_ref()?;
            let (queue, mut choices) = mpsc::unbounded_channel::<Choice>();
            let service = worker.service().clone();
            drop(worker.spawn(async move {
                while let Some((id, answer)) = choices.recv().await {
                    let _ = answer.send(service.set_current_instance(&id).await);
                }
            }));
            self.queue = Some(queue);
        }
        self.queue.as_ref()
    }
}

impl LauncherShell {
    /// Saves `id` as the current game after the choices before it. Without
    /// a launcher to ask, the answer never comes (its sender is dropped).
    pub(crate) fn save_current(&mut self, id: String) -> Saved {
        let (answer, saved) = oneshot::channel();
        if let Some(queue) = self.current_choices.queue() {
            let _ = queue.send((id, answer));
        }
        saved
    }

    /// Makes `id` the current game. A failed save is told and leaves it as
    /// it was.
    pub fn choose_current(&mut self, id: String, cx: &mut Context<Self>) {
        #[cfg(test)]
        self.current_choices.chosen.push(id.clone());
        if self.current_choices.worker.is_none() {
            return;
        }
        let saved = self.save_current(id);
        cx.spawn(async move |_, cx| {
            let toast = match saved.await {
                Ok(Ok(())) => return,
                Ok(Err(error)) => {
                    Toast::error(tr!("library-current-save-failed")).technical(error.to_string())
                }
                // The saving worker stopped without answering.
                Err(_) => Toast::error(tr!("library-current-save-failed"))
                    .technical(tr!("common-work-stopped")),
            };
            cx.update(|cx| toast::post(toast, cx));
        })
        .detach();
    }

    /// Makes the account `key` the one later launches use. A failed save is
    /// told and leaves it as it was.
    pub fn choose_account(&mut self, key: String, cx: &mut Context<Self>) {
        #[cfg(test)]
        self.current_choices.accounts_chosen.push(key.clone());
        let Some(worker) = &self.current_choices.worker else {
            return;
        };
        let service = worker.service().clone();
        let saved = worker.spawn(async move { service.select_account(&key).await });
        cx.spawn(async move |_, cx| {
            if let Some(Err(error)) = runtime::answer(saved, cx).await {
                let toast = Toast::error(tr!("account-switch-failed")).technical(error.to_string());
                cx.update(|cx| toast::post(toast, cx));
            }
        })
        .detach();
    }
}
