use super::super::live::LiveCtx;
use super::rows::{row, send};
use crate::kit;
use crate::live::{LiveIntent, SettingsView};
use gpui::{AnyElement, IntoElement};

pub(super) fn about(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    let colors = ctx.colors;
    let handler = &ctx.handler;
    let log = view.data_dir.join("activity.jsonl");
    let rows = vec![
        // ia[settings]: 版本 | 关于 · 值 | 显示 LumilioCL 版本号
        row(
            "settings-version",
            "版本",
            None,
            format!("LumilioCL {}", env!("CARGO_PKG_VERSION")),
            None,
            colors,
        ),
        row(
            "settings-updates",
            "检查更新",
            Some("启动器自动更新还没有提供。".to_owned()),
            "暂未提供",
            None,
            colors,
        ),
        // ia[settings]: 启动器日志 | 关于 · 按键 | 在访达中显示日志目录
        row(
            "settings-logs",
            "启动器日志",
            Some("下载与安装的记录。".to_owned()),
            "",
            Some(
                kit::ghost(
                    "settings-reveal-log",
                    "在访达中显示",
                    send(handler, LiveIntent::Reveal(log)),
                )
                .into_any_element(),
            ),
            colors,
        ),
        // ia[settings]: 导出诊断包 | 关于 · 按键 | 打包版本、设置摘要、Java 列表和各游戏最近日志；玩家名、UUID、路径脱敏
        row(
            "settings-diagnostics",
            "诊断包",
            Some(
                "打包版本、设置摘要、Java 列表和各游戏的最近日志；玩家名、UUID 和你的文件夹路径会被替换，启动前/包装/退出后命令只记录有没有填。"
                    .to_owned(),
            ),
            "",
            Some(
                kit::ghost(
                    "settings-export",
                    "导出…",
                    send(handler, LiveIntent::ExportDiagnostics),
                )
                .into_any_element(),
            ),
            colors,
        ),
        // ia[settings]: 开源许可 | 关于 · 值 | 显示 AGPL-3.0
        row(
            "settings-license",
            "开源许可",
            None,
            "AGPL-3.0",
            None,
            colors,
        ),
    ];
    kit::list(rows, colors).into_any_element()
}
