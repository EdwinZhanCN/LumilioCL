//! The options as settings groups (design language §10): one numbered
//! section per group, one row per option with its value on the trailing side.
//! A preset and a game draw the same table; only where values come from
//! differs.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, SharedString, Window};
use gpui_component::v_flex;
use lumilio_core::{GameSettingKind, GameSettingValue, game_setting_group, game_setting_kind};

use super::labels::{GROUPS, group_label, setting_label, value_label};
use crate::theme::ShellColors;
use crate::{kit, tr};

/// Where an option's value comes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    /// The place's own value: a preset's, or a game file's that no preset shares.
    Own,
    /// The game follows its preset for this option.
    Following,
    /// This game keeps its own value instead of the preset's.
    Override,
    /// Changed in the game and waiting for a decision.
    Pending,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptionLine {
    pub key: String,
    pub value: GameSettingValue,
    pub source: Source,
}

/// Opens the edit dialog for an option.
pub type OnEdit = Rc<dyn Fn(String, &mut Window, &mut App)>;
/// Switches an option the place owns, at once.
pub type OnToggle = Rc<dyn Fn(String, bool, &mut Window, &mut App)>;

/// The order the game's own menus list options in; anything else follows.
const ORDER: &[&str] = &[
    "mouseSensitivity",
    "invertYMouse",
    "rawMouseInput",
    "autoJump",
    "toggleCrouch",
    "toggleSprint",
    "key_key.forward",
    "key_key.back",
    "key_key.left",
    "key_key.right",
    "key_key.jump",
    "key_key.sneak",
    "key_key.sprint",
    "key_key.attack",
    "key_key.use",
    "key_key.pickItem",
    "key_key.drop",
    "key_key.inventory",
    "key_key.swapOffhand",
    "key_key.hotbar.1",
    "key_key.hotbar.2",
    "key_key.hotbar.3",
    "key_key.hotbar.4",
    "key_key.hotbar.5",
    "key_key.hotbar.6",
    "key_key.hotbar.7",
    "key_key.hotbar.8",
    "key_key.hotbar.9",
    "key_key.chat",
    "key_key.command",
    "key_key.playerlist",
    "key_key.advancements",
    "key_key.screenshot",
    "key_key.togglePerspective",
    "key_key.smoothCamera",
    "key_key.fullscreen",
    "key_key.spectatorOutlines",
    "key_key.saveToolbarActivator",
    "key_key.loadToolbarActivator",
    "soundCategory_master",
    "soundCategory_music",
    "soundCategory_record",
    "soundCategory_weather",
    "soundCategory_block",
    "soundCategory_hostile",
    "soundCategory_neutral",
    "soundCategory_player",
    "soundCategory_ambient",
    "soundCategory_voice",
    "soundCategory_ui",
    "renderDistance",
    "simulationDistance",
    "maxFps",
    "guiScale",
    "fov",
    "gamma",
    "entityShadows",
    "bobView",
    "mipmapLevels",
    "chatVisibility",
    "chatColors",
    "chatLinks",
    "chatLinksPrompt",
    "chatOpacity",
    "chatBackgroundOpacity",
    "chatScale",
    "chatLineSpacing",
    "chatWidth",
    "chatHeightFocused",
    "chatHeightUnfocused",
    "subtitles",
    "highContrast",
    "screenEffectScale",
    "fovEffectScale",
    "darknessEffectScale",
    "damageTiltStrength",
    "lang",
];

/// Puts lines in the game's menu order.
pub fn sort_lines(lines: &mut [OptionLine]) {
    lines.sort_by(|a, b| {
        let at = |key: &str| ORDER.iter().position(|known| *known == key);
        at(&a.key)
            .unwrap_or(usize::MAX)
            .cmp(&at(&b.key).unwrap_or(usize::MAX))
            .then_with(|| a.key.cmp(&b.key))
    });
}

/// Whether a line matches what is typed in search: its name, value or key.
pub fn matches(line: &OptionLine, needle: &str) -> bool {
    needle.is_empty()
        || setting_label(&line.key).to_lowercase().contains(needle)
        || value_label(&line.key, &line.value)
            .to_lowercase()
            .contains(needle)
        || line.key.to_lowercase().contains(needle)
}

/// The value as a row shows it, saying where it comes from (§10: an
/// inherited value says so in the value itself).
pub fn shown_value(line: &OptionLine) -> String {
    let value = value_label(&line.key, &line.value);
    match line.source {
        Source::Own => value,
        Source::Following => tr!("options-following", value = value),
        Source::Override => tr!("options-override", value = value),
        Source::Pending => tr!("options-pending", value = value),
    }
}

/// One numbered section per group that has a line. `lines` are already
/// sorted and filtered. `disabled` keeps every control still (a running game,
/// a write in progress).
pub fn sections(
    id: &'static str,
    lines: &[OptionLine],
    disabled: bool,
    colors: ShellColors,
    on_edit: &OnEdit,
    on_toggle: &OnToggle,
) -> Vec<AnyElement> {
    let mut sections = Vec::new();
    for group in GROUPS {
        let rows: Vec<_> = lines
            .iter()
            .filter(|line| game_setting_group(&line.key) == *group)
            .map(|line| row(id, line, disabled, colors, on_edit, on_toggle))
            .collect();
        if rows.is_empty() {
            continue;
        }
        sections.push(
            kit::section_at(
                sections.len() + 1,
                group_label(*group),
                colors,
                kit::list(rows, colors),
            )
            .into_any_element(),
        );
    }
    sections
}

fn row(
    id: &'static str,
    line: &OptionLine,
    disabled: bool,
    colors: ShellColors,
    on_edit: &OnEdit,
    on_toggle: &OnToggle,
) -> AnyElement {
    let row_id = SharedString::from(format!("{id}-{}", line.key));
    let key = line.key.clone();
    // A switch is a quick action only where the value is the place's own;
    // a shared value is edited in a dialog that can also return it to the preset.
    let control = match (&line.value, line.source) {
        (GameSettingValue::Boolean(on), Source::Own)
            if game_setting_kind(&line.key) == Some(GameSettingKind::Toggle) =>
        {
            let (on, toggle) = (*on, on_toggle.clone());
            crate::controls::Fader::new(
                SharedString::from(format!("{row_id}-switch")),
                on,
                setting_label(&line.key),
                move |window, cx| toggle(key.clone(), !on, window, cx),
            )
            .disabled(disabled)
            .into_any_element()
        }
        _ => {
            let edit = on_edit.clone();
            let selector = format!("{row_id}-edit");
            crate::theme::clickable(
                kit::ghost(
                    SharedString::from(format!("{row_id}-edit")),
                    tr!("common-edit"),
                    move |window, cx| edit(key.clone(), window, cx),
                )
                .disabled(disabled)
                .debug_selector(move || selector.clone()),
                !disabled,
            )
            .into_any_element()
        }
    };
    let value = match (&line.value, line.source) {
        // The switch itself shows the state.
        (GameSettingValue::Boolean(_), Source::Own) => String::new(),
        _ => shown_value(line),
    };
    kit::value_row(
        row_id,
        setting_label(&line.key),
        None,
        value,
        Some(control),
        colors,
    )
    .into_any_element()
}

/// The table, or one sentence when search leaves nothing.
pub fn table(
    id: &'static str,
    lines: &[OptionLine],
    disabled: bool,
    colors: ShellColors,
    on_edit: &OnEdit,
    on_toggle: &OnToggle,
) -> AnyElement {
    let sections = sections(id, lines, disabled, colors, on_edit, on_toggle);
    if sections.is_empty() {
        return kit::empty(tr!("options-no-match"), "", colors).into_any_element();
    }
    v_flex()
        .w_full()
        .gap_6()
        .children(sections)
        .into_any_element()
}
