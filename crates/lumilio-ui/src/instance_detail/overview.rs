use super::editors::Editor;
use super::{InstanceDetailView, editors, panels};
use crate::theme::ShellColors;
use crate::{kit, live, tr};
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
                        tr!("common-name"),
                        Some(editors::RENAME_HELP.into()),
                        record.name.clone(),
                        Some(self.edit_button("instance-rename", Editor::Rename, cx)),
                        colors,
                    ),
                    kit::value_row(
                        "overview-version",
                        tr!("instance-field-game-version"),
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
                        tr!("instance-overview-install-record"),
                        Some(
                            if record.installed {
                                tr!("instance-overview-installed-help")
                            } else {
                                tr!("instance-overview-pending-help")
                            }
                            .into(),
                        ),
                        if record.installed {
                            tr!("discover-installed")
                        } else {
                            tr!("instance-overview-pending")
                        },
                        None,
                        colors,
                    ),
                    // ia[instance.overview]: 占用空间 | 概览「占用空间」行 | 后台计算，只算这个游戏自己的文件
                    kit::value_row(
                        "overview-size",
                        tr!("instance-overview-size"),
                        Some(tr!("instance-overview-size-help").into()),
                        match &self.data.size {
                            None => tr!("instance-overview-size-computing").to_owned(),
                            Some(Ok(bytes)) => panels::size_label(*bytes),
                            Some(Err(_)) => tr!("instance-overview-size-failed").to_owned(),
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
