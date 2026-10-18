//! How `tools/entity-dump` is run, shared by the bake (`cargo xtask
//! bake-blocks`) and the launcher's runtime dump of a new 26.x client (plan
//! `schematic-studio`, T6/D4). Only the process start differs between the
//! two; the class, its arguments and the JSON kept afterwards are the same,
//! so the same jar gives the same bytes either way.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::EntityDump;
use crate::ContainerError;

/// The compiled dumper (`javac --release 21`), carried in the binary so the
/// launcher needs no JDK compiler and no files beside itself.
pub const DUMPER_CLASS: &[u8] =
    include_bytes!("../../../../tools/entity-dump/lumilio/entitydump/EntityDump.class");
/// The dumper's main class.
pub const DUMPER_MAIN: &str = "lumilio.entitydump.EntityDump";
/// Where the class goes below a class-path folder.
pub const DUMPER_CLASS_FILE: &str = "lumilio/entitydump/EntityDump.class";
/// The class file version the dumper is compiled for.
pub const DUMPER_JAVA: u32 = 21;
/// A class only an unobfuscated client jar (26.1 on) has under its own name.
pub const LAYER_DEFINITIONS_CLASS: &str = "net/minecraft/client/model/geom/LayerDefinitions.class";

/// Writes [`DUMPER_CLASS`] below `folder` unless the same bytes are there,
/// and returns `folder`, the class-path entry that holds it.
pub fn install_dumper(folder: &Path) -> io::Result<PathBuf> {
    let class = folder.join(DUMPER_CLASS_FILE);
    if fs::read(&class).is_ok_and(|bytes| bytes == DUMPER_CLASS) {
        return Ok(folder.to_owned());
    }
    let parent = class.parent().unwrap_or(folder);
    fs::create_dir_all(parent)?;
    let partial = class.with_extension(format!("partial-{}", std::process::id()));
    fs::write(&partial, DUMPER_CLASS)?;
    fs::rename(&partial, &class)?;
    Ok(folder.to_owned())
}

/// The arguments after the `java` executable: the dumper folder, then
/// `classpath` (the client jar and its libraries), the main class, the
/// output file and, for an obfuscated jar, the names file.
#[must_use]
pub fn dumper_arguments(
    dumper: &Path,
    classpath: &[PathBuf],
    out: &Path,
    names: Option<&Path>,
) -> Vec<OsString> {
    let separator = if cfg!(windows) { ";" } else { ":" };
    let mut joined = OsString::from(dumper.as_os_str());
    for path in classpath {
        joined.push(separator);
        joined.push(path.as_os_str());
    }
    let mut arguments: Vec<OsString> = vec![
        "-Djava.awt.headless=true".into(),
        "-cp".into(),
        joined,
        DUMPER_MAIN.into(),
        out.as_os_str().to_owned(),
    ];
    if let Some(names) = names {
        arguments.push(names.as_os_str().to_owned());
    }
    arguments
}

/// The exception and its causes from the dumper's standard error, without
/// stack frames or JVM warnings.
#[must_use]
pub fn dumper_failure(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.starts_with("Exception")
                || line.starts_with("Caused by")
                || line.starts_with("Error")
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Reads what the dumper wrote to `out` and returns it with its canonical
/// JSON ([`EntityDump::to_json`]), the bytes both callers keep.
pub fn read_dump(out: &Path) -> Result<(EntityDump, Vec<u8>), ContainerError> {
    let bytes = fs::read(out).map_err(|error| ContainerError::Io(error.to_string()))?;
    let dump = EntityDump::from_json(&bytes)?;
    let canonical = dump.to_json()?;
    Ok((dump, canonical))
}
