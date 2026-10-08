//! One edit dialog for every free-form Settings value (design language §10):
//! the page shows values read-only, and anything typed is edited here. The
//! dialog knows nothing about what it edits: it shows its fields, hands the
//! typed text to a parser from [`crate::settings_forms`], and sends the
//! resulting intent.

use std::rc::Rc;

use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use gpui_component::{ActiveTheme as _, StyledExt as _, WindowExt as _, h_flex, v_flex};

use crate::kit;
use crate::theme::{self, ShellColors};
use crate::tr;

const DIALOG_WIDTH: f32 = 480.;

/// Turns the field values (a choice's index as text) into a change.
pub type ParseFn<I> = Rc<dyn Fn(&[String]) -> Result<I, String>>;

/// Where a saved or reset change goes.
pub type Handler<I> = Rc<dyn Fn(I, &mut Window, &mut App)>;

/// A field's label, help and, for a choice, its labels.
type FieldHead = (
    &'static str,
    Option<&'static str>,
    Option<&'static [&'static str]>,
);

pub enum FieldKind {
    /// One line of text.
    Line,
    /// One line holding a path, with a button that chooses it from the disk.
    Path {
        files: bool,
        directories: bool,
        prompt: &'static str,
    },
    /// Several lines; `rows` tall.
    Lines { rows: usize },
    /// A choice among a few labels; the value is the chosen index.
    Choice {
        labels: &'static [&'static str],
        selected: usize,
    },
}

pub struct FieldSpec {
    pub label: &'static str,
    pub help: Option<&'static str>,
    pub placeholder: &'static str,
    pub value: String,
    pub kind: FieldKind,
}

pub struct DialogSpec<I> {
    pub title: &'static str,
    /// A sentence under the title.
    pub intro: Option<&'static str>,
    pub fields: Vec<FieldSpec>,
    /// Turns the field values (a choice's index as text) into a change.
    pub parse: ParseFn<I>,
    /// The change that puts the defaults back; shows "恢复默认".
    pub reset: Option<I>,
}

enum Field {
    Line(Entity<InputState>),
    Lines(Entity<TextareaState>),
    Choice(usize),
}

pub struct SettingsDialog<I> {
    spec_fields: Vec<FieldHead>,
    inputs: Vec<Field>,
    /// Per field: what its 浏览… button may choose, if it has one.
    browse: Vec<Option<(bool, bool, &'static str)>>,
    intro: Option<&'static str>,
    parse: ParseFn<I>,
    handler: Handler<I>,
    error: Option<String>,
}

impl<I: Clone + 'static> SettingsDialog<I> {
    pub fn new(
        spec: DialogSpec<I>,
        handler: Handler<I>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = Vec::new();
        let mut browse = Vec::new();
        let mut spec_fields = Vec::new();
        for field in &spec.fields {
            let choices = match field.kind {
                FieldKind::Choice { labels, .. } => Some(labels),
                _ => None,
            };
            spec_fields.push((field.label, field.help, choices));
            browse.push(match field.kind {
                FieldKind::Path {
                    files,
                    directories,
                    prompt,
                } => Some((files, directories, prompt)),
                _ => None,
            });
            inputs.push(match &field.kind {
                FieldKind::Line | FieldKind::Path { .. } => Field::Line(cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(field.placeholder)
                        .default_value(field.value.clone())
                })),
                FieldKind::Lines { rows } => Field::Lines(cx.new(|cx| {
                    TextareaState::new(window, cx)
                        .auto_grow(*rows, (*rows).max(8))
                        .placeholder(field.placeholder)
                        .default_value(field.value.clone())
                })),
                FieldKind::Choice { selected, .. } => Field::Choice(*selected),
            });
        }
        Self {
            spec_fields,
            inputs,
            browse,
            intro: spec.intro,
            parse: spec.parse,
            handler,
            error: None,
        }
    }

    fn values(&self, cx: &App) -> Vec<String> {
        self.inputs
            .iter()
            .map(|field| match field {
                Field::Line(state) => state.read(cx).value().to_string(),
                Field::Lines(state) => state.read(cx).value().to_string(),
                Field::Choice(index) => index.to_string(),
            })
            .collect()
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match (self.parse)(&self.values(cx)) {
            Ok(intent) => {
                (self.handler)(intent, window, cx);
                window.close_dialog(cx);
            }
            Err(message) => {
                self.error = Some(message);
                cx.notify();
            }
        }
    }

    /// Opens the dialog for `spec`.
    pub fn open(spec: DialogSpec<I>, handler: Handler<I>, window: &mut Window, cx: &mut App) {
        let title = spec.title;
        let reset = spec.reset.clone();
        let form = cx.new(|cx| Self::new(spec, handler.clone(), window, cx));
        Self::show(form, title, reset, handler, window, cx);
    }

    fn show(
        form: Entity<Self>,
        title: &'static str,
        reset: Option<I>,
        handler: Handler<I>,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.open_dialog(cx, move |dialog, _, cx| {
            let weak = form.downgrade();
            let save = {
                let weak = weak.clone();
                theme::clickable(
                    Key::new("settings-save")
                        .label(tr!("common-save"))
                        .primary()
                        .debug_selector(|| "settings-save".into())
                        .on_click(move |_, window, cx| {
                            let _ = weak.update(cx, |form, cx| form.save(window, cx));
                        }),
                    true,
                )
            };
            let cancel = theme::clickable(
                Key::new("settings-cancel")
                    .label(tr!("common-cancel"))
                    .white()
                    .on_click(|_, window, cx| window.close_dialog(cx)),
                true,
            );
            let restore = reset.clone().map(|intent| {
                let handler = handler.clone();
                theme::clickable(
                    Key::new("settings-reset")
                        .label(tr!("common-restore-defaults"))
                        .ghost()
                        .debug_selector(|| "settings-reset".into())
                        .on_click(move |_, window, cx| {
                            handler(intent.clone(), window, cx);
                            window.close_dialog(cx);
                        }),
                    true,
                )
            });
            theme::dialog(dialog, cx)
                .title(title)
                .w(px(DIALOG_WIDTH))
                .child(form.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .children(restore)
                        .child(div().flex_1())
                        .child(cancel)
                        .child(save),
                )
        });
    }
}

impl<I: Clone + 'static> Render for SettingsDialog<I> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::from_theme(cx.theme());
        let entity = cx.entity().downgrade();
        let fields = self.spec_fields.iter().enumerate().map(|(index, spec)| {
            let (label, help, choices) = *spec;
            let control = match (&self.inputs[index], choices) {
                (Field::Line(state), _) => match self.browse[index] {
                    Some((files, directories, prompt)) => {
                        let state = state.clone();
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&state)))
                            .child(
                                kit::ghost(
                                    "settings-browse",
                                    tr!("common-browse"),
                                    move |window, cx| {
                                        let chosen = crate::platform::pick_path(
                                            cx,
                                            files,
                                            directories,
                                            prompt,
                                        );
                                        let (handle, state) =
                                            (window.window_handle(), state.clone());
                                        cx.spawn(async move |cx| {
                                            if let Some(path) = chosen.await {
                                                let text = path.display().to_string();
                                                let _ =
                                                    cx.update_window(handle, |_, window, cx| {
                                                        state.update(cx, |state, cx| {
                                                            state.set_value(text, window, cx)
                                                        });
                                                    });
                                            }
                                        })
                                        .detach();
                                    },
                                )
                                .debug_selector(|| "settings-browse".into()),
                            )
                            .into_any_element()
                    }
                    None => Input::new(state).into_any_element(),
                },
                (Field::Lines(state), _) => Textarea::new(state).into_any_element(),
                (Field::Choice(selected), Some(labels)) => {
                    let entity = entity.clone();
                    h_flex()
                        .child(kit::segments(
                            "settings-choice",
                            labels,
                            *selected,
                            move |choice, _, cx| {
                                let _ = entity.update(cx, |form, cx| {
                                    form.inputs[index] = Field::Choice(choice);
                                    form.error = None;
                                    cx.notify();
                                });
                            },
                        ))
                        .into_any_element()
                }
                (Field::Choice(_), None) => div().into_any_element(),
            };
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .font_medium()
                                .text_color(colors.foreground)
                                .child(label),
                        )
                        .children(help.map(|help| kit::info(("settings-info", index), help))),
                )
                .child(control)
        });
        v_flex()
            .id("settings-dialog")
            .w_full()
            .gap_4()
            .children(
                self.intro
                    .map(|intro| div().text_sm().text_color(colors.muted).child(intro)),
            )
            .children(fields)
            .children(self.error.clone().map(|message| {
                div()
                    .text_sm()
                    .text_color(colors.danger)
                    .debug_selector(|| "settings-error".into())
                    .child(message)
            }))
    }
}

#[cfg(test)]
mod tests;
