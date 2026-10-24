//! Making and naming presets, and choosing what a preset shares.

use std::collections::BTreeSet;

use gpui::prelude::*;
use gpui::{App, Context, Entity, IntoElement, Render, SharedString, Window, div, px};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_component::{IndexPath, StyledExt as _, h_flex, v_flex};
use lumilio_core::{GameSettingGroup, SettingsPreset, game_setting_group};

use super::form;
use super::form::{Form, Sending};
use super::labels::{GROUPS, group_detail, group_label, setting_label};
use super::table::{OptionLine, Source, sort_lines};
use crate::controls::Checkbox;
use crate::theme::ShellColors;
use crate::{kit, tr};

const NAME_LIMIT: usize = 80;

fn name_ok(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && name.chars().count() <= NAME_LIMIT
}

/// A field's label over its control (§10).
fn field(label: &'static str, control: impl IntoElement, colors: ShellColors) -> gpui::Div {
    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_medium()
                .text_color(colors.foreground)
                .child(label),
        )
        .child(control)
}

fn name_input<V: 'static>(
    value: String,
    window: &mut Window,
    cx: &mut Context<V>,
) -> Entity<InputState> {
    let input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(tr!("preset-name-placeholder"))
            .default_value(value)
    });
    cx.subscribe(&input, |_, _, _: &InputEvent, cx| cx.notify())
        .detach();
    input
}

/// A game a preset can be made from.
#[derive(Clone, Debug)]
pub struct SourceGame {
    pub id: String,
    pub name: String,
    pub version: String,
}

/// What a new preset is made of.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewPreset {
    pub source: String,
    pub name: String,
    pub groups: Vec<GameSettingGroup>,
}

pub struct CreatePreset {
    games: Vec<SourceGame>,
    name: Entity<InputState>,
    source: Entity<SelectState<SearchableVec<String>>>,
    groups: Vec<GameSettingGroup>,
    sending: Sending,
    commit: form::Submit<NewPreset, CreatePreset>,
}

impl CreatePreset {
    pub fn open(
        games: Vec<SourceGame>,
        preferred: Option<String>,
        commit: form::Submit<NewPreset, CreatePreset>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let form = cx.new(|cx| Self::new(games, preferred, commit, window, cx));
        form::open(form.clone(), tr!("preset-create-title"), 480., window, cx);
        form
    }

    pub(super) fn new(
        games: Vec<SourceGame>,
        preferred: Option<String>,
        commit: form::Submit<NewPreset, CreatePreset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let at = preferred
            .and_then(|id| games.iter().position(|game| game.id == id))
            .unwrap_or(0);
        let labels: Vec<String> = games
            .iter()
            .map(|game| format!("{} · {}", game.name, game.version))
            .collect();
        let source = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(labels),
                (!games.is_empty()).then(|| IndexPath::new(at)),
                window,
                cx,
            )
            .searchable(true)
        });
        cx.subscribe(
            &source,
            |_, _, _: &SelectEvent<SearchableVec<String>>, cx| cx.notify(),
        )
        .detach();
        Self {
            name: name_input(String::new(), window, cx),
            games,
            source,
            // Personal habits travel by default; the picture depends on
            // the modpack, so it is shared only when asked (plan D2).
            groups: GROUPS
                .iter()
                .copied()
                .filter(|group| *group != GameSettingGroup::Video)
                .collect(),
            sending: Sending::default(),
            commit,
        }
    }

    #[cfg(test)]
    pub(super) fn name_field(&self) -> &Entity<InputState> {
        &self.name
    }

    fn draft(&self, cx: &App) -> Option<NewPreset> {
        let name = self.name.read(cx).value().trim().to_owned();
        let at = self.source.read(cx).selected_index(cx)?.row;
        let game = self.games.get(at)?;
        (name_ok(&name) && !self.groups.is_empty()).then(|| NewPreset {
            source: game.id.clone(),
            name,
            groups: self.groups.clone(),
        })
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
}

impl Form for CreatePreset {
    fn commit_label(&self) -> &'static str {
        tr!("preset-create")
    }
    fn ready(&self, cx: &App) -> bool {
        self.draft(cx).is_some()
    }
    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(draft) = self.draft(cx) {
            (self.commit)(draft, cx.entity().downgrade(), window, cx);
        }
    }
    fn sending(&mut self) -> &mut Sending {
        &mut self.sending
    }
    fn sending_ref(&self) -> &Sending {
        &self.sending
    }
}

impl Render for CreatePreset {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sending.flush(window, cx);
        let colors = ShellColors::current(cx);
        let busy = self.sending.busy;
        let weak = cx.entity().downgrade();
        let groups = GROUPS.iter().map(|group| {
            let group = *group;
            let on = self.groups.contains(&group);
            let weak = weak.clone();
            v_flex()
                .gap_1()
                .child(
                    Checkbox::new(("preset-group", group as usize))
                        .label(group_label(group))
                        .checked(on)
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let _ = weak.update(cx, |form, cx| {
                                if on {
                                    form.groups.retain(|kept| *kept != group);
                                } else {
                                    form.groups.push(group);
                                }
                                cx.notify();
                            });
                        }),
                )
                .child(
                    div()
                        .pl(px(28.))
                        .text_xs()
                        .text_color(colors.muted)
                        .child(group_detail(group)),
                )
        });
        v_flex()
            .id("preset-create")
            .w_full()
            .gap_4()
            .child(field(
                tr!("preset-name"),
                Input::new(&self.name).disabled(busy),
                colors,
            ))
            .child(field(
                tr!("preset-source"),
                Select::new(&self.source)
                    .disabled(busy || self.games.is_empty())
                    .placeholder(tr!("preset-source-none")),
                colors,
            ))
            .child(field(
                tr!("preset-share"),
                v_flex().gap_3().children(groups),
                colors,
            ))
    }
}

/// What a name dialog does with the name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Naming {
    Rename,
    Copy,
}

pub struct NamePreset {
    naming: Naming,
    original: String,
    name: Entity<InputState>,
    sending: Sending,
    commit: form::Submit<String, NamePreset>,
}

impl NamePreset {
    pub fn open(
        naming: Naming,
        preset: &SettingsPreset,
        commit: form::Submit<String, NamePreset>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let original = preset.name.clone();
        let start = match naming {
            Naming::Rename => original.clone(),
            Naming::Copy => tr!("preset-copy-name", name = original.as_str()),
        };
        let form = cx.new(|cx| Self {
            naming,
            name: name_input(start, window, cx),
            original,
            sending: Sending::default(),
            commit,
        });
        let title = match naming {
            Naming::Rename => tr!("preset-rename-title"),
            Naming::Copy => tr!("preset-copy-title"),
        };
        form::open(form.clone(), title, 420., window, cx);
        form
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
}

impl Form for NamePreset {
    fn commit_label(&self) -> &'static str {
        match self.naming {
            Naming::Rename => tr!("common-save"),
            Naming::Copy => tr!("preset-copy-commit"),
        }
    }
    fn ready(&self, cx: &App) -> bool {
        let name = self.name.read(cx).value();
        name_ok(&name) && (self.naming == Naming::Copy || name.trim() != self.original)
    }
    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim().to_owned();
        (self.commit)(name, cx.entity().downgrade(), window, cx);
    }
    fn sending(&mut self) -> &mut Sending {
        &mut self.sending
    }
    fn sending_ref(&self) -> &Sending {
        &self.sending
    }
}

impl Render for NamePreset {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sending.flush(window, cx);
        let colors = ShellColors::current(cx);
        v_flex().id("preset-name").w_full().child(field(
            tr!("preset-name"),
            Input::new(&self.name).disabled(self.sending.busy),
            colors,
        ))
    }
}

/// Which of the options a preset holds it shares. Leaving one out keeps its
/// value in the preset, so it can be shared again later (plan D9).
pub struct PresetScope {
    preset: SettingsPreset,
    shared: BTreeSet<String>,
    scroll: gpui::ScrollHandle,
    sending: Sending,
    commit: form::Submit<BTreeSet<String>, PresetScope>,
}

impl PresetScope {
    pub fn open(
        preset: SettingsPreset,
        commit: form::Submit<BTreeSet<String>, PresetScope>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let shared = preset
            .values
            .keys()
            .filter(|key| !preset.excluded.contains(*key))
            .cloned()
            .collect();
        let form = cx.new(|_| Self {
            preset,
            shared,
            scroll: gpui::ScrollHandle::new(),
            sending: Sending::default(),
            commit,
        });
        form::open(form.clone(), tr!("preset-scope-title"), 480., window, cx);
        form
    }

    /// What leaving the rest out means for the preset.
    fn excluded(&self) -> BTreeSet<String> {
        self.preset
            .values
            .keys()
            .filter(|key| !self.shared.contains(*key))
            .cloned()
            .collect()
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
}

impl Form for PresetScope {
    fn commit_label(&self) -> &'static str {
        tr!("common-save")
    }
    fn ready(&self, _: &App) -> bool {
        !self.shared.is_empty() && self.excluded() != self.preset.excluded
    }
    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        (self.commit)(self.excluded(), cx.entity().downgrade(), window, cx);
    }
    fn sending(&mut self) -> &mut Sending {
        &mut self.sending
    }
    fn sending_ref(&self) -> &Sending {
        &self.sending
    }
}

impl Render for PresetScope {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sending.flush(window, cx);
        let colors = ShellColors::current(cx);
        let busy = self.sending.busy;
        let weak = cx.entity().downgrade();
        let mut lines: Vec<OptionLine> = self
            .preset
            .values
            .iter()
            .map(|(key, value)| OptionLine {
                key: key.clone(),
                value: value.clone(),
                source: Source::Own,
            })
            .collect();
        sort_lines(&mut lines);
        let mut groups = Vec::new();
        for group in GROUPS {
            let keys: Vec<String> = lines
                .iter()
                .filter(|line| game_setting_group(&line.key) == *group)
                .map(|line| line.key.clone())
                .collect();
            if keys.is_empty() {
                continue;
            }
            let count = keys.iter().filter(|key| self.shared.contains(*key)).count();
            let all = count == keys.len();
            let toggle_all = {
                let (weak, keys) = (weak.clone(), keys.clone());
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                    let _ = weak.update(cx, |form, cx| {
                        for key in &keys {
                            if all {
                                form.shared.remove(key);
                            } else {
                                form.shared.insert(key.clone());
                            }
                        }
                        cx.notify();
                    });
                }
            };
            let fields = keys.into_iter().map(|key| {
                let on = self.shared.contains(&key);
                let weak = weak.clone();
                Checkbox::new(SharedString::from(format!("preset-scope-{key}")))
                    .label(setting_label(&key))
                    .checked(on)
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |form, cx| {
                            if on {
                                form.shared.remove(&key);
                            } else {
                                form.shared.insert(key.clone());
                            }
                            cx.notify();
                        });
                    })
            });
            groups.push(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Checkbox::new(("preset-scope-group", *group as usize))
                                    .label(group_label(*group))
                                    .checked(count > 0)
                                    .disabled(busy)
                                    .on_click(toggle_all),
                            )
                            .child(
                                div()
                                    .font_family(crate::theme::mono_font())
                                    .text_xs()
                                    .text_color(colors.muted)
                                    .child(format!("{count}/{}", lines_in(&lines, *group))),
                            ),
                    )
                    .child(v_flex().pl(px(28.)).gap_2().children(fields)),
            );
        }
        let list = div()
            .id("preset-scope-list")
            .max_h(px(360.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(v_flex().gap_4().pr_2().children(groups));
        v_flex()
            .id("preset-scope")
            .w_full()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .child(tr!("preset-scope-help")),
            )
            .child(kit::keep_wheel(list, &self.scroll))
    }
}

fn lines_in(lines: &[OptionLine], group: GameSettingGroup) -> usize {
    lines
        .iter()
        .filter(|line| game_setting_group(&line.key) == group)
        .count()
}
