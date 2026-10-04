//! 世界 › 服务器: the instance's multiplayer list (`servers.dat`).

use super::super::editors::Editor;
use super::super::{InstanceDetailView, InstanceIntent, Section};
use super::data::{Confirm, ServerState};
use super::helpers::act;
use crate::assets::UiIcon;
use crate::kit;
use crate::theme::ShellColors;
use crate::toast::Toast;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Window, div};
use gpui_component::{h_flex, v_flex};
use lumilio_core::ServerEntry;

/// What a server row says under its name: where it is and how it is.
#[must_use]
pub fn server_detail(address: &str, state: Option<&ServerState>) -> String {
    match state {
        None | Some(ServerState::Checking) => format!("{address} · 正在检查…"),
        Some(ServerState::Offline) => format!("{address} · 无法连接"),
        Some(ServerState::Online(status)) => {
            let mut parts = vec![address.to_owned()];
            if let Some(online) = status.online {
                parts.push(match status.max {
                    Some(max) => format!("{online}/{max} 在线"),
                    None => format!("{online} 在线"),
                });
            }
            if let Some(ms) = status.latency_ms {
                parts.push(format!("{ms} ms"));
            }
            if let Some(version) = &status.version {
                parts.push(version.clone());
            }
            let motd = status.motd.split('\n').next().unwrap_or_default().trim();
            if !motd.is_empty() {
                parts.push(motd.chars().take(40).collect());
            }
            parts.join(" · ")
        }
    }
}

impl InstanceDetailView {
    pub(super) fn servers_panel(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        if let Some(status) = self.status(&self.data.servers, colors, Section::Servers, cx) {
            return status;
        }
        let Some(Ok(servers)) = &self.data.servers else {
            return div().into_any_element();
        };
        let running = self.live_output.is_some();
        // While a game runs it owns the file: nothing here can change.
        let busy = self.busy || running;
        // ia[instance.worlds]: 添加服务器 | L2 次要「添加服务器」→ 弹窗（名称、地址） | 追加到服务器列表，游戏里立刻可见；游戏运行时不可改
        let add = kit::action(
            "server-add",
            "添加服务器",
            Some(UiIcon::Plus),
            false,
            act(cx, |view, window, cx| {
                view.open_server_editor(None, window, cx)
            }),
        )
        .disabled(busy)
        .debug_selector(|| "server-add".into());
        // ia[instance.worlds]: 刷新状态 | L2 次要「刷新状态」 | 逐个检查服务器：在线人数、延迟、版本和 MOTD；连不上显示“无法连接”（5 秒超时）
        let refresh = kit::action(
            "server-refresh",
            "刷新状态",
            Some(UiIcon::Refresh),
            false,
            act(cx, |view, _, cx| {
                view.ping_servers = true;
                cx.notify();
            }),
        )
        .debug_selector(|| "server-refresh".into());
        let controls = h_flex()
            .w_full()
            .justify_end()
            .gap_2()
            .child(refresh)
            .child(add);
        if servers.is_empty() {
            return v_flex()
                .w_full()
                .gap_3()
                .child(controls)
                .child(kit::empty(
                    "还没有服务器",
                    "添加后，游戏的多人游戏列表里就有它",
                    colors,
                ))
                .into_any_element();
        }
        let entity = cx.entity().downgrade();
        let too_old = self
            .record
            .as_ref()
            .is_some_and(|record| lumilio_core::quick_play_world_unsupported(&record.game_version));
        let last = servers.len() - 1;
        let rows: Vec<_> = servers
            .iter()
            .enumerate()
            .map(|(row, server)| {
                let detail =
                    server_detail(&server.address, self.server_status.get(&server.address));
                let address = server.address.clone();
                // ia[instance.worlds]: 进入服务器 | 服务器行「进入」 | 启动并直连该服务器；1.20 以前的版本置灰并说明
                let enter = {
                    let button = kit::ghost(
                        ("server-play", row),
                        "进入",
                        act(cx, move |view, window, cx| {
                            view.send(InstanceIntent::PlayServer(address.clone()), window, cx)
                        }),
                    )
                    .disabled(busy || too_old)
                    .debug_selector(move || format!("server-play-{row}"));
                    if too_old {
                        button.tooltip("这个游戏版本不能直接进入服务器，请从多人游戏列表进入")
                    } else {
                        button
                    }
                };
                let entry = |label: &'static str,
                             disabled: bool,
                             run: fn(
                    &mut InstanceDetailView,
                    usize,
                    &ServerEntry,
                    &mut Window,
                    &mut Context<InstanceDetailView>,
                )| {
                    let entity = entity.clone();
                    let server = server.clone();
                    kit::MenuEntry::new(label, move |window, app| {
                        let server = server.clone();
                        let _ = entity.update(app, |view, cx| run(view, row, &server, window, cx));
                    })
                    .disabled(disabled)
                };
                let menu = kit::more_menu(
                    ("server-more", row),
                    vec![
                        // ia[instance.worlds]: 编辑服务器 | 服务器行 ⋯ 菜单 → 弹窗 | 改名称或地址，保留游戏记下的图标等其他信息
                        entry("编辑", busy, |view, index, _, window, cx| {
                            view.open_server_editor(Some(index), window, cx)
                        }),
                        // ia[instance.worlds]: 排序服务器 | 服务器行 ⋯ 菜单「上移 / 下移」 | 调整在游戏里的显示顺序
                        entry(
                            "上移",
                            busy || row == 0,
                            |view, index, server, window, cx| {
                                view.send(
                                    InstanceIntent::MoveServer {
                                        index,
                                        expected: server.clone(),
                                        to: index.saturating_sub(1),
                                    },
                                    window,
                                    cx,
                                )
                            },
                        ),
                        entry(
                            "下移",
                            busy || row == last,
                            |view, index, server, window, cx| {
                                view.send(
                                    InstanceIntent::MoveServer {
                                        index,
                                        expected: server.clone(),
                                        to: index + 1,
                                    },
                                    window,
                                    cx,
                                )
                            },
                        ),
                        // ia[instance.worlds]: 复制服务器地址 | 服务器行 ⋯ 菜单 | 复制到剪贴板，toast“已复制地址”
                        entry("复制地址", false, |view, _, server, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                server.address.clone(),
                            ));
                            view.toast(Toast::info("已复制地址"), cx);
                        }),
                    ],
                    colors,
                );
                let trail = h_flex()
                    .gap_1()
                    .items_center()
                    .child(enter)
                    // ia[instance.worlds]: 删除服务器 | 服务器行 🗑 → 警告弹窗 | 只从列表移除，不影响服务器本身
                    .child(self.asking(
                        ("server-delete", row),
                        "删除",
                        Confirm::DeleteServer {
                            index: row,
                            entry: server.clone(),
                        },
                        cx,
                    ))
                    .child(menu);
                kit::row(
                    server.name.clone(),
                    detail,
                    None,
                    Some(trail.into_any_element()),
                    colors,
                )
            })
            .collect();
        v_flex()
            .w_full()
            .gap_3()
            .child(controls)
            .child(div().text_sm().text_color(colors.muted).child(if running {
                "游戏正在运行，服务器列表先不能修改；结束游戏后再来。"
            } else {
                "这里的改动就是游戏里的多人游戏列表。游戏运行时不能修改。"
            }))
            .child(kit::list(rows, colors))
            .into_any_element()
    }

    /// Opens the add (`None`) or edit dialog for a server.
    pub(in super::super) fn open_server_editor(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = index.and_then(|index| {
            let Some(Ok(servers)) = &self.data.servers else {
                return None;
            };
            Some((index, servers.get(index)?.clone()))
        });
        if index.is_some() && current.is_none() {
            return;
        }
        self.ensure_fields(window, cx);
        if let Some(fields) = &self.fields {
            let (name, address) = current
                .as_ref()
                .map(|(_, entry)| (entry.name.clone(), entry.address.clone()))
                .unwrap_or_default();
            fields
                .server_name
                .update(cx, |field, cx| field.set_value(name, window, cx));
            fields
                .server_address
                .update(cx, |field, cx| field.set_value(address, window, cx));
        }
        self.server_edit = current;
        self.open_editor(Editor::Server, window, cx);
    }

    /// Saves what the server dialog holds.
    pub(in super::super) fn submit_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(fields) = &self.fields else { return };
        let name = fields.server_name.read(cx).value().trim().to_owned();
        let address = fields.server_address.read(cx).value().trim().to_owned();
        if name.is_empty() || address.is_empty() {
            self.editor_error = Some("请输入名称和地址".into());
            cx.notify();
            return;
        }
        let (index, expected, packs) = match self.server_edit.clone() {
            Some((index, entry)) => (Some(index), Some(entry.clone()), entry.packs),
            None => (None, None, lumilio_core::PackPolicy::Ask),
        };
        self.send(
            InstanceIntent::SaveServer {
                index,
                expected,
                entry: ServerEntry {
                    name,
                    address,
                    packs,
                },
            },
            window,
            cx,
        );
    }
}
