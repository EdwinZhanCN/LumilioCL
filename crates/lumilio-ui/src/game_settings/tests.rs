use super::form::{Form, submit};
use super::*;
use gpui::{Entity, TestAppContext, VisualTestContext, WeakEntity, Window};
use gpui_component::input::InputEvent;
use gpui_component::slider::{SliderEvent, SliderValue};
use lumilio_core::{GameSettingKind, OptionChange, OptionsFile, SettingChange};
use std::cell::RefCell;
use std::rc::Rc;

fn preset() -> SettingsPreset {
    serde_json::from_value(serde_json::json!({
        "id": 1,
        "name": "Daily",
        "revision": 1,
        "values": {
            "soundCategory_master": {"Decimal": "0.8"},
            "renderDistance": {"Integer": 16},
            "fov": {"Integer": 90},
            "autoJump": {"Boolean": false}
        }
    }))
    .unwrap()
}

/// A game that follows "Daily", has kept its own render distance, changed
/// the volume in the game, and owns its field of view.
fn bound_game() -> InstanceOptions {
    let mut shared = preset();
    shared.excluded.insert("fov".into());
    let mut options = InstanceOptions::default();
    options.game_version = "1.21.1".into();
    options.file = OptionsFile::Ready;
    options.view.instance = "game".into();
    options.view.preset = Some(shared);
    options
        .view
        .overrides
        .insert("renderDistance".into(), GameSettingValue::Integer(12));
    options.view.pending = vec![
        serde_json::from_value::<SettingChange>(serde_json::json!({
            "setting": "soundCategory_master",
            "before": {"Decimal": "0.8"},
            "value": {"Decimal": "0.4"},
            "shared_before": {"Decimal": "0.8"}
        }))
        .unwrap(),
    ];
    for (key, value) in [
        (
            "soundCategory_master",
            GameSettingValue::Decimal("0.4".into()),
        ),
        ("renderDistance", GameSettingValue::Integer(12)),
        ("fov", GameSettingValue::Integer(70)),
        ("autoJump", GameSettingValue::Boolean(false)),
    ] {
        options.values.insert(key.into(), value);
    }
    options
}

#[test]
fn values_read_in_the_units_the_game_shows() {
    let label = |key: &str, value: GameSettingValue| value_label(key, &value);
    assert_eq!(setting_label("soundCategory_master"), "主音量");
    assert_eq!(setting_label("key_key.swapOffhand"), "交换副手物品");
    assert_eq!(label("fov", GameSettingValue::Integer(90)), "90°");
    assert_eq!(
        label("mouseSensitivity", GameSettingValue::Decimal("0.5".into())),
        "100%",
        "the game labels sensitivity as twice its stored fraction"
    );
    assert_eq!(
        label(
            "soundCategory_master",
            GameSettingValue::Decimal("0.4".into())
        ),
        "40%"
    );
    assert_eq!(
        label("renderDistance", GameSettingValue::Integer(12)),
        "12 区块"
    );
    assert_eq!(label("maxFps", GameSettingValue::Integer(260)), "无限制");
    assert_eq!(label("guiScale", GameSettingValue::Integer(0)), "自动");
    assert_eq!(
        label("chatVisibility", GameSettingValue::Integer(1)),
        "仅命令"
    );
    let key = |token: &str| label("key_key.sneak", GameSettingValue::Text(token.into()));
    assert_eq!(key("key.keyboard.left.shift"), "左 Shift");
    assert_eq!(key("key.keyboard.left"), "←");
    assert_eq!(key("key.keyboard.left.bracket"), "[");
    assert_eq!(key("key.keyboard.keypad.0"), "小键盘 0");
    assert_eq!(key("key.mouse.right"), "鼠标右键");
    assert_eq!(key("key.keyboard.unknown"), "未绑定");
    assert_eq!(key("key.keyboard.w"), "W");
    assert_eq!(
        label("lang", GameSettingValue::Text("en_us".into())),
        "English (US)"
    );
}

#[test]
fn a_game_row_says_where_its_value_comes_from() {
    let lines = instance_lines(&bound_game());
    let source = |key: &str| {
        let line = lines.iter().find(|line| line.key == key).unwrap();
        (line.source, line.value.clone())
    };
    assert_eq!(
        source("soundCategory_master"),
        (Source::Pending, GameSettingValue::Decimal("0.4".into()))
    );
    assert_eq!(
        source("renderDistance"),
        (Source::Override, GameSettingValue::Integer(12))
    );
    assert_eq!(
        source("autoJump"),
        (Source::Following, GameSettingValue::Boolean(false))
    );
    assert_eq!(
        source("fov"),
        (Source::Own, GameSettingValue::Integer(70)),
        "an option the preset does not share is the game's own"
    );
    assert_eq!(
        lines
            .iter()
            .map(|line| line.key.as_str())
            .collect::<Vec<_>>(),
        ["autoJump", "soundCategory_master", "renderDistance", "fov"],
        "in the order of the game's own menus"
    );
    let spec = option_spec(&bound_game(), "renderDistance").unwrap();
    assert_eq!(spec.follow, Some(GameSettingValue::Integer(16)));
    assert!(!spec.following && spec.note.is_some());
    let own = option_spec(&bound_game(), "fov").unwrap();
    assert!(own.follow.is_none() && own.note.is_none());
}

type Seen = Rc<RefCell<Vec<OptionChange>>>;

fn option_dialog(
    cx: &mut TestAppContext,
    spec: OptionSpec,
) -> (Entity<OptionDialog>, &mut VisualTestContext, Seen) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Seen = Rc::default();
    let sink = seen.clone();
    let commit: Commit = Rc::new(
        move |change, _: WeakEntity<OptionDialog>, _: &mut Window, _: &mut gpui::App| {
            sink.borrow_mut().push(change);
        },
    );
    let kind = lumilio_core::game_setting_kind(&spec.key).unwrap();
    let (form, cx) =
        cx.add_window_view(move |window, cx| OptionDialog::new(spec, kind, commit, window, cx));
    cx.run_until_parked();
    (form, cx, seen)
}

fn own(key: &str, value: GameSettingValue) -> OptionSpec {
    OptionSpec {
        key: key.into(),
        value,
        follow: None,
        following: false,
        note: None,
    }
}

#[gpui::test]
fn a_range_is_typed_or_slid_and_only_a_valid_change_is_saved_once(cx: &mut TestAppContext) {
    let (form, cx, seen) = option_dialog(
        cx,
        own("mouseSensitivity", GameSettingValue::Decimal("0.5".into())),
    );
    assert_eq!(
        lumilio_core::game_setting_kind("mouseSensitivity"),
        Some(GameSettingKind::Percent { max: 200 })
    );
    form.read_with(cx, |form, cx| {
        assert!(!form.ready(cx), "nothing changed yet")
    });
    let (slider, input) = form.read_with(cx, |form, _| form.range_parts().unwrap());
    // Typing a number moves the slider with it.
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("150", window, cx);
            cx.emit(InputEvent::Change);
        })
    });
    cx.run_until_parked();
    assert_eq!(slider.read_with(cx, |slider, _| slider.value().end()), 150.);
    form.read_with(cx, |form, cx| {
        assert_eq!(
            form.draft(cx),
            Some(GameSettingValue::Decimal("0.75".into()))
        )
    });
    // A number outside the game's range cannot be saved.
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("250", window, cx);
            cx.emit(InputEvent::Change);
        })
    });
    cx.run_until_parked();
    form.read_with(cx, |form, cx| assert!(!form.ready(cx)));
    // Sliding writes the number back into the field.
    slider.update(cx, |_, cx| {
        cx.emit(SliderEvent::Release(SliderValue::Single(40.)))
    });
    cx.run_until_parked();
    assert_eq!(
        input.read_with(cx, |input, _| input.value().to_string()),
        "40"
    );
    cx.update(|window, cx| {
        form.update(cx, |form, cx| {
            submit(form, window, cx);
            submit(form, window, cx);
        })
    });
    assert_eq!(
        *seen.borrow(),
        vec![OptionChange::Set(GameSettingValue::Decimal("0.2".into()))],
        "a second press while it is written sends nothing"
    );
    cx.update(|window, cx| {
        form.update(cx, |form, cx| {
            form.finished(Err("没能保存".into()), window, cx)
        })
    });
    form.read_with(cx, |form, cx| {
        assert!(
            !form.sending_ref().busy && form.ready(cx),
            "a failure keeps the draft"
        )
    });
}

#[gpui::test]
fn a_shared_option_can_go_back_to_the_preset(cx: &mut TestAppContext) {
    let spec = option_spec(&bound_game(), "renderDistance").unwrap();
    let (form, cx, seen) = option_dialog(cx, spec);
    let aside = cx.update(|_, cx| form.read(cx).aside(&form, cx));
    assert!(aside.is_some(), "an override offers the preset's value");
    cx.update(|window, cx| {
        form.update(cx, |form, cx| {
            form.follow(window, cx);
            form.follow(window, cx);
        })
    });
    assert_eq!(*seen.borrow(), vec![OptionChange::Follow]);
}

#[gpui::test]
fn a_followed_option_has_nothing_to_return_to(cx: &mut TestAppContext) {
    let following = option_spec(&bound_game(), "autoJump").unwrap();
    let (form, cx, _) = option_dialog(cx, following);
    form.read_with(cx, |form, cx| {
        assert!(
            !form.ready(cx),
            "saving the preset's own value pins nothing"
        )
    });
    assert!(
        cx.update(|_, cx| form.read(cx).aside(&form, cx)).is_none(),
        "a value that already follows has nothing to return to"
    );
}

#[gpui::test]
fn a_preset_is_made_from_a_named_game_without_the_picture_by_default(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let made: Rc<RefCell<Vec<NewPreset>>> = Rc::default();
    let sink = made.clone();
    let commit: Submit<NewPreset, CreatePreset> =
        Rc::new(move |draft, _, _: &mut Window, _: &mut gpui::App| sink.borrow_mut().push(draft));
    let games = vec![
        SourceGame {
            id: "a".into(),
            name: "A".into(),
            version: "1.21.1".into(),
        },
        SourceGame {
            id: "b".into(),
            name: "B".into(),
            version: "1.20.1".into(),
        },
    ];
    let (form, cx) = cx.add_window_view(move |window, cx| {
        CreatePreset::new(games, Some("b".into()), commit, window, cx)
    });
    form.read_with(cx, |form, cx| {
        assert!(!form.ready(cx), "a preset needs a name")
    });
    cx.update(|window, cx| {
        form.update(cx, |form, cx| {
            form.name_field()
                .update(cx, |input, cx| input.set_value("Daily", window, cx));
            submit(form, window, cx);
        })
    });
    let made = made.borrow();
    assert_eq!(made.len(), 1);
    assert_eq!(
        made[0].source, "b",
        "the current game is the default source"
    );
    assert_eq!(made[0].name, "Daily");
    assert!(
        !made[0]
            .groups
            .contains(&lumilio_core::GameSettingGroup::Video)
    );
    assert!(
        made[0]
            .groups
            .contains(&lumilio_core::GameSettingGroup::Controls)
    );
}

#[gpui::test]
fn choosing_a_preset_waits_for_undecided_game_changes(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let chosen: Rc<RefCell<Vec<Option<i64>>>> = Rc::default();
    let sink = chosen.clone();
    let commit: Submit<Option<i64>, ChoosePreset> =
        Rc::new(move |id, _, _: &mut Window, _: &mut gpui::App| sink.borrow_mut().push(id));
    let mut other = preset();
    other.id = 2;
    other.name = "Other".into();
    let options = bound_game();
    assert!(bind_blocked(&options).is_some());
    let (form, cx) = cx.add_window_view({
        let (other, commit) = (other.clone(), commit.clone());
        move |_, _| ChoosePreset::new(vec![preset(), other], options, commit)
    });
    cx.update(|window, cx| {
        form.update(cx, |form, cx| {
            form.choose(Some(2));
            submit(form, window, cx);
        })
    });
    assert!(chosen.borrow().is_empty(), "undecided changes come first");
    let mut decided = bound_game();
    decided.view.pending.clear();
    let (form, cx) =
        cx.add_window_view(move |_, _| ChoosePreset::new(vec![preset(), other], decided, commit));
    cx.update(|window, cx| {
        form.update(cx, |form, cx| {
            form.choose(Some(2));
            submit(form, window, cx);
        })
    });
    assert_eq!(*chosen.borrow(), vec![Some(2)]);
}

#[test]
fn home_asks_only_about_games_whose_options_need_the_player() {
    let record = lumilio_core::InstanceRecord {
        id: "game".into(),
        name: "Game".into(),
        game_version: "1.21.1".into(),
        loader: lumilio_core::Loader::Vanilla,
        loader_version: None,
        favorite: false,
        created_at: 0,
        last_played: None,
        play_seconds: 0,
        installed: true,
        settings: Default::default(),
        source_project: None,
    };
    let games = vec![crate::live::library_card(&record, 0)];
    let mut view = bound_game().view;
    view.pending.clear();
    assert!(
        attention_rows(std::slice::from_ref(&view), &games).is_empty(),
        "a kept override is settled, not noise"
    );
    view.recovery_required = true;
    let rows = attention_rows(std::slice::from_ref(&view), &games);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].action.map(|(action, _)| action),
        Some(crate::instance_detail::ProblemAction::Settings(
            crate::instance_detail::SETTINGS_OPTIONS
        ))
    );
}
