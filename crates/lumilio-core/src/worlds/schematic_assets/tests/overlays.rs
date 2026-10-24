use super::super::instance::{enabled_resource_packs, overlays};

#[test]
fn only_file_packs_from_options_are_enabled_in_their_order() {
    let options = "version:3955\nresourcePacks:[\"vanilla\",\"file/Faithful 32x.zip\",\"fabric\",\"file/Hand\",\"file/../x\"]\nincompatibleResourcePacks:[]\n";
    assert_eq!(
        enabled_resource_packs(options),
        ["Faithful 32x.zip", "Hand"]
    );
    assert!(enabled_resource_packs("resourcePacks:not json").is_empty());
    assert!(enabled_resource_packs("").is_empty());
}

#[test]
fn mods_by_name_come_before_the_enabled_packs() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path();
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::create_dir_all(game.join("resourcepacks/Hand/assets")).unwrap();
    for name in ["zeta.jar", "alpha.jar", "off.jar.disabled", "notes.txt"] {
        std::fs::write(game.join("mods").join(name), b"").unwrap();
    }
    std::fs::write(game.join("resourcepacks/Faithful.zip"), b"").unwrap();
    std::fs::write(game.join("resourcepacks/Unused.zip"), b"").unwrap();
    std::fs::write(
        game.join("options.txt"),
        "resourcePacks:[\"vanilla\",\"file/Hand\",\"file/Missing.zip\",\"file/Faithful.zip\"]\n",
    )
    .unwrap();
    assert_eq!(
        overlays(game),
        [
            game.join("mods/alpha.jar"),
            game.join("mods/zeta.jar"),
            game.join("resourcepacks/Hand"),
            game.join("resourcepacks/Faithful.zip"),
        ]
    );
}

/// T36: the kept order places what it names; what it does not name keeps
/// its default order below.
#[test]
fn a_kept_order_places_the_sources_it_names() {
    use std::path::PathBuf;

    use super::super::order::{arrange, swapped};
    let [a, b, c, d] = ["a.jar", "b.jar", "pack.zip", "new.jar"].map(PathBuf::from);
    let default = vec![a.clone(), b.clone(), d.clone(), c.clone()];
    assert_eq!(arrange(default.clone(), &[]), default);
    assert_eq!(
        arrange(default.clone(), &[c.clone(), a.clone(), b.clone()]),
        [d.clone(), c.clone(), a.clone(), b.clone()]
    );
    // A source the order names that is gone is skipped.
    assert_eq!(
        arrange(
            vec![a.clone(), b.clone()],
            &[b.clone(), c.clone(), a.clone()]
        ),
        [b.clone(), a.clone()]
    );

    // `b` is a library mod the list does not show: `a` moves past it to `c`.
    let list = [a.clone(), b.clone(), c.clone()];
    assert_eq!(
        swapped(&list, &a, &c).unwrap(),
        [c.clone(), b.clone(), a.clone()]
    );
    assert_eq!(swapped(&list, &a, &a), None);
    assert_eq!(swapped(&list, &d, &a), None, "not a source");
}
