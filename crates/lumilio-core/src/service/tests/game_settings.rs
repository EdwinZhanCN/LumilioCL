use super::{fake_java, file_url, publish_release, world};
use crate::{
    CancellationToken, GameSettingGroup, GameSettingValue, Loader, OptionChange, SettingResolution,
};
use std::fs;
use tokio::sync::mpsc;

#[tokio::test]
async fn game_settings_launch_boundary_applies_after_hook_and_reviews_after_exit() {
    let world = world();
    publish_release(&world);
    let manifest = fs::read_to_string(world.server.join("1.0.json"))
        .unwrap()
        .replace("\"id\":\"1.0\"", "\"id\":\"1.21.1\"");
    fs::write(world.server.join("1.21.1.json"), manifest).unwrap();
    world.net.forget_all();
    world.net.answer("piston-meta.mojang.com/mc/game/version_manifest",format!(r#"{{"latest":{{"release":"1.21.1","snapshot":"1.21.1"}},"versions":[{{"id":"1.21.1","type":"release","url":"{}"}}]}}"#,file_url(&world.server.join("1.21.1.json"))));
    // The actual Java substitute proves the applied value reached the game's working
    // directory, then saves a gameplay change before it exits.
    fake_java(
        &world,
        "test \"$(cat options.txt)\" = 'soundCategory_master:0.8' || exit 7\nprintf 'soundCategory_master:0.4\\n' > options.txt",
    );
    let source = world
        .service
        .create_instance("Source", Some("1.21.1"), Loader::Vanilla, None)
        .await
        .unwrap();
    fs::create_dir_all(world.service.layout().game(&source.id)).unwrap();
    fs::write(
        world.service.layout().game(&source.id).join("options.txt"),
        "soundCategory_master:0.8\n",
    )
    .unwrap();
    let preset = world
        .service
        .create_settings_preset(&source.id, "Daily".into(), &[GameSettingGroup::Sound])
        .await
        .unwrap();
    let target = world
        .service
        .create_instance("Target", Some("1.21.1"), Loader::Vanilla, None)
        .await
        .unwrap();
    fs::create_dir_all(world.service.layout().game(&target.id)).unwrap();
    fs::write(
        world.service.layout().game(&target.id).join("options.txt"),
        "soundCategory_master:0.2\n",
    )
    .unwrap();
    let mut settings = target.settings.clone();
    settings.launch.pre_launch = Some("printf 'soundCategory_master:0.1\\n' > options.txt".into());
    world
        .service
        .update_instance_settings(&target.id, settings)
        .await
        .unwrap();
    world
        .service
        .bind_settings_preset(&target.id, Some(preset))
        .await
        .unwrap();
    let (updates, _rx) = mpsc::unbounded_channel();
    let exit = world
        .service
        .launch(&target.id, updates, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        exit.code,
        Some(0),
        "preset applies after the before-launch command"
    );
    let review = world.service.game_settings(&target.id).await.unwrap();
    assert_eq!(review.pending.len(), 1);
    assert_eq!(
        review.pending[0].value,
        GameSettingValue::Decimal("0.4".into())
    );
    assert_eq!(
        world.service.settings_presets().await.unwrap()[0].values["soundCategory_master"],
        GameSettingValue::Decimal("0.8".into())
    );
    world
        .service
        .resolve_game_settings(
            review,
            vec!["soundCategory_master".into()],
            SettingResolution::Instance,
        )
        .await
        .unwrap();
    assert!(
        world
            .service
            .game_settings(&target.id)
            .await
            .unwrap()
            .pending
            .is_empty()
    );
}

#[tokio::test]
async fn game_settings_scope_is_reversible_and_does_not_turn_excluded_edits_into_pending_changes() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Scope", Some("1.21.1"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    fs::create_dir_all(&game).unwrap();
    fs::write(
        game.join("options.txt"),
        "soundCategory_master:0.8\nsoundCategory_music:0.5\nfov:0.5\n",
    )
    .unwrap();
    let preset_id = world
        .service
        .create_settings_preset(&record.id, "Daily".into(), &[GameSettingGroup::Sound])
        .await
        .unwrap();
    let mut preset = world.service.settings_presets().await.unwrap().remove(0);
    assert!(preset.excluded.contains("fov"));
    world
        .service
        .bind_settings_preset(&record.id, Some(preset_id))
        .await
        .unwrap();
    world.service.apply_game_settings(&record.id).await.unwrap();
    let original = preset.clone();
    preset.excluded.insert("soundCategory_master".into());
    assert!(
        world
            .service
            .edit_settings_preset(preset.clone())
            .await
            .is_err(),
        "scope cannot change while a session is active"
    );
    assert_eq!(world.service.settings_presets().await.unwrap()[0], original);
    world
        .service
        .capture_game_settings(&record.id)
        .await
        .unwrap();
    world.service.edit_settings_preset(preset).await.unwrap();
    fs::write(
        game.join("options.txt"),
        "soundCategory_master:0.3\nsoundCategory_music:0.5\nfov:0.5\n",
    )
    .unwrap();
    world.service.apply_game_settings(&record.id).await.unwrap();
    world
        .service
        .capture_game_settings(&record.id)
        .await
        .unwrap();
    assert!(
        world
            .service
            .game_settings(&record.id)
            .await
            .unwrap()
            .pending
            .is_empty()
    );
    let mut preset = world.service.settings_presets().await.unwrap().remove(0);
    preset.excluded.remove("soundCategory_master");
    world.service.edit_settings_preset(preset).await.unwrap();
    world.service.apply_game_settings(&record.id).await.unwrap();
    world
        .service
        .capture_game_settings(&record.id)
        .await
        .unwrap();
    assert!(
        fs::read_to_string(game.join("options.txt"))
            .unwrap()
            .contains("soundCategory_master:0.8")
    );
}

#[tokio::test]
async fn launcher_edits_check_values_and_wait_for_the_game_to_close() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Edit", Some("1.21.1"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    fs::create_dir_all(&game).unwrap();
    fs::write(game.join("options.txt"), "renderDistance:12\nfov:0.5\n").unwrap();
    world
        .service
        .create_settings_preset(&record.id, "Daily".into(), &[GameSettingGroup::Video])
        .await
        .unwrap();
    let preset = world.service.settings_presets().await.unwrap().remove(0);
    let mut edited = preset.clone();
    edited
        .values
        .insert("renderDistance".into(), GameSettingValue::Integer(40));
    assert!(
        world
            .service
            .edit_settings_preset(edited.clone())
            .await
            .is_err(),
        "out of the game's range"
    );
    edited
        .values
        .insert("renderDistance".into(), GameSettingValue::Integer(20));
    edited
        .values
        .insert("gamma".into(), GameSettingValue::Decimal("1".into()));
    assert!(
        world
            .service
            .edit_settings_preset(edited.clone())
            .await
            .is_err(),
        "a preset only holds the options it was made with"
    );
    edited.values.remove("gamma");
    world.service.edit_settings_preset(edited).await.unwrap();
    let saved = world.service.settings_presets().await.unwrap().remove(0);
    assert_eq!(
        saved.values["renderDistance"],
        GameSettingValue::Integer(20)
    );
    assert_eq!(saved.revision, preset.revision + 1);

    let options = world.service.game_options(&record.id).await.unwrap();
    assert_eq!(options.values["fov"], GameSettingValue::Integer(90));
    let busy = world.service.reserve_instance(&record.id).unwrap();
    assert!(
        world
            .service
            .game_options(&record.id)
            .await
            .unwrap()
            .view
            .is_busy
    );
    assert!(
        world
            .service
            .edit_game_option(
                options.view.clone(),
                "fov".into(),
                OptionChange::Set(GameSettingValue::Integer(100)),
            )
            .await
            .is_err(),
        "a running game owns its options file"
    );
    drop(busy);
    world
        .service
        .edit_game_option(
            options.view,
            "fov".into(),
            OptionChange::Set(GameSettingValue::Integer(100)),
        )
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(game.join("options.txt")).unwrap(),
        "renderDistance:12\nfov:0.75\n"
    );
}

#[tokio::test]
async fn one_option_changes_on_the_latest_preset_unless_it_changed_itself() {
    let world = world();
    publish_release(&world);
    let record = world
        .service
        .create_instance("Quick", Some("1.21.1"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout().game(&record.id);
    fs::create_dir_all(&game).unwrap();
    fs::write(
        game.join("options.txt"),
        "autoJump:false\nrawMouseInput:true\n",
    )
    .unwrap();
    let id = world
        .service
        .create_settings_preset(&record.id, "Daily".into(), &[GameSettingGroup::Controls])
        .await
        .unwrap();
    let off = GameSettingValue::Boolean(false);
    let on = GameSettingValue::Boolean(true);
    // Two switches in a row: the second was read before the first was saved.
    world
        .service
        .set_preset_value(id, "autoJump".into(), off.clone(), on.clone())
        .await
        .unwrap();
    world
        .service
        .set_preset_value(id, "rawMouseInput".into(), on.clone(), off.clone())
        .await
        .unwrap();
    let saved = world.service.settings_presets().await.unwrap().remove(0);
    assert_eq!(saved.values["autoJump"], on);
    assert_eq!(saved.values["rawMouseInput"], off);
    assert!(
        matches!(
            world
                .service
                .set_preset_value(id, "autoJump".into(), off.clone(), off)
                .await,
            Err(crate::ServiceError::GameSettings(
                crate::GameSettingsError::Stale
            ))
        ),
        "an option changed since it was shown is not overwritten"
    );
}
