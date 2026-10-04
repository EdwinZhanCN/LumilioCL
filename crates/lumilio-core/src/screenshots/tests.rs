use super::*;
use image::{ImageFormat, RgbaImage};

fn game() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir_all(game.join("screenshots")).unwrap();
    (dir, game)
}

fn picture(game: &Path, name: &str, width: u32, height: u32) {
    let image = RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
    image
        .save_with_format(game.join("screenshots").join(name), ImageFormat::Png)
        .unwrap();
}

fn at(game: &Path, name: &str, seconds: u64) {
    let file = fs::File::options()
        .write(true)
        .open(game.join("screenshots").join(name))
        .unwrap();
    file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(seconds))
        .unwrap();
}

#[test]
fn pictures_are_listed_newest_first_and_other_files_are_not() {
    let (_dir, game) = game();
    picture(&game, "old.png", 4, 4);
    picture(&game, "new.PNG", 4, 4);
    at(&game, "old.png", 1000);
    at(&game, "new.PNG", 2000);
    fs::write(game.join("screenshots/notes.txt"), "x").unwrap();
    fs::write(game.join("screenshots/.hidden.png"), "x").unwrap();
    fs::create_dir_all(game.join("screenshots/folder.png")).unwrap();
    let names: Vec<_> = scan(&game).unwrap().into_iter().map(|s| s.file).collect();
    assert_eq!(names, ["new.PNG", "old.png"]);
}

#[test]
fn a_missing_folder_is_an_empty_list() {
    let dir = tempfile::tempdir().unwrap();
    assert!(scan(dir.path()).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn a_linked_folder_or_file_is_refused() {
    let (dir, game) = game();
    let outside = dir.path().join("outside.png");
    RgbaImage::new(2, 2).save(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, game.join("screenshots/link.png")).unwrap();
    assert!(scan(&game).unwrap().is_empty(), "links are not listed");
    assert!(matches!(
        delete(&game, "link.png"),
        Err(ScreenshotError::UnsafeName(_))
    ));
    assert!(outside.is_file());

    let elsewhere = tempfile::tempdir().unwrap();
    let linked = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), linked.path().join("screenshots")).unwrap();
    assert!(matches!(scan(linked.path()), Err(ScreenshotError::Linked)));
}

#[test]
fn thumbnails_are_small_cached_and_follow_the_file() {
    let (dir, game) = game();
    let cache = dir.path().join("thumbs");
    picture(&game, "big.png", 1920, 1080);
    let thumb = thumbnail(&game, &cache, "big.png").unwrap();
    let small = image::open(&thumb).unwrap();
    assert_eq!((small.width(), small.height()), (480, 270));
    assert!(cache.starts_with(dir.path()) && thumb.starts_with(&cache));

    // Asking again reuses it; replacing the picture makes a new one.
    let first = fs::metadata(&thumb).unwrap().modified().unwrap();
    assert_eq!(thumbnail(&game, &cache, "big.png").unwrap(), thumb);
    assert_eq!(fs::metadata(&thumb).unwrap().modified().unwrap(), first);
    picture(&game, "big.png", 800, 400);
    at(&game, "big.png", 5000);
    let replaced = thumbnail(&game, &cache, "big.png").unwrap();
    assert_ne!(replaced, thumb);
    assert_eq!(image::open(&replaced).unwrap().width(), 480);
    // The game directory got nothing.
    assert_eq!(scan(&game).unwrap().len(), 1);
}

#[test]
fn a_file_that_is_not_a_picture_is_an_error_not_a_panic() {
    let (dir, game) = game();
    fs::write(game.join("screenshots/broken.png"), b"not an image").unwrap();
    assert!(matches!(
        thumbnail(&game, &dir.path().join("t"), "broken.png"),
        Err(ScreenshotError::Unreadable(_))
    ));
}

#[test]
fn only_plain_picture_names_in_the_folder_can_be_touched() {
    let (_dir, game) = game();
    picture(&game, "a.png", 2, 2);
    fs::write(game.join("level.dat"), "x").unwrap();
    for bad in ["../level.dat", "a/../a.png", "..", "level.dat", ""] {
        assert!(
            matches!(delete(&game, bad), Err(ScreenshotError::UnsafeName(_))),
            "{bad:?}"
        );
    }
    assert!(matches!(
        delete(&game, "gone.png"),
        Err(ScreenshotError::NotFound(_))
    ));
    assert!(game.join("level.dat").is_file());
    assert!(read(&game, "a.png").unwrap().len() > 8);
    delete(&game, "a.png").unwrap();
    assert!(scan(&game).unwrap().is_empty());
}
