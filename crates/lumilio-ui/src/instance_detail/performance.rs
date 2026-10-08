use super::editors::Editor;
use super::forms::effective_label;
use super::{InstanceDetailView, editors};
use crate::kit;
use crate::theme::ShellColors;
use crate::tr;
use gpui::Context;
use gpui::prelude::*;
use gpui_component::{h_flex, v_flex};

impl InstanceDetailView {
    pub(super) fn performance(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let record = self.record.as_ref().expect("loaded");
        let machine = self.machine_memory_row(colors);
        v_flex()
            .w_full()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(kit::section_label(tr!("settings-memory"), colors))
                    // ia[instance.settings]: 内存 | 设置 · 性能组，「内存」区 [编辑] 弹窗（最小 / 最大） | 留空跟随默认；性能组另有只读的“本机内存 · 推荐最大”一行
                    .child(self.edit_button("instance-memory-edit", Editor::Memory, cx)),
            )
            .child(kit::list(
                vec![
                    kit::value_row(
                        "memory-min",
                        tr!("settings-field-min-memory"),
                        Some(editors::MIN_MEMORY_HELP.into()),
                        effective_label(
                            record.settings.min_memory_mb,
                            self.defaults.default_min_memory_mb,
                        ),
                        None,
                        colors,
                    ),
                    kit::value_row(
                        "memory-max",
                        tr!("settings-field-max-memory"),
                        Some(editors::MAX_MEMORY_HELP.into()),
                        effective_label(
                            record.settings.max_memory_mb,
                            self.defaults.default_max_memory_mb,
                        ),
                        None,
                        colors,
                    ),
                ]
                .into_iter()
                .chain(machine)
                .collect(),
                colors,
            ))
            .into_any_element()
    }
}
