use super::InstanceDetailView;
use super::editors::Editor;
use super::intent::InstanceIntent;
use crate::{kit, theme};
use gpui::prelude::*;
use gpui::{Context, Entity, Window};
use gpui_component::input::InputState;
use lumilio_core::{InstanceSettings, LauncherSettings};

pub(super) struct Fields {
    pub(super) name: Entity<InputState>,
    pub(super) min: Entity<InputState>,
    pub(super) max: Entity<InputState>,
    pub(super) copy_name: Entity<InputState>,
    pub(super) content_search: Entity<InputState>,
    pub(super) snapshot_note: Entity<InputState>,
    pub(super) world_search: Entity<InputState>,
    pub(super) server_name: Entity<InputState>,
    pub(super) server_address: Entity<InputState>,
    pub(super) log_search: Entity<InputState>,
    pub(super) file_search: Entity<InputState>,
}

/// Parsing belongs to the form; range and effective-pair validation stays in core.
pub(super) fn memory_draft(
    saved: &InstanceSettings,
    defaults: &LauncherSettings,
    min: &str,
    max: &str,
) -> Result<InstanceSettings, &'static str> {
    let parse = |value: &str| {
        let value = value.trim();
        if value.is_empty() {
            Ok(None)
        } else {
            value
                .parse::<u32>()
                .map(Some)
                .map_err(|_| "请输入整数 MB，留空可继承默认值")
        }
    };
    let mut next = saved.clone();
    next.min_memory_mb = parse(min)?;
    next.max_memory_mb = parse(max)?;
    next.validate_memory(
        defaults.default_min_memory_mb,
        defaults.default_max_memory_mb,
    )
    .map_err(|_| "内存需在 1–1048576 MB 内，最小值不能超过生效的最大值")?;
    Ok(next)
}

pub(super) fn memory_text(value: Option<u32>) -> String {
    value.map_or_else(String::new, |value| value.to_string())
}

/// The read-only value of a memory row: inherited values say so (§10).
pub(super) fn effective_label(value: Option<u32>, default: Option<u32>) -> String {
    match (value, default) {
        (Some(value), _) => format!("{value} MB"),
        (None, Some(value)) => format!("跟随默认 · {value} MB"),
        (None, None) => "由 Java 决定".to_owned(),
    }
}

impl InstanceDetailView {
    pub(super) fn submit_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(fields) = &self.fields else { return };
        let name = fields.copy_name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.editor_error = Some("请输入新游戏的名称".into());
            cx.notify();
            return;
        }
        let include_worlds = self.include_worlds;
        self.send(
            InstanceIntent::Copy {
                name,
                include_worlds,
            },
            window,
            cx,
        );
    }

    pub(super) fn ensure_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(record) = &self.record else { return };
        let name = record.name.clone();
        let copy_name = format!("{name} 副本");
        let min = memory_text(record.settings.min_memory_mb);
        let max = memory_text(record.settings.max_memory_mb);
        if let Some(fields) = &self.fields {
            for (field, text, mask) in [
                (&fields.name, name, 1),
                (&fields.min, min, 2),
                (&fields.max, max, 2),
                (&fields.copy_name, copy_name, 4),
            ] {
                if self.sync_fields & mask != 0 {
                    field.update(cx, |field, cx| field.set_value(text, window, cx));
                }
            }
        } else {
            let mut input = |text: String, placeholder: &'static str, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(text)
                        .placeholder(placeholder)
                })
            };
            self.fields = Some(Fields {
                name: input(name, "游戏名称", cx),
                min: input(min, "继承默认", cx),
                max: input(max, "继承默认", cx),
                copy_name: input(copy_name, "新游戏的名称", cx),
                content_search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索")),
                snapshot_note: cx.new(|cx| InputState::new(window, cx).placeholder("备注（可空）")),
                world_search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索世界")),
                server_name: cx.new(|cx| InputState::new(window, cx).placeholder("服务器名称")),
                server_address: cx.new(|cx| {
                    InputState::new(window, cx).placeholder("地址，例如 mc.example.com:25565")
                }),
                log_search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索日志")),
                file_search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索文件")),
            });
            if let Some(fields) = &self.fields {
                Self::watch_content_search(&fields.content_search.clone(), cx);
                Self::watch_content_search(&fields.log_search.clone(), cx);
                Self::watch_content_search(&fields.world_search.clone(), cx);
                Self::watch_content_search(&fields.file_search.clone(), cx);
            }
        }
        self.sync_fields = 0;
    }

    pub(super) fn submit(&mut self, memory: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let (Some(record), Some(fields)) = (&self.record, &self.fields) else {
            return;
        };
        let intent = if memory {
            match memory_draft(
                &record.settings,
                &self.defaults,
                fields.min.read(cx).value().as_ref(),
                fields.max.read(cx).value().as_ref(),
            ) {
                Ok(settings) => InstanceIntent::SaveMemory(settings),
                Err(message) => {
                    self.editor_error = Some(message.into());
                    cx.notify();
                    return;
                }
            }
        } else {
            let name = fields.name.read(cx).value().trim().to_owned();
            if name.is_empty() {
                self.editor_error = Some("请输入游戏名称".into());
                cx.notify();
                return;
            }
            InstanceIntent::Rename(name)
        };
        self.busy = true;
        self.editor_error = None;
        cx.notify();
        (self.handler)(intent, window, cx);
    }

    pub(super) fn reset_memory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(fields) = &self.fields else { return };
        for field in [&fields.min, &fields.max] {
            field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        self.submit(true, window, cx);
    }

    pub(super) fn edit_button(
        &self,
        id: &'static str,
        editor: Editor,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        theme::clickable(
            kit::ghost(id, "编辑", |_, _| {})
                .disabled(self.busy)
                .debug_selector(move || id.into())
                .on_click(
                    cx.listener(move |view, _, window, cx| view.open_editor(editor, window, cx)),
                ),
            !self.busy,
        )
        .into_any_element()
    }
}
