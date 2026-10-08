//! Appearance work is bound to the initiating detail's revision. Closing it
//! cancels delayed reads; an accepted write is never retried automatically.
use super::{Reload, Wiring, jobs::reload};
use gpui_kit::App;
use lumilio_core::{AccountKind, AppearanceChange, ServiceError};
use lumilio_ui::{
    live::account_failure, platform, skin_dialog::SkinIntent, wardrobe::WardrobeAction,
};

pub(super) fn load_look(wiring: &Wiring, key: String, revision: u64, cx: &mut App) {
    super::accounts::load_look_only(wiring, key, revision, cx);
}

pub(super) fn list(wiring: &Wiring, key: String, revision: u64, cx: &mut App) {
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
        let profile = if microsoft {
            Some(
                service
                    .account_profile(&target)
                    .await
                    .map_err(|e| account_failure(&e)),
            )
        } else {
            None
        };
        (library, profile)
    });
    let shell = wiring.shell.clone();
    cx.spawn(async move |cx| {
        let (library, profile) = match work.await {
            Ok(answer) => answer,
            Err(error) => (
                Err(account_failure(&ServiceError::Remote(error.to_string()))),
                None,
            ),
        };
        let _ = shell.update(cx, |shell, cx| {
            shell.wardrobe_listed(&key, revision, library, profile, cx)
        });
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
            WardrobeAction::Wear(id) => service.wear_library_skin(&target, id).await?.is_some(),
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
        };
        Ok::<_, ServiceError>(changed_online)
    });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = work
            .await
            .unwrap_or_else(|e| Err(ServiceError::Remote(e.to_string())));
        let online = result.as_ref().copied().unwrap_or(false);
        let success = result.is_ok();
        let refresh = matches!(action, WardrobeAction::Reload);
        let offline_wear = matches!(action, WardrobeAction::Wear(_)) && success && !online;
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.wardrobe_finished(
                &key,
                revision,
                action,
                result.map(|_| ()).map_err(|e| account_failure(&e)),
                cx,
            )
        });
        if !success {
            return;
        }
        cx.update(|cx| {
            // Library edits don't replace the account form or its unsaved fields.
            if !online {
                list(&wiring, key.clone(), revision, cx);
            }
            if refresh {
                load_look(&wiring, key.clone(), revision, cx);
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
            cx.update(|cx| load_look(&wiring, key, revision, cx));
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
                let profile = service.account_profile(&target).await?;
                verified(&service, profile, pending).await
            });
            if let Ok(Ok((profile, matches))) = work.await {
                let _ = wiring.shell.update(cx, |shell, cx| {
                    shell.wardrobe_confirmed(&key, revision, Ok(profile), matches, cx)
                });
            }
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
