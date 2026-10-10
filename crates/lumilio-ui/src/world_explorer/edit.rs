//! Editing an overlay object: the host draws the dialog from the fields the
//! plugin declared (plan W10) and hands the confirmed values back. Nothing is
//! written here; the plugin validates and writes through the host.
use super::{Command, MapView};
use crate::controls::Fader;
use crate::theme::{self, ShellColors};
use crate::{key::Key, tr};
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, Render, SharedString, WeakEntity, Window, div,
    prelude::*, px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use lumilio_plugin_api::map::{EditAction, MapObject, MapPoint, ObjectEdit};
use lumilio_plugin_api::{SettingField, SettingKind, SettingValue};
use std::collections::BTreeMap;
use std::rc::Rc;

/// The 16 Minecraft text colours, which the colour field of a Choice offers by
/// index. A field named `color` whose options are indices shows these.
pub(super) const SWATCHES: [u32; 16] = [
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA, 0x555555,
    0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

enum Control {
    Text(Entity<InputState>),
    Number(Entity<InputState>),
    Toggle(bool),
    Choice(String),
}

pub(super) type Submit = Rc<dyn Fn(BTreeMap<String, SettingValue>, &mut Window, &mut App)>;

pub(super) struct EditDialog {
    fields: Vec<(SettingField, Control)>,
    submit: Submit,
    busy: bool,
    error: Option<String>,
}

fn title_of(field: &SettingField) -> String {
    field.label.get(crate::i18n::locale().tag()).to_owned()
}

/// What a failure message id says, in the person's language; an id the
/// catalog does not know is shown as it is.
pub(super) fn failure_text(id: &str) -> String {
    crate::i18n::lookup(id).unwrap_or_else(|| id.to_owned())
}

impl EditDialog {
    /// Opens the dialog over `fields`, whose defaults are the starting values;
    /// `overrides` replaces some of them (the position a new object is placed
    /// at). `submit` receives the checked values.
    pub fn open(
        title: SharedString,
        fields: Vec<SettingField>,
        overrides: BTreeMap<String, SettingValue>,
        submit: Submit,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let controls = fields
            .into_iter()
            .map(|field| {
                let start = overrides
                    .get(&field.key)
                    .filter(|value| field.kind.accepts(value))
                    .cloned()
                    .unwrap_or_else(|| field.kind.default_value());
                let control =
                    match start {
                        SettingValue::Text(text) => Control::Text(
                            cx.new(|cx| InputState::new(window, cx).default_value(text)),
                        ),
                        SettingValue::Number(number) => Control::Number(cx.new(|cx| {
                            InputState::new(window, cx).default_value(number.to_string())
                        })),
                        SettingValue::Toggle(on) => Control::Toggle(on),
                        SettingValue::Choice(choice) => Control::Choice(choice),
                    };
                (field, control)
            })
            .collect();
        let form = cx.new(|_| Self {
            fields: controls,
            submit,
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
                            crate::kit::action(
                                "map-edit-cancel",
                                tr!("common-cancel"),
                                None,
                                false,
                                |window, cx| window.close_dialog(cx),
                            )
                            .disabled(busy),
                        )
                        .child(
                            crate::kit::action(
                                "map-edit-save",
                                tr!("common-save"),
                                None,
                                true,
                                move |window, cx| {
                                    let _ = weak.update(cx, |form, cx| form.confirm(window, cx));
                                },
                            )
                            .debug_selector(|| "map-edit-save".into())
                            .loading(busy),
                        ),
                )
        });
        shown
    }

    /// Reads every control back into values, or says what is wrong.
    fn values(&self, cx: &App) -> Result<BTreeMap<String, SettingValue>, String> {
        let mut values = BTreeMap::new();
        for (field, control) in &self.fields {
            let value = match control {
                Control::Text(input) => SettingValue::Text(input.read(cx).value().to_string()),
                Control::Number(input) => {
                    let number = input
                        .read(cx)
                        .value()
                        .trim()
                        .parse::<i64>()
                        .map_err(|_| tr!("plugin-setting-integer").to_string())?;
                    SettingValue::Number(number)
                }
                Control::Toggle(on) => SettingValue::Toggle(*on),
                Control::Choice(choice) => SettingValue::Choice(choice.clone()),
            };
            if !field.kind.accepts(&value) {
                return Err(match &field.kind {
                    SettingKind::Number { min, max, .. } => {
                        format!(
                            "{}: {}",
                            title_of(field),
                            tr!("plugin-setting-integer-range", min = *min, max = *max)
                        )
                    }
                    _ => title_of(field),
                });
            }
            values.insert(field.key.clone(), value);
        }
        Ok(values)
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        match self.values(cx) {
            Ok(values) => {
                self.busy = true;
                self.error = None;
                (self.submit.clone())(values, window, cx);
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }

    /// The host's answer to the submitted edit: close on success, otherwise
    /// stay open with the reason.
    pub fn finish(
        &mut self,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        match result {
            Ok(()) => window.close_dialog(cx),
            Err(id) => self.error = Some(failure_text(&id)),
        }
        cx.notify();
    }
}

impl Render for EditDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::from_theme(cx.theme());
        let busy = self.busy;
        let rows: Vec<AnyElement> = self
            .fields
            .iter()
            .enumerate()
            .map(|(at, (field, control))| {
                let label = title_of(field);
                let key = field.key.clone();
                let body: AnyElement = match control {
                    Control::Text(input) | Control::Number(input) => div()
                        .debug_selector({
                            let key = key.clone();
                            move || format!("map-edit-{key}")
                        })
                        .child(Input::new(input).small().disabled(busy))
                        .into_any_element(),
                    Control::Toggle(on) => {
                        let weak = cx.weak_entity();
                        Fader::new(
                            SharedString::from(format!("map-edit-{key}")),
                            *on,
                            label.clone(),
                            move |_, cx| {
                                let _ = weak.update(cx, |form, cx| {
                                    if let Some((_, Control::Toggle(on))) = form.fields.get_mut(at)
                                    {
                                        *on = !*on;
                                    }
                                    cx.notify();
                                });
                            },
                        )
                        .disabled(busy)
                        .into_any_element()
                    }
                    Control::Choice(chosen) => {
                        let SettingKind::Choice { options, .. } = &field.kind else {
                            return div().into_any_element();
                        };
                        let swatches = key == "color" && options.len() == SWATCHES.len();
                        h_flex()
                            .gap_1()
                            .flex_wrap()
                            .children(options.iter().enumerate().map(|(index, option)| {
                                let picked = option == chosen;
                                let weak: WeakEntity<Self> = cx.weak_entity();
                                let value = option.clone();
                                let chip = div()
                                    .id(SharedString::from(format!("map-edit-{key}-{option}")))
                                    .debug_selector({
                                        let id = format!("map-edit-{key}-{option}");
                                        move || id.clone()
                                    })
                                    .cursor_pointer()
                                    .border_2()
                                    .rounded(px(4.))
                                    .border_color(if picked {
                                        colors.foreground
                                    } else {
                                        colors.border
                                    })
                                    .on_click(move |_, _, cx| {
                                        let _ = weak.update(cx, |form, cx| {
                                            if !form.busy
                                                && let Some((_, Control::Choice(now))) =
                                                    form.fields.get_mut(at)
                                            {
                                                *now = value.clone();
                                            }
                                            cx.notify();
                                        });
                                    });
                                if swatches {
                                    chip.size(px(22.)).bg(gpui::rgb(SWATCHES[index]))
                                } else {
                                    chip.px_2().child(option.clone())
                                }
                            }))
                            .into_any_element()
                    }
                };
                // A fader's label is only its accessible name, so a switch row
                // draws its own title, leading, with the switch trailing.
                if matches!(control, Control::Toggle(_)) {
                    return h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .gap_2()
                        .child(div().text_sm().child(label))
                        .child(body)
                        .into_any_element();
                }
                v_flex()
                    .gap_1()
                    .child(div().text_xs().text_color(colors.muted).child(label))
                    .child(body)
                    .into_any_element()
            })
            .collect();
        v_flex()
            .gap_3()
            .w_full()
            .children(rows)
            .children(self.error.clone().map(|error| {
                div()
                    .text_sm()
                    .text_color(colors.danger)
                    .debug_selector(|| "map-edit-error".into())
                    .child(error)
            }))
    }
}

impl MapView {
    /// The layer an object came from, with the plugin that owns it.
    fn layer_of(&self, object: &MapObject) -> Option<(String, String)> {
        self.objects
            .layers
            .iter()
            .filter(|(plugin, layer)| *plugin == object.source && object.id.starts_with(&layer.id))
            .max_by_key(|(_, layer)| layer.id.len())
            .map(|(plugin, layer)| (plugin.clone(), layer.id.clone()))
    }

    fn send_edit(&mut self, plugin: String, overlay: String, action: EditAction) -> bool {
        let (Some(context), Some(connection)) = (self.context.clone(), &self.connection) else {
            return false;
        };
        connection
            .send
            .try_send(Command::Apply {
                plugin,
                edit: Box::new(ObjectEdit {
                    context,
                    overlay,
                    action,
                }),
            })
            .is_ok()
    }

    /// Asks the host whether the files could be written now.
    pub(super) fn probe_edit(&self) {
        if let Some(connection) = &self.connection {
            let _ = connection.send.try_send(Command::CanEdit);
        }
    }

    /// Opens the dialog for the selected object's fields.
    pub(super) fn edit_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(object) = self.selected.clone() else {
            return;
        };
        let Some((plugin, overlay)) = self.layer_of(&object) else {
            return;
        };
        let target = cx.weak_entity();
        let id = object.id.clone();
        let submit: Submit = Rc::new(move |values, _, cx| {
            let (plugin, overlay, id) = (plugin.clone(), overlay.clone(), id.clone());
            let _ = target.update(cx, |this, _| {
                if !this.send_edit(plugin, overlay, EditAction::Update { id, values }) {
                    this.edit_failed("map-edit-failed");
                }
            });
        });
        let title = object
            .label
            .clone()
            .map(SharedString::from)
            .unwrap_or_else(|| tr!("map-edit-title").into());
        let dialog = EditDialog::open(
            title,
            object.editable.clone(),
            BTreeMap::new(),
            submit,
            window,
            cx,
        );
        self.edit_dialog = Some(dialog.downgrade());
    }

    /// Opens the dialog for a new object placed at `at` in layer `overlay`.
    pub(super) fn create_at(&mut self, at: MapPoint, window: &mut Window, cx: &mut Context<Self>) {
        self.placing = false;
        let Some((plugin, layer)) = self
            .objects
            .layers
            .iter()
            .find(|(_, layer)| !layer.creatable.is_empty())
            .map(|(plugin, layer)| (plugin.clone(), layer.clone()))
        else {
            return;
        };
        let overrides = BTreeMap::from([
            ("x".to_owned(), SettingValue::Number(at.x.round() as i64)),
            ("z".to_owned(), SettingValue::Number(at.z.round() as i64)),
        ]);
        let target = cx.weak_entity();
        let overlay = layer.id.clone();
        let submit: Submit = Rc::new(move |values, _, cx| {
            let (plugin, overlay) = (plugin.clone(), overlay.clone());
            let _ = target.update(cx, |this, _| {
                if !this.send_edit(plugin, overlay, EditAction::Create { at, values }) {
                    this.edit_failed("map-edit-failed");
                }
            });
        });
        let dialog = EditDialog::open(
            tr!("map-edit-new-title").into(),
            layer.creatable,
            overrides,
            submit,
            window,
            cx,
        );
        self.edit_dialog = Some(dialog.downgrade());
        cx.notify();
    }

    /// Asks before deleting the selected object; a death point asks in its own
    /// words, since it cannot be recreated.
    pub(super) fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(object) = self.selected.clone() else {
            return;
        };
        let Some((plugin, overlay)) = self.layer_of(&object) else {
            return;
        };
        let death = object.label_id.as_deref() == Some("map-xaero-death");
        let name = object
            .label
            .clone()
            .unwrap_or_else(|| object.raw_id.clone());
        let target = cx.weak_entity();
        let id = object.id.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (target, plugin, overlay, id) =
                (target.clone(), plugin.clone(), overlay.clone(), id.clone());
            alert
                .title(tr!("map-delete-title", name = name.clone()))
                .description(if death {
                    tr!("map-delete-death-body")
                } else {
                    tr!("map-delete-body")
                })
                .ok_text(tr!("common-delete"))
                .ok_variant(gpui_component::button::ButtonVariant::Danger)
                .cancel_text(tr!("common-cancel"))
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let (plugin, overlay, id) = (plugin.clone(), overlay.clone(), id.clone());
                    let _ = target.update(cx, |this, cx| {
                        if !this.send_edit(plugin, overlay, EditAction::Delete { id }) {
                            this.edit_failed("map-edit-failed");
                        }
                        cx.notify();
                    });
                    true
                })
        });
    }

    /// An edit could not be applied: the open dialog shows why, otherwise the
    /// status line does.
    pub(super) fn edit_failed(&mut self, id: &str) {
        self.edit_error = Some(id.to_owned());
    }

    /// The answer to an applied edit.
    pub(super) fn edit_applied(
        &mut self,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ok = result.is_ok();
        let open = self.edit_dialog.take().and_then(|weak| weak.upgrade());
        match (&open, &result) {
            (Some(dialog), _) => {
                let result = result.clone();
                dialog.update(cx, |dialog, cx| dialog.finish(result, window, cx));
                if !ok {
                    self.edit_dialog = Some(dialog.downgrade());
                }
            }
            (None, Err(id)) => self.edit_error = Some(id.clone()),
            (None, Ok(())) => {}
        }
        if ok {
            self.edit_error = None;
            self.selected = None;
            // What was written is read again from the files.
            self.objects.clear();
            self.refresh_objects();
        }
        self.probe_edit();
        cx.notify();
    }

    /// The selection card's edit keys, or the reason they are unavailable.
    pub(super) fn edit_keys(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let object = self.selected.as_ref()?;
        if object.editable.is_empty() {
            return None;
        }
        let allowed = self.can_edit;
        let keys = h_flex()
            .gap_2()
            .child(
                Key::new("map-edit")
                    .label(tr!("map-edit"))
                    .white()
                    .small()
                    .disabled(!allowed)
                    .debug_selector(|| "map-edit".into())
                    .on_click(cx.listener(|this, _, window, cx| this.edit_selected(window, cx))),
            )
            .child(
                Key::new("map-delete")
                    .label(tr!("common-delete"))
                    .white()
                    .small()
                    .disabled(!allowed)
                    .debug_selector(|| "map-delete".into())
                    .on_click(cx.listener(|this, _, window, cx| this.delete_selected(window, cx))),
            );
        Some(
            v_flex()
                .gap_1()
                .child(keys)
                .children((!allowed).then(|| {
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .debug_selector(|| "map-edit-locked".into())
                        .child(tr!("map-edit-locked"))
                }))
                .into_any_element(),
        )
    }

    /// The key that turns on placing a new object, when a layer can make one.
    pub(super) fn place_key(&self, cx: &mut Context<Self>) -> Option<Key> {
        self.objects
            .active()
            .any(|(_, layer)| !layer.creatable.is_empty())
            .then(|| {
                // ia[plugin.world-explorer]: 新建路径点 | 地图左上角「添加路径点」→ 点一下地图 | 打开新建对话框，位置取点击处；游戏运行中禁用并说明原因
                Key::new("map-place")
                    .label(if self.placing {
                        tr!("map-place-cancel")
                    } else {
                        tr!("map-place")
                    })
                    .white()
                    .small()
                    .disabled(!self.can_edit)
                    .debug_selector(|| "map-place".into())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.placing = !this.placing;
                        cx.notify();
                    }))
            })
    }
}

#[cfg(test)]
impl EditDialog {
    /// Types `text` into the number field `key`.
    pub(super) fn set_number_for_test(
        &mut self,
        key: &str,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (field, control) in &self.fields {
            if let (true, Control::Number(input)) = (field.key == key, control) {
                input.update(cx, |input, cx| input.set_value(text.to_owned(), window, cx));
            }
        }
    }
}
