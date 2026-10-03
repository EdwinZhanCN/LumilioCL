//! The Instance page's Content tab (IA `instance/content.md`): each file with
//! where it came from (P-CONTENT-ITEM), filters, bulk actions (P-BULK), local
//! files (P-ADD-FILES) and the switch-version dialog (P-VERSION-SWITCH).
//!
//! A child module of `instance_detail`, so it shares the view's private state.

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::controls::Checkbox;
use crate::key::Key;
use gpui::{
    AnyElement, App, ClipboardItem, Context, Entity, IntoElement, Render, WeakEntity, Window, div,
    prelude::*, px, uniform_list,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::text::TextView;
use gpui_component::{
    ActiveTheme as _, Icon, Sizable as _, StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use lumilio_core::{ContentEntry, ContentList, Loader, ProjectKind, ReleaseChannel, Version};

use super::panels::{CONTENT_KINDS, CONTENT_LABELS};
use super::{InstanceDetailView, InstanceIntent, Section};
use crate::assets::UiIcon;
use crate::kit;
use crate::theme::{self, ShellColors};

/// Which files the list shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ContentFilter {
    #[default]
    All,
    Updates,
    Disabled,
    Unknown,
}

const FILTERS: [ContentFilter; 4] = [
    ContentFilter::All,
    ContentFilter::Updates,
    ContentFilter::Disabled,
    ContentFilter::Unknown,
];
const FILTER_LABELS: [&str; 4] = ["全部", "有更新", "已停用", "未识别"];

/// What a bulk-bar button does to the view.
type BulkAction =
    Rc<dyn Fn(&mut InstanceDetailView, &mut Window, &mut Context<InstanceDetailView>)>;

/// The toolbar and the bulk bar share one height (P-BULK).
const BAR_HEIGHT: gpui::Pixels = px(40.);

/// The name a row shows: the Modrinth project, else the file.
#[must_use]
pub fn title_of(entry: &ContentEntry) -> &str {
    entry
        .source
        .as_ref()
        .map(|source| source.title.as_str())
        .filter(|title| !title.is_empty())
        .unwrap_or(&entry.item.display_name)
}

/// The rows a query and a filter leave, in list order.
#[must_use]
pub fn visible<'a>(
    entries: &'a [ContentEntry],
    query: &str,
    filter: ContentFilter,
) -> Vec<&'a ContentEntry> {
    let query = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|entry| match filter {
            ContentFilter::All => true,
            ContentFilter::Updates => entry.update.is_some(),
            ContentFilter::Disabled => !entry.item.enabled,
            ContentFilter::Unknown => entry.source.is_none(),
        })
        .filter(|entry| {
            query.is_empty()
                || title_of(entry).to_lowercase().contains(&query)
                || entry.item.file_name.to_lowercase().contains(&query)
                || entry
                    .source
                    .as_ref()
                    .and_then(|source| source.author.as_ref())
                    .is_some_and(|author| author.to_lowercase().contains(&query))
        })
        .collect()
}

const fn channel_label(channel: ReleaseChannel) -> &'static str {
    match channel {
        ReleaseChannel::Release => "正式",
        ReleaseChannel::Beta => "测试",
        ReleaseChannel::Alpha => "内测",
    }
}

impl InstanceDetailView {
    fn content_index(&self) -> usize {
        self.content_kind.min(2)
    }

    fn content_list(&self) -> Option<&ContentList> {
        match &self.data.content[self.content_index()] {
            Some(Ok(list)) => Some(list),
            _ => None,
        }
    }

    fn content_query(&self, cx: &App) -> String {
        self.fields
            .as_ref()
            .map(|fields| fields.content_search.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// Delete asks once, saying what goes (§10).
    fn confirm_delete_content(
        &mut self,
        kind: ProjectKind,
        files: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || files.is_empty() {
            return;
        }
        let view = cx.entity().downgrade();
        let title = if files.len() == 1 {
            format!("删除“{}”？", files[0].trim_end_matches(".disabled"))
        } else {
            format!("删除这 {} 个文件？", files.len())
        };
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            let files = files.clone();
            alert
                .title(title.clone())
                .description("文件会从这个游戏里移走，删除记录会写进历史。")
                .ok_text("删除")
                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                .cancel_text("取消")
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let files = files.clone();
                    let _ = view.update(cx, |view, cx| {
                        view.selected.clear();
                        view.send(InstanceIntent::DeleteContent { kind, files }, window, cx);
                    });
                    true
                })
        });
    }

    fn set_content_enabled(
        &mut self,
        files: Vec<String>,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let kind = CONTENT_KINDS[self.content_index()];
        self.selected.clear();
        self.send(
            InstanceIntent::SetContent {
                kind,
                files,
                enabled,
            },
            window,
            cx,
        );
    }

    /// Opens the switch-version dialog for one file. With `update`, the
    /// newest compatible version is chosen at first; otherwise the current.
    pub(super) fn open_switch(
        &mut self,
        entry: &ContentEntry,
        update: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(source), Some(record)) = (&entry.source, &self.record) else {
            return;
        };
        if self.busy {
            return;
        }
        let kind = CONTENT_KINDS[self.content_index()];
        let parent = cx.entity().downgrade();
        let switch = cx.new(|cx| {
            VersionSwitch::new(
                SwitchTarget {
                    kind,
                    file_name: entry.item.file_name.clone(),
                    project: source.project_id.clone(),
                    title: title_of(entry).to_owned(),
                    current: source.version_id.clone(),
                    preferred: if update {
                        entry.update.as_ref().map(|version| version.id.clone())
                    } else {
                        None
                    },
                    game_version: record.game_version.clone(),
                    loader: record.loader,
                },
                parent,
                window,
                cx,
            )
        });
        self.switch = Some(switch.clone());
        (self.handler)(
            InstanceIntent::LoadVersions(source.project_id.clone()),
            window,
            cx,
        );
        let closed = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let closed = closed.clone();
            VersionSwitch::dialog(&switch, dialog, cx).on_close(move |_, _, cx| {
                let _ = closed.update(cx, |view, cx| {
                    view.switch = None;
                    cx.notify();
                });
            })
        });
    }

    /// A project's versions arrived; only the open dialog for it takes them.
    pub fn versions_arrived(
        &mut self,
        project: &str,
        result: Result<Vec<Version>, String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(switch) = &self.switch {
            switch.update(cx, |switch, cx| {
                switch.versions_arrived(project, result, cx)
            });
        }
    }

    fn row(
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
            None => "本地文件".to_owned(),
        };

        let version = source
            .as_ref()
            .map(|source| source.version_number.clone())
            .filter(|number| !number.is_empty())
            .unwrap_or_else(|| "未知".to_owned());

        let switch_button = source.as_ref().map(|_| {
            let entry = entry.clone();
            let view = view.clone();
            let update = entry.update.is_some();
            let button = if update {
                Key::new(("content-update", index))
                    .icon(Icon::new(UiIcon::Refresh))
                    .label("更新")
                    .primary()
                    .small()
            } else {
                Key::new(("content-switch-version", index))
                    .icon(Icon::new(UiIcon::Switch))
                    .ghost()
                    .small()
                    .tooltip("切换版本")
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
                .child(
                    crate::controls::Fader::new(
                        ("content-switch", index),
                        entry.item.enabled,
                        "启用",
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
            let name = name.clone();
            theme::clickable(
                Key::new(("content-delete", index))
                    .icon(Icon::new(UiIcon::Trash))
                    .ghost()
                    .small()
                    .tooltip("删除")
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

        let mut more = vec![{
            let handler = self.handler.clone();
            let name = name.clone();
            kit::MenuEntry::new("在访达中显示", move |window, cx| {
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
            more.push(kit::MenuEntry::new("复制链接", move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(link.clone()));
                let _ = view.update(cx, |view, cx| {
                    view.toast(crate::toast::Toast::success("链接已复制"), cx)
                });
            }));
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
                    .w(px(220.))
                    .flex_none()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(version),
                            )
                            .children(entry.update.as_ref().map(|newest| {
                                div()
                                    .text_xs()
                                    .text_color(colors.primary)
                                    .whitespace_nowrap()
                                    .child(format!("→ {}", newest.number))
                            })),
                    )
                    .child(
                        div()
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
                    .w(px(176.))
                    .children(switch_button)
                    .child(toggle)
                    .child(delete)
                    .child(kit::more_menu(("content-more", index), more, colors)),
            )
            .into_any_element()
    }

    pub(super) fn content_panel(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let index = self.content_index();
        let kind = CONTENT_KINDS[index];
        let noun = CONTENT_LABELS[index];
        let busy = self.busy;
        let view = cx.entity().downgrade();

        let segments = kit::segments("instance-content-kinds", &CONTENT_LABELS, index, {
            let view = view.clone();
            move |index: usize, window: &mut Window, cx: &mut App| {
                let _ = view.update(cx, |view, cx| {
                    view.content_kind = index;
                    view.selected.clear();
                    view.ensure(Section::Content(CONTENT_KINDS[index.min(2)]), window, cx);
                    cx.notify();
                });
            }
        });
        let add = {
            let handler = self.handler.clone();
            kit::action(
                "content-add-files",
                "添加文件",
                Some(UiIcon::Plus),
                false,
                move |window, cx| handler(InstanceIntent::ImportContent(kind), window, cx),
            )
            .disabled(busy)
        };
        let browse = {
            let handler = self.handler.clone();
            kit::action(
                "content-browse",
                ["浏览 Mod", "浏览资源包", "浏览光影"][index],
                Some(UiIcon::Search),
                true,
                move |window, cx| handler(InstanceIntent::BrowseContent(kind), window, cx),
            )
        };
        let region = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_3()
            .child(segments)
            .child(h_flex().gap_2().child(add).child(browse));

        let body = if let Some(status) = self.status(
            &self.data.content[index],
            colors,
            Section::Content(kind),
            cx,
        ) {
            status
        } else {
            self.content_body(noun, colors, cx)
        };

        v_flex()
            .w_full()
            .gap_4()
            .child(region)
            .child(body)
            .into_any_element()
    }

    fn content_body(&self, noun: &str, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let Some(list) = self.content_list() else {
            return div().into_any_element();
        };
        if list.entries.is_empty() {
            return kit::empty(
                format!("还没有{noun}"),
                "点「浏览」找一个装进来，或者添加本地文件",
                colors,
            )
            .into_any_element();
        }
        let busy = self.busy;
        let kind = CONTENT_KINDS[self.content_index()];
        let view = cx.entity().downgrade();
        let query = self.content_query(cx);
        let shown = visible(&list.entries, &query, self.content_filter);
        let updates: Vec<(String, String, String)> = list
            .entries
            .iter()
            .filter_map(|entry| {
                let source = entry.source.as_ref()?;
                let newest = entry.update.as_ref()?;
                Some((
                    entry.item.file_name.clone(),
                    source.project_id.clone(),
                    newest.id.clone(),
                ))
            })
            .collect();

        let toolbar = if self.selected.is_empty() {
            let filter_index = FILTERS
                .iter()
                .position(|filter| *filter == self.content_filter)
                .unwrap_or(0);
            let filters = kit::segments("content-filters", &FILTER_LABELS, filter_index, {
                let view = view.clone();
                move |index: usize, _: &mut Window, cx: &mut App| {
                    let _ = view.update(cx, |view, cx| {
                        view.content_filter = FILTERS[index.min(3)];
                        cx.notify();
                    });
                }
            });
            let update_all = (!updates.is_empty()).then(|| {
                let view = view.clone();
                let count = updates.len();
                let updates = updates.clone();
                theme::clickable(
                    Key::new("content-update-all")
                        .icon(Icon::new(UiIcon::Refresh))
                        .label(format!("全部更新（{count}）"))
                        .white()
                        .small()
                        .disabled(busy),
                    !busy,
                )
                .on_click(move |_, window, cx| {
                    let updates = updates.clone();
                    let _ = view.update(cx, |view, cx| {
                        view.confirm_update_all(kind, updates, window, cx)
                    });
                })
            });
            let refresh = {
                let view = view.clone();
                theme::clickable(
                    Key::new("content-refresh")
                        .icon(Icon::new(UiIcon::Refresh))
                        .ghost()
                        .small()
                        .tooltip("重新读取并识别"),
                    true,
                )
                .on_click(move |_, window, cx| {
                    let _ = view.update(cx, |view, cx| {
                        view.request(Section::Content(kind), window, cx)
                    });
                })
            };
            let search = self.fields.as_ref().map(|fields| {
                div().w(px(220.)).child(
                    Input::new(&fields.content_search).small().prefix(
                        Icon::new(UiIcon::Search)
                            .size(px(14.))
                            .text_color(colors.muted),
                    ),
                )
            });
            // Same height as the bulk bar that replaces it, so selecting a
            // row never moves the list under the pointer.
            h_flex()
                .w_full()
                .h(BAR_HEIGHT)
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .children(search)
                        .child(filters),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .children(update_all)
                        .child(refresh),
                )
                .into_any_element()
        } else {
            self.bulk_bar(colors, cx)
        };

        let rows: Vec<AnyElement> = shown
            .iter()
            .enumerate()
            .map(|(index, entry)| self.row(index, entry, colors, cx))
            .collect();
        let list_block = if rows.is_empty() {
            kit::empty("没有符合的条目", "换个关键词或筛选试试", colors).into_any_element()
        } else {
            kit::panel_list(rows, colors).into_any_element()
        };
        v_flex()
            .w_full()
            .gap_3()
            .child(toolbar)
            .child(list_block)
            .when(list.sources_unavailable, |body| {
                body.child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child("连不上 Modrinth，来源信息暂时不可用；文件照常可以启停和删除。"),
                )
            })
            .into_any_element()
    }

    /// P-BULK: what can be done to every selected file at once.
    fn bulk_bar(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy;
        let kind = CONTENT_KINDS[self.content_index()];
        let files: Vec<String> = self.selected.iter().cloned().collect();
        let view = cx.entity().downgrade();
        let action = |id: &'static str, label: &'static str, run: BulkAction| {
            let view = view.clone();
            theme::clickable(
                Key::new(id)
                    .label(label)
                    .ghost()
                    .small()
                    .disabled(busy)
                    .debug_selector(move || id.into()),
                !busy,
            )
            .on_click(move |_, window, cx| {
                let run = run.clone();
                let _ = view.update(cx, |view, cx| run(view, window, cx));
            })
        };
        let enable = {
            let files = files.clone();
            action(
                "content-bulk-enable",
                "启用",
                Rc::new(move |view, window, cx| {
                    view.set_content_enabled(files.clone(), true, window, cx)
                }),
            )
        };
        let disable = {
            let files = files.clone();
            action(
                "content-bulk-disable",
                "停用",
                Rc::new(move |view, window, cx| {
                    view.set_content_enabled(files.clone(), false, window, cx)
                }),
            )
        };
        let delete = {
            let files = files.clone();
            let view = view.clone();
            theme::clickable(
                Key::new("content-bulk-delete")
                    .icon(Icon::new(UiIcon::Trash))
                    .label("删除")
                    .danger()
                    .small()
                    .disabled(busy),
                !busy,
            )
            .on_click(move |_, window, cx| {
                let files = files.clone();
                let _ = view.update(cx, |view, cx| {
                    view.confirm_delete_content(kind, files, window, cx)
                });
            })
        };
        let clear = action(
            "content-bulk-clear",
            "清除选择",
            Rc::new(|view, _, cx| {
                view.selected.clear();
                cx.notify();
            }),
        );
        h_flex()
            .w_full()
            .h(BAR_HEIGHT)
            .px_3()
            .items_center()
            .justify_between()
            .rounded(px(10.))
            .bg(colors.surface_active)
            .debug_selector(|| "content-bulk".into())
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(colors.foreground)
                            .child(format!("已选 {} 项", files.len())),
                    )
                    .child(clear),
            )
            .child(h_flex().gap_1().child(enable).child(disable).child(delete))
            .into_any_element()
    }

    fn confirm_update_all(
        &mut self,
        kind: ProjectKind,
        updates: Vec<(String, String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || updates.is_empty() {
            return;
        }
        let view = cx.entity().downgrade();
        let count = updates.len();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            let updates = updates.clone();
            alert
                .title(format!("更新 {count} 个文件？"))
                .description(
                    "每个都会换成兼容这个游戏的最新版本；换版本可能让游戏出问题，必要时先建快照。",
                )
                .ok_text("全部更新")
                .cancel_text("取消")
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let updates = updates.clone();
                    let _ = view.update(cx, |view, cx| {
                        view.send(InstanceIntent::UpdateContent { kind, updates }, window, cx);
                    });
                    true
                })
        });
    }

    /// Wires the content search field: typing refilters the list.
    pub(super) fn watch_content_search(search: &Entity<InputState>, cx: &mut Context<Self>) {
        cx.subscribe(search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
    }
}

/// The file a switch-version dialog is for.
#[derive(Clone, Debug)]
pub struct SwitchTarget {
    pub kind: ProjectKind,
    pub file_name: String,
    pub project: String,
    pub title: String,
    /// The installed version's id.
    pub current: String,
    /// Chosen at first instead of the current one ("update" opens with the newest).
    pub preferred: Option<String>,
    pub game_version: String,
    pub loader: Loader,
}

/// P-VERSION-SWITCH: one dialog for "update" and "switch version".
pub struct VersionSwitch {
    target: SwitchTarget,
    parent: WeakEntity<InstanceDetailView>,
    versions: Option<Result<Vec<Version>, String>>,
    selected: Option<String>,
    query: Entity<InputState>,
    show_incompatible: bool,
    busy: bool,
    error: Option<String>,
    close: bool,
}

impl VersionSwitch {
    fn new(
        target: SwitchTarget,
        parent: WeakEntity<InstanceDetailView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("查找版本"));
        cx.subscribe(&query, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        Self {
            target,
            parent,
            versions: None,
            selected: None,
            query,
            show_incompatible: false,
            busy: false,
            error: None,
            close: false,
        }
    }

    fn fits(&self, version: &Version) -> bool {
        lumilio_core::version_fits(
            version,
            self.target.kind,
            &self.target.game_version,
            self.target.loader,
        )
    }

    fn versions_arrived(
        &mut self,
        project: &str,
        result: Result<Vec<Version>, String>,
        cx: &mut Context<Self>,
    ) {
        if project != self.target.project {
            return;
        }
        if let Ok(versions) = &result {
            let wanted = self
                .target
                .preferred
                .clone()
                .unwrap_or_else(|| self.target.current.clone());
            self.selected = versions
                .iter()
                .find(|version| version.id == wanted)
                .or_else(|| versions.iter().find(|version| self.fits(version)))
                .map(|version| version.id.clone());
        }
        self.versions = Some(result);
        cx.notify();
    }

    /// The write finished: success closes, a failure stays here.
    pub(super) fn finished(&mut self, failure: Option<String>, cx: &mut Context<Self>) {
        self.busy = false;
        match failure {
            Some(message) => self.error = Some(message),
            None => self.close = true,
        }
        cx.notify();
    }

    fn chosen(&self) -> Option<&Version> {
        let Some(Ok(versions)) = &self.versions else {
            return None;
        };
        let id = self.selected.as_ref()?;
        versions.iter().find(|version| &version.id == id)
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(version) = self
            .chosen()
            .filter(|v| v.id != self.target.current)
            .cloned()
        else {
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        let target = self.target.clone();
        let _ = self.parent.update(cx, |view, cx| {
            view.send(
                InstanceIntent::SwitchContent {
                    kind: target.kind,
                    file_name: target.file_name,
                    project: target.project,
                    version_id: version.id,
                },
                window,
                cx,
            );
        });
    }

    fn dialog(
        entity: &Entity<Self>,
        dialog: gpui_component::dialog::Dialog,
        cx: &mut App,
    ) -> gpui_component::dialog::Dialog {
        let this = entity.read(cx);
        let busy = this.busy;
        let chosen = this.chosen().cloned();
        let differs = chosen
            .as_ref()
            .is_some_and(|version| version.id != this.target.current);
        let label = match &chosen {
            Some(version) if differs => format!("切换到 {}", version.number),
            _ => "切换版本".to_owned(),
        };
        let weak = entity.downgrade();
        let commit = theme::clickable(
            Key::new("switch-commit")
                .label(label)
                .primary()
                .loading(busy)
                .disabled(busy || !differs)
                .debug_selector(|| "switch-commit".into())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |switch, cx| switch.submit(window, cx));
                }),
            !busy && differs,
        );
        let cancel = theme::clickable(
            Key::new("switch-cancel")
                .label("取消")
                .white()
                .disabled(busy)
                .on_click(|_, window, cx| window.close_dialog(cx)),
            !busy,
        );
        let colors = ShellColors::from_theme(cx.theme());
        theme::dialog(dialog, cx)
            .title(format!("切换版本 · {}", this.target.title))
            .w(px(760.))
            .keyboard(!busy)
            .overlay_closable(!busy)
            .close_button(!busy)
            .child(entity.clone())
            .footer(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(colors.muted)
                            .child("换版本可能让游戏出问题，必要时先在「历史」里建一个快照。"),
                    )
                    .child(cancel)
                    .child(commit),
            )
    }
}

impl Render for VersionSwitch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.close) && window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
        let colors = ShellColors::from_theme(cx.theme());
        let left: AnyElement = match &self.versions {
            None => kit::empty("正在读取版本…", "", colors).into_any_element(),
            Some(Err(message)) => v_flex()
                .gap_2()
                .child(kit::empty("读不到版本列表", "可以关掉再试一次", colors))
                .child(kit::technical("switch-technical", message.clone()))
                .into_any_element(),
            Some(Ok(versions)) => {
                let query = self.query.read(cx).value().trim().to_lowercase();
                let shown: Vec<(Version, bool)> = versions
                    .iter()
                    .filter(|version| {
                        query.is_empty() || version.number.to_lowercase().contains(&query)
                    })
                    .map(|version| (version.clone(), self.fits(version)))
                    .filter(|(version, fits)| {
                        *fits || self.show_incompatible || version.id == self.target.current
                    })
                    .collect();
                let current = self.target.current.clone();
                let newest_fit = versions
                    .iter()
                    .find(|version| self.fits(version))
                    .map(|v| v.id.clone());
                let selected = self.selected.clone();
                let view = cx.entity().downgrade();
                let count = shown.len();
                let shown = Rc::new(shown);
                uniform_list("switch-versions", count, move |range, _, _| {
                    range
                        .map(|index| {
                            let (version, fits) = &shown[index];
                            let id = version.id.clone();
                            let picked = selected.as_deref() == Some(id.as_str());
                            let view = view.clone();
                            let marker = if version.id == current {
                                Some("当前")
                            } else if newest_fit.as_deref() == Some(id.as_str()) {
                                Some("最新")
                            } else {
                                None
                            };
                            h_flex()
                                .id(("switch-version", index))
                                .debug_selector(move || format!("switch-version-{index}"))
                                .w_full()
                                .h(px(34.))
                                .px_2()
                                .gap_2()
                                .items_center()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .when(picked, |row| row.bg(colors.surface_active))
                                .hover(|row| row.bg(colors.surface_subtle))
                                .on_click(move |_, _, cx| {
                                    let id = id.clone();
                                    let _ = view.update(cx, |switch, cx| {
                                        switch.selected = Some(id);
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.muted)
                                        .w(px(28.))
                                        .child(channel_label(version.channel)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_sm()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .text_color(if *fits {
                                            colors.foreground
                                        } else {
                                            colors.muted
                                        })
                                        .child(version.number.clone()),
                                )
                                .children(marker.map(|marker| {
                                    div().text_xs().text_color(colors.primary).child(marker)
                                }))
                                .when(!fits, |row| {
                                    row.child(
                                        div().text_xs().text_color(colors.danger).child("不兼容"),
                                    )
                                })
                        })
                        .collect()
                })
                .h(px(300.))
                .into_any_element()
            }
        };

        let toggle = {
            let view = cx.entity().downgrade();
            h_flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child("显示不兼容的版本"),
                )
                .child(kit::switch(
                    "switch-incompatible",
                    self.show_incompatible,
                    "显示不兼容的版本",
                    move |_, cx| {
                        let _ = view.update(cx, |switch, cx| {
                            switch.show_incompatible = !switch.show_incompatible;
                            cx.notify();
                        });
                    },
                ))
        };

        let right: AnyElement = match self.chosen() {
            None => div().into_any_element(),
            Some(version) => {
                let date = version.published.get(..10).unwrap_or_default().to_owned();
                let fits = self.fits(version);
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_base()
                                    .font_semibold()
                                    .child(version.number.clone()),
                            )
                            .child(kit::chip(channel_label(version.channel), None, colors))
                            .child(div().flex_1())
                            .child(div().text_xs().text_color(colors.muted).child(date)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if fits { colors.muted } else { colors.danger })
                            .child(format!(
                                "{} · {}{}",
                                version.loaders.join(" / "),
                                version
                                    .game_versions
                                    .iter()
                                    .rev()
                                    .take(4)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", "),
                                if fits {
                                    ""
                                } else {
                                    " · 和这个游戏不兼容"
                                }
                            )),
                    )
                    .child(
                        div()
                            .id("switch-changelog")
                            .h(px(260.))
                            .overflow_y_scroll()
                            .text_sm()
                            .child(if version.changelog.trim().is_empty() {
                                div()
                                    .text_color(colors.muted)
                                    .child("这个版本没有写更新日志")
                                    .into_any_element()
                            } else {
                                TextView::markdown(
                                    "switch-changelog-text",
                                    version.changelog.clone(),
                                )
                                .selectable(true)
                                .into_any_element()
                            }),
                    )
                    .into_any_element()
            }
        };

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .gap_4()
                    .items_start()
                    .child(
                        v_flex()
                            .w(px(280.))
                            .flex_none()
                            .gap_2()
                            .child(
                                Input::new(&self.query).small().prefix(
                                    Icon::new(UiIcon::Search)
                                        .size(px(14.))
                                        .text_color(colors.muted),
                                ),
                            )
                            .child(left)
                            .child(toggle),
                    )
                    .child(v_flex().flex_1().min_w_0().child(right)),
            )
            .children(self.error.clone().map(|message| {
                div()
                    .text_sm()
                    .text_color(colors.danger)
                    .debug_selector(|| "switch-error".into())
                    .child(message)
            }))
    }
}

/// The selection, kept per kind: file names.
pub type Selection = BTreeSet<String>;

#[cfg(test)]
mod tests {
    use super::*;
    use lumilio_core::{ContentItem, ContentSource};

    fn entry(name: &str, enabled: bool, title: Option<&str>, update: bool) -> ContentEntry {
        ContentEntry {
            item: ContentItem {
                file_name: name.to_owned(),
                display_name: name.trim_end_matches(".disabled").to_owned(),
                enabled,
                size: 1,
                modified: 0,
                is_directory: false,
            },
            sha1: Some("h".into()),
            source: title.map(|title| ContentSource {
                project_id: "P".into(),
                slug: "p".into(),
                title: title.to_owned(),
                author: Some("squeek502".into()),
                icon_url: None,
                version_id: "v1".into(),
                version_number: "1".into(),
            }),
            update: update.then(|| Version {
                id: "v2".into(),
                project_id: "P".into(),
                name: "2".into(),
                number: "2".into(),
                channel: ReleaseChannel::Release,
                game_versions: vec![],
                loaders: vec![],
                published: String::new(),
                files: vec![],
                dependencies: vec![],
                downloads: 0,
                changelog: String::new(),
            }),
        }
    }

    #[test]
    fn filters_and_search_narrow_the_list() {
        let entries = [
            entry("appleskin.jar", true, Some("AppleSkin"), true),
            entry("mine.jar", true, None, false),
            entry("off.jar.disabled", false, Some("Off"), false),
        ];
        let titles = |shown: Vec<&ContentEntry>| {
            shown
                .iter()
                .map(|e| title_of(e).to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(visible(&entries, "", ContentFilter::All).len(), 3);
        assert_eq!(
            titles(visible(&entries, "", ContentFilter::Updates)),
            ["AppleSkin"]
        );
        assert_eq!(
            titles(visible(&entries, "", ContentFilter::Disabled)),
            ["Off"]
        );
        assert_eq!(
            titles(visible(&entries, "", ContentFilter::Unknown)),
            ["mine.jar"]
        );
        assert_eq!(
            titles(visible(&entries, "apple", ContentFilter::All)),
            ["AppleSkin"]
        );
        assert_eq!(
            visible(&entries, "squeek", ContentFilter::All).len(),
            2,
            "the author matches too"
        );
        assert_eq!(
            titles(visible(&entries, "MINE", ContentFilter::All)),
            ["mine.jar"]
        );
    }
}
