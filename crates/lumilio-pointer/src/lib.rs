//! Scoped native cursor capture for a first-person viewport. No GPUI dependency.
//! The guard stays on the UI thread and releases the OS cursor on every drop path.

use raw_window_handle::RawWindowHandle;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{marker::PhantomData, rc::Rc};

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
static CAPTURED: AtomicBool = AtomicBool::new(false);

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
struct Lease;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
impl Drop for Lease {
    fn drop(&mut self) {
        CAPTURED.store(false, Ordering::Release);
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as native;
#[cfg(target_os = "macos")]
use macos as native;
#[cfg(target_os = "windows")]
use windows as native;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    Unsupported,
    Unavailable,
}

/// Owns a hidden cursor, confined/recentered to the initiating viewport.
/// The caller must drop this before the native window is destroyed or loses focus.
pub struct Capture {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    native: native::Capture,
    _main_thread: PhantomData<Rc<()>>,
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    _lease: Lease,
}

impl Capture {
    /// Offset from the click to the viewport centre, in logical pixels.
    pub fn new(
        handle: RawWindowHandle,
        offset: [f32; 2],
        scale: f32,
    ) -> Result<Self, CaptureError> {
        if !scale.is_finite() || scale <= 0. || !offset.iter().all(|axis| axis.is_finite()) {
            return Err(CaptureError::Unavailable);
        }
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            // The OS cursor is shared by all application windows. A second
            // viewer must not overwrite an existing guard's hide/clip state.
            CAPTURED
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .map_err(|_| CaptureError::Unavailable)?;
            let lease = Lease;
            Ok(Self {
                native: native::Capture::new(handle, offset, scale)?,
                _main_thread: PhantomData,
                _lease: lease,
            })
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = (handle, offset, scale);
            Err(CaptureError::Unsupported)
        }
    }

    /// Accumulated cursor travel since the last sample, in logical pixels.
    /// Recentering does not synthesize game input; zero travel returns zero.
    pub fn sample(&mut self) -> Result<[f32; 2], CaptureError> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            self.native.sample()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Err(CaptureError::Unsupported)
        }
    }
}
