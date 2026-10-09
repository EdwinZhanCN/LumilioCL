use crate::release::Result;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub(super) fn write_json(path: &Path, value: &Value) -> Result {
    std::fs::write(
        path,
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())
}
fn hash(path: &Path) -> Result<String> {
    Ok(
        Sha1::digest(std::fs::read(path).map_err(|e| e.to_string())?)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}
fn download(url: &str, path: &Path, expected: Option<&str>) -> Result {
    if expected.is_some_and(|sha| hash(path).ok().as_deref() == Some(sha)) {
        return Ok(());
    }
    let status = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--output",
        ])
        .arg(path)
        .arg(url)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("download failed: {url}"));
    }
    if let Some(sha) = expected
        && hash(path)? != sha
    {
        return Err(format!("SHA1 mismatch: {}", path.display()));
    }
    Ok(())
}
pub(super) fn java() -> String {
    std::env::var("WORLDGEN_JAVA").unwrap_or_else(|_| "java".into())
}
pub(super) fn execute(command: &mut Command) -> Result {
    let status = command.status().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("command failed ({status}): {command:?}"));
    }
    Ok(())
}
pub(super) fn path(root: &Path, version: &str) -> Result<PathBuf> {
    if !(2..=3).contains(&version.split('.').count())
        || !version
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("expected an exact release version".into());
    }
    Ok(root.join("target/worldgen").join(version))
}

pub(super) fn prepare(root: &Path, version: &str) -> Result<PathBuf> {
    let work = path(root, version)?;
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let manifest = work.join("manifest.json");
    download(
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
        &manifest,
        None,
    )?;
    let manifest = json(&manifest)?;
    let entry = manifest["versions"]
        .as_array()
        .and_then(|vs| {
            vs.iter()
                .find(|v| v["id"] == version && v["type"] == "release")
        })
        .ok_or("release absent from Mojang manifest")?;
    let metadata = work.join("version.json");
    download(
        entry["url"].as_str().ok_or("missing metadata URL")?,
        &metadata,
        entry["sha1"].as_str(),
    )?;
    let metadata = json(&metadata)?;
    let server = &metadata["downloads"]["server"];
    let bundled = work.join("server.jar");
    download(
        server["url"].as_str().ok_or("missing server URL")?,
        &bundled,
        Some(server["sha1"].as_str().ok_or("missing server SHA1")?),
    )?;
    let file = std::fs::File::open(&bundled).map_err(|e| e.to_string())?;
    let mut bundle = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let libraries = work.join("libraries");
    std::fs::create_dir_all(&libraries).map_err(|e| e.to_string())?;
    let mut found = false;
    for i in 0..bundle.len() {
        let mut entry = bundle.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_owned();
        let dest = if name.starts_with("META-INF/versions/") && name.ends_with(".jar") {
            if found {
                return Err("multiple bundled game jars".into());
            }
            found = true;
            Some(work.join("game.jar"))
        } else if name.starts_with("META-INF/libraries/") && name.ends_with(".jar") {
            Some(libraries.join(Path::new(&name).file_name().ok_or("invalid library name")?))
        } else {
            None
        };
        if let Some(dest) = dest {
            std::io::copy(
                &mut entry,
                &mut std::fs::File::create(dest).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
    }
    if !found {
        return Err("missing bundled server".into());
    }
    let mut jar = zip::ZipArchive::new(
        std::fs::File::open(work.join("game.jar")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut data = BTreeMap::new();
    for i in 0..jar.len() {
        let mut entry = jar.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_owned();
        if name.ends_with(".json")
            && (name.starts_with("data/minecraft/worldgen/")
                || name.starts_with("data/minecraft/tags/worldgen/"))
        {
            let mut bytes = vec![];
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            data.insert(
                name,
                serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string())?,
            );
        }
    }
    write_json(
        &work.join("data.json"),
        &serde_json::to_value(data).map_err(|e| e.to_string())?,
    )?;
    execute(
        Command::new(java())
            .args([
                "-Xmx1G",
                "-DbundlerMainClass=net.minecraft.data.Main",
                "-jar",
                "server.jar",
                "--reports",
            ])
            .current_dir(&work),
    )?;
    Ok(work)
}

pub(super) fn named(root: &Path, work: &Path) -> Result<PathBuf> {
    let metadata = json(&work.join("version.json"))?;
    let mapping = &metadata["downloads"]["server_mappings"];
    if mapping.is_null() {
        return Ok(work.join("game.jar"));
    }
    let mappings = work.join("mappings.txt");
    download(
        mapping["url"].as_str().ok_or("missing mappings URL")?,
        &mappings,
        Some(mapping["sha1"].as_str().ok_or("missing mappings SHA1")?),
    )?;
    let tool = root.join("target/worldgen/art-1.1.2.jar");
    download(
        "https://maven.minecraftforge.net/net/minecraftforge/ForgeAutoRenamingTool/1.1.2/ForgeAutoRenamingTool-1.1.2-all.jar",
        &tool,
        Some("1c0e9093bf1483039e3f342c4051cbbf9764d23c"),
    )?;
    let remapped = work.join("named.jar");
    execute(
        Command::new(java())
            .args(["-Xmx1G", "-jar"])
            .arg(tool)
            .arg("--input")
            .arg(work.join("game.jar"))
            .arg("--output")
            .arg(&remapped)
            .arg("--names")
            .arg(mappings)
            .args(["--reverse", "--threads", "2"]),
    )?;
    // Renaming invalidates the original jar signatures. Strip them only from
    // this local development copy; the downloaded server's hash is checked.
    let mut input =
        zip::ZipArchive::new(std::fs::File::open(&remapped).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let output = work.join("unsigned.jar");
    let mut out = zip::ZipWriter::new(std::fs::File::create(&output).map_err(|e| e.to_string())?);
    for i in 0..input.len() {
        let mut entry = input.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name();
        let upper = name.to_ascii_uppercase();
        if upper.starts_with("META-INF/")
            && (upper.ends_with(".SF")
                || upper.ends_with(".RSA")
                || upper.ends_with(".DSA")
                || upper == "META-INF/MANIFEST.MF")
        {
            continue;
        }
        if entry.is_dir() {
            continue;
        }
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![];
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        out.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    out.finish().map_err(|e| e.to_string())?;
    Ok(output)
}
