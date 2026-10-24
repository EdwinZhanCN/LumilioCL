//! Scoped native cursor capture for a first-person viewport. No GPUI dependency.
//! The guard stays on the UI thread and releases the OS cursor on every drop path.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub use capture::{Capture, CaptureError};

mod capture;
