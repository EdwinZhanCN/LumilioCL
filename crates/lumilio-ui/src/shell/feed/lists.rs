use gpui::App;
use lumilio_core::unix_now;

use super::{ACTIVITY_LIMIT, Feed};
use crate::live::{CollectionRow, account_rows, activity_rows, library_cards};
use crate::runtime::answer;

/// Reads again what the game installs go into already has of what Discover
/// lists, if that may have changed.
pub(super) fn refresh_installed(feed: &Feed, cx: &mut App) {
    if let Ok(discover) = feed
        .shell
        .read_with(cx, |shell, _| shell.discover().clone())
    {
        discover.update(cx, |discover, cx| discover.refresh_installed(cx));
    }
}

/// What the game installs go into has changed outside Discover: it reads that
/// again.
pub(super) fn installed_changed(feed: &Feed, cx: &mut App) {
    if let Ok(discover) = feed
        .shell
        .read_with(cx, |shell, _| shell.discover().clone())
    {
        discover.update(cx, |discover, cx| discover.invalidate_installed(cx));
    }
}

/// Reloads the library, Activity, accounts and game settings from the
/// service. Home and what follows the current game are kept up to date by
/// [`super::context::follow`].
pub(in crate::shell) fn reload(feed: &Feed, cx: &mut App) {
    let seq = {
        let mut state = feed.state.borrow_mut();
        state.reload_seq += 1;
        state.reload_seq
    };
    let service = feed.worker.service().clone();
    let handle = feed.worker.spawn(async move {
        let library = service.library().await;
        (
            library.instances,
            library.collections,
            service.activity(ACTIVITY_LIMIT),
            service.settings().await,
            async {
                Ok::<_, lumilio_core::ServiceError>((
                    service.settings_presets().await?,
                    service.all_game_settings().await?,
                ))
            }
            .await,
        )
    });
    let feed = feed.clone();
    cx.spawn(async move |cx| {
        let Some((records, collections, activity, settings, presets)) = answer(handle, cx).await
        else {
            return;
        };
        if feed.state.borrow().reload_seq != seq {
            return;
        }
        let now = unix_now();
        let cards = library_cards(&records, now);
        let rows = activity_rows(&activity, now);
        let traffic = activity.traffic;
        let accounts = account_rows(&settings);
        let _ = feed.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    match presets {
                        Ok((presets, instances)) => {
                            model.settings_presets = presets;
                            model.game_settings = instances;
                            model.presets_error = None;
                        }
                        Err(error) => model.presets_error = Some(error.to_string()),
                    }
                    model.set_library(cards);
                    model.collections = collections
                        .into_iter()
                        .map(|collection| CollectionRow {
                            name: collection.name,
                            members: collection.members,
                        })
                        .collect();
                    let now_ms = unix_millis();
                    model.set_activity(rows, now_ms);
                    model.set_traffic(traffic, now_ms);
                    model.set_accounts(accounts);
                },
                cx,
            );
        });
    })
    .detach();
}

/// Milliseconds since the Unix epoch, for working out speeds.
fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}
