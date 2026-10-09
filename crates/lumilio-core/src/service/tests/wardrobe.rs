use super::{MS_ID, microsoft_chain, sign_in, world};
use crate::microsoft::PROFILE_URL;
use crate::{AppearanceChange, AppearanceError, CancellationToken, ServiceError, SkinModel};
use std::io::Write as _;

#[tokio::test]
async fn default_preview_reads_only_registered_installed_clients_and_preserves_a_local_cape() {
    let world = world();
    assert!(
        world
            .service
            .account_look("Steve")
            .await
            .unwrap()
            .skin
            .is_none()
    );
    let record = super::instance_with_profile(&world, "Installed").await;
    let versions = world.service.layout().versions().join("1.0");
    std::fs::create_dir_all(&versions).unwrap();
    let jar = versions.join("1.0.jar");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
    writer
        .start_file(
            "assets/minecraft/textures/entity/steve.png",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(64, 32, image::Rgba([3, 4, 5, 255]))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    writer.write_all(bytes.get_ref()).unwrap();
    writer.finish().unwrap();
    assert!(
        world
            .service
            .account_look("Steve")
            .await
            .unwrap()
            .skin
            .is_none(),
        "uninstalled records are ignored"
    );
    world
        .service
        .store
        .lock()
        .await
        .mark_installed(&record.id, true)
        .unwrap();
    let cape = world.server.join("cape.png");
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(64, 32, image::Rgba([6, 7, 8, 255]))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    std::fs::write(&cape, bytes.get_ref()).unwrap();
    world
        .service
        .set_account_skin(
            "Steve",
            Some(crate::SkinChoice::Local {
                model: SkinModel::Wide,
                skin: None,
                cape: Some(cape),
            }),
        )
        .await
        .unwrap();
    let look = world.service.account_look("Steve").await.unwrap();
    assert_eq!(look.skin.unwrap().rgba[..4], [3, 4, 5, 255]);
    assert_eq!(look.cape.unwrap().rgba[..4], [6, 7, 8, 255]);
    assert!(
        world.net.sent.lock().unwrap().is_empty(),
        "preview never downloads default artwork"
    );
}

#[tokio::test]
async fn appearance_uses_the_target_accounts_token_and_marks_expired_credentials() {
    let world = world();
    microsoft_chain(&world, "Online", "secret-minecraft-token");
    let (key, _) = sign_in(&world).await.unwrap();
    // The offline account stays selected: viewing does not select Microsoft.
    assert_eq!(
        world.service.settings().await.selected_account.as_deref(),
        Some("Steve")
    );
    let profile = world.service.account_profile(&key).await.unwrap();
    assert_eq!(profile.id, MS_ID);
    {
        let sent = world.net.sent.lock().unwrap();
        assert!(sent.last().unwrap().headers.iter().any(
            |(name, value)| name == "authorization" && value == "Bearer secret-minecraft-token"
        ));
    }
    world.net.clear_replies(PROFILE_URL);
    world.net.reply(PROFILE_URL, 401, "");
    assert!(matches!(
        world.service.refresh_account_profile(&key).await,
        Err(ServiceError::Appearance(AppearanceError::SignInRequired))
    ));
    let requests = world.net.sent_to(PROFILE_URL);
    assert!(matches!(
        world.service.refresh_account_profile(&key).await,
        Err(ServiceError::Appearance(AppearanceError::SignInRequired))
    ));
    assert_eq!(world.net.sent_to(PROFILE_URL), requests);
    let account = world
        .service
        .settings()
        .await
        .accounts
        .into_iter()
        .find(|entry| entry.key() == key)
        .unwrap();
    assert!(account.needs_sign_in);
    let settings =
        std::fs::read_to_string(world.service.layout().root().join("settings.json")).unwrap();
    assert!(!settings.contains("secret-minecraft-token"));
}

#[tokio::test(start_paused = true)]
async fn profile_reads_share_a_fresh_snapshot_and_force_makes_one_new_get() {
    let world = world();
    microsoft_chain(&world, "Online", "token");
    let (key, _) = sign_in(&world).await.unwrap();
    let before = world.net.sent_to(PROFILE_URL);
    let (first, second) = tokio::join!(
        world.service.account_profile(&key),
        world.service.account_profile(&key),
    );
    assert_eq!(first.unwrap(), second.unwrap());
    assert_eq!(world.net.sent_to(PROFILE_URL), before + 1);
    world.service.account_profile(&key).await.unwrap();
    assert_eq!(world.net.sent_to(PROFILE_URL), before + 1);
    world.net.clear_replies(PROFILE_URL);
    world.net.reply(
        PROFILE_URL,
        200,
        &format!(r#"{{"id":"{MS_ID}","name":"Online"}}"#),
    );
    world.service.refresh_account_profile(&key).await.unwrap();
    assert_eq!(world.net.sent_to(PROFILE_URL), before + 2);
    tokio::time::advance(std::time::Duration::from_secs(61)).await;
    world.net.clear_replies(PROFILE_URL);
    world.net.reply(
        PROFILE_URL,
        200,
        &format!(r#"{{"id":"{MS_ID}","name":"Online"}}"#),
    );
    world.service.account_profile(&key).await.unwrap();
    assert_eq!(world.net.sent_to(PROFILE_URL), before + 3);
    tokio::time::advance(std::time::Duration::from_secs(61)).await;
    world.net.clear_replies(PROFILE_URL);
    world.net.reply(PROFILE_URL, 500, "");
    let stale = world.service.account_profile_snapshot(&key).await.unwrap();
    assert_eq!(stale.profile.id, MS_ID);
    assert!(matches!(stale.warning, Some(AppearanceError::Refused(500))));
    assert_eq!(world.net.sent_to(PROFILE_URL), before + 4);
    assert!(matches!(
        world.service.refresh_account_profile(&key).await,
        Err(ServiceError::Appearance(AppearanceError::Refused(500)))
    ));
    assert_eq!(world.net.sent_to(PROFILE_URL), before + 4);
}

#[tokio::test]
async fn offline_or_missing_targets_and_wrong_profile_identity_are_refused() {
    let world = world();
    let sent = world.net.sent.lock().unwrap().len();
    assert!(matches!(
        world
            .service
            .change_account_appearance("Steve", AppearanceChange::DefaultSkin)
            .await,
        Err(ServiceError::Appearance(AppearanceError::NotMicrosoft))
    ));
    assert!(world.service.account_profile("missing").await.is_err());
    assert_eq!(world.net.sent.lock().unwrap().len(), sent);
    microsoft_chain(&world, "Online", "token");
    let (key, _) = sign_in(&world).await.unwrap();
    world.net.clear_replies(PROFILE_URL);
    world.net.reply(
        PROFILE_URL,
        200,
        r#"{"id":"00000000000000000000000000000000","name":"Other"}"#,
    );
    assert!(matches!(
        world.service.account_profile(&key).await,
        Err(ServiceError::Appearance(AppearanceError::Protocol(_)))
    ));
}

#[tokio::test]
async fn delayed_confirmation_does_not_repeat_the_write_and_can_be_cancelled() {
    let world = world();
    microsoft_chain(&world, "Online", "token");
    let (key, _) = sign_in(&world).await.unwrap();
    let endpoint = format!("{PROFILE_URL}/skins/active");
    world.net.reply(&endpoint, 204, "");
    assert!(
        world
            .service
            .change_account_appearance(&key, AppearanceChange::DefaultSkin)
            .await
            .unwrap()
            .profile
            .is_none()
    );
    let read_count = world.net.sent_to(PROFILE_URL);
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        world.service.confirm_account_appearance(&key, cancel).await,
        Err(ServiceError::Cancelled)
    ));
    assert_eq!(world.net.sent_to(PROFILE_URL), read_count);
    let start = tokio::time::Instant::now();
    let confirm = world
        .service
        .confirm_account_appearance(&key, CancellationToken::new());
    tokio::pin!(confirm);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), &mut confirm)
            .await
            .is_err()
    );
    assert_eq!(
        world.net.sent_to(PROFILE_URL),
        read_count,
        "no immediate confirmation of an accepted write"
    );
    confirm.await.unwrap();
    assert!(start.elapsed() >= std::time::Duration::from_secs(11));
    assert_eq!(world.net.sent_to(&endpoint), 1);
}

#[tokio::test]
async fn service_library_copies_and_serializes_concurrent_imports() {
    let world = world();
    let mut bytes = Vec::new();
    image::RgbaImage::from_pixel(64, 32, image::Rgba([255, 0, 0, 255]))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    let path = world._dir.path().join("Red.png");
    std::fs::write(&path, bytes).unwrap();
    let (a, b) = tokio::join!(
        world
            .service
            .import_library_skin(path.clone(), Some(SkinModel::Wide)),
        world
            .service
            .import_library_skin(path.clone(), Some(SkinModel::Wide))
    );
    let a = a.unwrap();
    assert_eq!(a, b.unwrap());
    assert_eq!(world.service.skin_library().await.unwrap().len(), 1);
    std::fs::remove_file(path).unwrap();
    let (_, model, png) = world.service.library_skin(a.id).await.unwrap();
    assert_eq!(model, SkinModel::Wide);
    assert_eq!(image::load_from_memory(&png).unwrap().height(), 64);
}

#[tokio::test]
async fn wearing_a_library_skin_keeps_the_offline_cape_and_the_copied_skin_after_removal() {
    let world = world();
    let mut bytes = Vec::new();
    image::RgbaImage::from_pixel(64, 64, image::Rgba([30, 40, 50, 255]))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    let path = world._dir.path().join("skin.png");
    let cape = world._dir.path().join("cape.png");
    std::fs::write(&path, &bytes).unwrap();
    bytes.clear();
    image::RgbaImage::from_pixel(64, 32, image::Rgba([50, 40, 30, 255]))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    std::fs::write(&cape, bytes).unwrap();
    world
        .service
        .set_account_skin(
            "Steve",
            Some(crate::SkinChoice::Local {
                model: SkinModel::Wide,
                skin: None,
                cape: Some(cape.clone()),
            }),
        )
        .await
        .unwrap();
    let saved = world
        .service
        .import_library_skin(path.clone(), Some(SkinModel::Slim))
        .await
        .unwrap();
    assert!(
        world
            .service
            .wear_library_skin("Steve", saved.id.clone())
            .await
            .unwrap()
            .update
            .is_none()
    );
    let account = world
        .service
        .settings()
        .await
        .accounts
        .into_iter()
        .find(|entry| entry.key() == "Steve")
        .unwrap();
    let Some(crate::SkinChoice::Local {
        model,
        skin: Some(copied),
        cape: selected_cape,
    }) = account.skin
    else {
        panic!("local skin selected")
    };
    assert_eq!(model, SkinModel::Slim);
    assert_eq!(selected_cape, Some(cape));
    assert_ne!(copied, path);
    std::fs::remove_file(path).unwrap();
    world.service.remove_library_skin(saved.id).await.unwrap();
    let look = world.service.account_look("Steve").await.unwrap();
    assert_eq!(look.model, SkinModel::Slim);
    assert!(look.skin.is_some() && look.cape.is_some());
}

#[tokio::test]
async fn a_microsoft_wear_brings_the_paired_cape_and_keep_leaves_it() {
    let world = world();
    microsoft_chain(&world, "Online", "token");
    let (key, _) = sign_in(&world).await.unwrap();
    let mut bytes = Vec::new();
    image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 255, 255]))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    let path = world._dir.path().join("Blue.png");
    std::fs::write(&path, bytes).unwrap();
    let skin = world
        .service
        .import_library_skin(path, Some(SkinModel::Wide))
        .await
        .unwrap();
    let upload = format!("{PROFILE_URL}/skins");
    let cape = format!("{PROFILE_URL}/capes/active");
    world.net.reply(&upload, 200, "");
    world.net.reply(&upload, 200, "");
    world.net.reply(&cape, 204, "");

    // Keep: the skin changes, the cape is left alone.
    world
        .service
        .wear_library_skin(&key, skin.id.clone())
        .await
        .unwrap();
    assert_eq!(world.net.sent_to(&upload), 1);
    assert_eq!(world.net.sent_to(&cape), 0);

    // Hidden: the cape is taken off after the skin is on.
    world
        .service
        .set_library_skin_cape(skin.id.clone(), crate::skin::PairedCape::Hidden)
        .await
        .unwrap();
    world
        .service
        .wear_library_skin(&key, skin.id)
        .await
        .unwrap();
    assert_eq!(world.net.sent_to(&upload), 2);
    assert_eq!(world.net.sent_to(&cape), 1);
}

#[tokio::test]
async fn cape_failure_after_upload_reports_only_the_remaining_step() {
    let world = world();
    microsoft_chain(&world, "Online", "token");
    let (key, _) = sign_in(&world).await.unwrap();
    let path = world._dir.path().join("Partial.png");
    let mut bytes = Vec::new();
    image::RgbaImage::from_pixel(64, 64, image::Rgba([20, 30, 40, 255]))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    std::fs::write(&path, bytes).unwrap();
    let skin = world
        .service
        .import_library_skin(path, Some(SkinModel::Wide))
        .await
        .unwrap();
    world
        .service
        .set_library_skin_cape(skin.id.clone(), crate::PairedCape::Hidden)
        .await
        .unwrap();
    let upload = format!("{PROFILE_URL}/skins");
    let cape = format!("{PROFILE_URL}/capes/active");
    world.net.reply(&upload, 200, "");
    world.net.reply(&cape, 500, "");
    let applied = world
        .service
        .wear_library_skin(&key, skin.id)
        .await
        .unwrap();
    assert!(applied.update.is_some());
    assert!(matches!(
        applied.cape_failure,
        Some((
            None,
            ServiceError::Appearance(AppearanceError::Refused(500))
        ))
    ));
    assert_eq!(world.net.sent_to(&upload), 1);
    assert_eq!(world.net.sent_to(&cape), 1);
}

fn script_account(world: &super::World, id: &str, name: &str, token: &str) {
    microsoft_chain(world, name, token);
    world.net.clear_replies(PROFILE_URL);
    world.net.reply(
        PROFILE_URL,
        200,
        &format!(r#"{{"id":"{id}","name":"{name}"}}"#),
    );
}

#[tokio::test]
async fn profile_caches_do_not_cross_accounts() {
    let world = world();
    let other = "223e4567e89b12d3a456426614174000";
    script_account(&world, MS_ID, "Ada", "token-a");
    let (ada, _) = sign_in(&world).await.unwrap();
    assert_eq!(
        world.service.account_profile(&ada).await.unwrap().name,
        "Ada"
    );
    script_account(&world, other, "Bea", "token-b");
    let (bea, _) = sign_in(&world).await.unwrap();
    assert_ne!(ada, bea);
    assert_eq!(
        world.service.account_profile(&bea).await.unwrap().name,
        "Bea"
    );
    world.net.clear_replies(PROFILE_URL);
    world.net.reply(
        PROFILE_URL,
        200,
        r#"{"id":"00000000000000000000000000000000","name":"Nope"}"#,
    );
    let sent = world.net.sent_to(PROFILE_URL);
    assert_eq!(
        world.service.account_profile(&ada).await.unwrap().name,
        "Ada"
    );
    assert_eq!(
        world.service.account_profile(&bea).await.unwrap().name,
        "Bea"
    );
    assert_eq!(world.net.sent_to(PROFILE_URL), sent);
}

#[tokio::test]
async fn removing_an_account_drops_its_profile_and_a_later_sign_in_reads_again() {
    let world = world();
    script_account(&world, MS_ID, "Ada", "token-a");
    let (key, _) = sign_in(&world).await.unwrap();
    assert_eq!(
        world.service.account_profile(&key).await.unwrap().name,
        "Ada"
    );
    world.service.remove_account(&key).await.unwrap();
    script_account(&world, MS_ID, "Renewed", "token-a");
    let (again, _) = sign_in(&world).await.unwrap();
    assert_eq!(again, key);
    let after_sign_in = world.net.sent_to(PROFILE_URL);
    let profile = world.service.account_profile(&key).await.unwrap();
    assert_eq!(profile.name, "Renewed");
    assert_eq!(world.net.sent_to(PROFILE_URL), after_sign_in + 1);
}

#[tokio::test(start_paused = true)]
async fn rate_limit_keeps_the_previous_profile_and_a_new_token_does_not_end_it() {
    let world = world();
    microsoft_chain(&world, "Online", "token");
    let (key, _) = sign_in(&world).await.unwrap();
    assert_eq!(
        world.service.account_profile(&key).await.unwrap().name,
        "Online"
    );
    tokio::time::advance(std::time::Duration::from_secs(61)).await;
    world.net.clear_replies(PROFILE_URL);
    world
        .net
        .reply_with_headers(PROFILE_URL, 429, "", vec![("retry-after", "30")]);
    world.net.reply(
        PROFILE_URL,
        200,
        &format!(r#"{{"id":"{MS_ID}","name":"Later"}}"#),
    );
    let snapshot = world.service.account_profile_snapshot(&key).await.unwrap();
    assert_eq!(snapshot.profile.name, "Online");
    assert!(matches!(
        snapshot.warning,
        Some(AppearanceError::RateLimitedFor(30))
    ));
    let cooled = world.net.sent_to(PROFILE_URL);
    let again = world.service.account_profile_snapshot(&key).await.unwrap();
    assert_eq!(again.profile.name, "Online");
    assert!(again.warning.is_some());
    assert!(matches!(
        world.service.refresh_account_profile(&key).await,
        Err(ServiceError::Appearance(AppearanceError::RateLimitedFor(
            30
        )))
    ));
    assert_eq!(world.net.sent_to(PROFILE_URL), cooled);
    let mut login = crate::credentials::StoredLogin::load(world.secrets.as_ref(), &key)
        .unwrap()
        .unwrap();
    login.access_token = "token-next".into();
    login.save(world.secrets.as_ref(), &key).unwrap();
    assert!(matches!(
        world.service.account_profile(&key).await,
        Err(ServiceError::Appearance(AppearanceError::RateLimitedFor(
            30
        )))
    ));
    assert_eq!(
        world.net.sent_to(PROFILE_URL),
        cooled,
        "a new token does not lift a server cooldown"
    );
    tokio::time::advance(std::time::Duration::from_secs(30)).await;
    let renewed = world.service.refresh_account_profile(&key).await.unwrap();
    assert_eq!(renewed.name, "Later");
    assert_eq!(world.net.sent_to(PROFILE_URL), cooled + 1);
}

/// A plain-coloured skin PNG in the test's folder.
fn skin_file(world: &super::World, name: &str, colour: [u8; 4]) -> std::path::PathBuf {
    let mut bytes = Vec::new();
    image::RgbaImage::from_pixel(64, 64, image::Rgba(colour))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    let path = world._dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[tokio::test(start_paused = true)]
async fn library_edits_make_no_profile_reads_even_after_the_snapshot_expires() {
    let world = world();
    microsoft_chain(&world, "Online", "token");
    let (key, _) = sign_in(&world).await.unwrap();
    world.service.account_profile(&key).await.unwrap();
    // Past the 60-second snapshot: only the call paths keep Mojang quiet now.
    tokio::time::advance(std::time::Duration::from_secs(61)).await;
    let before = world.net.sent_to(PROFILE_URL);
    let service = &world.service;
    let first = service
        .import_library_skin(skin_file(&world, "A.png", [1, 2, 3, 255]), None)
        .await
        .unwrap();
    let second = service
        .import_library_skin(skin_file(&world, "B.png", [4, 5, 6, 255]), None)
        .await
        .unwrap();
    service
        .rename_library_skin(first.id.clone(), "Renamed".into())
        .await
        .unwrap();
    service
        .set_library_skin_model(first.id.clone(), SkinModel::Slim)
        .await
        .unwrap();
    service
        .set_library_skin_cape(first.id.clone(), crate::PairedCape::Hidden)
        .await
        .unwrap();
    let replaced = service
        .replace_library_skin(first.id, skin_file(&world, "C.png", [7, 8, 9, 255]))
        .await
        .unwrap();
    service
        .reorder_library_skins(vec![second.id.clone(), replaced.id.clone()])
        .await
        .unwrap();
    service.remove_library_skin(second.id).await.unwrap();
    service.skin_library().await.unwrap();
    service.library_looks().await.unwrap();
    assert_eq!(world.net.sent_to(PROFILE_URL), before);
    // Reading the account itself still refreshes the expired snapshot.
    service.account_profile(&key).await.unwrap();
    assert_eq!(world.net.sent_to(PROFILE_URL), before + 1);
}

#[tokio::test]
async fn a_refused_picture_does_not_hold_back_the_next_write_but_a_rate_limit_does() {
    let world = world();
    microsoft_chain(&world, "Online", "token");
    let (key, _) = sign_in(&world).await.unwrap();
    let upload = format!("{PROFILE_URL}/skins");
    let path = skin_file(&world, "Upload.png", [9, 9, 9, 255]);
    world.net.reply(&upload, 400, "");
    world.net.reply(&upload, 200, "");
    assert!(
        world
            .service
            .upload_account_skin_file(&key, path.clone(), SkinModel::Wide)
            .await
            .is_err()
    );
    world
        .service
        .upload_account_skin_file(&key, path.clone(), SkinModel::Wide)
        .await
        .expect("a corrected upload is sent at once");
    assert_eq!(world.net.sent_to(&upload), 2);
    world.net.clear_replies(&upload);
    world
        .net
        .reply_with_headers(&upload, 429, "", vec![("retry-after", "30")]);
    world.net.reply(&upload, 200, "");
    assert!(
        world
            .service
            .upload_account_skin_file(&key, path.clone(), SkinModel::Wide)
            .await
            .is_err()
    );
    assert!(matches!(
        world
            .service
            .upload_account_skin_file(&key, path, SkinModel::Wide)
            .await,
        Err(ServiceError::Appearance(AppearanceError::RateLimitedFor(
            30
        )))
    ));
    assert_eq!(
        world.net.sent_to(&upload),
        3,
        "the cooldown holds the next write without sending it"
    );
}
