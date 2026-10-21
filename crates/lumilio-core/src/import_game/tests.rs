use super::*;

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

#[test]
fn a_prism_instance_is_read_with_its_name_game_and_loader() {
    let dir = tempfile::tempdir().unwrap();
    let instance = dir.path().join("inst");
    write(
        &instance.join("instance.cfg"),
        "InstanceType=OneSix\nname=我的生存\n",
    );
    write(
        &instance.join("mmc-pack.json"),
        r#"{"components":[
            {"uid":"org.lwjgl3","version":"3.3.1"},
            {"uid":"net.minecraft","version":"1.20.1"},
            {"uid":"net.fabricmc.fabric-loader","version":"0.15.7"}],"formatVersion":1}"#,
    );
    write(&instance.join(".minecraft/mods/a.jar"), "mod");
    let found = detect(&instance).unwrap();
    assert_eq!(found.len(), 1);
    let game = &found[0];
    assert_eq!(
        (game.name.as_str(), game.game_version.as_str(), game.loader),
        ("我的生存", "1.20.1", Loader::Fabric)
    );
    assert_eq!(game.loader_version.as_deref(), Some("0.15.7"));
    assert!(game.game_dir.ends_with(".minecraft"));

    // No game folder, or no Minecraft component, is unreadable.
    let bare = dir.path().join("bare");
    write(&bare.join("instance.cfg"), "name=x");
    write(&bare.join("mmc-pack.json"), r#"{"components":[]}"#);
    assert!(matches!(detect(&bare), Err(ImportError::Unreadable(_))));
}

type Reading = Option<(String, Loader, Option<String>)>;

/// Writes `versions/<name>/<name>.json` and reads that version back.
fn version(versions: &Path, name: &str, json: &str) -> Reading {
    write(&versions.join(name).join(format!("{name}.json")), json);
    profile::read(versions, name)
}

fn modded(game: &str, loader: Loader, version: &str) -> Reading {
    Some((game.into(), loader, Some(version.into())))
}

fn vanilla(game: &str) -> Reading {
    Some((game.into(), Loader::Vanilla, None))
}

/// A game jar that names its version in `version.json`, as 1.14 and later do.
fn jar(path: &Path, id: &str) {
    use std::io::Write as _;
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    writer
        .start_file("version.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    write!(writer, r#"{{"id":"{id}","name":"{id}"}}"#).unwrap();
    writer.finish().unwrap();
}

#[test]
fn the_loader_comes_from_the_libraries_whatever_the_version_is_called() {
    let dir = tempfile::tempdir().unwrap();
    let versions = dir.path();
    // PCL (issue #58): a merged json, a display name for an id, no inheritsFrom.
    assert_eq!(
        version(
            versions,
            "26.3-Fabric 0.19.5",
            r#"{"id":"26.3-Fabric 0.19.5","clientVersion":"26.3",
                "mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
                "libraries":[{"name":"org.ow2.asm:asm:9.8"},
                             {"name":"net.fabricmc:fabric-loader:0.19.5"}]}"#,
        ),
        modded("26.3", Loader::Fabric, "0.19.5")
    );
    // HMCL: merged too, the game and the loader kept as patches.
    assert_eq!(
        version(
            versions,
            "我的整合包",
            r#"{"id":"我的整合包","mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher",
                "patches":[{"id":"game","version":"1.20.1"},{"id":"forge","version":"47.2.0"}],
                "libraries":[{"name":"net.minecraftforge:fmlloader:1.20.1-47.2.0"}]}"#,
        ),
        modded("1.20.1", Loader::Forge, "47.2.0")
    );
    // The official installers: inheritsFrom names the game.
    assert_eq!(
        version(
            versions,
            "fabric-loader-0.16.0-1.21.1",
            r#"{"id":"fabric-loader-0.16.0-1.21.1","inheritsFrom":"1.21.1",
                "mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
                "libraries":[{"name":"net.fabricmc:intermediary:1.21.1"},
                             {"name":"net.fabricmc:fabric-loader:0.16.0"}]}"#,
        ),
        modded("1.21.1", Loader::Fabric, "0.16.0")
    );
    assert_eq!(
        version(
            versions,
            "quilt-loader-0.26.0-1.21.1",
            r#"{"id":"quilt-loader-0.26.0-1.21.1","inheritsFrom":"1.21.1",
                "libraries":[{"name":"org.quiltmc:quilt-loader:0.26.0"},
                             {"name":"net.fabricmc:intermediary:1.21.1"}]}"#,
        ),
        modded("1.21.1", Loader::Quilt, "0.26.0")
    );
    assert_eq!(
        version(
            versions,
            "1.20.1-forge-47.2.0",
            r#"{"id":"1.20.1-forge-47.2.0","inheritsFrom":"1.20.1",
                "mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher",
                "arguments":{"game":["--launchTarget","forgeclient",
                    "--fml.forgeVersion","47.2.0","--fml.mcVersion","1.20.1"]},
                "libraries":[{"name":"net.minecraftforge:fmlloader:1.20.1-47.2.0"}]}"#,
        ),
        modded("1.20.1", Loader::Forge, "47.2.0")
    );
    // A renamed NeoForge, its version only in the arguments.
    assert_eq!(
        version(
            versions,
            "NeoForge 生存",
            r#"{"id":"NeoForge 生存","mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher",
                "arguments":{"game":["--fml.neoForgeVersion","21.1.50","--fml.mcVersion","1.21.1"]},
                "libraries":[{"name":"net.neoforged.fancymodloader:loader:4.0.24"},
                             {"name":"net.minecraftforge:forgespi:7.1.0"}]}"#,
        ),
        modded("1.21.1", Loader::NeoForge, "21.1.50")
    );
    // An old merged Forge: the library names both the game and the build.
    assert_eq!(
        version(
            versions,
            "1.12.2-Forge",
            r#"{"id":"1.12.2-Forge","mainClass":"net.minecraft.launchwrapper.Launch",
                "minecraftArguments":"--username ${auth_player_name} --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker",
                "libraries":[{"name":"net.minecraftforge:forge:1.12.2-14.23.5.2860"}]}"#,
        ),
        modded("1.12.2", Loader::Forge, "14.23.5.2860")
    );
}

#[test]
fn a_version_without_a_loader_is_vanilla_wherever_its_game_version_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let versions = dir.path();
    let main = r#""mainClass":"net.minecraft.client.main.Main""#;
    assert_eq!(
        version(versions, "1.21.1", &format!(r#"{{"id":"1.21.1",{main}}}"#)),
        vanilla("1.21.1")
    );
    assert_eq!(
        version(versions, "24w14a", &format!(r#"{{"id":"24w14a",{main}}}"#)),
        vanilla("24w14a")
    );
    assert_eq!(
        version(
            versions,
            "生存服",
            &format!(r#"{{"id":"生存服","clientVersion":"1.21.1",{main}}}"#)
        ),
        vanilla("1.21.1")
    );
    // Nothing in the json: the game jar says which game it is.
    jar(&versions.join("整合包/整合包.jar"), "1.14 / 5a1b2c");
    assert_eq!(
        version(versions, "整合包", &format!(r#"{{"id":"整合包",{main}}}"#)),
        vanilla("1.14")
    );
    assert_eq!(
        version(versions, "随便", &format!(r#"{{"id":"随便",{main}}}"#)),
        None,
        "a game version is never made up from a name"
    );
}

#[test]
fn an_inherited_json_is_read_with_its_parents_and_unnameable_mods_are_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let versions = dir.path();
    // The parent is a renamed vanilla; the loader json only inherits from it.
    write(
        &versions.join("原版/原版.json"),
        r#"{"id":"原版","clientVersion":"1.20.1","mainClass":"net.minecraft.client.main.Main"}"#,
    );
    assert_eq!(
        version(
            versions,
            "Fabric",
            r#"{"id":"Fabric","inheritsFrom":"原版",
                "libraries":[{"name":"net.fabricmc:fabric-loader:0.15.7"}]}"#,
        ),
        modded("1.20.1", Loader::Fabric, "0.15.7")
    );
    // OptiFine alone, Legacy Fabric and a loader without a library are not
    // turned into vanilla.
    assert_eq!(
        version(
            versions,
            "1.12.2-OptiFine",
            r#"{"id":"1.12.2-OptiFine","inheritsFrom":"1.12.2",
                "minecraftArguments":"--tweakClass optifine.OptiFineTweaker",
                "libraries":[{"name":"optifine:OptiFine:1.12.2_HD_U_G5"},
                             {"name":"optifine:launchwrapper-of:2.2"}]}"#,
        ),
        None
    );
    assert_eq!(
        version(
            versions,
            "legacy",
            r#"{"id":"legacy","clientVersion":"1.8.9",
                "libraries":[{"name":"net.fabricmc:fabric-loader:0.15.7"},
                             {"name":"net.legacyfabric:intermediary:1.8.9"}]}"#,
        ),
        None
    );
    assert_eq!(
        version(
            versions,
            "odd",
            r#"{"id":"odd","clientVersion":"1.21.1",
                "mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient"}"#,
        ),
        None
    );
    assert_eq!(
        version(
            versions,
            "child",
            r#"{"id":"child","inheritsFrom":"1.21.1"}"#
        ),
        None
    );
    // A loop of inheritsFrom ends.
    write(
        &versions.join("a/a.json"),
        r#"{"id":"a","inheritsFrom":"b"}"#,
    );
    assert_eq!(
        version(versions, "b", r#"{"id":"b","inheritsFrom":"a"}"#),
        None
    );
}

#[test]
fn a_minecraft_folder_lists_each_readable_version_and_finds_isolated_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join(".minecraft");
    write(
        &root.join("versions/1.21.1/1.21.1.json"),
        r#"{"id":"1.21.1"}"#,
    );
    write(
        &root.join("versions/fab/fab.json"),
        r#"{"id":"fabric-loader-0.16.0-1.21.1","inheritsFrom":"1.21.1",
            "libraries":[{"name":"net.fabricmc:fabric-loader:0.16.0"}]}"#,
    );
    write(&root.join("versions/fab/mods/a.jar"), "mod");
    write(&root.join("versions/junk/junk.json"), "not json");
    write(&root.join("saves/W/level.dat"), "world");
    let found = detect(&root).unwrap();
    let names: Vec<_> = found.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["1.21.1", "fab"]);
    assert_eq!(
        found[0].game_dir, root,
        "shared folder when nothing is isolated"
    );
    assert_eq!(found[1].game_dir, root.join("versions/fab"));
    assert_eq!(found[1].loader, Loader::Fabric);
    // The folder that contains .minecraft works too.
    assert_eq!(detect(dir.path()).unwrap().len(), 2);
    assert!(matches!(
        detect(&dir.path().join("nothing")),
        Err(ImportError::Unrecognised)
    ));
}

#[test]
fn copying_takes_the_players_files_and_leaves_the_other_launchers_behind() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join(".minecraft");
    for path in [
        "mods/a.jar",
        "saves/W/level.dat",
        "options.txt",
        "libraries/x.jar",
        "assets/y",
        "versions/1.21.1/1.21.1.json",
        "logs/latest.log",
        ".hidden",
        "launcher_profiles.json",
    ] {
        write(&root.join(path), "x");
    }
    let game = FoundGame {
        name: "g".into(),
        game_version: "1.21.1".into(),
        loader: Loader::Vanilla,
        loader_version: None,
        game_dir: root.clone(),
        origin: GameOrigin::MultiMcPrism,
    };
    let into = dir.path().join("into");
    let copied = copy_game_files(&game, &into, &crate::activity::CancellationToken::new()).unwrap();
    assert_eq!(copied, 3);
    for kept in ["mods/a.jar", "saves/W/level.dat", "options.txt"] {
        assert!(into.join(kept).is_file(), "{kept}");
    }
    for left in [
        "libraries",
        "assets",
        "versions",
        "logs",
        ".hidden",
        "launcher_profiles.json",
    ] {
        assert!(!into.join(left).exists(), "{left}");
    }
    assert!(root.join("mods/a.jar").is_file(), "the source is untouched");
}
