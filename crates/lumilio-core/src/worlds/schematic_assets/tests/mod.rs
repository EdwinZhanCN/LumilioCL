mod bundle;
mod cache;
mod data_version;
mod entities;
mod overlays;
mod rules;
mod scan;
mod sources;
mod switches;

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use lumilio_block_assets::{AssetSource, BakedPack};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// A zip at `path` holding `entries`.
fn zip_at(path: &Path, entries: &[(&str, &[u8])]) -> PathBuf {
    let mut writer = ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
    path.to_owned()
}

/// The vanilla pack of the client `jar`, as the bundle's bake makes one.
fn baked(jar: &Path) -> BakedPack {
    lumilio_block_assets::bake(&[AssetSource::Archive(jar.to_owned())]).unwrap()
}

/// A client jar of `version` with one block, `block` (stone by default).
fn client_jar(path: &Path, version: &str, block: &str) -> PathBuf {
    let version_json = format!(r#"{{"id":"{version}","world_version":5023}}"#);
    let blockstate = format!(r#"{{"variants":{{"":{{"model":"minecraft:block/{block}"}}}}}}"#);
    zip_at(
        path,
        &[
            ("version.json", version_json.as_bytes()),
            ("net/minecraft/Main.class", b"code"),
            (
                &format!("assets/minecraft/blockstates/{block}.json"),
                blockstate.as_bytes(),
            ),
            (
                &format!("assets/minecraft/models/block/{block}.json"),
                br#"{"textures":{"all":"minecraft:block/stone"}}"#,
            ),
        ],
    )
}
