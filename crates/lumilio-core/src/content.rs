//! What is installed in an instance: mods, resource packs, shaders.
//!
//! Behavior notes: `docs/behavior/content.md`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use sha1::{Digest, Sha1};

use crate::discover::{ProjectKind, is_safe_file_name};

/// Suffix that switches a file off without deleting it.
pub(crate) const DISABLED_SUFFIX: &str = ".disabled";

/// Largest metadata entry read out of a mod archive.
const METADATA_LIMIT: u64 = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentItem {
    /// The name on disk, including a `.disabled` suffix when switched off.
    pub file_name: String,
    /// The name without the `.disabled` suffix.
    pub display_name: String,
    pub enabled: bool,
    pub size: u64,
    /// Seconds since the Unix epoch; 0 when unknown.
    pub modified: u64,
    /// A folder-style pack rather than an archive.
    pub is_directory: bool,
}

#[derive(Debug)]
pub enum ContentError {
    Io(io::Error),
    /// Modpacks are not a folder of files.
    NotAFileKind,
    UnsafeFileName(String),
    NotFound(String),
    /// The other state of this file already exists.
    Conflict(String),
    /// The kind's folder is a link, so its files may live outside the instance.
    EscapesInstance(String),
    /// A file of another type than this kind holds (a `.zip` offered as a mod).
    WrongType(String),
}

impl Display for ContentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "content storage failed: {error}"),
            Self::NotAFileKind => f.write_str("modpacks are installed as instances"),
            Self::UnsafeFileName(name) => write!(f, "unsafe file name {name:?}"),
            Self::NotFound(name) => write!(f, "{name:?} is not installed"),
            Self::Conflict(name) => write!(f, "{name:?} already exists"),
            Self::EscapesInstance(name) => {
                write!(f, "{name:?} is a link; it could lead outside the instance")
            }
            Self::WrongType(name) => write!(f, "{name:?} is not this kind of content"),
        }
    }
}

impl Error for ContentError {}

impl From<io::Error> for ContentError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// The folder of one kind inside the game directory. A folder that is itself a
/// link is refused: every operation would act on whatever it points at.
pub(crate) fn folder(game_dir: &Path, kind: ProjectKind) -> Result<PathBuf, ContentError> {
    let name = kind.install_folder().ok_or(ContentError::NotAFileKind)?;
    let path = game_dir.join(name);
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(ContentError::EscapesInstance(name.to_owned()))
        }
        _ => Ok(path),
    }
}

fn base_name(file_name: &str) -> (&str, bool) {
    match file_name.strip_suffix(DISABLED_SUFFIX) {
        Some(base) => (base, false),
        None => (file_name, true),
    }
}

fn is_content_file(kind: ProjectKind, base: &str) -> bool {
    let lower = base.to_ascii_lowercase();
    match kind {
        ProjectKind::Mod => lower.ends_with(".jar"),
        _ => lower.ends_with(".zip"),
    }
}

/// Lists the installed items of one kind, sorted by name (ignoring case).
///
/// Mods are `.jar` files; resource packs and shaders are `.zip` files or
/// folders. Anything else, and hidden files, is ignored. A missing folder is
/// an empty list.
pub fn scan(game_dir: &Path, kind: ProjectKind) -> Result<Vec<ContentItem>, ContentError> {
    let folder = folder(game_dir, kind)?;
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut items = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };
        if file_name.starts_with('.') {
            continue;
        }
        let metadata = entry.metadata()?;
        let (base, enabled) = base_name(&file_name);
        let is_directory = metadata.is_dir();
        let wanted = if is_directory {
            kind != ProjectKind::Mod
        } else {
            metadata.is_file() && is_content_file(kind, base)
        };
        if !wanted {
            continue;
        }
        items.push(ContentItem {
            display_name: base.to_owned(),
            enabled,
            size: if is_directory { 0 } else { metadata.len() },
            modified: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |elapsed| elapsed.as_secs()),
            is_directory,
            file_name,
        });
    }
    items.sort_by_key(|item| item.display_name.to_lowercase());
    Ok(items)
}

/// Copies a local file into the kind's folder (IA P-ADD-FILES). The file must
/// be of the kind's type; a file already there with the same content is
/// simply kept, and one with other content — in either state — is never
/// replaced (`Conflict`). The copy lands under a temporary name first, so a
/// half-written file is never visible. Returns the installed file name.
pub fn import(game_dir: &Path, kind: ProjectKind, source: &Path) -> Result<String, ContentError> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ContentError::UnsafeFileName(source.display().to_string()))?
        .to_owned();
    if !is_safe_file_name(&name) {
        return Err(ContentError::UnsafeFileName(name));
    }
    if !source.is_file() || !is_content_file(kind, &name) {
        return Err(ContentError::WrongType(name));
    }
    let folder = folder(game_dir, kind)?;
    fs::create_dir_all(&folder)?;
    let target = folder.join(&name);
    let disabled = folder.join(format!("{name}{DISABLED_SUFFIX}"));
    if target.symlink_metadata().is_ok() {
        return if sha1_hex(&target)? == sha1_hex(source)? {
            Ok(name)
        } else {
            Err(ContentError::Conflict(name))
        };
    }
    if disabled.symlink_metadata().is_ok() {
        return Err(ContentError::Conflict(name));
    }
    let partial = folder.join(format!(".{name}.part"));
    fs::copy(source, &partial)?;
    if let Err(error) = fs::rename(&partial, &target) {
        let _ = fs::remove_file(&partial);
        return Err(error.into());
    }
    Ok(name)
}

fn existing(game_dir: &Path, kind: ProjectKind, file_name: &str) -> Result<PathBuf, ContentError> {
    if !is_safe_file_name(file_name) {
        return Err(ContentError::UnsafeFileName(file_name.to_owned()));
    }
    let path = folder(game_dir, kind)?.join(file_name);
    if path.symlink_metadata().is_err() {
        return Err(ContentError::NotFound(file_name.to_owned()));
    }
    Ok(path)
}

/// Switches an item on or off by adding or removing the `.disabled` suffix.
/// Switching to the state it is already in changes nothing. Returns the file
/// name after the change.
pub fn set_enabled(
    game_dir: &Path,
    kind: ProjectKind,
    file_name: &str,
    enabled: bool,
) -> Result<String, ContentError> {
    let path = existing(game_dir, kind, file_name)?;
    let (base, currently_enabled) = base_name(file_name);
    if currently_enabled == enabled {
        return Ok(file_name.to_owned());
    }
    let target_name = if enabled {
        base.to_owned()
    } else {
        format!("{file_name}{DISABLED_SUFFIX}")
    };
    let target = path.with_file_name(&target_name);
    if target.symlink_metadata().is_ok() {
        return Err(ContentError::Conflict(target_name));
    }
    fs::rename(path, target)?;
    Ok(target_name)
}

/// Deletes an item (a folder pack is removed recursively).
pub fn remove(game_dir: &Path, kind: ProjectKind, file_name: &str) -> Result<(), ContentError> {
    let path = existing(game_dir, kind, file_name)?;
    let metadata = path.symlink_metadata()?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// SHA-1 of a file, as lowercase hex — the key Modrinth identifies files by.
pub fn sha1_hex(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha1::new();
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

/// What a mod archive says about itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModMetadata {
    pub id: String,
    pub name: Option<String>,
    pub version: Option<String>,
    /// `fabric`, `quilt`, `forge` or `neoforge`.
    pub loader: &'static str,
}

fn read_entry(archive: &mut zip::ZipArchive<fs::File>, name: &str) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    if entry.size() > METADATA_LIMIT {
        return None;
    }
    let mut text = String::new();
    entry.take(METADATA_LIMIT).read_to_string(&mut text).ok()?;
    Some(text)
}

#[derive(Deserialize)]
struct FabricManifest {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
struct QuiltManifest {
    quilt_loader: Option<QuiltLoader>,
}

#[derive(Deserialize)]
struct QuiltLoader {
    id: Option<String>,
    version: Option<String>,
    metadata: Option<QuiltMeta>,
}

#[derive(Deserialize)]
struct QuiltMeta {
    name: Option<String>,
}

#[derive(Deserialize)]
struct ForgeManifest {
    #[serde(default)]
    mods: Vec<ForgeMod>,
}

#[derive(Deserialize)]
struct ForgeMod {
    #[serde(rename = "modId")]
    mod_id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    version: Option<String>,
}

/// A version still holding a build-time placeholder (`${file.jarVersion}`) is
/// not a version.
fn real_version(version: Option<String>) -> Option<String> {
    version.filter(|v| !v.trim().is_empty() && !v.contains("${"))
}

/// Reads a mod's identity from its archive: `fabric.mod.json`,
/// `quilt.mod.json`, `META-INF/neoforge.mods.toml` or `META-INF/mods.toml`,
/// in that order. `None` when the archive is unreadable or declares nothing.
#[must_use]
pub fn read_mod_metadata(jar: &Path) -> Option<ModMetadata> {
    let mut archive = zip::ZipArchive::new(fs::File::open(jar).ok()?).ok()?;

    if let Some(text) = read_entry(&mut archive, "fabric.mod.json")
        && let Ok(manifest) = serde_json::from_str::<FabricManifest>(&text)
        && let Some(id) = manifest.id
    {
        return Some(ModMetadata {
            id,
            name: manifest.name,
            version: real_version(manifest.version),
            loader: "fabric",
        });
    }
    if let Some(text) = read_entry(&mut archive, "quilt.mod.json")
        && let Ok(manifest) = serde_json::from_str::<QuiltManifest>(&text)
        && let Some(loader) = manifest.quilt_loader
        && let Some(id) = loader.id
    {
        return Some(ModMetadata {
            id,
            name: loader.metadata.and_then(|meta| meta.name),
            version: real_version(loader.version),
            loader: "quilt",
        });
    }
    for (entry, loader) in [
        ("META-INF/neoforge.mods.toml", "neoforge"),
        ("META-INF/mods.toml", "forge"),
    ] {
        if let Some(text) = read_entry(&mut archive, entry)
            && let Ok(manifest) = toml::from_str::<ForgeManifest>(&text)
            && let Some(first) = manifest.mods.into_iter().next()
            && let Some(id) = first.mod_id
        {
            return Some(ModMetadata {
                id,
                name: first.display_name,
                version: real_version(first.version),
                loader,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn importing_copies_once_and_never_replaces_other_content() {
        let game = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let jar = outside.path().join("sodium.jar");
        fs::write(&jar, b"mod").unwrap();
        assert_eq!(
            import(game.path(), ProjectKind::Mod, &jar).unwrap(),
            "sodium.jar"
        );
        assert_eq!(
            fs::read(game.path().join("mods/sodium.jar")).unwrap(),
            b"mod"
        );
        // The same content again is fine; other content under that name is not.
        assert!(import(game.path(), ProjectKind::Mod, &jar).is_ok());
        fs::write(&jar, b"other").unwrap();
        assert!(matches!(
            import(game.path(), ProjectKind::Mod, &jar),
            Err(ContentError::Conflict(_))
        ));
        assert_eq!(
            fs::read(game.path().join("mods/sodium.jar")).unwrap(),
            b"mod"
        );
        // A disabled copy blocks the name too.
        let other = outside.path().join("lithium.jar");
        fs::write(&other, b"l").unwrap();
        fs::write(game.path().join("mods/lithium.jar.disabled"), b"l").unwrap();
        assert!(matches!(
            import(game.path(), ProjectKind::Mod, &other),
            Err(ContentError::Conflict(_))
        ));
        // Wrong type for the kind.
        let pack = outside.path().join("pack.zip");
        fs::write(&pack, b"zip").unwrap();
        assert!(matches!(
            import(game.path(), ProjectKind::Mod, &pack),
            Err(ContentError::WrongType(_))
        ));
        assert_eq!(
            import(game.path(), ProjectKind::ResourcePack, &pack).unwrap(),
            "pack.zip"
        );
        let leftovers: Vec<_> = fs::read_dir(game.path().join("mods"))
            .unwrap()
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| name.ends_with(".part"))
            .collect();
        assert!(leftovers.is_empty());
    }
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn write_zip(path: &Path, entries: &[(&str, &str)]) {
        let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, body) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    fn game() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("mods")).unwrap();
        (dir, game)
    }

    #[test]
    fn scans_mods_with_state_sizes_and_stable_order() {
        let (_dir, game) = game();
        fs::write(game.join("mods/b.jar"), "12345").unwrap();
        fs::write(game.join("mods/A.jar.disabled"), "1").unwrap();
        fs::write(game.join("mods/readme.txt"), "x").unwrap();
        fs::write(game.join("mods/.DS_Store"), "x").unwrap();
        fs::create_dir(game.join("mods/some-folder")).unwrap();
        let items = scan(&game, ProjectKind::Mod).unwrap();
        let names: Vec<_> = items
            .iter()
            .map(|i| (i.display_name.as_str(), i.enabled))
            .collect();
        assert_eq!(names, [("A.jar", false), ("b.jar", true)]);
        assert_eq!(items[1].size, 5);
        assert_eq!(items[0].file_name, "A.jar.disabled");
    }

    #[test]
    fn resource_packs_may_be_folders_and_a_missing_folder_is_empty() {
        let (_dir, game) = game();
        assert!(scan(&game, ProjectKind::ResourcePack).unwrap().is_empty());
        fs::create_dir_all(game.join("resourcepacks/folder-pack")).unwrap();
        fs::write(game.join("resourcepacks/pack.zip"), "z").unwrap();
        fs::write(game.join("resourcepacks/pack.rar"), "z").unwrap();
        let items = scan(&game, ProjectKind::ResourcePack).unwrap();
        assert_eq!(items.len(), 2);
        assert!(
            items
                .iter()
                .any(|i| i.is_directory && i.display_name == "folder-pack")
        );
        assert!(matches!(
            scan(&game, ProjectKind::Modpack),
            Err(ContentError::NotAFileKind)
        ));
    }

    #[test]
    fn disable_and_enable_rename_and_are_idempotent() {
        let (_dir, game) = game();
        fs::write(game.join("mods/a.jar"), "x").unwrap();
        assert_eq!(
            set_enabled(&game, ProjectKind::Mod, "a.jar", false).unwrap(),
            "a.jar.disabled"
        );
        assert!(game.join("mods/a.jar.disabled").is_file());
        assert_eq!(
            set_enabled(&game, ProjectKind::Mod, "a.jar.disabled", false).unwrap(),
            "a.jar.disabled"
        );
        assert_eq!(
            set_enabled(&game, ProjectKind::Mod, "a.jar.disabled", true).unwrap(),
            "a.jar"
        );
        assert!(game.join("mods/a.jar").is_file());
    }

    #[test]
    fn enabling_refuses_to_overwrite_a_twin() {
        let (_dir, game) = game();
        fs::write(game.join("mods/a.jar"), "new").unwrap();
        fs::write(game.join("mods/a.jar.disabled"), "old").unwrap();
        assert!(matches!(
            set_enabled(&game, ProjectKind::Mod, "a.jar.disabled", true),
            Err(ContentError::Conflict(_))
        ));
        assert_eq!(fs::read_to_string(game.join("mods/a.jar")).unwrap(), "new");
    }

    #[test]
    fn removal_deletes_files_and_folders_and_rejects_bad_names() {
        let (_dir, game) = game();
        fs::write(game.join("mods/a.jar"), "x").unwrap();
        fs::write(game.join("outside.jar"), "keep").unwrap();
        remove(&game, ProjectKind::Mod, "a.jar").unwrap();
        assert!(!game.join("mods/a.jar").exists());
        assert!(matches!(
            remove(&game, ProjectKind::Mod, "a.jar"),
            Err(ContentError::NotFound(_))
        ));
        for name in ["../outside.jar", "a/b.jar", ".."] {
            assert!(matches!(
                remove(&game, ProjectKind::Mod, name),
                Err(ContentError::UnsafeFileName(_))
            ));
        }
        assert!(game.join("outside.jar").is_file());
        fs::create_dir_all(game.join("shaderpacks/dir/inner")).unwrap();
        remove(&game, ProjectKind::Shader, "dir").unwrap();
        assert!(!game.join("shaderpacks/dir").exists());
    }

    #[test]
    fn a_linked_kind_folder_is_refused_and_a_linked_file_is_only_itself() {
        let (dir, game) = game();
        let outside = dir.path().join("elsewhere");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("victim.jar"), "keep").unwrap();
        std::os::unix::fs::symlink(&outside, game.join("resourcepacks")).unwrap();
        for result in [
            scan(&game, ProjectKind::ResourcePack).map(|_| ()),
            remove(&game, ProjectKind::ResourcePack, "victim.jar"),
            set_enabled(&game, ProjectKind::ResourcePack, "victim.jar", false).map(|_| ()),
        ] {
            assert!(matches!(result, Err(ContentError::EscapesInstance(_))));
        }
        assert_eq!(fs::read(outside.join("victim.jar")).unwrap(), b"keep");
        // A link inside a legitimate folder is just a name: acting on it never
        // reaches through to the target.
        std::os::unix::fs::symlink(outside.join("victim.jar"), game.join("mods/link.jar")).unwrap();
        remove(&game, ProjectKind::Mod, "link.jar").unwrap();
        assert_eq!(fs::read(outside.join("victim.jar")).unwrap(), b"keep");
    }

    #[test]
    fn sha1_matches_a_known_digest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        fs::write(&path, "abc").unwrap();
        assert_eq!(
            sha1_hex(&path).unwrap(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn reads_metadata_for_each_loader_family() {
        let dir = tempfile::tempdir().unwrap();
        let fabric = dir.path().join("f.jar");
        write_zip(
            &fabric,
            &[(
                "fabric.mod.json",
                r#"{"id":"sodium","name":"Sodium","version":"0.6.0"}"#,
            )],
        );
        assert_eq!(
            read_mod_metadata(&fabric).unwrap(),
            ModMetadata {
                id: "sodium".into(),
                name: Some("Sodium".into()),
                version: Some("0.6.0".into()),
                loader: "fabric"
            }
        );

        let quilt = dir.path().join("q.jar");
        write_zip(
            &quilt,
            &[(
                "quilt.mod.json",
                r#"{"quilt_loader":{"id":"qm","version":"1.0","metadata":{"name":"Q"}}}"#,
            )],
        );
        let meta = read_mod_metadata(&quilt).unwrap();
        assert_eq!(
            (meta.id.as_str(), meta.loader, meta.name.as_deref()),
            ("qm", "quilt", Some("Q"))
        );

        let forge = dir.path().join("g.jar");
        write_zip(
            &forge,
            &[(
                "META-INF/mods.toml",
                "modLoader=\"javafml\"\n[[mods]]\nmodId=\"jei\"\ndisplayName=\"JEI\"\nversion=\"${file.jarVersion}\"\n",
            )],
        );
        let meta = read_mod_metadata(&forge).unwrap();
        assert_eq!((meta.id.as_str(), meta.loader), ("jei", "forge"));
        assert_eq!(meta.version, None, "a build placeholder is not a version");

        let neo = dir.path().join("n.jar");
        write_zip(
            &neo,
            &[(
                "META-INF/neoforge.mods.toml",
                "[[mods]]\nmodId=\"nm\"\nversion=\"2.0\"\n",
            )],
        );
        assert_eq!(read_mod_metadata(&neo).unwrap().loader, "neoforge");
    }

    #[test]
    fn unreadable_or_anonymous_archives_have_no_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.jar");
        fs::write(&bad, "not a zip").unwrap();
        assert!(read_mod_metadata(&bad).is_none());
        let empty = dir.path().join("e.jar");
        write_zip(&empty, &[("readme.txt", "hi")]);
        assert!(read_mod_metadata(&empty).is_none());
        assert!(read_mod_metadata(&dir.path().join("missing.jar")).is_none());
        let no_id = dir.path().join("n.jar");
        write_zip(&no_id, &[("fabric.mod.json", r#"{"name":"x"}"#)]);
        assert!(read_mod_metadata(&no_id).is_none());
    }
}
