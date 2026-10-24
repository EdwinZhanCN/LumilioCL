use std::io::Read;

use super::*;

#[test]
fn names_ids_and_folders_are_replaced_longest_first() {
    let mut redactor = Redactor::new();
    redactor.hide("Steve", "<player>");
    redactor.hide_path(Path::new("/Users/steve_home"), "~");
    redactor.hide_path(
        Path::new("/Users/steve_home/Library/LumilioCL"),
        "<launcher>",
    );
    redactor.hide("a", "<x>");
    redactor.hide("  ", "<y>");
    let text = "Steve at /Users/steve_home/Library/LumilioCL/profiles and /Users/steve_home/x";
    assert_eq!(
        redactor.apply(text),
        "<player> at <launcher>/profiles and ~/x",
        "the longer folder wins, and one-letter secrets are ignored"
    );
}

#[test]
fn the_bundle_holds_the_files_and_leaves_nothing_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("diag.zip");
    write_bundle(
        &path,
        &[
            ("about.txt".to_owned(), "hello".to_owned()),
            ("logs/a.log".to_owned(), "line".to_owned()),
        ],
    )
    .unwrap();
    let mut archive = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    let mut text = String::new();
    archive
        .by_name("logs/a.log")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "line");
    assert!(!dir.path().join("diag.zip.part").exists());

    let blocked = dir.path().join("no-such-folder/diag.zip");
    assert!(write_bundle(&blocked, &[]).is_err());
    assert!(!dir.path().join("no-such-folder").exists());
}
