use super::super::panels::{act, clock, size_label};
use super::super::{InstanceDetailView, InstanceIntent, Section};
use super::helpers::{files_matching, join_path, parent_path};
use crate::assets::UiIcon;
use crate::kit;
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{AnyElement, Context, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::Input;
use gpui_component::{Icon, h_flex, v_flex};
use lumilio_core::FileEntry;

impl InstanceDetailView {
    // ia[instance.diagnostics]: 浏览文件 | 诊断 · 文件分段 | 逐级打开的只读文件夹列表，只在游戏目录内；右上角按名字搜当前文件夹 | H-INSTANCE-10
    pub(super) fn files_body(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        if let Some(status) = self.status(&self.data.files, colors, Section::Files, cx) {
            return status;
        }
        let Some(Ok((folder, entries))) = &self.data.files else {
            return div().into_any_element();
        };
        let trail = if folder.is_empty() {
            "游戏目录".to_owned()
        } else {
            format!("游戏目录 / {}", folder.replace('/', " / "))
        };
        let (here, up) = (folder.clone(), parent_path(folder));
        let header = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .children((!folder.is_empty()).then(|| {
                        kit::ghost(
                            "instance-files-up",
                            "上一级",
                            act(cx, move |view, window, cx| {
                                view.open_folder(up.clone(), window, cx)
                            }),
                        )
                    }))
                    .child(div().text_sm().text_color(colors.muted).child(trail)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .children(self.fields.as_ref().map(|fields| {
                        div().w(px(200.)).child(
                            Input::new(&fields.file_search).small().prefix(
                                Icon::new(UiIcon::Search)
                                    .size(px(14.))
                                    .text_color(colors.muted),
                            ),
                        )
                    }))
                    // ia[instance.diagnostics]: 在访达中显示 | 文件分段 · 按键 | 当前文件夹或所选文件 | H-INSTANCE-10
                    .child(kit::ghost(
                        "instance-files-reveal",
                        "在访达中显示",
                        act(cx, move |view, window, cx| {
                            (view.handler)(InstanceIntent::RevealPath(here.clone()), window, cx)
                        }),
                    )),
            );
        let query = self
            .fields
            .as_ref()
            .map(|fields| fields.file_search.read(cx).value().to_string())
            .unwrap_or_default();
        let rows: Vec<_> = files_matching(entries, &query)
            .into_iter()
            .enumerate()
            .map(|(row, entry)| file_row(row, folder, entry, colors, cx))
            .collect();
        v_flex()
            .w_full()
            .gap_3()
            .child(header)
            .child(if rows.is_empty() {
                if entries.is_empty() {
                    kit::empty("这个文件夹是空的", "", colors).into_any_element()
                } else {
                    kit::empty("没有匹配的文件", "换个关键词试试", colors).into_any_element()
                }
            } else {
                kit::list(rows, colors).into_any_element()
            })
            .into_any_element()
    }
}

pub(super) fn file_row(
    row: usize,
    folder: &str,
    entry: &FileEntry,
    colors: ShellColors,
    cx: &mut Context<InstanceDetailView>,
) -> gpui::Div {
    let path = join_path(folder, &entry.name);
    let detail = if entry.is_dir {
        format!("文件夹 · {}", clock(entry.modified))
    } else {
        format!("{} · {}", size_label(entry.size), clock(entry.modified))
    };
    let action = if entry.is_dir {
        kit::ghost(
            ("file-open", row),
            "打开",
            act(cx, move |view, window, cx| {
                view.open_folder(path.clone(), window, cx)
            }),
        )
        .debug_selector(move || format!("file-open-{row}"))
    } else {
        kit::ghost(
            ("file-reveal", row),
            "显示",
            act(cx, move |view, window, cx| {
                (view.handler)(InstanceIntent::RevealPath(path.clone()), window, cx)
            }),
        )
        .debug_selector(move || format!("file-reveal-{row}"))
    };
    kit::row(
        entry.name.clone(),
        detail,
        None,
        Some(action.into_any_element()),
        colors,
    )
}
