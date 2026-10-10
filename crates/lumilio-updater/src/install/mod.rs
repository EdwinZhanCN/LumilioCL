#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub(crate) fn install_and_restart(path: &std::path::Path) -> Result<(), String> {
    linux::install_and_restart(path)
}

#[cfg(target_os = "macos")]
pub(crate) fn install_and_restart(path: &std::path::Path) -> Result<(), String> {
    macos::install_and_restart(path)
}

#[cfg(target_os = "windows")]
pub(crate) fn install_and_restart(path: &std::path::Path) -> Result<(), String> {
    windows::install_and_restart(path)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(super) fn install_and_restart(_: &std::path::Path) -> Result<(), String> {
    Err("this platform cannot install updates".to_owned())
}
