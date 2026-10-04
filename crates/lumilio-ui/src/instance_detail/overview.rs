use super::editors::Editor;
use super::{InstanceDetailView, editors, panels};
use crate::theme::ShellColors;
use crate::{kit, live};
use gpui::Context;
use gpui::prelude::*;
use gpui_component::v_flex;

impl InstanceDetailView {
    pub(super) fn overview(&self, colors: ShellColors, cx: &mut Context<Self>) -> gpui::AnyElement {
        let record = self.record.as_ref().expect("loaded");
        v_flex()
            .w_full()
            .gap_5()
            .child(kit::list(
                vec![
                    // ia[instance.overview]: 改名 | 概览「名称」行 [编辑] → 弹窗 | 改名保留游戏目录、收藏和历史记录
                    kit::value_row(
                        "overview-name",
                        "名称",
                        Some(editors::RENAME_HELP.into()),
                        record.name.clone(),
                        Some(self.edit_button("instance-rename", Editor::Rename, cx)),
                        colors,
                    ),
                    kit::value_row(
                        "overview-version",
                        "游戏版本",
                        None,
                        format!(
                            "{} · {}",
                            record.game_version,
                            live::loader_label(record.loader)
                        ),
                        None,
                        colors,
                    ),
                    kit::value_row(
                        "overview-installed",
                        "安装记录",
                        Some(
                            if record.installed {
                                "曾完成安装，启动时仍会检查文件。"
                            } else {
                                "尚未完成安装，首次启动时准备游戏文件。"
                            }
                            .into(),
                        ),
                        if record.installed {
                            "已安装"
                        } else {
                            "待安装"
                        },
                        None,
                        colors,
                    ),
                    // ia[instance.overview]: 占用空间 | 概览「占用空间」行 | 后台计算，只算这个游戏自己的文件
                    kit::value_row(
                        "overview-size",
                        "占用空间",
                        Some("这个游戏自己的文件；多个游戏共用的游戏文件不算在内。".into()),
                        match &self.data.size {
                            None => "计算中…".to_owned(),
                            Some(Ok(bytes)) => panels::size_label(*bytes),
                            Some(Err(_)) => "读不到".to_owned(),
                        },
                        None,
                        colors,
                    ),
                ],
                colors,
            ))
            .children(self.problems_block(colors, cx))
            .children(self.recent_block(colors, cx))
            .into_any_element()
    }
}
