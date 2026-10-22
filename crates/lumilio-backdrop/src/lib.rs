//! A blurred backdrop behind a window's content, for themes whose body is
//! translucent (plan translucent-themes, D1). No GPUI dependency.
//!
//! GPUI's own blurred background strips the system effect view's layers to
//! keep it colourless, and on current macOS that removes the blur as well:
//! what is behind the window shows through sharp. This puts back a plain
//! system effect view instead.

use raw_window_handle::RawWindowHandle;

#[cfg(target_os = "macos")]
mod macos;

/// Puts a blurred backdrop behind the window's content, in a light or `dark`
/// material, or takes it away. Returns whether this system has one; elsewhere
/// the window is left as it is.
pub fn set_blur(handle: RawWindowHandle, on: bool, dark: bool) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::set_blur(handle, on, dark)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (handle, on, dark);
        false
    }
}
