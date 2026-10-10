use std::path::Path;
use std::process::Command;

pub(super) fn install_and_restart(update: &Path) -> Result<(), String> {
    Command::new(update)
        .args([
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
        ])
        .spawn()
        .map_err(|error| format!("cannot start the LumilioCL installer: {error}"))?;
    Ok(())
}
