//! Appearance work is bound to the initiating detail's revision. Closing it
//! cancels delayed reads; an accepted write is never retried automatically.

use gpui::Context;
use lumilio_core::{AccountKind, AppearanceChange, ServiceError};

use super::accounts_actions::{Accounts, AccountsEvent};
use crate::live::account_failure;
use crate::runtime::Service;
use crate::skin_dialog::SkinIntent;
use crate::wardrobe::WardrobeAction;

struct RunOutcome {
    online: bool,
    partial: Option<(WardrobeAction, ServiceError)>,
}

impl Accounts {
    /// Reads the skin library (and a Microsoft account's profile, with
    /// `include_profile`) for the wardrobe, then the tiles' pictures.
    pub(super) fn list_wardrobe(
        &mut self,
        key: String,
        revision: u64,
        include_profile: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let service = worker.service().clone();
        let target = key.clone();
        let work = worker.spawn(async move {
            let microsoft = service
                .settings()
                .await
                .accounts
                .iter()
                .any(|entry| entry.key() == target && entry.kind == AccountKind::Microsoft);
            let library = service
                .skin_library()
                .await
                .map_err(|e| account_failure(&e));
            let (profile, warning) = if microsoft && include_profile {
                match service.account_profile_snapshot(&target).await {
                    Ok(snapshot) => (
                        Some(Ok(snapshot.profile)),
                        snapshot
                            .warning
                            .map(|error| account_failure(&ServiceError::Appearance(error))),
                    ),
                    Err(error) => (Some(Err(account_failure(&error))), None),
                }
            } else {
                (None, None)
            };
            (library, profile, warning)
        });
        let service = worker.service().clone();
        cx.spawn(async move |this, cx| {
            let (library, profile, warning) = match work.await {
                Ok(answer) => answer,
                Err(error) => (Err(account_failure(&ServiceError::from(error))), None, None),
            };
            let owned = profile
                .as_ref()
                .and_then(|profile| profile.as_ref().ok())
                .cloned();
            let library_ids: Vec<String> = library
                .as_ref()
                .ok()
                .map(|entries| entries.iter().map(|entry| entry.id.clone()).collect())
                .unwrap_or_default();
            let _ = this.update(cx, |accounts, cx| {
                accounts.wardrobe_listed(&key, revision, library, profile, warning, cx)
            });
            // The tiles' pictures come after the list, so the list never waits
            // for cape downloads.
            let pictures = worker.spawn(async move {
                let skins = service.library_looks().await.unwrap_or_default();
                let capes = if let Some(profile) = &owned {
                    Some(service.owned_cape_pictures(profile).await)
                } else {
                    None
                };
                let worn = if let Some(profile) = &owned {
                    let mut ids = std::collections::HashSet::new();
                    for id in library_ids {
                        if service
                            .profile_wears_library_skin(profile, id.clone())
                            .await
                            .unwrap_or(false)
                        {
                            ids.insert(id);
                        }
                    }
                    Some(ids)
                } else {
                    None
                };
                (skins, capes, worn)
            });
            if let Ok((skins, capes, worn)) = pictures.await {
                let _ = this.update(cx, |accounts, cx| {
                    if let Some(view) = accounts.wardrobe_at(&key, revision) {
                        view.update(cx, |view, cx| {
                            view.update_pictures(skins, capes, cx);
                            if let Some(worn) = worn {
                                view.set_worn(worn, cx);
                            }
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Does what the wardrobe asked, for the detail made at `revision`; a
    /// request from an older detail does nothing.
    pub(super) fn run_wardrobe(
        &mut self,
        key: String,
        revision: u64,
        action: WardrobeAction,
        cx: &mut Context<Self>,
    ) {
        let Some(cancel) = self
            .detail_at(&key, revision)
            .map(|view| view.cancel.clone())
        else {
            return;
        };
        let Some(worker) = self.worker.clone() else {
            return;
        };
        let service = worker.service().clone();
        let target = key.clone();
        let operation = action.clone();
        let work = worker.spawn(async move {
            let mut partial = None;
            let changed_online = match operation {
                WardrobeAction::Reload => false,
                WardrobeAction::Import(paths) => {
                    for path in paths {
                        service.import_library_skin(path, None).await?;
                    }
                    false
                }
                WardrobeAction::SaveCurrent => {
                    service.save_account_skin(&target).await?;
                    false
                }
                WardrobeAction::Wear(id) => {
                    let applied = service.wear_library_skin(&target, id).await?;
                    partial = applied
                        .cape_failure
                        .map(|(cape, error)| (WardrobeAction::Cape(cape), error));
                    applied.update.is_some()
                }
                WardrobeAction::Default => {
                    service
                        .change_account_appearance(&target, AppearanceChange::DefaultSkin)
                        .await?;
                    true
                }
                WardrobeAction::Cape(id) => {
                    service
                        .change_account_appearance(&target, AppearanceChange::Cape(id))
                        .await?;
                    true
                }
                WardrobeAction::Model(id, model) => {
                    service.set_library_skin_model(id, model).await?;
                    false
                }
                WardrobeAction::Reorder(ids) => {
                    service.reorder_library_skins(ids).await?;
                    false
                }
                WardrobeAction::Remove(id) => {
                    service.remove_library_skin(id).await?;
                    false
                }
                WardrobeAction::Rename(id, name) => {
                    service.rename_library_skin(id, name).await?;
                    false
                }
                WardrobeAction::Replace(id, path) => {
                    service.replace_library_skin(id, path).await?;
                    false
                }
                WardrobeAction::PairCape(id, cape) => {
                    service.set_library_skin_cape(id, cape).await?;
                    false
                }
                WardrobeAction::SaveLibraryDraft {
                    id,
                    model,
                    cape,
                    replacement,
                } => {
                    let id = match replacement {
                        Some(path) => service.replace_library_skin(id, path).await?.id,
                        None => id,
                    };
                    service.set_library_skin_model(id.clone(), model).await?;
                    service.set_library_skin_cape(id, cape).await?;
                    false
                }
                WardrobeAction::WearDraft {
                    id,
                    model,
                    cape,
                    replacement,
                } => {
                    let applied = service
                        .wear_library_skin_draft(&target, id, model, cape, replacement)
                        .await?;
                    partial = applied
                        .cape_failure
                        .map(|(cape, error)| (WardrobeAction::Cape(cape), error));
                    applied.update.is_some()
                }
                WardrobeAction::ApplyCurrentDraft {
                    model,
                    cape,
                    replacement,
                    model_changed,
                    cape_changed,
                } => {
                    let mut changed = false;
                    if let Some(path) = replacement {
                        service
                            .upload_account_skin_file(&target, path, model)
                            .await?;
                        changed = true;
                    } else if model_changed {
                        changed = service
                            .change_account_skin_model(&target, model)
                            .await?
                            .is_some();
                    }
                    if cape_changed {
                        let cape = match cape {
                            lumilio_core::PairedCape::Cape(id) => Some(id),
                            lumilio_core::PairedCape::Hidden => None,
                            lumilio_core::PairedCape::Keep => {
                                unreachable!("current cape has a concrete choice")
                            }
                        };
                        match service
                            .change_account_appearance(
                                &target,
                                AppearanceChange::Cape(cape.clone()),
                            )
                            .await
                        {
                            Ok(_) => changed = true,
                            Err(error) if changed => {
                                partial = Some((WardrobeAction::Cape(cape), error))
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    changed
                }
            };
            Ok::<_, ServiceError>(RunOutcome {
                online: changed_online,
                partial,
            })
        });
        cx.spawn(async move |this, cx| {
            let result = work.await.unwrap_or_else(|e| Err(e.into()));
            let online = result.as_ref().is_ok_and(|outcome| outcome.online);
            let success = result.is_ok();
            let refresh = matches!(action, WardrobeAction::Reload);
            let offline_wear = matches!(
                action,
                WardrobeAction::Wear(_) | WardrobeAction::WearDraft { .. }
            ) && success
                && !online;
            let _ = this.update(cx, |accounts, cx| match result {
                Ok(RunOutcome {
                    partial: Some((remaining, error)),
                    ..
                }) => {
                    let failure = account_failure(&error);
                    if let Some(view) = accounts.wardrobe_at(&key, revision) {
                        view.update(cx, |view, cx| {
                            view.partially_finished(action, remaining, failure, cx)
                        });
                    }
                }
                Ok(_) => accounts.wardrobe_finished(&key, revision, action, Ok(()), cx),
                Err(error) => accounts.wardrobe_finished(
                    &key,
                    revision,
                    action,
                    Err(account_failure(&error)),
                    cx,
                ),
            });
            if !success {
                return;
            }
            let _ = this.update(cx, |accounts, cx| {
                // Library edits don't replace the account form or its unsaved fields.
                if !online && !refresh {
                    accounts.list_wardrobe(key.clone(), revision, false, cx);
                }
                if offline_wear {
                    cx.emit(AccountsEvent::ListsChanged);
                }
            });
            if !online && !refresh {
                return;
            }
            let pending = this
                .update(cx, |accounts, cx| {
                    accounts
                        .wardrobe_at(&key, revision)
                        .and_then(|view| view.read(cx).pending_skin())
                })
                .ok()
                .flatten();
            let service = worker.service().clone();
            let target = key.clone();
            let confirm = if online {
                worker.spawn(async move {
                    let profile = service
                        .confirm_account_appearance(&target, cancel.clone())
                        .await?;
                    tokio::select! {
                        () = cancel.cancelled() => Err(ServiceError::Cancelled),
                        result = verified(&service, profile, pending) => result,
                    }
                })
            } else {
                // Explicit refresh can confirm a profile whose propagation took longer.
                worker.spawn(async move {
                    let profile = service.refresh_account_profile(&target).await?;
                    verified(&service, profile, pending).await
                })
            };
            let answer = confirm.await.unwrap_or_else(|e| Err(e.into()));
            if online && matches!(answer, Err(ServiceError::Cancelled)) {
                return;
            }
            // After a refresh that failed, the list does not read the profile again.
            let with_profile = online || answer.is_ok();
            let (profile, matches) = match answer {
                Ok((profile, matches)) => (Ok(profile), matches),
                Err(error) => (Err(account_failure(&error)), None),
            };
            let _ = this.update(cx, |accounts, cx| {
                if let Some(view) = accounts.wardrobe_at(&key, revision) {
                    view.update(cx, |view, cx| view.confirmed(profile, matches, cx));
                }
                accounts.list_wardrobe(key.clone(), revision, with_profile, cx);
                accounts.load_look(key, revision, cx);
            });
        })
        .detach();
    }

    /// Saves the skin choice of an offline account from the detail's inline
    /// form, for the detail made at `revision`.
    pub(super) fn save_inline_skin(
        &mut self,
        revision: u64,
        intent: SkinIntent,
        cx: &mut Context<Self>,
    ) {
        let Some(worker) = &self.worker else {
            return;
        };
        let key = intent.key;
        let service = worker.service().clone();
        let target = key.clone();
        let work =
            worker.spawn(async move { service.set_account_skin(&target, intent.choice).await });
        cx.spawn(async move |this, cx| {
            let result = work.await.unwrap_or_else(|e| Err(e.into()));
            let success = result.is_ok();
            let _ = this.update(cx, |accounts, cx| {
                accounts.inline_skin_saved(
                    &key,
                    revision,
                    result.map_err(|e| account_failure(&e)),
                    cx,
                );
                if success {
                    accounts.load_look(key, revision, cx);
                    cx.emit(AccountsEvent::ListsChanged);
                }
            });
        })
        .detach();
    }
}

async fn verified(
    service: &Service,
    profile: lumilio_core::MojangProfile,
    pending: Option<String>,
) -> Result<(lumilio_core::MojangProfile, Option<bool>), ServiceError> {
    let matches = match pending {
        Some(id) => Some(service.profile_wears_library_skin(&profile, id).await?),
        None => None,
    };
    Ok((profile, matches))
}
