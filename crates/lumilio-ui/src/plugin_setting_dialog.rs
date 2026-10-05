//! Host-owned text and number drafts, retained until asynchronous persistence succeeds.
use crate::theme::ShellColors;
use crate::{kit, theme};
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use lumilio_plugin_api::{SettingField, SettingKind, SettingValue};
use std::{future::Future, pin::Pin, rc::Rc};

pub type SaveSetting =
    Rc<dyn Fn(SettingValue) -> Pin<Box<dyn Future<Output = Result<(), String>>>>>;
pub type SettingSaved = Rc<dyn Fn(&mut App)>;

pub struct PluginSettingDialog {
    field: SettingField,
    input: Entity<InputState>,
    save: SaveSetting,
    saved: SettingSaved,
    busy: bool,
    error: Option<String>,
}

impl PluginSettingDialog {
    pub fn open(
        field: SettingField,
        value: SettingValue,
        save: SaveSetting,
        saved: SettingSaved,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<Self>> {
        let title = format!("编辑{}", field.label);
        let initial = match value {
            SettingValue::Text(text) => text,
            SettingValue::Number(number) => number.to_string(),
            _ => return None,
        };
        let form = cx.new(|cx| Self {
            field,
            input: cx.new(|cx| InputState::new(window, cx).default_value(initial)),
            save,
            saved,
            busy: false,
            error: None,
        });
        let shown = form.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let busy = form.read(cx).busy;
            let weak = form.downgrade();
            theme::dialog(dialog, cx)
                .keyboard(!busy)
                .overlay_closable(!busy)
                .close_button(!busy)
                .title(title.clone())
                .child(form.clone())
                .footer(
                    h_flex()
                        .gap_2()
                        .child(
                            kit::action(
                                "plugin-setting-cancel",
                                "取消",
                                None,
                                false,
                                |window, cx| window.close_dialog(cx),
                            )
                            .disabled(busy),
                        )
                        .child(
                            kit::action(
                                "plugin-setting-save",
                                "保存",
                                None,
                                true,
                                move |window, cx| {
                                    let _ = weak.update(cx, |form, cx| form.submit(window, cx));
                                },
                            )
                            .debug_selector(|| "plugin-setting-save".into())
                            .loading(busy),
                        ),
                )
        });
        Some(shown)
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let value = match parse(&self.field.kind, self.input.read(cx).value().as_ref()) {
            Ok(value) => value,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.busy = true;
        self.error = None;
        cx.notify();
        let pending = (self.save)(value);
        let window = window.window_handle();
        cx.spawn(async move |weak, cx| {
            let result = pending.await;
            let _ = cx.update_window(window, |_, window, cx| {
                let _ = weak.update(cx, |form, cx| {
                    form.busy = false;
                    match result {
                        Ok(()) => {
                            window.close_dialog(cx);
                            (form.saved)(cx);
                        }
                        Err(message) => form.error = Some(message),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }
}

fn parse(kind: &SettingKind, text: &str) -> Result<SettingValue, String> {
    match kind {
        SettingKind::Text { .. } => Ok(SettingValue::Text(text.into())),
        SettingKind::Number { min, max, .. } => {
            let number = text
                .trim()
                .parse::<i64>()
                .map_err(|_| "请输入整数".to_owned())?;
            if !kind.accepts(&SettingValue::Number(number)) {
                return Err(format!("请输入 {min} 到 {max} 之间的整数"));
            }
            Ok(SettingValue::Number(number))
        }
        _ => Err("这个设置请在设置行里选择".into()),
    }
}

impl Render for PluginSettingDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::from_theme(cx.theme());
        v_flex()
            .gap_3()
            .w_full()
            .child(
                div()
                    .text_sm()
                    .text_color(colors.foreground)
                    .child(self.field.label.clone()),
            )
            .child(Input::new(&self.input).disabled(self.busy))
            .children((!self.field.help.is_empty()).then(|| {
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .child(self.field.help.clone())
            }))
            .children(self.error.clone().map(|error| {
                div()
                    .text_sm()
                    .text_color(colors.danger)
                    .debug_selector(|| "plugin-setting-error".into())
                    .child(error)
            }))
    }
}

#[cfg(test)]
mod tests;
