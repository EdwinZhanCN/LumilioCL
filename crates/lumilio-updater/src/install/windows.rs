use crate::Relaunch;
use std::path::Path;
use std::process::Command;

/// Starts the installer, which closes the launcher, installs silently and
/// opens the new version.
pub(super) fn install(update: &Path) -> Result<Relaunch, String> {
    Command::new(update)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
        ])
        .spawn()
        .map_err(|error| format!("cannot start the LumilioCL installer: {error}"))?;
    Ok(Relaunch::Quit)
}
