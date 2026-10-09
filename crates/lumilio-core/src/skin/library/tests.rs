use super::*;

fn png(color: [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(64, 64, image::Rgba(color));
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

#[test]
fn copied_skins_models_origins_and_order_survive_reopen_and_source_deletion() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.png");
    let red = png([255, 0, 0, 255]);
    fs::write(&source, &red).unwrap();
    let mut library = SkinLibrary::open(directory.path()).unwrap();
    let a = library
        .add(
            "Red".into(),
            &red,
            SkinModel::Wide,
            SkinSource::LocalFile(source.clone()),
        )
        .unwrap();
    let b = library
        .add(
            "Blue".into(),
            &png([0, 0, 255, 255]),
            SkinModel::Slim,
            SkinSource::Mojang("https://textures.minecraft.net/texture/blue".into()),
        )
        .unwrap();
    assert_eq!(
        library
            .add("Again".into(), &red, SkinModel::Wide, a.source.clone())
            .unwrap(),
        a
    );
    library.reorder(&[b.id.clone(), a.id.clone()]).unwrap();
    library.set_model(&a.id, SkinModel::Slim).unwrap();
    fs::remove_file(source).unwrap();
    let library = SkinLibrary::open(directory.path()).unwrap();
    assert_eq!(library.entries().len(), 2);
    assert_eq!(library.entries()[0], b);
    assert_eq!(library.entries()[1].model, SkinModel::Slim);
    assert_eq!(library.entries()[1].source, a.source);
    let image = image::load_from_memory(&library.picture(&a.id).unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(image.get_pixel(8, 8).0, [255, 0, 0, 255]);
}

#[test]
fn invalid_import_order_or_id_does_not_change_the_library_and_removal_keeps_selected_files() {
    let directory = tempfile::tempdir().unwrap();
    let mut library = SkinLibrary::open(directory.path()).unwrap();
    assert!(
        library
            .add(
                "Bad".into(),
                b"no picture",
                SkinModel::Wide,
                SkinSource::LocalFile("bad".into())
            )
            .is_err()
    );
    assert!(library.entries().is_empty());
    let a = library
        .add(
            "A".into(),
            &png([0, 255, 0, 255]),
            SkinModel::Wide,
            SkinSource::LocalFile("source".into()),
        )
        .unwrap();
    let before = fs::read(directory.path().join("skins/library.json")).unwrap();
    for order in [
        vec![],
        vec![a.id.clone(), a.id.clone()],
        vec!["../outside".into()],
    ] {
        assert!(library.reorder(&order).is_err());
        assert_eq!(
            fs::read(directory.path().join("skins/library.json")).unwrap(),
            before
        );
    }
    assert!(library.picture("../outside").is_err());
    let selected = library.path(&a.id).unwrap();
    library.remove(&a.id).unwrap();
    assert!(
        SkinLibrary::open(directory.path())
            .unwrap()
            .entries()
            .is_empty()
    );
    assert!(
        selected.exists(),
        "a selected offline skin must remain usable"
    );
}

#[test]
fn a_newer_or_traversing_index_is_refused_without_writing_it() {
    let directory = tempfile::tempdir().unwrap();
    let folder = directory.path().join("skins");
    fs::create_dir(&folder).unwrap();
    let path = folder.join("library.json");
    for text in [
        r#"{"schema":99,"entries":[]}"#,
        r#"{"schema":1,"entries":[{"id":"../outside","name":"Bad","model":"wide","source":{"LocalFile":"x"}}]}"#,
    ] {
        fs::write(&path, text).unwrap();
        assert!(SkinLibrary::open(directory.path()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
}

#[test]
fn entries_from_before_pairing_keep_the_cape_and_edits_persist() {
    let directory = tempfile::tempdir().unwrap();
    let mut library = SkinLibrary::open(directory.path()).unwrap();
    let skin = library
        .add(
            "Red".into(),
            &png([255, 0, 0, 255]),
            SkinModel::Wide,
            SkinSource::Mojang("https://textures.minecraft.net/texture/red".into()),
        )
        .unwrap();
    assert_eq!(skin.cape, PairedCape::Keep);
    // An index written before pairing has no `cape` field.
    let index = directory.path().join("skins/library.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index).unwrap()).unwrap();
    json["entries"][0]
        .as_object_mut()
        .unwrap()
        .remove("cape")
        .expect("written with a cape");
    fs::write(&index, json.to_string()).unwrap();
    let mut library = SkinLibrary::open(directory.path()).unwrap();
    assert_eq!(library.entries()[0].cape, PairedCape::Keep);

    library.rename(&skin.id, "  Crimson ").unwrap();
    library
        .set_cape(&skin.id, PairedCape::Cape("cape-id".into()))
        .unwrap();
    assert_eq!(
        library.rename(&skin.id, "   "),
        Err(AppearanceError::EmptyName)
    );
    let library = SkinLibrary::open(directory.path()).unwrap();
    assert_eq!(library.entries()[0].name, "Crimson");
    assert_eq!(
        library.entries()[0].cape,
        PairedCape::Cape("cape-id".into())
    );
}

#[test]
fn replacing_a_picture_keeps_the_entry_and_its_old_file() {
    let directory = tempfile::tempdir().unwrap();
    let mut library = SkinLibrary::open(directory.path()).unwrap();
    let source = |name: &str| SkinSource::LocalFile(directory.path().join(name));
    let first = library
        .add(
            "First".into(),
            &png([255, 0, 0, 255]),
            SkinModel::Slim,
            source("a.png"),
        )
        .unwrap();
    let second = library
        .add(
            "Second".into(),
            &png([0, 255, 0, 255]),
            SkinModel::Wide,
            source("b.png"),
        )
        .unwrap();
    library.set_cape(&first.id, PairedCape::Hidden).unwrap();
    let old_file = library.path(&first.id).unwrap();

    let replaced = library
        .replace(&first.id, &png([0, 0, 255, 255]), source("c.png"))
        .unwrap();
    assert_ne!(replaced.id, first.id);
    assert_eq!(
        (replaced.name.as_str(), replaced.model, &replaced.cape),
        ("First", SkinModel::Slim, &PairedCape::Hidden)
    );
    let ids: Vec<_> = library.entries().iter().map(|e| e.id.clone()).collect();
    assert_eq!(ids, [replaced.id.clone(), second.id.clone()], "same place");
    assert!(old_file.exists(), "an offline account may still use it");

    // A picture already in the library takes the place once, not twice.
    let merged = library
        .replace(&replaced.id, &png([0, 255, 0, 255]), source("d.png"))
        .unwrap();
    assert_eq!(merged.id, second.id);
    assert_eq!(library.entries().len(), 1);
    assert_eq!(library.entries()[0].name, "First");
}
