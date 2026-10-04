//! The Instance page's edit dialogs (design language §10): the page shows
//! values read-only; renaming, memory and copying are edited here.
//!
//! A child module of `instance_detail`, so it shares the view's private state.

use crate::key::Key;
use gpui::{App, Context, Entity, Window, div, prelude::*, px};
use gpui_component::dialog::Dialog;
use gpui_component::input::Input;
use gpui_component::{ActiveTheme as _, StyledExt as _, WindowExt as _, h_flex, v_flex};

use super::panels::Confirm;
use super::{InstanceDetailView, InstanceIntent, Section};
use crate::{kit, theme};

/// Which edit dialog is open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Editor {
    Rename,
    Memory,
    Copy,
    Snapshot,
    Server,
}

pub const MIN_MEMORY_HELP: &str =
    "游戏启动时就占用的内存（-Xms）。留空则跟随启动器默认；默认也没有时由 Java 决定。";
pub const MAX_MEMORY_HELP: &str =
    "游戏最多能用的内存（-Xmx）。留空则跟随启动器默认；默认也没有时由 Java 决定。";
pub const RENAME_HELP: &str = "改名保留游戏目录、收藏和历史记录。";
pub const SERVER_ADDRESS_HELP: &str =
    "主机名或 IP，端口可选（默认 25565），例如 mc.example.com 或 mc.example.com:25570。";
pub const COPY_HELP: &str = "日志与崩溃报告不会复制；收藏、游玩时间、历史和快照从零开始。";

impl InstanceDetailView {
    /// Opens an edit dialog with a fresh draft taken from the saved record.
    pub(super) fn open_editor(
        &mut self,
        editor: Editor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.record.is_none() || self.editor.is_some() {
            return;
        }
        self.sync_fields = match editor {
            Editor::Rename => 1,
            Editor::Memory => 2,
            Editor::Copy => 4,
            Editor::Snapshot | Editor::Server => 0,
        };
        self.ensure_fields(window, cx);
        if editor == Editor::Snapshot {
            self.snapshot_scope = 0;
            if let Some(fields) = &self.fields {
                fields
                    .snapshot_note
                    .update(cx, |note, cx| note.set_value("", window, cx));
            }
            // The worlds are the choices of where to take it from.
            self.ensure(Section::Worlds, window, cx);
        }
        if editor == Editor::Copy {
            self.include_worlds = false;
        }
        self.editor = Some(editor);
        self.editor_error = None;
        self.close_editor = false;
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| match view.upgrade() {
            Some(view) => InstanceDetailView::editor_dialog(&view, dialog, cx),
            None => dialog,
        });
        cx.notify();
    }

    /// The dialog went away (cancel, Esc, overlay, or a successful save).
    fn editor_closed(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        self.editor_error = None;
        cx.notify();
    }

    /// A finished write closes its dialog; a failed one keeps it open with
    /// the draft and one sentence about what happened. `false` when no
    /// dialog was open, so the caller tells the result another way.
    pub(super) fn editor_result(
        &mut self,
        failure: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.editor.is_none() {
            return false;
        }
        match failure {
            Some(message) => self.editor_error = Some(message),
            None => {
                self.editor = None;
                self.editor_error = None;
                self.close_editor = true;
            }
        }
        cx.notify();
        true
    }

    /// Closing needs the window, so a finished save is closed on the next render.
    pub(super) fn settle_editor(&mut self, window: &mut Window, cx: &mut App) {
        if std::mem::take(&mut self.close_editor) && window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.editor {
            Some(Editor::Rename) => self.submit(false, window, cx),
            Some(Editor::Memory) => self.submit(true, window, cx),
            Some(Editor::Copy) => self.submit_copy(window, cx),
            Some(Editor::Snapshot) => self.submit_snapshot(window, cx),
            Some(Editor::Server) => self.submit_server(window, cx),
            None => {}
        }
    }

    fn editor_dialog(view: &Entity<Self>, dialog: Dialog, cx: &mut App) -> Dialog {
        let this = view.read(cx);
        let Some(editor) = this.editor else {
            return dialog;
        };
        let Some(fields) = this.fields.as_ref() else {
            return dialog;
        };
        let busy = this.busy;
        let colors = theme::ShellColors::from_theme(cx.theme());
        let label = |text: &'static str, help: Option<&'static str>, id: &'static str| {
            h_flex()
                .gap_1()
                .items_center()
                .child(div().text_sm().font_medium().child(text))
                .children(help.map(|help| kit::info(id, help)))
        };
        let field = |text: &'static str,
                     help: Option<&'static str>,
                     id: &'static str,
                     input: &Entity<gpui_component::input::InputState>| {
            v_flex()
                .gap_2()
                .child(label(text, help, id))
                .child(Input::new(input).disabled(busy))
        };

        let (title, commit_label, body) = match editor {
            Editor::Rename => (
                "重命名",
                "保存",
                v_flex().child(field(
                    "名称",
                    Some(RENAME_HELP),
                    "rename-info",
                    &fields.name,
                )),
            ),
            Editor::Memory => (
                "编辑内存",
                "保存",
                v_flex()
                    .gap_4()
                    .child(field(
                        "最小内存（MB）",
                        Some(MIN_MEMORY_HELP),
                        "min-info",
                        &fields.min,
                    ))
                    .child(field(
                        "最大内存（MB）",
                        Some(MAX_MEMORY_HELP),
                        "max-info",
                        &fields.max,
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .child("留空则跟随默认，下次启动时生效。"),
                    ),
            ),
            Editor::Server => (
                if this.server_edit.is_some() {
                    "编辑服务器"
                } else {
                    "添加服务器"
                },
                "保存",
                v_flex()
                    .gap_4()
                    .child(field("名称", None, "server-name-info", &fields.server_name))
                    .child(field(
                        "地址",
                        Some(SERVER_ADDRESS_HELP),
                        "server-address-info",
                        &fields.server_address,
                    )),
            ),
            Editor::Snapshot => {
                let weak = view.downgrade();
                let mut scopes: Vec<String> = vec!["全部世界与设置".to_owned()];
                if let Some(Ok(worlds)) = &this.data.worlds {
                    scopes.extend(
                        worlds
                            .iter()
                            .map(|world| format!("只备份世界“{}”", world.name)),
                    );
                }
                (
                    "创建快照",
                    "创建",
                    v_flex()
                        .gap_4()
                        .child(field("备注", None, "snapshot-info", &fields.snapshot_note))
                        .child(
                            v_flex()
                                .child(div().text_sm().font_medium().child("范围"))
                                .children(scopes.into_iter().enumerate().map(|(index, text)| {
                                    let weak = weak.clone();
                                    kit::led_option(
                                        ("snapshot-scope", index),
                                        text,
                                        this.snapshot_scope == index,
                                        colors,
                                        move |_, cx| {
                                            let _ = weak.update(cx, |view, cx| {
                                                view.snapshot_scope = index;
                                                cx.notify();
                                            });
                                        },
                                    )
                                })),
                        ),
                )
            }
            Editor::Copy => {
                let include = this.include_worlds;
                let toggle = view.downgrade();
                (
                    "复制游戏",
                    "开始复制",
                    v_flex()
                        .gap_4()
                        .child(field(
                            "新游戏名称",
                            Some(COPY_HELP),
                            "copy-info",
                            &fields.copy_name,
                        ))
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(div().text_sm().font_medium().child("同时复制存档"))
                                .child(kit::switch("copy-worlds", include, "同时复制存档", {
                                    move |_, cx| {
                                        let _ = toggle.update(cx, |view, cx| {
                                            view.include_worlds = !view.include_worlds;
                                            cx.notify();
                                        });
                                    }
                                })),
                        ),
                )
            }
        };
        let body = body.children(this.editor_error.clone().map(|message| {
            div()
                .text_sm()
                .text_color(colors.danger)
                .debug_selector(|| "instance-editor-error".into())
                .child(message)
        }));

        let commit_id = match editor {
            Editor::Rename => "instance-rename-save",
            Editor::Memory => "instance-memory-save",
            Editor::Copy => "instance-copy-go",
            Editor::Snapshot => "instance-snapshot-go",
            Editor::Server => "instance-server-save",
        };
        let save = {
            let view = view.downgrade();
            theme::clickable(
                Key::new(commit_id)
                    .label(commit_label)
                    .primary()
                    .disabled(busy)
                    .debug_selector(move || commit_id.into()),
                !busy,
            )
            .on_click(move |_, window, cx| {
                let _ = view.update(cx, |view, cx| view.commit(window, cx));
            })
        };
        let cancel = theme::clickable(
            Key::new("instance-editor-cancel")
                .label("取消")
                .white()
                .disabled(busy),
            !busy,
        )
        .on_click(|_, window, cx| window.close_dialog(cx));
        let reset = (editor == Editor::Memory).then(|| {
            let view = view.downgrade();
            theme::clickable(
                Key::new("instance-memory-reset")
                    .label("全部跟随默认")
                    .ghost()
                    .disabled(busy)
                    .debug_selector(|| "instance-memory-reset".into()),
                !busy,
            )
            .on_click(move |_, window, cx| {
                let _ = view.update(cx, |view, cx| view.reset_memory(window, cx));
            })
        });
        let closed = view.downgrade();

        theme::dialog(dialog, cx)
            .title(title)
            .w(px(420.))
            .keyboard(!busy)
            .overlay_closable(!busy)
            .close_button(!busy)
            .on_close(move |_, _, cx| {
                let _ = closed.update(cx, |view, cx| view.editor_closed(cx));
            })
            .child(body)
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(div().flex_1().children(reset))
                    .child(cancel)
                    .child(save),
            )
    }

    /// Takes the snapshot the dialog describes.
    fn submit_snapshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let note = self
            .fields
            .as_ref()
            .map(|fields| fields.snapshot_note.read(cx).value().trim().to_owned())
            .unwrap_or_default();
        let world = match (self.snapshot_scope, &self.data.worlds) {
            (0, _) => None,
            (at, Some(Ok(worlds))) => worlds.get(at - 1).map(|world| world.folder.clone()),
            _ => None,
        };
        self.send(InstanceIntent::CreateSnapshotAs { note, world }, window, cx);
    }

    /// Asks before a destructive step, in a dialog that says what it costs.
    /// `confirm` shows what is being asked until it is answered.
    pub(super) fn ask_confirm(
        &mut self,
        what: Confirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let worlds = match &self.data.worlds {
            Some(Ok(worlds)) => worlds.as_slice(),
            _ => &[],
        };
        let snapshots = match &self.data.snapshots {
            Some(Ok(snapshots)) => snapshots.as_slice(),
            _ => &[],
        };
        let (title, description, ok) = what.words(worlds, snapshots);
        self.confirm = Some(what.clone());
        cx.notify();
        let view = cx.entity().downgrade();
        let cancelled = view.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (view, cancelled, what) = (view.clone(), cancelled.clone(), what.clone());
            alert
                .title(title.clone())
                .description(description.clone())
                .ok_text(ok)
                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                .cancel_text("取消")
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let _ = view.update(cx, |view, cx| view.confirmed(&what, window, cx));
                    true
                })
                .on_cancel(move |_, _, cx| {
                    let _ = cancelled.update(cx, |view, cx| {
                        view.confirm = None;
                        cx.notify();
                    });
                    true
                })
        });
    }

    /// Delete asks once, in an alert dialog that says what goes away (§10).
    pub(super) fn confirm_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let view = cx.entity().downgrade();
        let name = self.title().to_owned();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            alert
                .title(format!("删除“{name}”？"))
                .description("游戏目录、存档和历史会一起删除；中途中断也会在下次启动时补完或恢复。")
                .ok_text("删除")
                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                .cancel_text("取消")
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let _ = view.update(cx, |view, cx| {
                        view.send(InstanceIntent::Delete, window, cx);
                    });
                    true
                })
        });
    }
}
