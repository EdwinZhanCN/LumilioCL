//! Exporting an instance as a Modrinth modpack (`.mrpack`).
//!
//! The pack is a zip with `modrinth.index.json`, the files Modrinth can serve
//! listed there with their hashes and addresses, and every other chosen file
//! under `overrides/`. The planning is plain functions over file names and
//! hashes so it can be tested without a network; the service does the looking
//! up. The format is the public Modrinth pack format that `modpack` imports.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use serde_json::json;
use sha2::{Digest, Sha512};

use crate::discover::Version;
use crate::instance::Loader;
use crate::modpack::{is_safe_relative, is_trusted_source};

const INDEX: &str = "modrinth.index.json";
/// The folders whose files Modrinth may know, and so may be linked.
const LINKABLE_FOLDERS: [&str; 3] = ["mods", "resourcepacks", "shaderpacks"];
/// The game's lock file; it means nothing outside a running game.
const LOCK_FILE: &str = "session.lock";

/// Which pack format to write.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PackFormat {
    /// Modrinth `.mrpack`: files Modrinth knows are listed by address.
    #[default]
    Modrinth,
    /// A MultiMC / Prism instance in a zip: every chosen file is carried.
    Prism,
}

impl PackFormat {
    /// The extension a saved pack of this format has.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Modrinth => "mrpack",
            Self::Prism => "zip",
        }
    }
}

/// What the person chose for the pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportSpec {
    pub format: PackFormat,
    pub name: String,
    pub version: String,
    pub summary: Option<String>,
    /// Files and folders of the game directory to include, `/`-separated.
    pub include: Vec<String>,
}

/// A file the pack lists by address instead of carrying.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkedFile {
    pub path: String,
    pub sha1: String,
    pub sha512: String,
    pub size: u64,
    pub url: String,
}

/// What an export came to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportReport {
    /// Files listed by address.
    pub linked: usize,
    /// Files carried inside the pack.
    pub bundled: usize,
    /// Modrinth could not be asked, so everything was carried.
    pub lookup_failed: bool,
    /// The pack's size in bytes.
    pub size: u64,
}

#[derive(Debug)]
pub enum ExportError {
    Io(io::Error),
    Zip(String),
    /// The pack needs a name and a version.
    MissingDetails,
    /// A chosen path leaves the game directory.
    UnsafePath(String),
    /// A pack with a mod loader must say which version of it.
    NoLoaderVersion,
    NothingChosen,
    Cancelled,
}

impl Display for ExportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "modpack export failed: {error}"),
            Self::Zip(message) => write!(f, "modpack archive error: {message}"),
            Self::MissingDetails => f.write_str("the pack needs a name and a version"),
            Self::UnsafePath(path) => write!(f, "{path:?} is not inside the game directory"),
            Self::NoLoaderVersion => {
                f.write_str("the game has no loader version to put in the pack")
            }
            Self::NothingChosen => f.write_str("nothing is chosen for the pack"),
            Self::Cancelled => f.write_str("the export was cancelled"),
        }
    }
}

impl Error for ExportError {}

impl From<io::Error> for ExportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<zip::result::ZipError> for ExportError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error.to_string())
    }
}

/// Every file under the chosen paths, relative to `game_dir`, sorted and
/// without repeats. Links are skipped rather than followed.
pub fn collect_files(game_dir: &Path, include: &[String]) -> Result<Vec<String>, ExportError> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeSet<String>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let path = format!("{prefix}/{name}");
            let kind = entry.file_type()?;
            if kind.is_dir() {
                walk(&entry.path(), &path, out)?;
            } else if kind.is_file() && name != LOCK_FILE {
                out.insert(path);
            }
        }
        Ok(())
    }
    let mut files = BTreeSet::new();
    for chosen in include {
        let raw = chosen.as_str();
        let chosen = raw.trim_end_matches('/');
        if raw.starts_with('/') || !is_safe_relative(chosen) {
            return Err(ExportError::UnsafePath(chosen.to_owned()));
        }
        let path = game_dir.join(chosen);
        let Ok(metadata) = path.symlink_metadata() else {
            continue;
        };
        if metadata.is_dir() {
            walk(&path, chosen, &mut files)?;
        } else if metadata.is_file() && !chosen.ends_with(LOCK_FILE) {
            files.insert(chosen.to_owned());
        }
    }
    if files.is_empty() {
        return Err(ExportError::NothingChosen);
    }
    Ok(files.into_iter().collect())
}

/// The files Modrinth might know: archives directly inside `mods/`,
/// `resourcepacks/` or `shaderpacks/` that are switched on.
#[must_use]
pub fn linkable(files: &[String]) -> Vec<&String> {
    files
        .iter()
        .filter(|path| {
            let mut parts = path.split('/');
            let (Some(folder), Some(name), None) = (parts.next(), parts.next(), parts.next())
            else {
                return false;
            };
            LINKABLE_FOLDERS.contains(&folder) && (name.ends_with(".jar") || name.ends_with(".zip"))
        })
        .collect()
}

/// Matches files Modrinth identified (by SHA-1) with the address to fetch
/// them from. A file whose address is not a trusted source, or that differs
/// from what Modrinth lists, stays out and is carried instead.
#[must_use]
pub fn link_identified(
    hashed: &[(String, String, String, u64)],
    identified: &BTreeMap<String, Version>,
) -> Vec<LinkedFile> {
    hashed
        .iter()
        .filter_map(|(path, sha1, sha512, size)| {
            let version = identified.get(sha1)?;
            let file = version
                .files
                .iter()
                .find(|file| file.sha1.as_deref() == Some(sha1.as_str()))?;
            is_trusted_source(&file.url).then(|| LinkedFile {
                path: path.clone(),
                sha1: sha1.clone(),
                sha512: sha512.clone(),
                size: *size,
                url: file.url.clone(),
            })
        })
        .collect()
}

pub fn sha512_hex(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha512::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// `modrinth.index.json` for a pack of this game.
pub fn index_json(
    spec: &ExportSpec,
    minecraft: &str,
    loader: Loader,
    loader_version: Option<&str>,
    linked: &[LinkedFile],
) -> Result<String, ExportError> {
    if spec.name.trim().is_empty() || spec.version.trim().is_empty() {
        return Err(ExportError::MissingDetails);
    }
    let mut dependencies = BTreeMap::new();
    dependencies.insert("minecraft".to_owned(), minecraft.to_owned());
    let key = match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("fabric-loader"),
        Loader::Quilt => Some("quilt-loader"),
        Loader::Forge => Some("forge"),
        Loader::NeoForge => Some("neoforge"),
    };
    if let Some(key) = key {
        let version = loader_version
            .filter(|version| !version.is_empty())
            .ok_or(ExportError::NoLoaderVersion)?;
        dependencies.insert(key.to_owned(), version.to_owned());
    }
    let files: Vec<_> = linked
        .iter()
        .map(|file| {
            json!({
                "path": file.path,
                "hashes": { "sha1": file.sha1, "sha512": file.sha512 },
                "env": { "client": "required", "server": "required" },
                "downloads": [file.url],
                "fileSize": file.size,
            })
        })
        .collect();
    let mut index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": spec.version.trim(),
        "name": spec.name.trim(),
        "files": files,
        "dependencies": dependencies,
    });
    if let Some(summary) = spec
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        index["summary"] = json!(summary);
    }
    serde_json::to_string_pretty(&index).map_err(|error| ExportError::Zip(error.to_string()))
}

/// The `mmc-pack.json` of a MultiMC / Prism instance.
pub fn prism_pack_json(
    minecraft: &str,
    loader: Loader,
    loader_version: Option<&str>,
) -> Result<String, ExportError> {
    let mut components =
        vec![json!({"uid": "net.minecraft", "version": minecraft, "important": true})];
    let uid = match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("net.fabricmc.fabric-loader"),
        Loader::Quilt => Some("org.quiltmc.quilt-loader"),
        Loader::Forge => Some("net.minecraftforge"),
        Loader::NeoForge => Some("net.neoforged"),
    };
    if let Some(uid) = uid {
        let version = loader_version
            .filter(|version| !version.is_empty())
            .ok_or(ExportError::NoLoaderVersion)?;
        components.push(json!({"uid": uid, "version": version}));
    }
    serde_json::to_string_pretty(&json!({"components": components, "formatVersion": 1}))
        .map_err(|error| ExportError::Zip(error.to_string()))
}

/// Writes a MultiMC / Prism instance zip: `instance.cfg`, `mmc-pack.json` and
/// every chosen file under `.minecraft/`, through a `.part` file.
pub fn write_prism(
    destination: &Path,
    game_dir: &Path,
    name: &str,
    pack_json: &str,
    files: &[String],
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, ExportError> {
    let part = destination.with_extension("zip.part");
    let written = (|| {
        let mut writer = zip::ZipWriter::new(fs::File::create(&part)?);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        writer.start_file("instance.cfg", options)?;
        io::Write::write_all(
            &mut writer,
            format!(
                "InstanceType=OneSix\nname={}\n",
                name.replace(['\n', '\r'], " ")
            )
            .as_bytes(),
        )?;
        writer.start_file("mmc-pack.json", options)?;
        io::Write::write_all(&mut writer, pack_json.as_bytes())?;
        for path in files {
            if cancelled() {
                return Err(ExportError::Cancelled);
            }
            writer.start_file(format!(".minecraft/{path}"), options)?;
            io::copy(&mut fs::File::open(game_dir.join(path))?, &mut writer)?;
        }
        writer.finish()?;
        fs::rename(&part, destination)?;
        Ok::<_, ExportError>(fs::metadata(destination)?.len())
    })();
    if written.is_err() {
        let _ = fs::remove_file(&part);
    }
    written
}

/// Writes the pack: the index, then each carried file under `overrides/`,
/// through a `.part` file renamed at the end. `cancelled` is asked between
/// files; a cancelled export leaves nothing. Returns the pack's size.
pub fn write_pack(
    destination: &Path,
    game_dir: &Path,
    index: &str,
    carried: &[String],
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, ExportError> {
    let part = destination.with_extension("mrpack.part");
    let written = (|| {
        let mut writer = zip::ZipWriter::new(fs::File::create(&part)?);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        writer.start_file(INDEX, options)?;
        io::Write::write_all(&mut writer, index.as_bytes())?;
        for path in carried {
            if cancelled() {
                return Err(ExportError::Cancelled);
            }
            writer.start_file(format!("overrides/{path}"), options)?;
            io::copy(&mut fs::File::open(game_dir.join(path))?, &mut writer)?;
        }
        writer.finish()?;
        fs::rename(&part, destination)?;
        Ok::<_, ExportError>(fs::metadata(destination)?.len())
    })();
    if written.is_err() {
        let _ = fs::remove_file(&part);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::{ReleaseChannel, VersionFile};
    use crate::modpack::{extract_overrides, parse_index, read_index};

    fn spec(include: &[&str]) -> ExportSpec {
        ExportSpec {
            format: PackFormat::Modrinth,
            name: " My Pack ".into(),
            version: "1.0.0".into(),
            summary: Some("  hello ".into()),
            include: include.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn game() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        for (path, data) in [
            ("mods/sodium.jar", "jar"),
            ("mods/local.jar", "mine"),
            ("mods/off.jar.disabled", "off"),
            ("config/sodium.json", "{}"),
            ("config/deep/a.toml", "a"),
            ("saves/w/level.dat", "world"),
            ("options.txt", "fov:70"),
        ] {
            let path = game.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, data).unwrap();
        }
        fs::write(game.join("saves/w/session.lock"), "x").unwrap();
        (dir, game)
    }

    #[test]
    fn only_the_chosen_files_are_collected_and_nothing_escapes() {
        let (_dir, game) = game();
        let files = collect_files(
            &game,
            &["mods".into(), "config/".into(), "options.txt".into()],
        )
        .unwrap();
        assert_eq!(
            files,
            [
                "config/deep/a.toml",
                "config/sodium.json",
                "mods/local.jar",
                "mods/off.jar.disabled",
                "mods/sodium.jar",
                "options.txt",
            ]
        );
        let with_world = collect_files(&game, &["saves".into()]).unwrap();
        assert_eq!(with_world, ["saves/w/level.dat"], "the lock file stays out");
        for bad in ["../outside", "/etc", "mods/../../x"] {
            assert!(matches!(
                collect_files(&game, &[bad.into()]),
                Err(ExportError::UnsafePath(_))
            ));
        }
        assert!(matches!(
            collect_files(&game, &["nothing-here".into()]),
            Err(ExportError::NothingChosen)
        ));
    }

    #[test]
    fn only_switched_on_archives_in_the_content_folders_can_be_linked() {
        let files: Vec<String> = [
            "mods/a.jar",
            "mods/b.jar.disabled",
            "mods/sub/c.jar",
            "resourcepacks/p.zip",
            "shaderpacks/s.zip",
            "config/x.zip",
            "options.txt",
        ]
        .map(String::from)
        .into();
        let linkable: Vec<_> = linkable(&files).into_iter().map(String::as_str).collect();
        assert_eq!(
            linkable,
            ["mods/a.jar", "resourcepacks/p.zip", "shaderpacks/s.zip"]
        );
    }

    fn version(sha1: &str, url: &str) -> Version {
        Version {
            id: "v".into(),
            project_id: "p".into(),
            name: "n".into(),
            number: "1".into(),
            channel: ReleaseChannel::Release,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            published: String::new(),
            files: vec![VersionFile {
                url: url.into(),
                filename: "f.jar".into(),
                primary: true,
                size: 3,
                sha1: Some(sha1.into()),
            }],
            dependencies: Vec::new(),
            downloads: 0,
            changelog: String::new(),
        }
    }

    #[test]
    fn identified_files_are_linked_only_from_trusted_addresses() {
        let hashed = vec![
            ("mods/a.jar".to_owned(), "aa".to_owned(), "AA".to_owned(), 3),
            ("mods/b.jar".to_owned(), "bb".to_owned(), "BB".to_owned(), 3),
            ("mods/c.jar".to_owned(), "cc".to_owned(), "CC".to_owned(), 3),
        ];
        let identified = BTreeMap::from([
            (
                "aa".to_owned(),
                version("aa", "https://cdn.modrinth.com/data/a.jar"),
            ),
            ("bb".to_owned(), version("bb", "https://evil.example/b.jar")),
            // Modrinth names a file with another hash: not this one.
            (
                "cc".to_owned(),
                version("zz", "https://cdn.modrinth.com/data/c.jar"),
            ),
        ]);
        let linked = link_identified(&hashed, &identified);
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].path, "mods/a.jar");
        assert_eq!(linked[0].url, "https://cdn.modrinth.com/data/a.jar");
    }

    #[test]
    fn the_index_reads_back_through_the_importer() {
        let linked = vec![LinkedFile {
            path: "mods/a.jar".into(),
            sha1: "aa".into(),
            sha512: "AA".into(),
            size: 3,
            url: "https://cdn.modrinth.com/data/a.jar".into(),
        }];
        let json = index_json(
            &spec(&[]),
            "1.21.1",
            Loader::Fabric,
            Some("0.16.0"),
            &linked,
        )
        .unwrap();
        let index = parse_index(&json).unwrap();
        assert_eq!(index.name, "My Pack");
        assert_eq!(index.version, "1.0.0");
        assert_eq!(index.summary.as_deref(), Some("hello"));
        assert_eq!(index.minecraft, "1.21.1");
        assert_eq!(
            (index.loader, index.loader_version.as_deref()),
            (Loader::Fabric, Some("0.16.0"))
        );
        assert_eq!(index.files.len(), 1);
        assert_eq!(
            index.files[0].downloads,
            ["https://cdn.modrinth.com/data/a.jar"]
        );

        let vanilla = index_json(&spec(&[]), "1.21.1", Loader::Vanilla, None, &[]).unwrap();
        assert_eq!(parse_index(&vanilla).unwrap().loader, Loader::Vanilla);
        assert!(matches!(
            index_json(&spec(&[]), "1.21.1", Loader::Forge, None, &[]),
            Err(ExportError::NoLoaderVersion)
        ));
        let mut unnamed = spec(&[]);
        unnamed.name = "  ".into();
        assert!(matches!(
            index_json(&unnamed, "1", Loader::Vanilla, None, &[]),
            Err(ExportError::MissingDetails)
        ));
    }

    #[test]
    fn a_written_pack_restores_its_carried_files_through_the_importer() {
        let (dir, game) = game();
        let carried = vec!["config/sodium.json".to_owned(), "mods/local.jar".to_owned()];
        let index = index_json(&spec(&[]), "1.21.1", Loader::Vanilla, None, &[]).unwrap();
        let pack = dir.path().join("out.mrpack");
        let size = write_pack(&pack, &game, &index, &carried, &|| false).unwrap();
        assert!(size > 0 && !dir.path().join("out.mrpack.part").exists());

        assert_eq!(read_index(&pack).unwrap().name, "My Pack");
        let restored = dir.path().join("restored");
        fs::create_dir_all(&restored).unwrap();
        assert_eq!(extract_overrides(&pack, &restored).unwrap(), 2);
        assert_eq!(fs::read(restored.join("mods/local.jar")).unwrap(), b"mine");
        assert_eq!(
            fs::read(restored.join("config/sodium.json")).unwrap(),
            b"{}"
        );

        // A failure leaves neither a pack nor a half pack.
        let missing = vec!["mods/ghost.jar".to_owned()];
        let broken = dir.path().join("broken.mrpack");
        assert!(write_pack(&broken, &game, &index, &missing, &|| false).is_err());
        assert!(!broken.exists() && !dir.path().join("broken.mrpack.part").exists());
    }

    #[test]
    fn a_cancelled_export_leaves_neither_a_pack_nor_a_half_pack() {
        let (dir, game) = game();
        let index = index_json(&spec(&[]), "1.21.1", Loader::Vanilla, None, &[]).unwrap();
        let pack = dir.path().join("never.mrpack");
        let carried = vec!["config/sodium.json".to_owned(), "mods/local.jar".to_owned()];
        assert!(matches!(
            write_pack(&pack, &game, &index, &carried, &|| true),
            Err(ExportError::Cancelled)
        ));
        assert!(!pack.exists() && !dir.path().join("never.mrpack.part").exists());
    }

    #[test]
    fn a_prism_zip_reads_back_through_the_importer_of_other_launchers() {
        let (dir, game) = game();
        let pack = prism_pack_json("1.21.1", Loader::Fabric, Some("0.16.0")).unwrap();
        let files = vec!["config/sodium.json".to_owned(), "mods/local.jar".to_owned()];
        let zip = dir.path().join("p.zip");
        let size = write_prism(&zip, &game, "My\nPack", &pack, &files, &|| false).unwrap();
        assert!(size > 0 && !dir.path().join("p.zip.part").exists());

        let unpacked = dir.path().join("unpacked");
        let mut archive = zip::ZipArchive::new(fs::File::open(&zip).unwrap()).unwrap();
        archive.extract(&unpacked).unwrap();
        let found = crate::import_game::detect(&unpacked).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "My Pack");
        assert_eq!(
            (found[0].game_version.as_str(), found[0].loader),
            ("1.21.1", Loader::Fabric)
        );
        assert_eq!(found[0].loader_version.as_deref(), Some("0.16.0"));
        assert_eq!(
            fs::read(unpacked.join(".minecraft/mods/local.jar")).unwrap(),
            b"mine"
        );

        assert!(matches!(
            prism_pack_json("1.21.1", Loader::Forge, None),
            Err(ExportError::NoLoaderVersion)
        ));
        assert!(matches!(
            write_prism(
                &dir.path().join("c.zip"),
                &game,
                "x",
                &pack,
                &files,
                &|| true
            ),
            Err(ExportError::Cancelled)
        ));
        assert!(!dir.path().join("c.zip").exists() && !dir.path().join("c.zip.part").exists());
    }

    #[test]
    fn hashes_match_known_digests() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a");
        fs::write(&file, b"abc").unwrap();
        assert_eq!(
            sha512_hex(&file).unwrap(),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
    }
}
