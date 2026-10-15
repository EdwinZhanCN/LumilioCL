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
