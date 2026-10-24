//! A game's content beyond the list itself: a project's versions and
//! changelogs for the switch-version dialog, and the files a pack is still
//! waiting for.

use gpui_kit::{App, AppContext as _, WeakEntity, Window};
use lumilio_ui::instance_detail::{InstanceDetailView, InstanceIntent};
use lumilio_ui::tr;

use super::Wiring;
use super::instance_write::write_instance;
use super::jobs::answer;

/// Reads a project's versions for the open switch-version dialog, then the
/// chosen version's changelog when its source gives them one at a time.
pub(super) fn load_versions(
    wiring: &Wiring,
    view: WeakEntity<InstanceDetailView>,
    source: String,
    project: String,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let (from, wanted) = (source, project.clone());
    let handle = wiring
        .backend
        .spawn(async move { service.project_versions(&from, &wanted).await });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let missing = view
            .update(cx, |view, cx| view.versions_arrived(&project, result, cx))
            .ok()
            .flatten();
        if let Some((source, project, version)) = missing {
            cx.update(|cx| load_changelog(&wiring, view, source, project, version, cx));
        }
    })
    .detach();
}

/// Reads one version's changelog for the open switch-version dialog.
pub(super) fn load_changelog(
    wiring: &Wiring,
    view: WeakEntity<InstanceDetailView>,
    source: String,
    project: String,
    version: String,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let (from, wanted, which) = (source, project.clone(), version.clone());
    let handle = wiring.backend.spawn(async move {
        service
            .version_changelog(&from, &wanted, &which)
            .await
            .map_err(|error| error.to_string())
    });
    cx.spawn(async move |cx| {
        let result = handle.await.unwrap_or_else(|error| Err(error.to_string()));
        let _ = view.update(cx, |view, cx| {
            view.changelog_arrived(&project, &version, result, cx)
        });
    })
    .detach();
}

/// Reads the files the game waits for (a pack's files the launcher could
/// not download) and hands them to the view; a failure shows none.
pub(super) fn load_pending(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.pending_downloads(&id).await.unwrap_or_default() });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(pending) = answer(&wiring, handle, cx).await else {
            return;
        };
        let _ = view.update(cx, |view, cx| view.set_pending(pending, cx));
    })
    .detach();
}

/// Asks for the files the person downloaded and adds those the game waits
/// for. Cancelling the picker does nothing.
pub(super) fn pick_pending_files(
    wiring: &Wiring,
    id: &str,
    view: WeakEntity<InstanceDetailView>,
    window: &mut Window,
    cx: &mut App,
) {
    let picked = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
        files: true,
        directories: false,
        multiple: true,
        prompt: Some(tr!("instance-pending-add").into()),
    });
    let wiring = wiring.clone();
    let id = id.to_owned();
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(files))) = picked.await else {
            return;
        };
        let _ = cx.update_window(handle, |_, window, cx| {
            write_instance(
                &wiring,
                id,
                view,
                InstanceIntent::PendingFilesChosen(files),
                window,
                cx,
            );
        });
    })
    .detach();
}
