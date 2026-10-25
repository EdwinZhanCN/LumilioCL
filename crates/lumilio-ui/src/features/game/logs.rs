//! A game's logs on its page: reading one, explaining what it says, and
//! saving it where the person chooses.

use gpui::{App, WeakEntity};
use lumilio_core::GameLogSource;

use super::game_actions::GameWork;
use crate::instance_detail::InstanceDetailView;
use crate::toast::{self, Toast};
use crate::{platform, tr};

pub(super) fn read(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    source: GameLogSource,
    cx: &mut App,
) {
    let service = work.service().clone();
    let requested = source.clone();
    let task = work.spawn(async move { service.game_log(&id, requested).await });
    cx.spawn(async move |cx| {
        let result = match task.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| view.log_arrived(source, result, cx));
    })
    .detach();
}

pub(super) fn analyze(
    work: &GameWork,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    request: u64,
    text: String,
    crash: bool,
    cx: &mut App,
) {
    let service = work.service().clone();
    let task = work.spawn(async move { service.analyze_game_log(&id, text, crash).await });
    cx.spawn(async move |cx| {
        let result = match task.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };
        let _ = view.update(cx, |view, cx| {
            view.log_analysis_arrived(request, result, cx)
        });
    })
    .detach();
}

pub(super) fn export(
    work: &GameWork,
    id: String,
    source: GameLogSource,
    live: String,
    cx: &mut App,
) {
    let start = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| work.service().layout().root().to_owned());
    let name = match &source {
        GameLogSource::Live => "live-output.log".to_owned(),
        GameLogSource::Latest => "latest.log".to_owned(),
        GameLogSource::File(name) | GameLogSource::Crash(name) => {
            name.strip_suffix(".gz").unwrap_or(name).to_owned()
        }
    };
    let chosen = platform::pick_save_path(cx, &start, &name);
    let work = work.clone();
    cx.spawn(async move |cx| {
        let Some(path) = chosen.await else {
            return;
        };
        let service = work.service().clone();
        let target = path.clone();
        let Ok(result) = work
            .spawn(async move { service.export_game_log(&id, source, live, &target).await })
            .await
        else {
            return;
        };
        let told = match result {
            Ok(()) => Toast::success(tr!("logs-saved", path = path.display().to_string())),
            Err(error) => Toast::error(tr!("logs-export-failed")).technical(error.to_string()),
        };
        cx.update(|cx| toast::post(told, cx));
    })
    .detach();
}
