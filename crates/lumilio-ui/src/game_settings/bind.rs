//! Choosing which preset a game follows, with what that changes before it
//! changes it (plan D6).
use super::form::{self, Form, Sending};
use super::labels::{setting_label, value_label};
use super::table::{OptionLine, Source, sort_lines};
use crate::theme::ShellColors;
use crate::{kit, tr};
use gpui::{App, Context, Entity, IntoElement, Render, SharedString, Window, div, prelude::*, px};
use gpui_component::{StyledExt as _, h_flex, v_flex};
use lumilio_core::{InstanceOptions, OptionsFile, SettingsPreset};

pub struct ChoosePreset {
    presets: Vec<SettingsPreset>,
    options: InstanceOptions,
    chosen: Option<i64>,
    scroll: gpui::ScrollHandle,
    sending: Sending,
    commit: form::Submit<Option<i64>, ChoosePreset>,
}

/// Why the game cannot change its preset now, if it cannot.
#[must_use]
pub fn bind_blocked(options: &InstanceOptions) -> Option<&'static str> {
    let view = &options.view;
    if view.is_busy {
        Some(tr!("options-busy"))
    } else if view.recovery_required {
        Some(tr!("options-bind-recovery"))
    } else if !view.pending.is_empty() {
        Some(tr!("options-bind-pending"))
    } else {
        None
    }
}

impl ChoosePreset {
    pub fn open(
        presets: Vec<SettingsPreset>,
        options: InstanceOptions,
        commit: form::Submit<Option<i64>, ChoosePreset>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let form = cx.new(|_| Self::new(presets, options, commit));
        form::open(form.clone(), tr!("options-bind-title"), 480., window, cx);
        form
    }

    pub(super) fn new(
        presets: Vec<SettingsPreset>,
        options: InstanceOptions,
        commit: form::Submit<Option<i64>, ChoosePreset>,
    ) -> Self {
        Self {
            chosen: options.view.preset.as_ref().map(|preset| preset.id),
            presets,
            options,
            scroll: gpui::ScrollHandle::new(),
            sending: Sending::default(),
            commit,
        }
    }

    /// Picks a preset (`None` is none) without committing.
    pub(super) fn choose(&mut self, id: Option<i64>) {
        self.chosen = id;
    }

    fn current(&self) -> Option<i64> {
        self.options.view.preset.as_ref().map(|preset| preset.id)
    }

    /// The options the chosen preset would change at the next launch, and how
    /// many it shares that this game's file does not hold.
    fn changes(&self, preset: &SettingsPreset) -> (Vec<(String, String, String)>, usize) {
        let mut lines: Vec<OptionLine> = preset
            .values
            .iter()
            .filter(|(key, _)| !preset.excluded.contains(*key))
            .map(|(key, value)| OptionLine {
                key: key.clone(),
                value: value.clone(),
                source: Source::Own,
            })
            .collect();
        sort_lines(&mut lines);
        let mut changes = Vec::new();
        let mut missing = 0;
        for line in lines {
            match self.options.values.get(&line.key) {
                Some(now) if *now != line.value => changes.push((
                    setting_label(&line.key),
                    value_label(&line.key, now),
                    value_label(&line.key, &line.value),
                )),
                Some(_) => {}
                None => missing += 1,
            }
        }
        (changes, missing)
    }

    pub fn finished(
        &mut self,
        result: Result<String, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sending.finish(result, window, cx);
        cx.notify();
    }

    fn preview(&self, colors: ShellColors) -> Option<gpui::AnyElement> {
        if self.chosen == self.current() {
            return None;
        }
        let Some(preset) = self
            .chosen
            .and_then(|id| self.presets.iter().find(|preset| preset.id == id))
        else {
            return Some(
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .child(tr!("options-bind-none-preview"))
                    .into_any_element(),
            );
        };
        if self.options.file != OptionsFile::Ready {
            return Some(
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .child(tr!("options-bind-first-launch"))
                    .into_any_element(),
            );
        }
        let (changes, missing) = self.changes(preset);
        let headline = if changes.is_empty() {
            tr!("options-bind-same").to_owned()
        } else {
            tr!("options-bind-changes", count = changes.len())
        };
        let rows: Vec<_> = changes
            .into_iter()
            .map(|(label, now, next)| {
                h_flex()
                    .w_full()
                    .gap_3()
                    .py(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(label),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family(crate::theme::mono_font())
                            .text_xs()
                            .text_color(colors.muted)
                            .child(tr!("options-change", before = now, after = next)),
                    )
            })
            .collect();
        let has_rows = !rows.is_empty();
        let list = div()
            .id("options-bind-changes")
            .max_h(px(220.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(kit::list(rows, colors));
        Some(
            v_flex()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(colors.foreground)
                        .child(headline),
                )
                .when(has_rows, |column| {
                    column.child(kit::keep_wheel(list, &self.scroll))
                })
                .when(missing > 0, |column| {
                    column.child(
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .child(tr!("options-bind-missing", count = missing)),
                    )
                })
                .into_any_element(),
        )
    }
}

impl Form for ChoosePreset {
    fn commit_label(&self) -> &'static str {
        if self.chosen.is_some() {
            tr!("options-bind-commit")
        } else {
            tr!("options-unbind-commit")
        }
    }
    fn ready(&self, _: &App) -> bool {
        self.chosen != self.current() && bind_blocked(&self.options).is_none()
    }
    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        (self.commit)(self.chosen, cx.entity().downgrade(), window, cx);
    }
    fn sending(&mut self) -> &mut Sending {
        &mut self.sending
    }
    fn sending_ref(&self) -> &Sending {
        &self.sending
    }
}

impl Render for ChoosePreset {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sending.flush(window, cx);
        let colors = ShellColors::current(cx);
        let body = colors.body;
        let busy = self.sending.busy;
        let blocked = bind_blocked(&self.options);
        let weak = cx.entity().downgrade();
        let choice = |id: Option<i64>, title: String, detail: String| {
            let chosen = self.chosen == id;
            let weak = weak.clone();
            let still = busy || blocked.is_some();
            h_flex()
                .id(SharedString::from(format!(
                    "options-bind-{}",
                    id.map_or_else(|| "none".to_owned(), |id| id.to_string())
                )))
                .w_full()
                .gap_3()
                .items_center()
                .px_3()
                .py(px(8.))
                .rounded_md()
                .when(chosen, |row| row.bg(colors.surface_subtle))
                .when(!still, |row| {
                    row.cursor_pointer()
                        .hover(|row| row.bg(colors.surface_subtle))
                        .on_click(move |_, _, cx| {
                            let _ = weak.update(cx, |form, cx| {
                                form.choose(id);
                                cx.notify();
                            });
                        })
                })
                .child(crate::controls::radio(chosen, body))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .text_sm()
                                .when(chosen, |label| label.font_medium())
                                .text_color(crate::controls::radio_label(chosen, body))
                                .child(title),
                        )
                        .child(div().text_xs().text_color(colors.muted).child(detail)),
                )
        };
        let mut choices = vec![choice(
            None,
            tr!("options-preset-none").to_owned(),
            tr!("options-preset-none-detail").to_owned(),
        )];
        for preset in &self.presets {
            let shared = preset.values.len() - preset.excluded.len();
            choices.push(choice(
                Some(preset.id),
                preset.name.clone(),
                tr!("preset-fields", count = shared),
            ));
        }
        v_flex()
            .id("options-bind")
            .w_full()
            .gap_4()
            .children(blocked.map(|reason| {
                div()
                    .text_sm()
                    .text_color(colors.danger)
                    .debug_selector(|| "options-bind-blocked".into())
                    .child(reason)
            }))
            .child(v_flex().gap_1().children(choices))
            .when(self.presets.is_empty(), |column| {
                column.child(
                    div()
                        .text_sm()
                        .text_color(colors.muted)
                        .child(tr!("options-bind-no-presets")),
                )
            })
            .children(self.preview(colors))
    }
}
