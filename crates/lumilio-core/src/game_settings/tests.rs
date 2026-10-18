use super::*;
use crate::{InstanceStore, Loader, NewInstance};
use std::fs;

fn setup() -> (tempfile::TempDir, InstanceStore, String, i64) {
    let root = tempfile::tempdir().unwrap();
    let mut store = InstanceStore::open(root.path()).unwrap();
    let id = store
        .create(
            NewInstance {
                name: "Game".into(),
                game_version: "1.21.1".into(),
                loader: Loader::Vanilla,
                loader_version: None,
            },
            0,
        )
        .unwrap()
        .id
        .clone();
    fs::create_dir_all(store.layout().game(&id)).unwrap();
    fs::write(
        store.layout().game(&id).join("options.txt"),
        "version:3955\nsoundCategory_master:0.8\nfov:0.5\nrenderDistance:16\nmod:keep:all\n",
    )
    .unwrap();
    let values = read_values(&store.layout().game(&id).join("options.txt"), "1.21.1").unwrap();
    let preset = save_preset(&store, &new_preset("Daily".into(), values)).unwrap();
    save_binding(
        &store,
        &id,
        Some(&Binding {
            preset,
            ..Default::default()
        }),
    )
    .unwrap();
    (root, store, id, preset)
}
fn change(store: &InstanceStore, id: &str, volume: &str) {
    let path = store.layout().game(id).join("options.txt");
    let mut doc = document::Document::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    doc.set("soundCategory_master", volume);
    fs::write(path, doc.text()).unwrap();
}
#[test]
fn gameplay_changes_never_publish_without_confirmation() {
    let (_root, store, id, preset) = setup();
    apply(&store, &id).unwrap();
    change(&store, &id, "0.4");
    capture(&store, &id, true).unwrap();
    assert_eq!(
        storage::preset(&store, preset).unwrap().values["soundCategory_master"].to_string(),
        "0.8"
    );
    let pending = view(&store, &id).unwrap();
    assert_eq!(pending.pending.len(), 1);
    resolve(
        &store,
        &id,
        &["soundCategory_master".into()],
        SettingResolution::Instance,
        &pending,
    )
    .unwrap();
    apply(&store, &id).unwrap();
    capture(&store, &id, true).unwrap();
    let result = view(&store, &id).unwrap();
    assert!(result.pending.is_empty());
    assert_eq!(result.overrides["soundCategory_master"].to_string(), "0.4");
}
#[test]
fn pending_fields_survive_relaunch_while_other_fields_follow() {
    let (_root, store, id, preset) = setup();
    apply(&store, &id).unwrap();
    change(&store, &id, "0.4");
    capture(&store, &id, true).unwrap();
    let mut shared = storage::preset(&store, preset).unwrap();
    shared
        .values
        .insert("renderDistance".into(), GameSettingValue::Integer(24));
    save_preset(&store, &shared).unwrap();
    apply(&store, &id).unwrap();
    capture(&store, &id, true).unwrap();
    let values = read_values(&store.layout().game(&id).join("options.txt"), "1.21.1").unwrap();
    assert_eq!(values["renderDistance"], GameSettingValue::Integer(24));
    assert_eq!(values["soundCategory_master"].to_string(), "0.4");
    assert_eq!(view(&store, &id).unwrap().pending.len(), 1);
    assert!(
        fs::read_to_string(store.layout().game(&id).join("options.txt"))
            .unwrap()
            .contains("mod:keep:all")
    );
}
#[test]
fn publishing_merges_only_selected_fields_and_resumes_following() {
    let (_root, store, id, preset) = setup();
    apply(&store, &id).unwrap();
    change(&store, &id, "0.4");
    capture(&store, &id, true).unwrap();
    let pending = view(&store, &id).unwrap();
    resolve(
        &store,
        &id,
        &["soundCategory_master".into()],
        SettingResolution::Instance,
        &pending,
    )
    .unwrap();
    let overridden = view(&store, &id).unwrap();
    resolve(
        &store,
        &id,
        &["soundCategory_master".into()],
        SettingResolution::Shared,
        &overridden,
    )
    .unwrap();
    assert!(view(&store, &id).unwrap().overrides.is_empty());
    assert_eq!(
        storage::preset(&store, preset).unwrap().values["soundCategory_master"].to_string(),
        "0.4"
    );
}
#[test]
fn a_concurrent_shared_change_needs_explicit_confirmation() {
    let (_root, store, id, preset) = setup();
    apply(&store, &id).unwrap();
    change(&store, &id, "0.4");
    capture(&store, &id, true).unwrap();
    let mut shared = storage::preset(&store, preset).unwrap();
    shared.values.insert(
        "soundCategory_master".into(),
        GameSettingValue::Decimal("0.6".into()),
    );
    save_preset(&store, &shared).unwrap();
    let pending = view(&store, &id).unwrap();
    assert!(matches!(
        resolve(
            &store,
            &id,
            &["soundCategory_master".into()],
            SettingResolution::Shared,
            &pending
        ),
        Err(GameSettingsError::Conflict(_))
    ));
    assert_eq!(view(&store, &id).unwrap(), pending);
    resolve(
        &store,
        &id,
        &["soundCategory_master".into()],
        SettingResolution::ReplaceShared,
        &pending,
    )
    .unwrap();
    assert!(view(&store, &id).unwrap().pending.is_empty());
}
#[test]
fn restore_uses_latest_shared_value_without_rediscovering_the_old_file() {
    let (_root, store, id, preset) = setup();
    apply(&store, &id).unwrap();
    change(&store, &id, "0.4");
    capture(&store, &id, true).unwrap();
    let mut shared = storage::preset(&store, preset).unwrap();
    shared.values.insert(
        "soundCategory_master".into(),
        GameSettingValue::Decimal("0.6".into()),
    );
    save_preset(&store, &shared).unwrap();
    let pending = view(&store, &id).unwrap();
    resolve(
        &store,
        &id,
        &["soundCategory_master".into()],
        SettingResolution::Restore,
        &pending,
    )
    .unwrap();
    apply(&store, &id).unwrap();
    capture(&store, &id, true).unwrap();
    assert!(view(&store, &id).unwrap().pending.is_empty());
    assert_eq!(read_values(&store.layout().game(&id).join("options.txt"),"1.21.1").unwrap()["soundCategory_master"].to_string(),"0.6");
}
#[test]
fn an_interrupted_session_retains_baseline_until_game_closed_confirmation() {
    let (root, store, id, _) = setup();
    apply(&store, &id).unwrap();
    change(&store, &id, "0.4");
    drop(store);
    let store = InstanceStore::open(root.path()).unwrap();
    assert!(matches!(
        apply(&store, &id),
        Err(GameSettingsError::RecoveryRequired)
    ));
    capture(&store, &id, true).unwrap();
    assert_eq!(view(&store, &id).unwrap().pending.len(), 1);
}
#[test]
fn failed_publication_does_not_invent_game_changes() {
    let (_root, store, id, _) = setup();
    let mut b = binding(&store, &id).unwrap().unwrap();
    let text = fs::read_to_string(store.layout().game(&id).join("options.txt")).unwrap();
    let mut effective = Values::new();
    effective.insert(
        "soundCategory_master".into(),
        GameSettingValue::Decimal("0.1".into()),
    );
    b.snapshot = Some(Snapshot {
        game_version: "1.21.1".into(),
        preset_revision: 1,
        shared: effective.clone(),
        effective,
        original: Some(text),
        applied: "soundCategory_master:0.1\n".into(),
        active: true,
        published: false,
    });
    save_binding(&store, &id, Some(&b)).unwrap();
    capture(&store, &id, true).unwrap();
    assert!(view(&store, &id).unwrap().pending.is_empty());
    assert!(binding(&store, &id).unwrap().unwrap().snapshot.is_none());
}
#[test]
fn semantic_comparison_ignores_formatting_and_unknown_fields() {
    let (_root, store, id, _) = setup();
    apply(&store, &id).unwrap();
    fs::write(store.layout().game(&id).join("options.txt"),"mod:new:value\r\nrenderDistance:16\r\nfov:0.5000\r\nsoundCategory_master:0.80000001192\r\nversion:4000\r\n").unwrap();
    capture(&store, &id, true).unwrap();
    assert!(view(&store, &id).unwrap().pending.is_empty());
}
#[test]
fn document_preserves_bom_comments_endings_and_values_with_colons() {
    let text = "\u{feff}fov:0.5\r\n# note\r\nexternal:abc:def\r\nfov:0.7";
    let mut doc = document::Document::parse(text).unwrap();
    doc.set("fov", "0.2");
    assert_eq!(
        doc.text(),
        "\u{feff}fov:0.2\r\n# note\r\nexternal:abc:def\r\nfov:0.2"
    );
    assert_eq!(doc.values("1.21.1")["fov"].to_string(), "78");
    assert!(document::Document::parse("fov:0\0").is_err());
}
#[test]
fn unsupported_versions_and_unsafe_fields_are_not_shared() {
    let text = "version:1\nsoundDevice:test\nresourcePacks:[]\ntutorialStep:none\nfullscreen:true\nkey_mod.test:key.keyboard.a\nfov:0.5\nsimulationDistance:8\n";
    let doc = document::Document::parse(text).unwrap();
    assert_eq!(doc.values("1.16.5").len(), 1);
    assert_eq!(doc.values("1.21.1").len(), 2);
    assert!(doc.values("26.4").is_empty());
    assert!(doc.values("26.3-snapshot").is_empty());
}
#[test]
fn file_publication_refuses_changed_source() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("options.txt");
    fs::write(&path, "fov:0.5").unwrap();
    assert!(document::publish(&path, Some("fov:0.4"), "fov:0.2").is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "fov:0.5");
}
#[test]
fn preset_deletion_requires_unbinding_and_game_removal_cleans_the_reference() {
    let (_root, mut store, id, preset) = setup();
    assert!(delete(&store, preset).is_err());
    store.remove(&id).unwrap();
    delete(&store, preset).unwrap();
    assert!(list(&store).unwrap().is_empty());
}

#[test]
fn excluded_settings_keep_their_values_and_can_resume_sharing() {
    let (_root, store, id, preset) = setup();
    let mut shared = storage::preset(&store, preset).unwrap();
    shared.excluded.insert("soundCategory_master".into());
    save_preset(&store, &shared).unwrap();
    change(&store, &id, "0.3");
    apply(&store, &id).unwrap();
    capture(&store, &id, true).unwrap();
    assert!(view(&store, &id).unwrap().pending.is_empty());
    assert_eq!(read_values(&store.layout().game(&id).join("options.txt"),"1.21.1").unwrap()["soundCategory_master"].to_string(),"0.3");
    let mut shared = storage::preset(&store, preset).unwrap();
    assert_eq!(shared.values["soundCategory_master"].to_string(), "0.8");
    shared.excluded.clear();
    save_preset(&store, &shared).unwrap();
    apply(&store, &id).unwrap();
    capture(&store, &id, true).unwrap();
    assert_eq!(read_values(&store.layout().game(&id).join("options.txt"),"1.21.1").unwrap()["soundCategory_master"].to_string(),"0.8");
}

#[test]
fn initial_preset_payload_migrates_without_losing_binding_or_values() {
    let (root, store, id, preset) = setup();
    store.settings_database().execute_batch("UPDATE game_settings_presets SET settings=json_extract(settings,'$.values'); PRAGMA user_version=5;").unwrap();
    drop(store);
    let store = InstanceStore::open(root.path()).unwrap();
    let shared = storage::preset(&store, preset).unwrap();
    assert!(shared.excluded.is_empty());
    assert_eq!(shared.values["soundCategory_master"].to_string(), "0.8");
    assert_eq!(view(&store, &id).unwrap().preset.unwrap().id, preset);
}

#[test]
fn copied_presets_are_independent_and_stale_edits_cannot_replace_newer_values() {
    let (root, store, id, preset) = setup();
    let original = storage::preset(&store, preset).unwrap();
    let mut copy = original.clone();
    copy.id = 0;
    copy.name = "Other".into();
    copy.excluded.insert("fov".into());
    let copied = save_preset(&store, &copy).unwrap();
    assert_ne!(copied, preset);
    let mut renamed = original.clone();
    renamed.name = "Renamed".into();
    save_preset(&store, &renamed).unwrap();
    assert!(matches!(
        save_preset(&store, &original),
        Err(GameSettingsError::Stale)
    ));
    assert_eq!(storage::preset(&store, copied).unwrap().name, "Other");
    assert!(storage::preset(&store, preset).unwrap().excluded.is_empty());
    drop(store);
    let store = InstanceStore::open(root.path()).unwrap();
    assert_eq!(view(&store, &id).unwrap().preset.unwrap().name, "Renamed");
    assert!(
        storage::preset(&store, copied)
            .unwrap()
            .excluded
            .contains("fov")
    );
}

#[test]
fn catalog_boundaries_normalize_fov_and_never_guess_legacy_bindings() {
    assert_eq!(
        catalog::decode("fov", "90", "1.21.1"),
        catalog::decode("fov", "0.5", "26.3")
    );
    assert_eq!(
        catalog::encode("fov", &GameSettingValue::Integer(90), "1.16.5"),
        Some("0.5".into())
    );
    assert!(catalog::decode("highContrast", "true", "1.19.3").is_none());
    assert_eq!(
        catalog::decode("highContrast", "true", "1.19.4"),
        Some(GameSettingValue::Boolean(true))
    );
    assert!(catalog::decode("soundCategory_ui", "0.8", "1.21.5").is_none());
    assert!(catalog::decode("soundCategory_ui", "0.8", "1.21.6").is_some());
    assert!(catalog::decode("key_key.forward", "17", "1.21.1").is_none());
    assert!(catalog::decode("fov", "NaN", "1.21.1").is_none());
    assert!(catalog::decode("soundCategory_master", "inf", "1.21.1").is_none());
}

#[test]
fn launcher_edits_write_only_the_option_into_an_independent_game() {
    let (_root, store, id, _) = setup();
    save_binding(&store, &id, None).unwrap();
    let path = store.layout().game(&id).join("options.txt");
    let expected = view(&store, &id).unwrap();
    let set = |value| OptionChange::Set(GameSettingValue::Integer(value));
    edit_option(&store, &id, "renderDistance", &set(8), &expected).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "version:3955\nsoundCategory_master:0.8\nfov:0.5\nrenderDistance:8\nmod:keep:all\n"
    );
    assert!(
        edit_option(&store, &id, "renderDistance", &set(64), &expected).is_err(),
        "out of the game's range"
    );
    assert!(
        edit_option(&store, &id, "simulationDistance", &set(8), &expected).is_err(),
        "the file does not hold it, so this game is not known to understand it"
    );
    assert!(edit_option(&store, &id, "fov", &OptionChange::Follow, &expected).is_err());
    let read = options(&store, &id).unwrap();
    assert_eq!(read.file, OptionsFile::Ready);
    assert_eq!(read.values["renderDistance"], GameSettingValue::Integer(8));
}
#[test]
fn a_shared_option_edited_for_one_game_overrides_until_it_follows_again() {
    let (_root, store, id, _) = setup();
    let path = store.layout().game(&id).join("options.txt");
    apply(&store, &id).unwrap();
    change(&store, &id, "0.4");
    capture(&store, &id, true).unwrap();
    let before = fs::read_to_string(&path).unwrap();
    let pending = view(&store, &id).unwrap();
    assert_eq!(pending.pending.len(), 1);
    let half = GameSettingValue::Decimal("0.5".into());
    edit_option(
        &store,
        &id,
        "soundCategory_master",
        &OptionChange::Set(half.clone()),
        &pending,
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        before,
        "a shared option changes at the next launch"
    );
    let overridden = view(&store, &id).unwrap();
    assert!(overridden.pending.is_empty(), "the edit decided the change");
    assert_eq!(overridden.overrides["soundCategory_master"], half);
    apply(&store, &id).unwrap();
    capture(&store, &id, true).unwrap();
    assert!(view(&store, &id).unwrap().pending.is_empty());
    assert_eq!(
        read_values(&path, "1.21.1").unwrap()["soundCategory_master"],
        half
    );
    let current = view(&store, &id).unwrap();
    edit_option(
        &store,
        &id,
        "soundCategory_master",
        &OptionChange::Follow,
        &current,
    )
    .unwrap();
    assert!(
        matches!(
            edit_option(&store, &id, "fov", &OptionChange::Follow, &current),
            Err(GameSettingsError::Stale)
        ),
        "a decision made on an older state is not applied"
    );
    apply(&store, &id).unwrap();
    capture(&store, &id, true).unwrap();
    let followed = view(&store, &id).unwrap();
    assert!(followed.pending.is_empty() && followed.overrides.is_empty());
    assert_eq!(
        read_values(&path, "1.21.1").unwrap()["soundCategory_master"].to_string(),
        "0.8"
    );
}
#[test]
fn option_kinds_follow_the_values_the_catalog_accepts() {
    assert_eq!(
        game_setting_kind("mouseSensitivity"),
        Some(GameSettingKind::Percent { max: 200 })
    );
    assert_eq!(
        game_setting_kind("chatVisibility"),
        Some(GameSettingKind::Choice { count: 3 })
    );
    assert_eq!(
        game_setting_kind("key_key.hotbar.9"),
        Some(GameSettingKind::Key)
    );
    assert_eq!(game_setting_kind("key_mod.test"), None);
    assert!(catalog::valid("fov", &GameSettingValue::Integer(110)));
    assert!(!catalog::valid("fov", &GameSettingValue::Integer(111)));
    assert!(catalog::valid(
        "chatVisibility",
        &GameSettingValue::Integer(2)
    ));
    assert!(!catalog::valid(
        "chatVisibility",
        &GameSettingValue::Integer(3)
    ));
    assert_eq!(
        GameSettingValue::fraction(1.0),
        Some(GameSettingValue::Decimal("1".into()))
    );
    assert!(catalog::valid(
        "soundCategory_ui",
        &GameSettingValue::fraction(0.25).unwrap()
    ));
    assert!(!catalog::valid(
        "soundCategory_master",
        &GameSettingValue::Decimal("0.50".into())
    ));
    assert!(GameSettingValue::fraction(1.5).is_none());
}
#[test]
fn missing_options_wait_for_game_initialization_without_a_false_baseline() {
    let (_root, store, id, _) = setup();
    let path = store.layout().game(&id).join("options.txt");
    fs::remove_file(&path).unwrap();
    apply(&store, &id).unwrap();
    assert!(!path.exists());
    assert!(binding(&store, &id).unwrap().unwrap().snapshot.is_none());
    fs::write(&path, "version:3955\nsoundCategory_master:0.3\n").unwrap();
    apply(&store, &id).unwrap();
    capture(&store, &id, true).unwrap();
    assert!(
        fs::read_to_string(path)
            .unwrap()
            .contains("soundCategory_master:0.8")
    );
    assert!(view(&store, &id).unwrap().pending.is_empty());
}
