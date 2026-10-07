use super::*;

fn store() -> (tempfile::TempDir, SettingsStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::open(dir.path()).unwrap();
    (dir, store)
}

#[test]
fn failed_settings_writes_preserve_saved_values_and_allow_retry() {
    for publication_failure in [false, true] {
        for operation in 0..6 {
            let (dir, mut store) = store();
            store.add_offline_account("Steve", None).unwrap();
            store.add_offline_account("Alex", None).unwrap();
            store.set_memory(Some(512), Some(2048)).unwrap();
            let before = store.get().clone();
            let saved = std::fs::read(&store.path).unwrap();
            let mutate = |store: &mut SettingsStore| -> Result<(), SettingsError> {
                match operation {
                    0 => store.set_memory(Some(1024), Some(4096)),
                    1 => store.set_java_roots(vec!["/extra/jdk".into()]),
                    2 => store.set_mirrors(
                        vec![MirrorRule {
                            official_prefix: "https://official.example/".into(),
                            mirror_prefix: "https://mirror.example/".into(),
                        }],
                        true,
                    ),
                    3 => store.add_offline_account("New", None),
                    4 => store.select_account("Alex"),
                    _ => store.remove_account("Steve"),
                }
            };
            let blocked = if publication_failure {
                std::fs::rename(&store.path, dir.path().join("saved.json")).unwrap();
                store.path.clone()
            } else {
                dir.path().join("settings.json.tmp")
            };
            std::fs::create_dir(&blocked).unwrap();
            assert!(mutate(&mut store).is_err());
            assert_eq!(store.get(), &before, "operation {operation}");
            assert_eq!(store.selected_profile().unwrap().name(), "Steve");
            std::fs::remove_dir(&blocked).unwrap();
            if publication_failure {
                std::fs::rename(dir.path().join("saved.json"), &store.path).unwrap();
            }
            assert_eq!(std::fs::read(&store.path).unwrap(), saved);
            assert_eq!(SettingsStore::open(dir.path()).unwrap().get(), &before);
            mutate(&mut store).unwrap();
            assert_eq!(SettingsStore::open(dir.path()).unwrap().get(), store.get());
        }
    }
}

#[test]
fn settings_files_from_before_custom_ids_and_current_instance_still_load() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(FILE),
        r#"{"schema":1,"accounts":[{"name":"Steve"}],"selected_account":"Steve"}"#,
    )
    .unwrap();
    let store = SettingsStore::open(dir.path()).unwrap();
    assert!(!store.recovered_from_damage());
    assert_eq!(store.get().current_instance, None);
    let steve = store.selected_profile().unwrap();
    assert_eq!(steve.id(), ProfileId::offline("Steve"));
}

#[test]
fn a_custom_id_wins_over_the_name_and_clashes_are_refused() {
    let (dir, mut store) = store();
    store
        .add_offline_account("Steve", Some("123e4567-e89b-12d3-a456-426614174000"))
        .unwrap();
    assert_eq!(
        store.selected_profile().unwrap().id().compact(),
        "123e4567e89b12d3a456426614174000"
    );
    assert!(matches!(
        store.add_offline_account("Alex", Some("123e4567e89b12d3a456426614174000")),
        Err(SettingsError::DuplicateUuid(_))
    ));
    for bad in ["xyz", "123e4567-e89b-12d3-a456", "0".repeat(32).as_str()] {
        assert!(matches!(
            store.add_offline_account("Alex", Some(bad)),
            Err(SettingsError::Profile(ProfileError::InvalidId))
        ));
    }
    // Blank means "derive from the name".
    store.add_offline_account("Alex", Some("  ")).unwrap();
    let reopened = SettingsStore::open(dir.path()).unwrap();
    let alex = &reopened.get().accounts[1];
    assert_eq!(alex.uuid, None);
    assert_eq!(alex.profile().unwrap().id(), ProfileId::offline("Alex"));
}

#[test]
fn the_current_instance_persists() {
    let (dir, mut store) = store();
    store.set_current_instance(Some("abc".into())).unwrap();
    assert_eq!(
        SettingsStore::open(dir.path())
            .unwrap()
            .get()
            .current_instance
            .as_deref(),
        Some("abc")
    );
    store.set_current_instance(None).unwrap();
    assert_eq!(
        SettingsStore::open(dir.path())
            .unwrap()
            .get()
            .current_instance,
        None
    );
}

#[test]
fn preferences_launch_defaults_and_concurrency_persist_and_validate() {
    use crate::tuning::{AfterLaunch, Appearance, EnvVar, Language};
    let (dir, mut store) = store();
    let preferences = Preferences {
        appearance: Appearance::Dark,
        language: Language::English,
        after_launch: AfterLaunch::Hide,
        foreground_on_exit: Some(false),
        ..Preferences::default()
    };
    store.set_preferences(preferences.clone()).unwrap();
    store
        .set_launch_defaults(LaunchTuning {
            window_width: Some(1280),
            window_height: Some(720),
            fullscreen: Some(false),
            game_arguments: vec![" --demo ".into(), "".into()],
            environment: vec![EnvVar {
                name: "A".into(),
                value: "1".into(),
            }],
            wrapper: Some("  ".into()),
            ..LaunchTuning::default()
        })
        .unwrap();
    store.set_download_concurrency(Some(8)).unwrap();
    store
        .set_disabled_java(vec!["/opt/old-jdk".into()])
        .unwrap();
    let reopened = SettingsStore::open(dir.path()).unwrap();
    let saved = reopened.get();
    assert_eq!(saved.preferences, preferences);
    assert_eq!(
        saved.launch.game_arguments,
        ["--demo"],
        "blank lines are dropped"
    );
    assert_eq!(saved.launch.wrapper, None);
    assert_eq!(saved.download_concurrency, Some(8));
    assert_eq!(saved.disabled_java, [PathBuf::from("/opt/old-jdk")]);

    // Rejections change nothing.
    let before = store.get().clone();
    assert!(matches!(
        store.set_launch_defaults(LaunchTuning {
            window_width: Some(800),
            ..LaunchTuning::default()
        }),
        Err(SettingsError::Tuning(_))
    ));
    for bad in [0, 33] {
        assert!(matches!(
            store.set_download_concurrency(Some(bad)),
            Err(SettingsError::InvalidConcurrency)
        ));
    }
    assert_eq!(store.get(), &before);
    store.set_download_concurrency(None).unwrap();
    assert_eq!(store.get().download_concurrency, None);
}

#[test]
fn a_microsoft_account_is_found_by_its_profile_id_and_survives_a_rename() {
    let (dir, mut store) = store();
    let id = ProfileId::parse("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let key = store.sign_in_microsoft(id, "Edwin_Zhan").unwrap();
    assert_eq!(key, "msa:123e4567e89b12d3a456426614174000");
    assert_eq!(store.get().selected_account.as_deref(), Some(key.as_str()));
    assert_eq!(store.selected_entry().unwrap().kind, AccountKind::Microsoft);
    assert!(store.selected_profile().is_none(), "not an offline account");

    // Signing in again after a rename updates the one account.
    store.set_needs_sign_in(&key, true).unwrap();
    assert!(store.get().accounts[0].needs_sign_in);
    store.sign_in_microsoft(id, "New_Name").unwrap();
    let accounts = &store.get().accounts;
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].name, "New_Name");
    assert!(!accounts[0].needs_sign_in, "a new sign-in clears the flag");

    // An offline account may share the name; the keys differ.
    store.add_offline_account("New_Name", None).unwrap();
    assert_eq!(store.get().accounts.len(), 2);
    store.select_account("New_Name").unwrap();
    assert_eq!(store.selected_profile().unwrap().name(), "New_Name");
    store.select_account(&key).unwrap();

    // Public facts only reach the file, and it reads back the same.
    let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
    assert!(
        text.contains("microsoft") && !text.to_lowercase().contains("token"),
        "{text}"
    );
    assert_eq!(SettingsStore::open(dir.path()).unwrap().get(), store.get());

    // Removing it by key reselects what remains.
    store.remove_account(&key).unwrap();
    assert_eq!(store.get().selected_account.as_deref(), Some("New_Name"));
    assert!(matches!(
        store.set_needs_sign_in(&key, true),
        Err(SettingsError::UnknownAccount(_))
    ));
}

#[test]
fn a_microsoft_id_cannot_collide_with_an_offline_account() {
    let (_dir, mut store) = store();
    store
        .add_offline_account("Steve", Some("123e4567e89b12d3a456426614174000"))
        .unwrap();
    let same = ProfileId::parse("123e4567e89b12d3a456426614174000").unwrap();
    assert!(matches!(
        store.sign_in_microsoft(same, "Other"),
        Err(SettingsError::DuplicateUuid(_))
    ));
    assert_eq!(store.get().accounts.len(), 1);
}

#[test]
fn defaults_are_empty_and_settings_persist() {
    let (dir, mut store) = store();
    assert_eq!(store.get().default_max_memory_mb, None);
    store.set_memory(Some(512), Some(4096)).unwrap();
    store.set_java_roots(vec!["/opt/jdks".into()]).unwrap();
    let reopened = SettingsStore::open(dir.path()).unwrap();
    assert_eq!(reopened.get().default_max_memory_mb, Some(4096));
    assert_eq!(
        reopened.get().extra_java_roots,
        [PathBuf::from("/opt/jdks")]
    );
}

#[test]
fn memory_is_validated_and_a_bad_value_changes_nothing() {
    let (_dir, mut store) = store();
    store.set_memory(None, Some(2048)).unwrap();
    for (min, max) in [
        (Some(0), None),
        (None, Some(MAX_MEMORY_MB + 1)),
        (Some(4096), Some(1024)),
    ] {
        assert!(matches!(
            store.set_memory(min, max),
            Err(SettingsError::InvalidMemory)
        ));
    }
    assert_eq!(store.get().default_max_memory_mb, Some(2048));
}

#[test]
fn accounts_first_becomes_selected_and_removal_reselects() {
    let (_dir, mut store) = store();
    assert!(store.selected_profile().is_none());
    store.add_offline_account("Steve", None).unwrap();
    store.add_offline_account("Alex", None).unwrap();
    assert_eq!(store.selected_profile().unwrap().name(), "Steve");
    assert!(matches!(
        store.add_offline_account("steve", None),
        Err(SettingsError::DuplicateAccount(_))
    ));
    assert!(matches!(
        store.add_offline_account("bad name", None),
        Err(SettingsError::Profile(_))
    ));
    store.select_account("Alex").unwrap();
    store.remove_account("Alex").unwrap();
    assert_eq!(store.selected_profile().unwrap().name(), "Steve");
    store.remove_account("Steve").unwrap();
    assert!(store.selected_profile().is_none());
    assert!(matches!(
        store.select_account("ghost"),
        Err(SettingsError::UnknownAccount(_))
    ));
}

#[test]
fn mirrors_order_follows_the_preference() {
    let (_dir, mut store) = store();
    let rule = MirrorRule {
        official_prefix: "https://piston-data.mojang.com".into(),
        mirror_prefix: "https://mirror.example/data".into(),
    };
    let url = "https://piston-data.mojang.com/v1/x.jar";
    store.set_mirrors(vec![rule.clone()], false).unwrap();
    assert_eq!(
        store.source_chain().unwrap().candidates(url),
        [url, "https://mirror.example/data/v1/x.jar"]
    );
    store.set_mirrors(vec![rule], true).unwrap();
    assert_eq!(
        store.source_chain().unwrap().candidates(url),
        ["https://mirror.example/data/v1/x.jar", url]
    );
    // With no mirrors, only the official address remains.
    store.set_mirrors(vec![], true).unwrap();
    assert_eq!(store.source_chain().unwrap().candidates(url), [url]);
}

#[test]
fn empty_mirror_prefixes_are_rejected() {
    let (_dir, mut store) = store();
    let bad = MirrorRule {
        official_prefix: " ".into(),
        mirror_prefix: "x".into(),
    };
    assert!(matches!(
        store.set_mirrors(vec![bad], false),
        Err(SettingsError::InvalidMirror)
    ));
    assert!(store.get().mirrors.is_empty());
}

#[test]
fn damaged_and_newer_files_follow_the_shared_policy() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), "garbage").unwrap();
    let store = SettingsStore::open(dir.path()).unwrap();
    assert!(store.recovered_from_damage());
    std::fs::write(dir.path().join("settings.json"), r#"{"schema": 42}"#).unwrap();
    assert!(matches!(
        SettingsStore::open(dir.path()),
        Err(SettingsError::Persist(PersistError::NewerSchema(42)))
    ));
}
