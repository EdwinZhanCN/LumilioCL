//! Entity geometry for a preview (plan schematic-studio, T6/D4, D23): the
//! bundle's `entities.dat.z` for the preview's game version. The geometry is
//! dumped from Mojang's jars only when the bundle is baked (`cargo xtask
//! bake-blocks`); the launcher never runs the dumper itself.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use lumilio_block_assets::{dumper_arguments, dumper_failure, read_dump};

use super::SchematicAssetsError;
use crate::process::background_command;

/// A dump that runs longer is stopped: the 26.3 dump takes seconds.
const DUMP_TIME_LIMIT: Duration = Duration::from_secs(180);

/// Where a preview's entity geometry comes from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EntityGeometry {
    /// No geometry: the bundle is out of reach, or covers no entities.
    #[default]
    None,
    /// This version's layers in the bundle's `entities.dat.z`.
    Bundle { version: String },
}

/// Runs the dumper (`dumper` is the folder `install_dumper` returned) with
/// `java` and `classpath`, writes the canonical JSON to `out` and returns it.
/// Standard output and error go to `log`; `work` is the working folder the
/// game's logger writes into.
///
/// The launcher no longer dumps (D23): this is kept only for the bake's
/// byte-equality test in `lumilio-xtask`
/// (`the_bake_and_the_launcher_keep_the_same_bytes_for_a_dump`), and goes with
/// it.
pub async fn run_dumper(
    java: &Path,
    dumper: &Path,
    classpath: &[PathBuf],
    out: &Path,
    work: &Path,
    log: &Path,
) -> Result<Vec<u8>, SchematicAssetsError> {
    tokio::fs::create_dir_all(work).await?;
    if let Some(parent) = out.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let _ = tokio::fs::remove_file(out).await;
    let child = background_command(java)
        .current_dir(work)
        .args(dumper_arguments(dumper, classpath, out, None))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| {
            SchematicAssetsError::EntityDump(format!("cannot run {}: {error}", java.display()))
        })?;
    let output = tokio::time::timeout(DUMP_TIME_LIMIT, child.wait_with_output())
        .await
        .map_err(|_| {
            SchematicAssetsError::EntityDump(format!(
                "the dumper ran longer than {} seconds",
                DUMP_TIME_LIMIT.as_secs()
            ))
        })??;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&stderr);
    let _ = tokio::fs::write(log, text).await;
    if !output.status.success() {
        return Err(SchematicAssetsError::EntityDump(format!(
            "the dumper failed ({}): {}",
            output.status,
            dumper_failure(&stderr)
        )));
    }
    let out = out.to_owned();
    tokio::task::spawn_blocking(move || {
        let (_, canonical) = read_dump(&out)?;
        fs::write(&out, &canonical)?;
        Ok::<_, SchematicAssetsError>(canonical)
    })
    .await
    .map_err(|error| SchematicAssetsError::Io(std::io::Error::other(error)))?
}
