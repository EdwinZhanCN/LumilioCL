use crate::Relaunch;
use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests;

pub(crate) const WHILE_RUNNING: bool = cfg!(any(target_os = "linux", target_os = "macos"));

/// `executable` is the running launcher's path, read at startup.
#[cfg(target_os = "linux")]
pub(crate) fn install(executable: &Path, update: &Path) -> Result<Relaunch, String> {
    linux::install(executable, update)
}

#[cfg(target_os = "macos")]
pub(crate) fn install(executable: &Path, update: &Path) -> Result<Relaunch, String> {
    macos::install(executable, update)
}

#[cfg(target_os = "windows")]
pub(crate) fn install(_executable: &Path, update: &Path) -> Result<Relaunch, String> {
    windows::install(update)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) fn install(_: &Path, _: &Path) -> Result<Relaunch, String> {
    Err("this platform cannot install updates".to_owned())
}
