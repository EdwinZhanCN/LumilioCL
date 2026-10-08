use super::super::panels::CONTENT_KINDS;
use super::super::{InstanceDetailView, InstanceIntent};
use super::model::title_of;
use crate::assets::UiIcon;
use crate::controls::Checkbox;
use crate::key::Key;
use crate::theme::ShellColors;
use crate::{kit, theme, tr};
use gpui::prelude::*;
use gpui::{AnyElement, ClipboardItem, Context, div, px};
use gpui_component::Sizable as _;
use gpui_component::StyledExt as _;
use gpui_component::{Icon, h_flex, v_flex};
use lumilio_core::ContentEntry;

impl InstanceDetailView {
    pub(super) fn row(
        &self,
        index: usize,
        entry: &ContentEntry,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self.busy;
        let kind = CONTENT_KINDS[self.content_index()];
        let name = entry.item.file_name.clone();
        let chosen = self.selected.contains(&name);
        let view = cx.entity().downgrade();
        let source = entry.source.clone();
        let seed = crate::live::seed_of(&entry.item.display_name);

        let select = {
            let view = view.clone();
            let name = name.clone();
            Checkbox::new(("content-select", index))
                .checked(chosen)
                .on_click(move |_, _, cx| {
                    let name = name.clone();
                    let _ = view.update(cx, |view, cx| {
                        if !view.selected.remove(&name) {
                            view.selected.insert(name);
                        }
                        cx.notify();
                    });
                })
        };

        // ia[instance.content]: 打开项目页 | 点标题（仅已识别） | 打开该项目的发现页详情（进历史）
        let title = {
            let label = div()
                .text_sm()
                .font_medium()
                .text_color(colors.foreground)
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(title_of(entry).to_owned());
            match &source {
                Some(source) => {
                    let handler = self.handler.clone();
                    let slug = source.slug.clone();
                    div()
                        .id(("content-title", index))
                        .cursor_pointer()
                        .hover(|title| title.opacity(0.8))
                        .on_click(move |_, window, cx| {
                            handler(
                                InstanceIntent::OpenProject {
                                    kind,
                                    slug: slug.clone(),
                                },
                                window,
                                cx,
                            )
                        })
                        .child(label)
                        .into_any_element()
                }
                None => label.into_any_element(),
            }
        };
        let byline = match source.as_ref().and_then(|source| source.author.clone()) {
            Some(author) => author,
            None if source.is_some() => "Modrinth".to_owned(),
            None => tr!("instance-content-local-file").to_owned(),
        };

        let version = source
            .as_ref()
            .map(|source| source.version_number.clone())
            .filter(|number| !number.is_empty())
            .unwrap_or_else(|| tr!("instance-content-unknown-version").to_owned());

        let switch_button = source.as_ref().map(|_| {
            let entry = entry.clone();
            let view = view.clone();
            let update = entry.update.is_some();
            // ia[instance.content]: 更新单个 | 行内「更新」→ 切换版本弹窗，默认选最新兼容 | 下载 → 校验 → 替换旧文件 → 重扫 → 写历史；停用的保持停用
            // ia[instance.content]: 切换版本 | 行内 ⇆ → 版本弹窗（仅已识别的文件） | 下载所选版本 → 校验 → 替换旧文件 → 重扫 → 写历史；停用的保持停用
            let button = if update {
                Key::new(("content-update", index))
                    .icon(Icon::new(UiIcon::Refresh))
                    .label(tr!("discover-update"))
                    .primary()
                    .small()
            } else {
                Key::new(("content-switch-version", index))
                    .icon(Icon::new(UiIcon::Switch))
                    .ghost()
                    .small()
                    .tooltip(tr!("project-switch-version"))
            };
            theme::clickable(button.disabled(busy), !busy)
                .debug_selector(move || format!("content-switch-version-{index}"))
                .on_click(move |_, window, cx| {
                    let entry = entry.clone();
                    let _ =
                        view.update(cx, |view, cx| view.open_switch(&entry, update, window, cx));
                })
        });

        let toggle = {
            let view = view.clone();
            let name = name.clone();
            let next = !entry.item.enabled;
            div()
                .debug_selector(move || format!("content-switch-{index}"))
                // ia[instance.content]: 启用 / 停用 | 行内开关 | 即时生效；游戏运行中拒绝并 toast“游戏运行时不能改”
                .child(
                    crate::controls::Fader::new(
                        ("content-switch", index),
                        entry.item.enabled,
                        tr!("instance-content-enable"),
                        move |window, cx| {
                            let name = name.clone();
                            let _ = view.update(cx, |view, cx| {
                                view.send(
                                    InstanceIntent::SetContent {
                                        kind,
                                        files: vec![name],
                                        enabled: next,
                                    },
                                    window,
                                    cx,
                                )
                            });
                        },
                    )
                    .disabled(busy),
                )
        };

        let delete = {
            let view = view.clone();
            // ia[instance.content]: 删除 | 行内 🗑 → 警告弹窗 | 删除文件、写历史、toast
            let name = name.clone();
            theme::clickable(
                Key::new(("content-delete", index))
                    .icon(Icon::new(UiIcon::Trash))
                    .ghost()
                    .small()
                    .tooltip(tr!("common-delete"))
                    .disabled(busy),
                !busy,
            )
            .debug_selector(move || format!("content-delete-{index}"))
            .on_click(move |_, window, cx| {
                let name = name.clone();
                let _ = view.update(cx, |view, cx| {
                    view.confirm_delete_content(kind, vec![name], window, cx)
                });
            })
        };

        // ia[instance.content]: 在访达中显示 | 行 ⋯ 菜单 | 打开并选中文件
        let mut more = vec![{
            let handler = self.handler.clone();
            let name = name.clone();
            kit::MenuEntry::new(crate::platform::reveal_label(), move |window, cx| {
                handler(
                    InstanceIntent::RevealContent {
                        kind,
                        file_name: name.clone(),
                    },
                    window,
                    cx,
                )
            })
        }];
        if let Some(source) = &source {
            let link = lumilio_core::project_page_url(kind, &source.slug);
            let view = view.clone();
            // ia[instance.content]: 复制链接 | 行 ⋯ 菜单（仅已识别） | 复制 Modrinth 项目链接，toast“链接已复制”
            more.push(kit::MenuEntry::new(
                tr!("discover-copy-link"),
                move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(link.clone()));
                    let _ = view.update(cx, |view, cx| {
                        view.toast(
                            crate::toast::Toast::success(tr!("instance-content-link-copied")),
                            cx,
                        )
                    });
                },
            ));
        }

        h_flex()
            .id(("content-row", index))
            .w_full()
            .items_center()
            .gap_3()
            .py(px(10.))
            .opacity(if entry.item.enabled { 1. } else { 0.55 })
            .child(select)
            .child(crate::pages::live::project_icon(
                source
                    .as_ref()
                    .and_then(|source| source.icon_url.as_deref()),
                seed,
                40.,
                colors,
            ))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(title)
                    .child(div().text_xs().text_color(colors.muted).child(byline)),
            )
            .child(
                v_flex()
                    .w(px(240.))
                    .flex_none()
                    .min_w_0()
                    .gap(px(2.))
                    .debug_selector(move || format!("content-version-{index}"))
                    .child(
                        div()
                            .w_full()
                            .text_sm()
                            .text_color(colors.foreground)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(version),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_xs()
                            .text_color(colors.muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(entry.item.file_name.clone()),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .items_center()
                    .justify_end()
                    // Sized by its keys, never narrower than the common case, so a
                    // row with an Update key cannot spill into the version column.
                    .min_w(px(176.))
                    .debug_selector(move || format!("content-actions-{index}"))
                    .children(switch_button)
                    .child(toggle)
                    .child(delete)
                    .child(kit::more_menu(("content-more", index), more, colors)),
            )
            .into_any_element()
    }
}
