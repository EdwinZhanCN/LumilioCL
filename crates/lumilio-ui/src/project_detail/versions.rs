//! The Versions tab: filters (channel, game version, platform), the table,
//! and its pages. After `ProjectPageVersions.vue` and
//! `VersionFilterControl.vue` (GPL-3.0-only; ADR 0022).

use super::{
    DetailIntent, ProjectDetailView, VERSIONS_PER_PAGE, channel_label, date_label, file_of, pill,
    size_label, toggle,
};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::kit;
use crate::live::{PageItem, page_items, parse_rfc3339, relative_time, tag_label};
use crate::theme::ShellColors;
use crate::tr;
use gpui::prelude::*;
use gpui::{Context, Entity, Window, div, px};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_component::{Icon, Sizable as _, StyledExt as _, h_flex, v_flex};
use lumilio_core::{ProjectDetail, ProjectKind, ReleaseChannel, Version, version_groups};

/// Game version groups a row lists before "+N".
const GROUPS_SHOWN: usize = 3;
/// Platforms a row lists before "+N".
const PLATFORMS_SHOWN: usize = 2;

pub struct Picker {
    state: Entity<SelectState<SearchableVec<String>>>,
    /// What the list was last filled for, to refill when it changes.
    key: (bool, usize),
}

fn channels() -> [(ReleaseChannel, &'static str); 3] {
    [
        (ReleaseChannel::Release, tr!("project-channel-release")),
        (ReleaseChannel::Beta, tr!("project-channel-beta")),
        (ReleaseChannel::Alpha, tr!("project-channel-alpha")),
    ]
}

impl ProjectDetailView {
    /// The game versions the list offers: the project's own.
    fn pickable_versions(&self, versions: &[Version]) -> Vec<String> {
        let mut seen: Vec<&String> = Vec::new();
        for version in versions {
            for game in &version.game_versions {
                if !seen.contains(&game) {
                    seen.push(game);
                }
            }
        }
        let index = |name: &String| self.game_tags.iter().position(|tag| &tag.version == name);
        seen.sort_by_key(|name| index(name).unwrap_or(usize::MAX));
        seen.into_iter().cloned().collect()
    }

    fn ensure_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SelectState<SearchableVec<String>>> {
        if let Some(picker) = &self.version_picker {
            return picker.state.clone();
        }
        let state = cx.new(|cx| {
            SelectState::new(SearchableVec::new(Vec::<String>::new()), None, window, cx)
                .searchable(true)
        });
        cx.subscribe_in(
            &state,
            window,
            |this, state, event: &SelectEvent<SearchableVec<String>>, window, cx| {
                if let SelectEvent::Confirm(Some(version)) = event {
                    let version = version.clone();
                    this.filter_versions(
                        |filters| {
                            if !filters.game_versions.contains(&version) {
                                filters.game_versions.push(version);
                            }
                        },
                        cx,
                    );
                    // The picker only adds; it does not hold a choice.
                    state.update(cx, |select, cx| select.set_selected_index(None, window, cx));
                }
            },
        )
        .detach();
        self.version_picker = Some(Picker {
            state: state.clone(),
            key: (false, usize::MAX),
        });
        state
    }

    /// Keeps the picker's list in step with the toggle and the versions.
    fn fill_picker(&mut self, versions: &[Version], window: &mut Window, cx: &mut Context<Self>) {
        let state = self.ensure_picker(window, cx);
        let items = self.pickable_versions(versions);
        let key = (false, items.len());
        if self
            .version_picker
            .as_ref()
            .is_some_and(|picker| picker.key != key)
        {
            state.update(cx, |select, cx| {
                select.set_items(SearchableVec::new(items), window, cx)
            });
            if let Some(picker) = &mut self.version_picker {
                picker.key = key;
            }
        }
    }

    // ia[discover]: 安装指定版本 | 详情页版本行「安装」/「切换」/「已安装」 | 后台任务；已装的这一版显示已安装，已装别的版本显示切换（换成这一版）；与目标游戏不兼容的版本禁用
    pub(super) fn version_action(
        &self,
        project: &lumilio_core::Project,
        version: &Version,
        id: impl Into<gpui::ElementId>,
    ) -> Option<Key> {
        if project.kind == ProjectKind::Modpack {
            return None;
        }
        let fits = self.fits_target(project, version);
        let installed_version = self.installed_version(project);
        let installed_file = self
            .installed
            .get(&project.id)
            .map(|have| have.file_name.clone());
        let (label, off, switching) = match (installed_version, &installed_file) {
            (Some(have), _) if have == version.id => (tr!("discover-installed"), true, false),
            (Some(_), Some(_)) => (tr!("project-switch"), !fits, true),
            _ => (
                tr!("discover-install"),
                !fits || self.target.is_none(),
                false,
            ),
        };
        let (handler, version_id, title) = (
            self.handler.clone(),
            version.id.clone(),
            self.title.to_string(),
        );
        Some(
            Key::new(id)
                .label(label)
                .icon(Icon::new(if switching {
                    UiIcon::Switch
                } else {
                    UiIcon::Download
                }))
                .white()
                .small()
                .loading(self.installing)
                .disabled(off || self.installing)
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    let intent = match (&installed_file, switching) {
                        (Some(file_name), true) => DetailIntent::Update {
                            file_name: file_name.clone(),
                            version_id: version_id.clone(),
                            title: title.clone(),
                        },
                        _ => DetailIntent::Install {
                            version_id: Some(version_id.clone()),
                            title: title.clone(),
                        },
                    };
                    handler(intent, window, cx);
                }),
        )
    }

    fn channel_badge(&self, channel: ReleaseChannel, colors: ShellColors) -> impl IntoElement {
        let tone = match channel {
            ReleaseChannel::Release => kit::tone_ok(),
            ReleaseChannel::Beta => kit::tone_warn(),
            ReleaseChannel::Alpha => colors.danger,
        };
        kit::chip(channel_label(channel), Some(tone), colors)
    }

    // ia[discover]: 版本筛选 | 版本页签顶部一行：发布渠道（正式 / 测试 / 早期）、加载器、添加游戏版本（可搜索），已选的游戏版本是可点掉的标签 | 列表即时过滤；从游戏页进入时默认筛到这个游戏的版本和加载器
    fn filter_bar(
        &mut self,
        detail: &ProjectDetail,
        colors: ShellColors,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        self.fill_picker(&detail.versions, window, cx);
        let picker = self.ensure_picker(window, cx);
        let view = cx.entity();
        let mut platforms: Vec<String> = Vec::new();
        for version in &detail.versions {
            for loader in version
                .loaders
                .iter()
                .filter(|name| name.as_str() != "mrpack")
            {
                if !platforms.contains(loader) {
                    platforms.push(loader.clone());
                }
            }
        }
        let key = |id: String, label: String, on: bool| {
            let key = Key::new(gpui::ElementId::Name(id.into()))
                .label(label)
                .small();
            if on { key.primary() } else { key.white() }
        };
        let channel_list = channels();
        let channels = channel_list.iter().map(|(channel, label)| {
            let (view, channel) = (view.clone(), *channel);
            let on = self.filters.channels.contains(&channel);
            key(format!("detail-channel-{label}"), (*label).to_owned(), on).on_click(
                move |_, _, cx| {
                    view.update(cx, |view, cx| {
                        view.filter_versions(|filters| toggle(&mut filters.channels, channel), cx)
                    })
                },
            )
        });
        let loaders = platforms.into_iter().map(|name| {
            let (view, id) = (view.clone(), name.clone());
            let on = self.filters.platforms.contains(&name);
            key(format!("detail-platform-{name}"), tag_label(&name), on).on_click(
                move |_, _, cx| {
                    view.update(cx, |view, cx| {
                        view.filter_versions(
                            |filters| toggle(&mut filters.platforms, id.clone()),
                            cx,
                        )
                    })
                },
            )
        });
        let chosen = self.filters.game_versions.iter().map(|name| {
            let (view, id) = (view.clone(), name.clone());
            h_flex()
                .id(gpui::ElementId::Name(
                    format!("detail-chip-version-{name}").into(),
                ))
                .h(px(24.))
                .px(px(8.))
                .gap(px(5.))
                .items_center()
                .border_1()
                .border_color(colors.border)
                .bg(colors.body.key_grey)
                .text_xs()
                .cursor_pointer()
                .child(name.clone())
                .child(
                    Icon::new(UiIcon::Close)
                        .size(px(11.))
                        .text_color(colors.muted),
                )
                .on_click(move |_, _, cx| {
                    view.update(cx, |view, cx| {
                        view.filter_versions(
                            |filters| filters.game_versions.retain(|n| *n != id),
                            cx,
                        )
                    })
                })
        });
        let clear = self.filters.active().then(|| {
            let view = view.clone();
            kit::ghost(
                "detail-clear-filters",
                tr!("discover-clear-filters"),
                move |_, cx| {
                    view.update(cx, |view, cx| {
                        view.filter_versions(
                            |filters| {
                                filters.channels.clear();
                                filters.game_versions.clear();
                                filters.platforms.clear();
                            },
                            cx,
                        )
                    })
                },
            )
        });
        h_flex()
            .w_full()
            .flex_wrap()
            .gap_2()
            .items_center()
            .children(channels)
            .children(loaders)
            .child(
                div().w(px(170.)).child(
                    Select::new(&picker)
                        .small()
                        .placeholder(tr!("project-add-game-version")),
                ),
            )
            .children(chosen)
            .children(clear)
            .into_any_element()
    }

    pub(super) fn versions(
        &mut self,
        detail: &ProjectDetail,
        colors: ShellColors,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if detail.versions.is_empty() {
            return kit::empty(tr!("project-versions-empty"), "", colors).into_any_element();
        }
        let bar = self.filter_bar(detail, colors, window, cx);
        let project = &detail.project;
        let shown: Vec<&Version> = detail
            .versions
            .iter()
            .filter(|version| self.filters.admits(version))
            .collect();
        let pages = shown.len().div_ceil(VERSIONS_PER_PAGE).max(1);
        let page = self.filters.page.min(pages - 1);
        let view = cx.entity();
        let now = lumilio_core::unix_now();

        // The header and every row share these widths, so the columns line up.
        const CHANNEL: f32 = 84.;
        const GAME: f32 = 160.;
        const LOADER: f32 = 120.;
        const DATE: f32 = 76.;
        const ACTIONS: f32 = 150.;
        let head = |text: &'static str| {
            div()
                .font_family(crate::theme::MONO_FONT)
                .text_size(px(10.))
                .text_color(colors.muted)
                .child(text)
        };
        let table_head = h_flex()
            .w_full()
            .gap_3()
            .pb_2()
            .border_b_1()
            .border_color(colors.foreground)
            .child(
                head(tr!("project-column-channel"))
                    .w(px(CHANNEL))
                    .flex_none(),
            )
            .child(head(tr!("project-tab-versions")).flex_1().min_w_0())
            .child(
                head(tr!("discover-section-version"))
                    .w(px(GAME))
                    .flex_none(),
            )
            .child(head(tr!("library-loader")).w(px(LOADER)).flex_none())
            .child(
                head(tr!("project-column-published"))
                    .w(px(DATE))
                    .flex_none(),
            )
            .child(div().w(px(ACTIONS)).flex_none());

        let rows = shown
            .iter()
            .skip(page * VERSIONS_PER_PAGE)
            .take(VERSIONS_PER_PAGE)
            .enumerate()
            .map(|(index, version)| {
                let fits = self.fits_target(project, version);
                let handler = self.handler.clone();
                let id = version.id.clone();
                let title = self.title.to_string();
                let groups: Vec<String> = if self.game_tags.is_empty() {
                    version.game_versions.clone()
                } else {
                    version_groups(&version.game_versions, &self.game_tags)
                        .into_iter()
                        .map(|group| group.label)
                        .collect()
                };
                let game_tags = h_flex()
                    .w(px(GAME))
                    .flex_none()
                    .flex_wrap()
                    .gap(px(4.))
                    .children(
                        groups
                            .iter()
                            .take(GROUPS_SHOWN)
                            .map(|label| pill(label.clone(), colors)),
                    )
                    .children(
                        (groups.len() > GROUPS_SHOWN)
                            .then(|| pill(format!("+{}", groups.len() - GROUPS_SHOWN), colors)),
                    );
                let loaders: Vec<&String> = version
                    .loaders
                    .iter()
                    .filter(|name| name.as_str() != "mrpack")
                    .collect();
                let platform_tags = h_flex()
                    .w(px(LOADER))
                    .flex_none()
                    .flex_wrap()
                    .gap(px(4.))
                    .children(
                        loaders
                            .iter()
                            .take(PLATFORMS_SHOWN)
                            .map(|name| kit::tag(tag_label(name), kit::TagKind::Ink, colors)),
                    )
                    .children(
                        (loaders.len() > PLATFORMS_SHOWN)
                            .then(|| pill(format!("+{}", loaders.len() - PLATFORMS_SHOWN), colors)),
                    );
                // ia[discover]: 另存为文件 | 详情页版本行「另存为」图标 | 选位置下载，任何类型，包括整合包文件
                let save = file_of(version).map(|file| {
                    let (handler, id, title) = (handler.clone(), id.clone(), title.clone());
                    let file_name = file.filename.clone();
                    Key::new(("detail-version-save", index))
                        .label(tr!("project-save-as"))
                        .tooltip(tr!(
                            "project-download-version-file",
                            size = size_label(file.size)
                        ))
                        .ghost()
                        .small()
                        .debug_selector(move || format!("detail-version-save-{index}"))
                        .on_click(move |_, window, cx| {
                            handler(
                                DetailIntent::SaveAs {
                                    version_id: id.clone(),
                                    file_name: file_name.clone(),
                                    title: title.clone(),
                                },
                                window,
                                cx,
                            );
                        })
                });
                let action =
                    self.version_action(project, version, ("detail-version-install", index));
                let published = parse_rfc3339(&version.published)
                    .map(|at| relative_time(at, now))
                    .unwrap_or_else(|| date_label(&version.published));
                h_flex()
                    .debug_selector(move || format!("detail-version-{index}"))
                    .w_full()
                    .gap_3()
                    .py(px(10.))
                    .items_center()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .w(px(CHANNEL))
                            .flex_none()
                            .child(self.channel_badge(version.channel, colors)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .font_semibold()
                                    .text_color(colors.foreground)
                                    .truncate()
                                    .child(version.number.clone()),
                            )
                            .children((version.name != version.number).then(|| {
                                div()
                                    .text_xs()
                                    .text_color(colors.muted)
                                    .truncate()
                                    .child(version.name.clone())
                            }))
                            .children((!fits).then(|| pill(tr!("project-version-unfit"), colors))),
                    )
                    .child(game_tags)
                    .child(platform_tags)
                    .child(
                        div()
                            .w(px(DATE))
                            .flex_none()
                            .text_xs()
                            .text_color(colors.muted)
                            .child(published),
                    )
                    .child(
                        h_flex()
                            .w(px(ACTIONS))
                            .flex_none()
                            .gap_1()
                            .justify_end()
                            .children(action)
                            .children(save),
                    )
            })
            .collect::<Vec<_>>();

        let pager = (pages > 1).then(|| {
            h_flex().gap_1().items_center().children(
                page_items(page as u32, pages as u32)
                    .into_iter()
                    .map(|item| match item {
                        PageItem::Page(target) => {
                            let view = view.clone();
                            let active = target as usize == page;
                            let key = Key::new(("detail-page", target as usize))
                                .label((target + 1).to_string())
                                .small();
                            crate::theme::clickable(
                                if active { key.primary() } else { key.ghost() },
                                !active,
                            )
                            .on_click(move |_, _, cx| {
                                view.update(cx, |view, cx| {
                                    view.filters.page = target as usize;
                                    cx.notify();
                                })
                            })
                            .into_any_element()
                        }
                        PageItem::Gap => div()
                            .px_1()
                            .text_color(colors.muted)
                            .child("…")
                            .into_any_element(),
                    }),
            )
        });
        let body = if rows.is_empty() {
            kit::empty(
                tr!("project-versions-no-match"),
                tr!("project-versions-no-match-help"),
                colors,
            )
            .into_any_element()
        } else {
            v_flex()
                .w_full()
                .child(table_head)
                .children(rows)
                .into_any_element()
        };
        v_flex()
            .w_full()
            .gap_4()
            .debug_selector(|| "detail-versions".into())
            .child(bar)
            .child(body)
            .children(pager)
            .into_any_element()
    }
}
