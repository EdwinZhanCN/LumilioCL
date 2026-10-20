//! The secret an agent presents, and where to connect. Made once per data
//! folder and kept beside the launcher's data with owner-only permissions,
//! never in the settings file, which a person may share for support.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Below the data root.
const FILE: &str = "agent/mcp.json";
const TOKEN_BYTES: usize = 32;

/// What `agent/mcp.json` holds: the current address and the lasting token.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Connection {
    /// The MCP endpoint of the launcher that last ran, such as
    /// `http://127.0.0.1:27315/mcp`.
    pub url: String,
    /// The bearer token every request must carry.
    pub token: String,
}

pub(crate) fn path(root: &Path) -> PathBuf {
    root.join(FILE)
}

/// The token the data folder already has, if any.
pub(crate) fn existing_token(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path(root)).ok()?;
    serde_json::from_str::<Connection>(&text)
        .ok()
        .map(|connection| connection.token)
        .filter(|token| !token.is_empty())
}

pub(crate) fn new_token() -> io::Result<String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Writes the file for this run, readable by the owner only.
pub(crate) fn write(root: &Path, connection: &Connection) -> io::Result<()> {
    let path = path(root);
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let text = serde_json::to_string_pretty(connection)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    #[cfg(unix)]
    {
        // An older file keeps its mode through `open`; make sure of it.
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    io::Write::write_all(&mut file, text.as_bytes())
}
