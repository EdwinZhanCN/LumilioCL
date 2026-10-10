use super::super::{Dropdown, InstanceDetailView, InstanceIntent, Section};
use super::data::Confirm;
use super::helpers::{act, act_index, clock};
use super::{WORLD_SORTS, WORLD_SUBS};
use crate::assets::UiIcon;
use crate::kit;
use crate::theme::ShellColors;
use crate::toast::Toast;
use crate::tr;
use gpui::prelude::*;
use gpui::{AnyElement, Context, ObjectFit, Window, div, img, px};
use gpui_component::IndexPath;
use gpui_component::Sizable as _;
use gpui_component::input::Input;
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_component::{Icon, h_flex, v_flex};
use lumilio_core::WorldInfo;

/// The world a running game most likely has open: the one whose lock file was
/// touched after the game started, the newest of them.
#[must_use]
pub fn playing_world(worlds: &[WorldInfo], started_ms: i64) -> Option<&str> {
    worlds
        .iter()
        .filter_map(|world| Some((world.lock_touched_ms.filter(|at| *at >= started_ms)?, world)))
        .max_by_key(|(at, _)| *at)
        .map(|(_, world)| world.folder.as_str())
}

/// A world row's leading mark: the cover the player chose in the game, or a
/// procedural cover seeded by the folder when there is none.
fn cover_lead(
    world: &WorldInfo,
    loader: Option<lumilio_core::Loader>,
    colors: ShellColors,
) -> AnyElement {
    let tile = |inner: AnyElement| {
        div()
            .relative()
            .flex_none()
            .size(px(40.))
            .overflow_hidden()
            .bg(colors.body.display)
            .child(inner)
            .into_any_element()
    };
    if let Some(path) = &world.icon {
        return tile(
            img(path.clone())
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
        );
    }
    let loader = loader.map_or(crate::cover::Loader::Vanilla, crate::live::cover_loader);
    tile(
        crate::cover::element(
            crate::live::seed_of(&world.folder),
            loader,
            crate::home::WorldHint::Overworld,
            colors.body.display,
            px(0.),
        )
        .into_any_element(),
    )
}

/// The worlds that match `query` (in the name or the folder, ignoring case),
/// most recently played first, or by name.
pub fn ordered_worlds<'a>(worlds: &'a [WorldInfo], query: &str, sort: usize) -> Vec<&'a WorldInfo> {
    let query = query.trim().to_lowercase();
    let mut shown: Vec<_> = worlds
        .iter()
        .filter(|world| {
            query.is_empty()
                || world.name.to_lowercase().contains(&query)
                || world.folder.to_lowercase().contains(&query)
        })
        .collect();
    if sort == 1 {
        shown.sort_by_key(|world| world.name.to_lowercase());
    } else {
        shown.sort_by_key(|world| std::cmp::Reverse(world.last_played_ms));
    }
    shown
}

impl InstanceDetailView {
    /// World archives dropped on the Worlds tab: one at a time, each its own
    /// write.
    pub(in super::super) fn import_dropped(
        &mut self,
        paths: &[std::path::PathBuf],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let zips: Vec<_> = paths
            .iter()
            .filter(|path| {
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
            })
            .collect();
        match zips.as_slice() {
            [] => self.toast(Toast::info(tr!("instance-world-drop-unsupported")), cx),
            [only] => self.send(InstanceIntent::AddWorld((*only).clone()), window, cx),
            [first, ..] => {
                self.toast(Toast::info(tr!("instance-world-drop-one")), cx);
                self.send(InstanceIntent::AddWorld((*first).clone()), window, cx);
            }
        }
    }

    /// The 世界 tab: saved worlds or the multiplayer server list.
    pub(in super::super) fn worlds_panel(
        &mut self,
        window: &mut Window,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sort_select = self.ensure_world_sort_select(window, cx);
        let running = self.live_output.is_some();
        // While a game runs the instance is in use: nothing here can change.
        let busy = self.busy || running;
        // The selected sub-view's actions sit on the segment row, as on the
        // Content tab's L4a.
        let actions: AnyElement = if self.worlds_sub == 1 {
            // ia[instance.worlds]: 刷新状态 | L4 次要「刷新状态」 | 逐个检查服务器：在线人数、延迟、版本和 MOTD；连不上显示“无法连接”（5 秒超时）
            let refresh = kit::action(
                "server-refresh",
                tr!("instance-server-refresh"),
                Some(UiIcon::Refresh),
                false,
                act(cx, |view, _, cx| {
                    view.ping_servers = true;
                    cx.notify();
                }),
            )
            .debug_selector(|| "server-refresh".into());
            // ia[instance.worlds]: 添加服务器 | L4 次要「添加服务器」→ 弹窗（名称、地址） | 追加到服务器列表，游戏里立刻可见；游戏运行时不可改
            let add = kit::action(
                "server-add",
                tr!("instance-server-add"),
                Some(UiIcon::Plus),
                false,
                act(cx, |view, window, cx| {
                    view.open_server_editor(None, window, cx)
                }),
            )
            .disabled(busy)
            .debug_selector(|| "server-add".into());
            h_flex()
                .gap_2()
                .items_center()
                .child(refresh)
                .child(add)
                .into_any_element()
        } else {
            // ia[instance.worlds]: 导入世界 | L4 次要「导入世界」→ 选 .zip；也可把 .zip 拖进世界页（一次一个） | 识别含 level.dat 的最浅文件夹，解压到 saves，重名自动加序号，不覆盖
            kit::action(
                "world-import",
                tr!("instance-world-import"),
                Some(UiIcon::Download),
                false,
                act(cx, |view, window, cx| {
                    (view.handler)(InstanceIntent::ImportWorld, window, cx)
                }),
            )
            .disabled(busy)
            .debug_selector(|| "world-import".into())
            .into_any_element()
        };
        let body = if self.worlds_sub == 1 {
            self.servers_panel(colors, cx)
        } else {
            self.worlds_list_panel(&sort_select, colors, cx)
        };
        v_flex()
            .w_full()
            .gap_4()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    // ia[instance.worlds]: 切换世界 / 服务器 | L4 分段：世界 / 服务器 | 视图状态；第一次进入服务器时读取列表并检查各服务器状态
                    .child(kit::segments(
                        "instance-worlds-sub",
                        &WORLD_SUBS,
                        self.worlds_sub.min(WORLD_SUBS.len() - 1),
                        act_index(cx, |view, index: usize, window, cx| {
                            view.open_worlds_sub(index, window, cx)
                        }),
                    ))
                    .child(actions),
            )
            .child(body)
            .into_any_element()
    }

    /// The Worlds sort dropdown, created on first use.
    fn ensure_world_sort_select(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Dropdown {
        if let Some(select) = &self.world_sort_select {
            return select.clone();
        }
        let labels: Vec<String> = WORLD_SORTS.iter().map(|sort| (*sort).to_owned()).collect();
        let select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(labels),
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &select,
            window,
            |this, _, event: &SelectEvent<SearchableVec<String>>, _, cx| {
                if let SelectEvent::Confirm(Some(label)) = event
                    && let Some(index) = WORLD_SORTS.iter().position(|text| *text == label)
                {
                    this.world_sort = index;
                    cx.notify();
                }
            },
        )
        .detach();
        self.world_sort_select = Some(select.clone());
        select
    }

    fn worlds_list_panel(
        &self,
        sort_select: &Dropdown,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(status) = self.status(&self.data.worlds, colors, Section::Worlds, cx) {
            return status;
        }
        let Some(Ok(worlds)) = &self.data.worlds else {
            return div().into_any_element();
        };
        let running = self.live_output.is_some();
        // While a game runs the instance is in use: nothing here can change.
        let busy = self.busy || running;
        let playing = self
            .game_started_ms
            .filter(|_| running)
            .and_then(|started| playing_world(worlds, started));
        if worlds.is_empty() {
            return kit::empty(
                tr!("instance-worlds-empty"),
                tr!("instance-worlds-empty-help"),
                colors,
            )
            .into_any_element();
        }
        let query = self
            .fields
            .as_ref()
            .map(|fields| fields.world_search.read(cx).value().to_string())
            .unwrap_or_default();
        let shown = ordered_worlds(worlds, &query, self.world_sort);
        let entity = cx.entity().downgrade();
        let rows: Vec<_> = shown
            .iter()
            .enumerate()
            .map(|(row, world)| {
                let mut detail = Vec::new();
                if playing == Some(world.folder.as_str()) {
                    detail.push(tr!("instance-world-playing").to_owned());
                }
                if world.damaged {
                    detail.push(tr!("instance-world-damaged").to_owned());
                }
                if world.hardcore {
                    detail.push(tr!("home-place-hardcore").to_owned());
                }
                if let Some(version) = &world.game_version {
                    detail.push(version.clone());
                }
                if let Some(played) = world.last_played_ms.filter(|ms| *ms > 0) {
                    detail.push(tr!(
                        "instance-world-played-at",
                        when = clock(played as u64 / 1000)
                    ));
                }
                if detail.is_empty() {
                    detail.push(world.folder.clone());
                }
                let folder = world.folder.clone();
                let too_old = self.record.as_ref().is_some_and(|record| {
                    lumilio_core::quick_play_world_unsupported(&record.game_version)
                });
                // ia[instance.worlds]: 进入世界 | 世界行「进入」 | 启动并直达该世界；1.20 以前的版本置灰并说明
                let enter = {
                    let folder = folder.clone();
                    let button = kit::ghost(
                        ("world-play", row),
                        tr!("instance-world-enter"),
                        act(cx, move |view, window, cx| {
                            view.send(InstanceIntent::PlayWorld(folder.clone()), window, cx)
                        }),
                    )
                    .disabled(busy || world.damaged || too_old)
                    .debug_selector(move || format!("world-play-{row}"));
                    if too_old {
                        button.tooltip(tr!("instance-world-old-version"))
                    } else {
                        button
                    }
                };
                // Writes go through `send` (one at a time); the rest hand the
                // application a request it answers itself.
                let entry = |label: &'static str, intent: InstanceIntent, write: bool| {
                    let entity = entity.clone();
                    kit::MenuEntry::new(label, move |window, app| {
                        let intent = intent.clone();
                        let _ = entity.update(app, |view, cx| {
                            if write {
                                view.send(intent, window, cx);
                            } else {
                                (view.handler)(intent, window, cx);
                            }
                        });
                    })
                    .disabled(busy)
                };
                let menu = kit::more_menu(
                    ("world-more", row),
                    vec![
                        // ia[instance.worlds]: 复制世界 | 世界行 ⋯ 菜单 | 复制到新文件夹（重名自动加序号）→ toast
                        entry(
                            tr!("instance-world-copy"),
                            InstanceIntent::CopyWorld(folder.clone()),
                            true,
                        ),
                        // ia[instance.worlds]: 创建备份 | 世界行 ⋯ 菜单 | 仅这个世界的快照 → 历史·快照可见
                        entry(
                            tr!("instance-world-backup"),
                            InstanceIntent::BackupWorld(folder.clone()),
                            true,
                        ),
                        // ia[instance.worlds]: 导出为 .zip | 世界行 ⋯ 菜单 → 选位置 | 后台打包，toast；不含 session.lock
                        entry(
                            tr!("instance-world-export"),
                            InstanceIntent::ExportWorld(folder.clone()),
                            false,
                        ),
                        // ia[instance.worlds]: 在访达中显示 | 世界行 ⋯ 菜单 | 打开该世界的目录
                        entry(
                            crate::platform::reveal_label(),
                            InstanceIntent::RevealPath(format!("saves/{folder}")),
                            false,
                        ),
                    ],
                    colors,
                );
                let trail = h_flex()
                    .gap_1()
                    .items_center()
                    .child(enter)
                    // ia[instance.worlds]: 删除世界 | 世界行 🗑 → 警告弹窗 | 删除世界文件夹，写历史
                    .child(self.asking(
                        ("world-delete", row),
                        tr!("common-delete"),
                        Confirm::DeleteWorld(folder),
                        cx,
                    ))
                    .child(menu);
                let lead = cover_lead(
                    world,
                    self.record.as_ref().map(|record| record.loader),
                    colors,
                );
                kit::row(
                    world.name.clone(),
                    detail.join(" · "),
                    Some(lead),
                    Some(trail.into_any_element()),
                    colors,
                )
            })
            .collect();
        let controls = h_flex()
            .w_full()
            .items_center()
            .gap_3()
            .children(self.fields.as_ref().map(|fields| {
                // ia[instance.worlds]: 搜索 | L4 搜索框 | 按世界名称或文件夹名过滤
                div().w(px(220.)).child(
                    Input::new(&fields.world_search).small().prefix(
                        Icon::new(UiIcon::Search)
                            .size(px(14.))
                            .text_color(colors.muted),
                    ),
                )
            }))
            // ia[instance.worlds]: 排序 | L4 下拉：最近游玩 / 名称 | 视图状态
            .child(div().w(px(140.)).child(Select::new(sort_select).small()));
        v_flex()
            .id("instance-worlds-drop")
            .w_full()
            .gap_3()
            .on_drop(
                cx.listener(|view, paths: &gpui::ExternalPaths, window, cx| {
                    view.import_dropped(paths.paths(), window, cx)
                }),
            )
            .child(controls)
            .child(div().text_sm().text_color(colors.muted).child(if running {
                tr!("instance-worlds-running")
            } else {
                tr!("instance-worlds-help")
            }))
            .child(if rows.is_empty() {
                kit::empty(
                    tr!("instance-worlds-no-match"),
                    tr!("library-no-match-help"),
                    colors,
                )
                .into_any_element()
            } else {
                kit::list(rows, colors).into_any_element()
            })
            .into_any_element()
    }
}
