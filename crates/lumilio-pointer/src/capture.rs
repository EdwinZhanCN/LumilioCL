use std::marker::PhantomData;
use std::rc::Rc;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use std::sync::atomic::{AtomicBool, Ordering};

use raw_window_handle::RawWindowHandle;

#[cfg(target_os = "linux")]
use super::linux as native;
#[cfg(target_os = "macos")]
use super::macos as native;
#[cfg(target_os = "windows")]
use super::windows as native;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub(super) static CAPTURED: AtomicBool = AtomicBool::new(false);

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub(super) struct Lease;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
impl Drop for Lease {
    fn drop(&mut self) {
        CAPTURED.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    Unsupported,
    Unavailable,
}

/// Owns a hidden cursor, confined/recentered to the initiating viewport.
/// The caller must drop this before the native window is destroyed or loses focus.
pub struct Capture {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    pub(super) native: native::Capture,
    pub(super) _main_thread: PhantomData<Rc<()>>,
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    pub(super) _lease: Lease,
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
