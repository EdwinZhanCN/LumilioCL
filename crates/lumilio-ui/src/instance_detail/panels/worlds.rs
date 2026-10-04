use super::super::{InstanceDetailView, InstanceIntent, Section};
use super::WORLD_SORTS;
use super::data::Confirm;
use super::helpers::{act, act_index, clock};
use crate::assets::UiIcon;
use crate::kit;
use crate::theme::ShellColors;
use crate::toast::Toast;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Window, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::Input;
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
            [] => self.toast(Toast::info("这里只能放世界的 .zip"), cx),
            [only] => self.send(InstanceIntent::AddWorld((*only).clone()), window, cx),
            [first, ..] => {
                self.toast(Toast::info("一次只导入一个世界，先导入了第一个"), cx);
                self.send(InstanceIntent::AddWorld((*first).clone()), window, cx);
            }
        }
    }

    pub(in super::super) fn worlds_panel(
        &self,
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
        // ia[instance.worlds]: 导入世界 | L2 次要「导入世界」→ 选 .zip；也可把 .zip 拖进世界页（一次一个） | 识别含 level.dat 的最浅文件夹，解压到 saves，重名自动加序号，不覆盖 | H-WORLD-02
        let import = kit::action(
            "world-import",
            "导入世界",
            Some(UiIcon::Download),
            false,
            act(cx, |view, window, cx| {
                (view.handler)(InstanceIntent::ImportWorld, window, cx)
            }),
        )
        .disabled(busy)
        .debug_selector(|| "world-import".into());
        if worlds.is_empty() {
            return v_flex()
                .w_full()
                .gap_3()
                .child(h_flex().justify_end().child(import))
                .child(kit::empty(
                    "还没有世界",
                    "进入游戏创建的世界会出现在这里，也可以导入一个 .zip",
                    colors,
                ))
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
                    detail.push("正在游玩".to_owned());
                }
                if world.damaged {
                    detail.push("存档信息无法读取".to_owned());
                }
                if world.hardcore {
                    detail.push("极限模式".to_owned());
                }
                if let Some(version) = &world.game_version {
                    detail.push(version.clone());
                }
                if let Some(played) = world.last_played_ms.filter(|ms| *ms > 0) {
                    detail.push(format!("{}游玩", clock(played as u64 / 1000)));
                }
                if detail.is_empty() {
                    detail.push(world.folder.clone());
                }
                let folder = world.folder.clone();
                let too_old = self.record.as_ref().is_some_and(|record| {
                    lumilio_core::quick_play_world_unsupported(&record.game_version)
                });
                // ia[instance.worlds]: 进入世界 | 世界行「进入」 | 启动并直达该世界；1.20 以前的版本置灰并说明 | H-PLAY-07、L-PLAY-02
                let enter = {
                    let folder = folder.clone();
                    let button = kit::ghost(
                        ("world-play", row),
                        "进入",
                        act(cx, move |view, window, cx| {
                            view.send(InstanceIntent::PlayWorld(folder.clone()), window, cx)
                        }),
                    )
                    .disabled(busy || world.damaged || too_old)
                    .debug_selector(move || format!("world-play-{row}"));
                    if too_old {
                        button.tooltip("这个游戏版本不能直接进入世界，请从主菜单进入")
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
                        // ia[instance.worlds]: 复制世界 | 世界行 ⋯ 菜单 | 复制到新文件夹（重名自动加序号）→ toast | H-WORLD-07
                        entry("复制", InstanceIntent::CopyWorld(folder.clone()), true),
                        // ia[instance.worlds]: 创建备份 | 世界行 ⋯ 菜单 | 仅这个世界的快照 → 历史·快照可见 | H-WORLD-10、L-HIST-01
                        entry(
                            "创建备份",
                            InstanceIntent::BackupWorld(folder.clone()),
                            true,
                        ),
                        // ia[instance.worlds]: 导出为 .zip | 世界行 ⋯ 菜单 → 选位置 | 后台打包，toast；不含 session.lock | H-WORLD-09
                        entry(
                            "导出为 .zip…",
                            InstanceIntent::ExportWorld(folder.clone()),
                            false,
                        ),
                        // ia[instance.worlds]: 在访达中显示 | 世界行 ⋯ 菜单 | 打开该世界的目录 | H-WORLD-12
                        entry(
                            "在访达中显示",
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
                    // ia[instance.worlds]: 删除世界 | 世界行 🗑 → 警告弹窗 | 删除世界文件夹，写历史 | H-WORLD-08
                    .child(self.asking(
                        ("world-delete", row),
                        "删除",
                        Confirm::DeleteWorld(folder),
                        cx,
                    ))
                    .child(menu);
                kit::row(
                    world.name.clone(),
                    detail.join(" · "),
                    None,
                    Some(trail.into_any_element()),
                    colors,
                )
            })
            .collect();
        let controls = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            // ia[instance.worlds]: 排序 | L4 分段：最近游玩 / 名称 | 视图状态 | H-WORLD-01
            .child(kit::segments(
                "instance-world-sort",
                &WORLD_SORTS,
                self.world_sort.min(WORLD_SORTS.len() - 1),
                act_index(cx, |view, index: usize, _, cx| {
                    view.world_sort = index;
                    cx.notify();
                }),
            ))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .children(self.fields.as_ref().map(|fields| {
                        // ia[instance.worlds]: 搜索 | L4 搜索框 | 按世界名称或文件夹名过滤 | H-WORLD-01
                        div().w(px(220.)).child(
                            Input::new(&fields.world_search).small().prefix(
                                Icon::new(UiIcon::Search)
                                    .size(px(14.))
                                    .text_color(colors.muted),
                            ),
                        )
                    }))
                    .child(import),
            );
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
                "游戏正在运行，世界先不能复制、备份、导出或删除；结束游戏后再来。"
            } else {
                "复制和导入总是另存为新世界，不会覆盖已有的世界。游戏运行时不能修改。"
            }))
            .child(if rows.is_empty() {
                kit::empty("没有匹配的世界", "换个关键词试试", colors).into_any_element()
            } else {
                kit::list(rows, colors).into_any_element()
            })
            .into_any_element()
    }
}
