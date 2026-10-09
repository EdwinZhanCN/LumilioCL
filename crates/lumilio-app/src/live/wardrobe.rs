//! Appearance work is bound to the initiating detail's revision. Closing it
//! cancels delayed reads; an accepted write is never retried automatically.
use super::{Reload, Wiring, jobs::reload};
use gpui_kit::App;
use lumilio_core::{AccountKind, AppearanceChange, ServiceError};
use lumilio_ui::{
    live::account_failure, platform, skin_dialog::SkinIntent, wardrobe::WardrobeAction,
};

struct RunOutcome {
    online: bool,
    partial: Option<(WardrobeAction, ServiceError)>,
}

pub(super) fn load_look(wiring: &Wiring, key: String, revision: u64, cx: &mut App) {
    super::accounts::load_look_only(wiring, key, revision, cx);
}

pub(super) fn list(wiring: &Wiring, key: String, revision: u64, cx: &mut App) {
    list_with_profile(wiring, key, revision, true, cx);
}

fn list_with_profile(
    wiring: &Wiring,
    key: String,
    revision: u64,
    include_profile: bool,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let target = key.clone();
    let work = wiring.backend.spawn(async move {
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
    let shell = wiring.shell.clone();
    let service = wiring.backend.service.clone();
    let backend = wiring.backend.clone();
    cx.spawn(async move |cx| {
        let (library, profile, warning) = match work.await {
            Ok(answer) => answer,
            Err(error) => (
                Err(account_failure(&ServiceError::Remote(error.to_string()))),
                None,
                None,
            ),
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
        let _ = shell.update(cx, |shell, cx| {
            shell.wardrobe_listed(&key, revision, library, profile, warning, cx)
        });
        // The tiles' pictures come after the list, so the list never waits
        // for cape downloads.
        let pictures = backend.spawn(async move {
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
            let _ = shell.update(cx, |shell, cx| {
                shell.wardrobe_pictures(&key, revision, skins, capes, cx);
                if let Some(worn) = worn {
                    shell.wardrobe_worn(&key, revision, worn, cx);
                }
            });
        }
    })
    .detach();
}

pub(super) fn run(
    wiring: &Wiring,
    key: String,
    revision: u64,
    action: WardrobeAction,
    cx: &mut App,
) {
    let Some((current, cancel)) = wiring
        .shell
        .update(cx, |shell, _| shell.account_detail(&key))
        .ok()
        .flatten()
    else {
        return;
    };
    if current != revision {
        return;
    }
    let service = wiring.backend.service.clone();
    let target = key.clone();
    let operation = action.clone();
    let work = wiring.backend.spawn(async move {
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
                        .change_account_appearance(&target, AppearanceChange::Cape(cape.clone()))
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
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = work
            .await
            .unwrap_or_else(|e| Err(ServiceError::Remote(e.to_string())));
        let online = result.as_ref().is_ok_and(|outcome| outcome.online);
        let success = result.is_ok();
        let refresh = matches!(action, WardrobeAction::Reload);
        let offline_wear = matches!(
            action,
            WardrobeAction::Wear(_) | WardrobeAction::WearDraft { .. }
        ) && success
            && !online;
        match result {
            Ok(RunOutcome {
                partial: Some((remaining, error)),
                ..
            }) => {
                let failure = account_failure(&error);
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell
                        .wardrobe_partially_finished(&key, revision, action, remaining, failure, cx)
                });
            }
            Ok(_) => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.wardrobe_finished(&key, revision, action, Ok(()), cx)
                });
            }
            Err(error) => {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.wardrobe_finished(
                        &key,
                        revision,
                        action,
                        Err(account_failure(&error)),
                        cx,
                    )
                });
            }
        }
        if !success {
            return;
        }
        cx.update(|cx| {
            // Library edits don't replace the account form or its unsaved fields.
            if !online && !refresh {
                list_with_profile(&wiring, key.clone(), revision, false, cx);
            }
            if offline_wear {
                reload(&wiring, Reload::Lists, cx);
            }
        });
        if online {
            let service = wiring.backend.service.clone();
            let target = key.clone();
            let pending = wiring
                .shell
                .update(cx, |shell, cx| {
                    shell.pending_library_skin(&key, revision, cx)
                })
                .ok()
                .flatten();
            let confirm = wiring.backend.spawn(async move {
                let profile = service
                    .confirm_account_appearance(&target, cancel.clone())
                    .await?;
                tokio::select! {
                    () = cancel.cancelled() => Err(ServiceError::Cancelled),
                    result = verified(&service, profile, pending) => result,
                }
            });
            let answer = confirm
                .await
                .unwrap_or_else(|e| Err(ServiceError::Remote(e.to_string())));
            if matches!(answer, Err(ServiceError::Cancelled)) {
                return;
            }
            let (profile, matches) = match answer {
                Ok((profile, matches)) => (Ok(profile), matches),
                Err(error) => (Err(account_failure(&error)), None),
            };
            let _ = wiring.shell.update(cx, |shell, cx| {
                shell.wardrobe_confirmed(&key, revision, profile, matches, cx)
            });
            cx.update(|cx| {
                list(&wiring, key.clone(), revision, cx);
                load_look(&wiring, key, revision, cx);
            });
        } else if refresh {
            // Explicit refresh can confirm a profile whose propagation took longer.
            let service = wiring.backend.service.clone();
            let target = key.clone();
            let pending = wiring
                .shell
                .update(cx, |shell, cx| {
                    shell.pending_library_skin(&key, revision, cx)
                })
                .ok()
                .flatten();
            let work = wiring.backend.spawn(async move {
                let profile = service.refresh_account_profile(&target).await?;
                verified(&service, profile, pending).await
            });
            let answer = work
                .await
                .unwrap_or_else(|error| Err(ServiceError::Remote(error.to_string())));
            let refresh_ok = answer.is_ok();
            let (profile, matches) = match answer {
                Ok((profile, matches)) => (Ok(profile), matches),
                Err(error) => (Err(account_failure(&error)), None),
            };
            let _ = wiring.shell.update(cx, |shell, cx| {
                shell.wardrobe_confirmed(&key, revision, profile, matches, cx)
            });
            cx.update(|cx| {
                list_with_profile(&wiring, key.clone(), revision, refresh_ok, cx);
                load_look(&wiring, key, revision, cx);
            });
        }
    })
    .detach();
}

async fn verified(
    service: &crate::backend::Service,
    profile: lumilio_core::MojangProfile,
    pending: Option<String>,
) -> Result<(lumilio_core::MojangProfile, Option<bool>), ServiceError> {
    let matches = match pending {
        Some(id) => Some(service.profile_wears_library_skin(&profile, id).await?),
        None => None,
    };
    Ok((profile, matches))
}

pub(super) fn save_offline(wiring: &Wiring, revision: u64, intent: SkinIntent, cx: &mut App) {
    let key = intent.key;
    let service = wiring.backend.service.clone();
    let target = key.clone();
    let work = wiring
        .backend
        .spawn(async move { service.set_account_skin(&target, intent.choice).await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = work
            .await
            .unwrap_or_else(|e| Err(ServiceError::Remote(e.to_string())));
        let success = result.is_ok();
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.inline_skin_saved(&key, revision, result.map_err(|e| account_failure(&e)), cx)
        });
        if success {
            cx.update(|cx| {
                load_look(&wiring, key, revision, cx);
                reload(&wiring, Reload::Lists, cx);
            });
        }
    })
    .detach();
}

pub(super) fn open_site(wiring: &Wiring, address: String, cx: &mut App) {
    if address == "https://littleskin.cn/" {
        platform::open_address(&address, cx);
        return;
    }
    let service = wiring.backend.service.clone();
    let work = wiring.backend.spawn(async move {
        let server = service.locate_auth_server(&address).await?;
        Ok::<_, ServiceError>(server.links.get("homepage").cloned().unwrap_or(address))
    });
    cx.spawn(async move |cx| {
        if let Ok(Ok(address)) = work.await
            && reqwest::Url::parse(&address)
                .is_ok_and(|url| matches!(url.scheme(), "https" | "http"))
        {
            cx.update(|cx| platform::open_address(&address, cx));
        }
    })
    .detach();
}
