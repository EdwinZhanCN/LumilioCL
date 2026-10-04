use super::super::error::ServiceError;
use super::world;
use crate::instance::Loader;
use crate::screenshots::ScreenshotError;

#[tokio::test]
async fn screenshots_are_listed_thumbnailed_and_deleted_even_while_the_game_is_busy() {
    let world = world();
    let record = world
        .service
        .create_instance("Shots", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let id = &record.id;
    assert!(world.service.screenshots(id).await.unwrap().is_empty());

    let folder = world.service.layout.game(id).join("screenshots");
    std::fs::create_dir_all(&folder).unwrap();
    image::RgbaImage::new(1000, 500)
        .save(folder.join("2026-01-01_12.00.00.png"))
        .unwrap();

    // Reading and deleting do not need the lease: the game takes screenshots while it runs.
    let _busy = world.service.reserve_instance(id).unwrap();
    let listed = world.service.screenshots(id).await.unwrap();
    assert_eq!(listed.len(), 1);
    let thumb = world
        .service
        .screenshot_thumbnail(id, &listed[0].file)
        .await
        .unwrap();
    assert!(thumb.starts_with(world.service.layout.thumbnails(id)));
    assert!(
        !world
            .service
            .screenshot_bytes(id, &listed[0].file)
            .await
            .unwrap()
            .is_empty()
    );

    assert!(matches!(
        world.service.delete_screenshot(id, "../x.png").await,
        Err(ServiceError::Screenshot(ScreenshotError::UnsafeName(_)))
    ));
    world
        .service
        .delete_screenshot(id, &listed[0].file)
        .await
        .unwrap();
    assert!(world.service.screenshots(id).await.unwrap().is_empty());
}
