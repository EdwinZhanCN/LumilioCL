use super::InstanceDetailView;
use super::editors::Editor;
use super::intent::InstanceIntent;
use crate::theme::ShellColors;
use crate::{kit, theme};
use gpui::prelude::*;
use gpui::{App, Context, Window};

impl InstanceDetailView {
    /// Primary 启动游戏; everything else in More (design language §7).
    pub(super) fn page_actions(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let record = self.record.as_ref()?;
        let busy = self.busy;
        let play = self.handler.clone();
        let entity = cx.entity().downgrade();
        let with_view = move |run: fn(&mut Self, &mut Window, &mut Context<Self>)| {
            let entity = entity.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = entity.update(cx, |view, cx| run(view, window, cx));
            }
        };
        let running = self.live_output.is_some();
        let primary = if running {
            // ia[instance]: 结束游戏 | 本启动器启动的游戏运行时，页头主按钮变「结束游戏」 | 终止进程，会话写入历史
            kit::action(
                "instance-stop",
                "结束游戏",
                Some(crate::assets::UiIcon::Close),
                false,
                move |window, cx| play(InstanceIntent::Stop, window, cx),
            )
            .debug_selector(|| "instance-stop".into())
        } else {
            // ia[instance]: 开始游戏 | 页头主按钮「启动游戏」 | 首页启动时刻接管
            kit::action(
                "instance-play",
                "启动游戏",
                Some(crate::assets::UiIcon::Play),
                true,
                move |window, cx| play(InstanceIntent::Play, window, cx),
            )
            .disabled(busy)
            .debug_selector(|| "instance-play".into())
        };
        let mut actions = kit::PageActions::new("instance-actions")
            .primary(theme::clickable(primary, running || !busy));
        if !record.installed {
            actions = actions.more(
                // ia[instance]: 安装游戏文件 | 页头 ⋯ 菜单（游戏还没装好时才有） | 后台任务，动态可见；完成 toast
                kit::MenuEntry::new(
                    "安装游戏文件",
                    with_view(|view, window, cx| view.send(InstanceIntent::Install, window, cx)),
                )
                .disabled(busy),
            );
        }
        actions
            // ia[instance]: 设为当前游戏 | 页头 ⋯ 菜单 | 导航右段芯片跟着换；之后的启动指向它
            .more(kit::MenuEntry::new(
                "设为当前游戏",
                with_view(|view, window, cx| {
                    (view.handler)(InstanceIntent::SetCurrent, window, cx)
                }),
            ))
            .more(
                // ia[instance]: 修复游戏文件 | 页头 ⋯ 菜单 | 核对并补齐/重下损坏文件，后台任务，动态可见 | 运行中或没装好时禁用
                kit::MenuEntry::new(
                    "修复游戏文件",
                    with_view(|view, window, cx| view.send(InstanceIntent::Repair, window, cx)),
                )
                .disabled(busy || running || !record.installed),
            )
            .more(
                // ia[instance]: 创建快照 | 页头 ⋯ 菜单 → 弹窗（备注可空；范围＝全部或某个世界） | 后台创建，历史·快照可见
                kit::MenuEntry::new(
                    "创建快照…",
                    with_view(|view, window, cx| view.open_editor(Editor::Snapshot, window, cx)),
                )
                .disabled(busy),
            )
            // ia[instance]: 在访达中显示 | 页头 ⋯ 菜单 | 打开游戏目录
            .more(kit::MenuEntry::new(
                crate::platform::reveal_label(),
                with_view(|view, window, cx| {
                    (view.handler)(InstanceIntent::RevealPath(String::new()), window, cx)
                }),
            ))
            .more(
                // ia[instance]: 复制游戏 | 页头 ⋯ 菜单 → 弹窗（新名称、是否复制存档） | 后台复制成独立副本；不复制历史、快照和游玩时间
                kit::MenuEntry::new(
                    "复制这个游戏…",
                    with_view(|view, window, cx| view.open_editor(Editor::Copy, window, cx)),
                )
                .disabled(busy),
            )
            .more(
                // ia[instance]: 完整备份 | 页头 ⋯ 菜单 → 选位置 | 后台打成一个 zip（含存档，不含日志），toast；之后可在游戏库「从备份恢复」 | ADR 0015；运行中禁用
                kit::MenuEntry::new(
                    "完整备份…",
                    with_view(|view, window, cx| {
                        (view.handler)(InstanceIntent::BackupGame, window, cx)
                    }),
                )
                .disabled(busy || running),
            )
            // ia[instance]: 导出整合包 | 页头 ⋯ 菜单 → 导出弹窗（格式、勾选文件） | 后台导出，可取消
            .more(kit::MenuEntry::new(
                "导出整合包…",
                with_view(|view, window, cx| {
                    (view.handler)(InstanceIntent::ExportPack, window, cx)
                }),
            ))
            .more(
                // ia[instance]: 删除游戏 | 页头 ⋯ 菜单 → 警告弹窗 | 删除后从历史中移除并后退
                kit::MenuEntry::new(
                    "删除游戏…",
                    with_view(|view, window, cx| view.confirm_delete(window, cx)),
                )
                .danger()
                .disabled(busy),
            )
            .render(colors)
    }
}
