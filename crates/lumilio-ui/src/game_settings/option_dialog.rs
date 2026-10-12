//! Editing one option outside the game. The control follows the value
//! (AGENTS.md): a slider with a typed number for a range, segment keys for a
//! few choices, a searchable list for keys and languages.
use super::choices::{key_label, key_tokens, language_codes, language_label};
use super::form::{self, Form, Sending};
use super::labels::{choice_label, number_label, setting_label, shown_percent, value_label};
use crate::controls::Segments;
use crate::key::Key;
use crate::theme::ShellColors;
use crate::{theme, tr};
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_component::slider::{Slider, SliderEvent, SliderState};
use gpui_component::{IndexPath, h_flex, v_flex};
use lumilio_core::{GameSettingKind, GameSettingValue, OptionChange, game_setting_kind};

/// Where a decided change goes. The application runs it and answers with
/// [`OptionDialog::finished`].
pub type Commit = form::Submit<OptionChange, OptionDialog>;

/// What the dialog edits and where the value comes from now.
#[derive(Clone, Debug)]
pub struct OptionSpec {
    pub key: String,
    pub value: GameSettingValue,
    /// The preset's value, when the game shares this option: the dialog can
    /// then return the option to it.
    pub follow: Option<GameSettingValue>,
    /// The game follows the preset for this option now.
    pub following: bool,
    /// One sentence on what saving changes, when the place does not say.
    pub note: Option<String>,
}

enum Control {
    Switch(bool),
    Choice {
        index: i64,
        count: u8,
    },
    Range {
        value: i64,
        min: i64,
        max: i64,
        slider: Entity<SliderState>,
        input: Entity<InputState>,
    },
    Pick {
        values: Vec<String>,
        select: Entity<SelectState<SearchableVec<String>>>,
    },
}

pub struct OptionDialog {
    spec: OptionSpec,
    kind: GameSettingKind,
    control: Control,
    /// What is typed is not a number in range.
    invalid: bool,
    sending: Sending,
    commit: Commit,
}

impl OptionDialog {
    /// Opens the editor for a known option; an option the catalog does not
    /// know has no editor.
    pub fn open(spec: OptionSpec, commit: Commit, window: &mut Window, cx: &mut App) {
        let Some(kind) = game_setting_kind(&spec.key) else {
            return;
        };
        let title = setting_label(&spec.key);
        let form = cx.new(|cx| Self::new(spec, kind, commit, window, cx));
        form::open(form, title, 440., window, cx);
    }

    pub(super) fn new(
        spec: OptionSpec,
        kind: GameSettingKind,
        commit: Commit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let control = match kind {
            GameSettingKind::Toggle => {
                Control::Switch(matches!(spec.value, GameSettingValue::Boolean(true)))
            }
            GameSettingKind::Choice { count } => Control::Choice {
                index: match spec.value {
                    GameSettingValue::Integer(index) => index,
                    _ => 0,
                },
                count,
            },
            GameSettingKind::Percent { max } => {
                let value = shown_percent(&spec.value, max).unwrap_or(0);
                Self::range(value, 0, i64::from(max), window, cx)
            }
            GameSettingKind::Number { min, max } => {
                let value = match spec.value {
                    GameSettingValue::Integer(value) => value,
                    _ => min,
                };
                Self::range(value, min, max, window, cx)
            }
            GameSettingKind::Key | GameSettingKind::Language => {
                let current = match &spec.value {
                    GameSettingValue::Text(text) => text.clone(),
                    other => other.to_string(),
                };
                let (values, labels): (Vec<String>, Vec<String>) = if kind == GameSettingKind::Key {
                    let mut tokens = key_tokens();
                    if !tokens.contains(&current) {
                        tokens.insert(0, current.clone());
                    }
                    let labels = tokens.iter().map(|token| key_label(token)).collect();
                    (tokens, labels)
                } else {
                    let codes = language_codes(Some(&current));
                    let labels = codes
                        .iter()
                        .map(|code| match language_label(code) {
                            name if name == *code => name,
                            name => format!("{name} · {code}"),
                        })
                        .collect();
                    (codes, labels)
                };
                let at = values.iter().position(|value| *value == current);
                let select = cx.new(|cx| {
                    SelectState::new(
                        SearchableVec::new(labels),
                        at.map(IndexPath::new),
                        window,
                        cx,
                    )
                    .searchable(true)
                });
                cx.subscribe(
                    &select,
                    |_, _, _: &SelectEvent<SearchableVec<String>>, cx| cx.notify(),
                )
                .detach();
                Control::Pick { values, select }
            }
        };
        Self {
            spec,
            kind,
            control,
            invalid: false,
            sending: Sending::default(),
            commit,
        }
    }

    /// A slider and a typed number that follow each other.
    fn range(
        value: i64,
        min: i64,
        max: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Control {
        let value = value.clamp(min, max);
        let slider = cx.new(|_| {
            SliderState::new()
                .min(min as f32)
                .max(max as f32)
                .step(1.)
                .default_value(value as f32)
        });
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(format!("{min}–{max}"))
                .default_value(value.to_string())
        });
        cx.subscribe_in(
            &slider,
            window,
            |form, _, event: &SliderEvent, window, cx| {
                let (SliderEvent::Change(moved) | SliderEvent::Release(moved)) = event;
                let moved = moved.end().round() as i64;
                if let Control::Range { value, input, .. } = &mut form.control {
                    *value = moved;
                    // Setting the text does not send a change back.
                    input.update(cx, |input, cx| {
                        input.set_value(moved.to_string(), window, cx)
                    });
                }
                form.invalid = false;
                cx.notify();
            },
        )
        .detach();
        cx.subscribe_in(
            &input,
            window,
            |form, input, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let typed = input.read(cx).value().trim().parse::<i64>().ok();
                if let Control::Range {
                    value,
                    min,
                    max,
                    slider,
                    ..
                } = &mut form.control
                {
                    match typed.filter(|typed| (*min..=*max).contains(typed)) {
                        Some(typed) => {
                            *value = typed;
                            slider.update(cx, |slider, cx| {
                                slider.set_value(typed as f32, window, cx)
                            });
                            form.invalid = false;
                        }
                        None => form.invalid = true,
                    }
                }
                cx.notify();
            },
        )
        .detach();
        Control::Range {
            value,
            min,
            max,
            slider,
            input,
        }
    }

    /// The value the dialog would save, if what is entered is valid.
    pub fn draft(&self, cx: &App) -> Option<GameSettingValue> {
        match (&self.control, self.kind) {
            (Control::Switch(on), _) => Some(GameSettingValue::Boolean(*on)),
            (Control::Choice { index, .. }, _) => Some(GameSettingValue::Integer(*index)),
            (Control::Range { .. }, _) if self.invalid => None,
            (Control::Range { value, .. }, GameSettingKind::Percent { max }) => {
                GameSettingValue::fraction(*value as f64 / f64::from(max))
            }
            (Control::Range { value, .. }, _) => Some(GameSettingValue::Integer(*value)),
            (Control::Pick { values, select }, _) => {
                let at = select.read(cx).selected_index(cx)?.row;
                values.get(at).cloned().map(GameSettingValue::Text)
            }
        }
    }

    /// Returns the option to the preset's value, once.
    pub(super) fn follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.follow.is_some() && self.sending.begin() {
            (self.commit)(OptionChange::Follow, cx.entity().downgrade(), window, cx);
            cx.notify();
        }
    }

    #[cfg(test)]
    pub(super) fn range_parts(&self) -> Option<(Entity<SliderState>, Entity<InputState>)> {
        match &self.control {
            Control::Range { slider, input, .. } => Some((slider.clone(), input.clone())),
            _ => None,
        }
    }

    /// A write finished; see [`Sending::finish`].
    pub fn finished(
        &mut self,
        result: Result<String, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sending.finish(result, window, cx);
        cx.notify();
    }

    fn control(&self, colors: ShellColors, cx: &mut Context<Self>) -> gpui::AnyElement {
        let busy = self.sending.busy;
        let weak = cx.entity().downgrade();
        match &self.control {
            Control::Switch(on) => {
                let labels = [tr!("preset-on"), tr!("preset-off")];
                h_flex()
                    .child(Segments::new(
                        "option-switch",
                        &labels,
                        usize::from(!*on),
                        move |index, _, cx| {
                            let _ = weak.update(cx, |form, cx| {
                                if !form.sending.busy {
                                    form.control = Control::Switch(index == 0);
                                    cx.notify();
                                }
                            });
                        },
                    ))
                    .into_any_element()
            }
            Control::Choice { index, count } => {
                let count = *count;
                let labels: Vec<String> = (0..i64::from(count))
                    .map(|index| choice_label(&self.spec.key, index))
                    .collect();
                h_flex()
                    .child(Segments::new(
                        "option-choice",
                        &labels,
                        usize::try_from(*index).unwrap_or(0),
                        move |chosen, _, cx| {
                            let _ = weak.update(cx, |form, cx| {
                                if !form.sending.busy {
                                    form.control = Control::Choice {
                                        index: chosen as i64,
                                        count,
                                    };
                                    cx.notify();
                                }
                            });
                        },
                    ))
                    .into_any_element()
            }
            Control::Range {
                value,
                slider,
                input,
                ..
            } => {
                let unit = match self.kind {
                    GameSettingKind::Percent { .. } => Some("%"),
                    _ if self.spec.key == "fov" => Some("°"),
                    _ => None,
                };
                // Distances, frame rates and the automatic scale read as the game says them.
                let reading = number_label(&self.spec.key, *value);
                let plain = matches!(self.kind, GameSettingKind::Percent { .. })
                    || reading == value.to_string()
                    || self.spec.key == "fov";
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_4()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .debug_selector(|| "option-slider".into())
                                    .child(Slider::new(slider).disabled(busy)),
                            )
                            .child(
                                div()
                                    .w(px(96.))
                                    .child(Input::new(input).disabled(busy).when_some(
                                        unit,
                                        |field, unit| {
                                            field.suffix(div().text_color(colors.muted).child(unit))
                                        },
                                    )),
                            ),
                    )
                    .when(!plain, |column| {
                        column.child(
                            div()
                                .font_family(theme::mono_font())
                                .text_xs()
                                .text_color(colors.muted)
                                .child(reading),
                        )
                    })
                    .into_any_element()
            }
            Control::Pick { select, .. } => Select::new(select).disabled(busy).into_any_element(),
        }
    }
}

impl Form for OptionDialog {
    fn commit_label(&self) -> &'static str {
        tr!("common-save")
    }

    fn ready(&self, cx: &App) -> bool {
        self.draft(cx).is_some_and(|draft| draft != self.spec.value)
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(draft) = self.draft(cx) {
            (self.commit)(
                OptionChange::Set(draft),
                cx.entity().downgrade(),
                window,
                cx,
            );
        }
    }

    fn sending(&mut self) -> &mut Sending {
        &mut self.sending
    }

    fn sending_ref(&self) -> &Sending {
        &self.sending
    }

    fn aside(&self, form: &Entity<Self>, _: &App) -> Option<Key> {
        let shared = self.spec.follow.as_ref().filter(|_| !self.spec.following)?;
        let weak = form.downgrade();
        Some(theme::clickable(
            Key::new("option-follow")
                .label(tr!(
                    "options-follow",
                    value = value_label(&self.spec.key, shared)
                ))
                .ghost()
                .disabled(self.sending.busy)
                .debug_selector(|| "option-follow".into())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |form, cx| form.follow(window, cx));
                }),
            !self.sending.busy,
        ))
    }
}

impl Render for OptionDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sending.flush(window, cx);
        let colors = ShellColors::current(cx);
        let range = match &self.control {
            Control::Range { min, max, .. } => Some((*min, *max)),
            _ => None,
        };
        v_flex()
            .id("option-dialog")
            .w_full()
            .gap_4()
            .children(
                self.spec
                    .note
                    .clone()
                    .map(|note| div().text_sm().text_color(colors.muted).child(note)),
            )
            .child(self.control(colors, cx))
            .children(range.filter(|_| self.invalid).map(|(min, max)| {
                div()
                    .text_sm()
                    .text_color(colors.danger)
                    .debug_selector(|| "option-invalid".into())
                    .child(tr!("options-out-of-range", min = min, max = max))
            }))
    }
}
