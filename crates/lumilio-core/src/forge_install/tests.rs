use super::*;
use std::io::Write;

fn installer(dir: &Path, profile: &str) -> PathBuf {
    let path = dir.join("installer.jar");
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("install_profile.json", options).unwrap();
    zip.write_all(profile.as_bytes()).unwrap();
    zip.start_file("version.json", options).unwrap();
    zip.write_all(br#"{"id":"x","inheritsFrom":"1.21.1"}"#)
        .unwrap();
    zip.start_file("data/client.lzma", options).unwrap();
    zip.write_all(b"patch").unwrap();
    zip.start_file("maven/a/b/1/b-1.jar", options).unwrap();
    zip.write_all(b"embedded").unwrap();
    zip.finish().unwrap();
    path
}

const PROFILE: &str = r#"{
    "spec": 1, "minecraft": "1.21.1", "json": "/version.json",
    "data": {
        "BINPATCH": {"client": "/data/client.lzma", "server": "/data/server.lzma"},
        "PATCHED": {"client": "[net.neoforged:neoforge:21.1.1:client]", "server": "[x:y:1:server]"},
        "MCP_VERSION": {"client": "'1.21.1-2024'", "server": "'x'"}
    },
    "processors": [
        {"sides": ["server"], "jar": "t:tools:1", "args": ["--server"]},
        {"jar": "t:tools:1", "classpath": ["t:dep:2"],
         "args": ["--input", "{MINECRAFT_JAR}", "--patch", "{BINPATCH}", "--mcp", "{MCP_VERSION}",
                  "--to", "{ROOT}/run.sh", "--lib", "[t:dep:2]", "--side", "{SIDE}"],
         "outputs": {"{PATCHED}": "'abc'"}}
    ],
    "libraries": [{"name": "t:tools:1", "downloads": {"artifact": {"path": "t/tools/1/tools-1.jar", "url": "https://x/tools-1.jar", "sha1": "00"}}}]
}"#;

fn paths(dir: &Path, installer: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let _ = installer;
    (
        dir.join("libraries"),
        dir.join("client.jar"),
        dir.join("root"),
        dir.join("work"),
    )
}

#[test]
fn reads_the_profile_and_fills_in_every_kind_of_value() {
    let dir = tempfile::tempdir().unwrap();
    let jar = installer(dir.path(), PROFILE);
    let read = read_installer(&jar).unwrap();
    assert!(String::from_utf8_lossy(&read.version_json).contains("inheritsFrom"));
    let (libraries, minecraft, root, work) = paths(dir.path(), &jar);
    let paths = Paths {
        installer: &jar,
        libraries: &libraries,
        minecraft_jar: &minecraft,
        root: &root,
        work: &work,
    };
    let vars = variables(&read.profile, "1.21.1", &paths).unwrap();
    assert_eq!(vars["MCP_VERSION"], "1.21.1-2024", "literal");
    assert_eq!(
        PathBuf::from(&vars["PATCHED"]),
        libraries.join("net/neoforged/neoforge/21.1.1/neoforge-21.1.1-client.jar"),
        "artifact"
    );
    let patch = PathBuf::from(&vars["BINPATCH"]);
    assert_eq!(
        std::fs::read(&patch).unwrap(),
        b"patch",
        "extracted from the installer"
    );
    assert_eq!(vars["SIDE"], "client");

    let client: Vec<_> = read.profile.client_processors().collect();
    assert_eq!(client.len(), 1, "server-only processors are skipped");
    let args: Vec<String> = client[0]
        .args
        .iter()
        .map(|arg| expand(arg, &vars, &libraries).unwrap())
        .collect();
    assert_eq!(args[1], minecraft.to_string_lossy());
    assert_eq!(args[5], "1.21.1-2024");
    assert_eq!(args[7], format!("{}/run.sh", root.to_string_lossy()));
    assert_eq!(PathBuf::from(&args[9]), libraries.join("t/dep/2/dep-2.jar"));
    assert!(expand("{NOPE}", &vars, &libraries).is_err());
    assert!(extract_embedded(&jar, "a/b/1/b-1.jar", &work.join("b.jar")));
    assert!(!extract_embedded(
        &jar,
        "a/b/2/b-2.jar",
        &work.join("c.jar")
    ));
}

#[test]
fn a_processor_is_skipped_only_when_its_outputs_match() {
    let dir = tempfile::tempdir().unwrap();
    let jar = installer(dir.path(), PROFILE);
    let profile = read_installer(&jar).unwrap().profile;
    let (libraries, minecraft, root, work) = paths(dir.path(), &jar);
    let paths = Paths {
        installer: &jar,
        libraries: &libraries,
        minecraft_jar: &minecraft,
        root: &root,
        work: &work,
    };
    let vars = variables(&profile, "1.21.1", &paths).unwrap();
    let processor = profile.client_processors().next().unwrap();
    assert!(
        !outputs_present(processor, &vars, &libraries).unwrap(),
        "missing"
    );
    let patched_file = PathBuf::from(&vars["PATCHED"]);
    std::fs::create_dir_all(patched_file.parent().unwrap()).unwrap();
    std::fs::write(&patched_file, b"anything").unwrap();
    assert!(
        !outputs_present(processor, &vars, &libraries).unwrap(),
        "present but the hash differs"
    );
    assert!(
        patched(&profile, &libraries),
        "no PATCHED_SHA: presence is enough"
    );
}

#[test]
fn old_installers_and_urls_are_told_apart() {
    let dir = tempfile::tempdir().unwrap();
    let jar = installer(dir.path(), r#"{"install": {}, "versionInfo": {}}"#);
    assert!(matches!(
        read_installer(&jar),
        Err(ForgeError::LegacyInstaller)
    ));
    assert_eq!(
        installer_url(Loader::Forge, "1.20.1", "47.4.10").unwrap(),
        "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.4.10/forge-1.20.1-47.4.10-installer.jar"
    );
    assert_eq!(
        installer_url(Loader::NeoForge, "1.21.1", "21.1.209").unwrap(),
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.209/neoforge-21.1.209-installer.jar"
    );
    assert_eq!(
        installer_url(Loader::NeoForge, "1.20.1", "47.1.5").unwrap(),
        "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.5/forge-1.20.1-47.1.5-installer.jar"
    );
    assert!(installer_url(Loader::Fabric, "1.21.1", "0.16").is_none());
}
