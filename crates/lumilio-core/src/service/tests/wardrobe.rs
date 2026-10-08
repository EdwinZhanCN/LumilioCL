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
        world.service.account_profile(&key).await,
        Err(ServiceError::Appearance(AppearanceError::SignInRequired))
    ));
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
