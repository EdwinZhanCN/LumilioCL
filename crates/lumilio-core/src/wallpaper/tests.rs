use super::*;
use image::{ImageFormat, RgbaImage};

fn picture(dir: &Path, name: &str, width: u32, height: u32, color: [u8; 4]) -> std::path::PathBuf {
    let path = dir.join(name);
    RgbaImage::from_pixel(width, height, image::Rgba(color))
        .save_with_format(&path, ImageFormat::Png)
        .unwrap();
    path
}

#[test]
fn a_picture_is_copied_into_the_folder_under_a_name_made_from_its_content() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("themes");
    let source = picture(dir.path(), "sky.png", 640, 360, [20, 40, 200, 255]);

    let name = import_wallpaper(&source, &store).unwrap();
    assert!(name.starts_with("wallpaper-") && name.ends_with(".jpg"));
    let stored = image::open(store.join(&name)).unwrap();
    assert_eq!((stored.width(), stored.height()), (640, 360));

    // Same picture, same file; the original may go.
    fs::remove_file(&source).unwrap();
    let source = picture(dir.path(), "again.png", 640, 360, [20, 40, 200, 255]);
    assert_eq!(import_wallpaper(&source, &store).unwrap(), name);
}

#[test]
fn a_huge_picture_is_scaled_to_fit_and_keeps_its_proportions() {
    let dir = tempfile::tempdir().unwrap();
    let source = picture(dir.path(), "wide.png", 5120, 1440, [200, 200, 200, 255]);
    let name = import_wallpaper(&source, dir.path()).unwrap();
    let stored = image::open(dir.path().join(name)).unwrap();
    assert_eq!((stored.width(), stored.height()), (2560, 720));
}

#[test]
fn a_new_picture_replaces_the_old_one_and_unrelated_files_stay() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("themes");
    fs::create_dir_all(&store).unwrap();
    fs::write(store.join("mine.json"), "{}").unwrap();
    let first = import_wallpaper(
        &picture(dir.path(), "a.png", 64, 64, [255, 0, 0, 255]),
        &store,
    )
    .unwrap();
    let second = import_wallpaper(
        &picture(dir.path(), "b.png", 64, 64, [0, 255, 0, 255]),
        &store,
    )
    .unwrap();
    assert_ne!(first, second);
    assert!(!store.join(&first).exists());
    assert!(store.join(&second).exists());
    assert!(store.join("mine.json").exists());

    remove_wallpapers(&store, None);
    assert!(!store.join(&second).exists());
    assert!(store.join("mine.json").exists());
}

#[test]
fn a_file_that_is_not_a_picture_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("themes");
    let text = dir.path().join("notes.png");
    fs::write(&text, "not a picture").unwrap();
    let error = import_wallpaper(&text, &store).unwrap_err();
    assert!(matches!(error, WallpaperError::Unreadable(_)), "{error}");
    assert!(!store.exists());
    assert!(matches!(
        import_wallpaper(&dir.path().join("missing.png"), &store),
        Err(WallpaperError::Io(_))
    ));
}
