//! When a game starts: Play makes it the current game and presses Continue;
//! Continue asks what must be settled first (its folder, an account, its
//! Java), then the launch feature starts it.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, Entity, Window};

use super::LauncherShell;
use crate::features::launch::{Launch, Readiness};
use crate::home::{HomePresentation, RecoveryDetail, Subject};
use crate::instance_detail::TAB_DIAGNOSTICS;
use crate::java_conflict::{self, JavaConflict, JavaConflictAnswer};
use crate::live::PlaceTarget;
use crate::missing_folder::{self, MissingFolderAnswer};
use crate::toast::{self, Toast};
use crate::tr;

impl LauncherShell {
    pub fn launch(&self) -> &Entity<Launch> {
        &self.launch
    }

    /// Plays game `id` (into a world or onto a server, if `target` says)
    /// through Home's launch moment. It becomes the current game first
    /// (decision D4), so Continue, the chip and the next start agree on
    /// what is playing.
    pub fn play(
        &mut self,
        id: String,
        target: Option<PlaceTarget>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(test)]
        self.current_choices
            .played
            .push((id.clone(), target.clone()));
        self.launch.update(cx, |launch, _| match target {
            Some(target) => launch.aim(target),
            None => launch.forget_target(),
        });
        if !self.can_start() {
            self.forget_target(cx);
            self.toast(Toast::info(tr!("launch-already-running")), cx);
            return;
        }
        let saved = self.save_current(id.clone());
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let failed = match saved.await {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(error.to_string()),
                // The saving worker stopped without answering.
                Err(_) => Some(tr!("common-work-stopped").to_owned()),
            };
            if let Some(cause) = failed {
                let _ = this.update(cx, |shell, cx| {
                    shell.forget_target(cx);
                    shell.toast(
                        Toast::error(tr!("library-current-save-failed")).technical(cause),
                        cx,
                    );
                });
                return;
            }
            let _ = cx.update_window(window, |_, window, cx| {
                let _ = this.update(cx, |shell, cx| shell.play_saved(id, window, cx));
            });
        })
        .detach();
    }

    fn play_saved(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        // Another launch began while the choice was saved.
        if !self.can_start() {
            self.forget_target(cx);
            return;
        }
        let Some(card) = self
            .live()
            .and_then(|model| model.library.iter().find(|card| card.id == id))
            .cloned()
        else {
            self.forget_target(cx);
            return;
        };
        self.set_home(
            HomePresentation::Continue {
                subject: Subject {
                    id: Some(id),
                    title: card.name,
                    metadata: format!("{} · {}", card.meta, card.played),
                    world: card.world,
                },
            },
            cx,
        );
        self.request_continue(window, cx);
    }

    fn forget_target(&self, cx: &mut Context<Self>) {
        self.launch.update(cx, |launch, _| launch.forget_target());
    }

    /// Continue (or Recover: Home explains a failure only while its game is
    /// current, so recovering retries it) starts the current game, once its
    /// folder is there, there is an account and its Java can run it.
    pub(super) fn continue_launch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let readiness = self.launch.update(cx, |launch, cx| launch.readiness(cx));
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let readiness = readiness.await;
            let _ = cx.update_window(window, |_, window, cx| {
                let _ = this.update(cx, |shell, cx| shell.launch_when(readiness, window, cx));
            });
        })
        .detach();
    }

    fn launch_when(&mut self, readiness: Readiness, window: &mut Window, cx: &mut Context<Self>) {
        if readiness == Readiness::Ready {
            let shown_in = window.window_handle();
            self.launch
                .update(cx, |launch, cx| launch.start(shown_in, cx));
            return;
        }
        self.abort_launch(cx);
        match readiness {
            Readiness::Ready => {}
            Readiness::MissingFolder { id, name } => {
                self.ask_missing_folder(id, &name, window, cx);
            }
            // The chosen Java cannot run the game: ask once, then start.
            Readiness::Java(conflict) => self.ask_java(&conflict, window, cx),
            // No account: ask for one, then start where the person left off.
            Readiness::NoAccount => {
                self.accounts
                    .update(cx, |accounts, cx| accounts.add_offline(true, window, cx));
            }
        }
    }

    /// Asks about a Java that cannot run the game. An answer starts the
    /// launch again (with the automatic Java this once, or as chosen);
    /// cancelling drops the world or server the launch was headed for.
    fn ask_java(&mut self, conflict: &JavaConflict, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.launch.update(cx, |launch, _| launch.take_target());
        let shell = cx.weak_entity();
        java_conflict::ask(
            conflict,
            Rc::new(move |answer, window, cx| {
                let switch = answer == JavaConflictAnswer::Switch;
                let target = target.clone();
                let _ = shell.update(cx, |shell, cx| {
                    let again = shell
                        .launch
                        .update(cx, |launch, _| launch.answer_java(switch, target));
                    if again {
                        shell.request_continue(window, cx);
                    }
                });
            }),
            window,
            cx,
        );
    }

    /// Asks about a game whose folder was moved or deleted (plan
    /// game-folder-missing). Playing it with an empty folder makes one and
    /// starts again; viewing opens its page. The world or server the launch
    /// was headed for is dropped: an empty folder has no worlds.
    fn ask_missing_folder(
        &mut self,
        id: String,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.forget_target(cx);
        let shell = cx.weak_entity();
        missing_folder::ask(
            name,
            Rc::new(move |choice, window, cx| {
                let id = id.clone();
                match choice {
                    MissingFolderAnswer::ViewGame => {
                        let _ = shell.update(cx, |shell, cx| {
                            shell.open_game(id, None, window, cx);
                        });
                    }
                    MissingFolderAnswer::StartEmpty => {
                        let Ok(made) = shell.update(cx, |shell, cx| {
                            shell
                                .launch
                                .update(cx, |launch, cx| launch.new_game_folder(id, cx))
                        }) else {
                            return;
                        };
                        let (shell, window) = (shell.clone(), window.window_handle());
                        cx.spawn(async move |cx| {
                            if let Err(cause) = made.await {
                                let told =
                                    Toast::error(tr!("missing-folder-failed")).technical(cause);
                                cx.update(|cx| toast::post(told, cx));
                                return;
                            }
                            let _ = cx.update_window(window, |_, window, cx| {
                                let _ = shell
                                    .update(cx, |shell, cx| shell.request_continue(window, cx));
                            });
                        })
                        .detach();
                    }
                }
            }),
            window,
            cx,
        );
    }

    /// 技术详情 on Home's recovery: why a preparation step failed, or what a
    /// game that quit while starting printed, in a dialog to read and copy;
    /// for a game that crashed, quit silently or did not end cleanly, that
    /// game's Diagnostics tab, where its logs and crash reports are.
    pub fn technical_details(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let HomePresentation::Recovery {
            subject, detail, ..
        } = self.home().clone()
        else {
            return;
        };
        let technical = match &detail {
            RecoveryDetail::Failed { failure, .. } => failure.technical_detail(),
            _ => None,
        };
        match technical {
            Some(text) => toast::technical_dialog(text, window, cx),
            None => {
                if let Some(id) = subject.id {
                    self.open_game_on(id, TAB_DIAGNOSTICS, window, cx);
                }
            }
        }
    }
}
