use super::Wiring;
use gpui_kit::{App, WeakEntity};
use lumilio_core::GameLogSource;
use lumilio_ui::instance_detail::InstanceDetailView;
use lumilio_ui::platform;
use lumilio_ui::toast::Toast;

pub(super) fn read(
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    source: GameLogSource,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let requested = source.clone();
    let task = wiring
        .backend
        .spawn(async move { service.game_log(&id, requested).await });
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
    wiring: &Wiring,
    id: String,
    view: WeakEntity<InstanceDetailView>,
    request: u64,
    text: String,
    crash: bool,
    cx: &mut App,
) {
    let service = wiring.backend.service.clone();
    let task = wiring
        .backend
        .spawn(async move { service.analyze_game_log(&id, text, crash).await });
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
    wiring: &Wiring,
    id: String,
    source: GameLogSource,
    live: String,
    cx: &mut App,
) {
    let start = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| wiring.backend.service.layout().root().to_owned());
    let name = match &source {
        GameLogSource::Live => "live-output.log".to_owned(),
        GameLogSource::Latest => "latest.log".to_owned(),
        GameLogSource::File(name) | GameLogSource::Crash(name) => {
            name.strip_suffix(".gz").unwrap_or(name).to_owned()
        }
    };
    let chosen = platform::pick_save_path(cx, &start, &name);
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(path) = chosen.await else {
            return;
        };
        let service = wiring.backend.service.clone();
        let target = path.clone();
        let Ok(result) = wiring
            .backend
            .spawn(async move { service.export_game_log(&id, source, live, &target).await })
            .await
        else {
            return;
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            shell.toast(
                match result {
                    Ok(()) => {
                        Toast::success(format!("已保存到 {}（名字和路径已隐去）", path.display()))
                    }
                    Err(error) => Toast::error("没能导出日志").technical(error.to_string()),
                },
                cx,
            )
        });
    })
    .detach();
}
