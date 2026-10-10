use super::super::live::LiveCtx;
use super::rows::{row, send};
use crate::kit;
use crate::live::{LiveIntent, SettingsView, UpdateStatus, UpdateUnavailability};
use crate::tr;
use gpui::{AnyElement, IntoElement};

pub(super) fn about(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let handler = &ctx.handler;
    let log = view.data_dir.join("activity.jsonl");
    let update_status = update_status_text(&ctx.model.update_status);
    let update_detail = match &ctx.model.update_status {
        UpdateStatus::Failed { detail } => Some(detail.clone()),
        _ => None,
    };
    let update_status_control = update_detail
        .map(|detail| kit::technical("settings-update-technical", detail).into_any_element());
    let update_busy = matches!(
        &ctx.model.update_status,
        UpdateStatus::Checking | UpdateStatus::Downloading { .. }
    );
    let auto_update = view.auto_update_enabled;
    let rows = vec![
        // ia[settings]: 版本 | 关于 · 值 | 显示 LumilioCL 版本号
        row(
            "settings-version",
            tr!("settings-version"),
            None,
            format!("LumilioCL {}", env!("CARGO_PKG_VERSION")),
            None,
            colors,
        ),
        row(
            "settings-update-status",
            tr!("settings-update-status"),
            None,
            update_status,
            update_status_control,
            colors,
        ),
        row(
            "settings-update-check",
            tr!("settings-update-check"),
            Some(tr!("settings-update-check-help").to_owned()),
            "",
            Some(
                // ia[settings]: 手动检查更新 | 关于 · 按键 | 检查正式版并显示进度、结果或失败原因
                kit::ghost(
                    "settings-update-check-button",
                    tr!("settings-update-check-now"),
                    send(handler, LiveIntent::CheckUpdates),
                )
                .disabled(update_busy)
                .into_any_element(),
            ),
            colors,
        ),
        row(
            "settings-auto-update",
            tr!("settings-auto-update"),
            Some(tr!("settings-auto-update-help").to_owned()),
            "",
            Some(
                // ia[settings]: 自动更新 | 关于 · 开关 | 开启时启动后与每小时静默检查并下载；关闭后只保留手动检查
                kit::switch(
                    "settings-auto-update-switch",
                    auto_update,
                    tr!("settings-auto-update"),
                    send(handler, LiveIntent::SetAutoUpdate(!auto_update)),
                )
                .into_any_element(),
            ),
            colors,
        ),
        // ia[settings]: 启动器日志 | 关于 · 按键 | 在访达中显示日志目录
        row(
            "settings-logs",
            tr!("settings-logs"),
            Some(tr!("settings-logs-help").to_owned()),
            "",
            Some(
                kit::ghost(
                    "settings-reveal-log",
                    crate::platform::reveal_label(),
                    send(handler, LiveIntent::Reveal(log)),
                )
                .into_any_element(),
            ),
            colors,
        ),
        // ia[settings]: 导出诊断包 | 关于 · 按键 | 打包版本、设置摘要、Java 列表和各游戏最近日志；玩家名、UUID、路径脱敏
        row(
            "settings-diagnostics",
            tr!("settings-diagnostics"),
            Some(tr!("settings-diagnostics-help").to_owned()),
            "",
            Some(
                kit::ghost(
                    "settings-export",
                    tr!("settings-diagnostics-export"),
                    send(handler, LiveIntent::ExportDiagnostics),
                )
                .into_any_element(),
            ),
            colors,
        ),
        // ia[settings]: 开源许可 | 关于 · 值 | 显示 AGPL-3.0-only
        row(
            "settings-license",
            tr!("settings-license"),
            None,
            "AGPL-3.0-only",
            None,
            colors,
        ),
    ];
    kit::list(rows, colors).into_any_element()
}

fn update_status_text(status: &UpdateStatus) -> String {
    match status {
        UpdateStatus::NotChecked => tr!("settings-update-not-checked").to_owned(),
        UpdateStatus::Checking => tr!("settings-update-checking").to_owned(),
        UpdateStatus::Downloading { version } => {
            tr!("settings-update-downloading", version = version.as_str())
        }
        UpdateStatus::Ready { version } => tr!("settings-update-ready", version = version.as_str()),
        UpdateStatus::UpToDate => tr!("settings-update-current").to_owned(),
        UpdateStatus::Unavailable(reason) => match reason {
            UpdateUnavailability::DebugBuild => tr!("settings-update-debug-build").to_owned(),
            UpdateUnavailability::PortableWindows => tr!("settings-update-portable").to_owned(),
            UpdateUnavailability::PackageManagedLinux => {
                tr!("settings-update-package-managed").to_owned()
            }
            UpdateUnavailability::UnsupportedInstall => {
                tr!("settings-update-unsupported-install").to_owned()
            }
        },
        UpdateStatus::Failed { .. } => tr!("settings-update-failed").to_owned(),
    }
}
