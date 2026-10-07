use super::super::live::LiveCtx;
use super::rows::{row, send};
use crate::kit;
use crate::live::{LiveIntent, SettingsView};
use crate::tr;
use gpui::{AnyElement, IntoElement};

pub(super) fn about(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let handler = &ctx.handler;
    let log = view.data_dir.join("activity.jsonl");
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
            "settings-updates",
            tr!("settings-updates"),
            Some(tr!("settings-updates-help").to_owned()),
            tr!("settings-updates-value"),
            None,
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
