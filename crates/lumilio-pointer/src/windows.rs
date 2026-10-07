//! Win32 cursor functions have no safe Rust wrapper; all pointer arguments below
//! refer to live local structs. No native window pointer is dereferenced here.
#![allow(unsafe_code)]

use super::CaptureError;
use raw_window_handle::RawWindowHandle;
use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(point: *mut Point) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn ShowCursor(show: i32) -> i32;
    fn GetClientRect(window: *mut c_void, rect: *mut Rect) -> i32;
    fn ClientToScreen(window: *mut c_void, point: *mut Point) -> i32;
    fn GetClipCursor(rect: *mut Rect) -> i32;
    fn ClipCursor(rect: *const Rect) -> i32;
    fn GetForegroundWindow() -> *mut c_void;
}

pub(super) struct Capture {
    window: *mut c_void,
    original: Point,
    anchor: Point,
    previous_clip: Rect,
    hide_calls: u32,
    scale: f32,
}

impl Capture {
    pub(super) fn new(
        handle: RawWindowHandle,
        offset: [f32; 2],
        scale: f32,
    ) -> Result<Self, CaptureError> {
        let RawWindowHandle::Win32(handle) = handle else {
            return Err(CaptureError::Unsupported);
        };
        let window = handle.hwnd.get() as *mut c_void;
        let (mut original, mut rect, mut origin, mut previous_clip) = (
            Point::default(),
            Rect::default(),
            Point::default(),
            Rect::default(),
        );
        // SAFETY: Win32 writes only to initialized, appropriately sized stack structs.
        unsafe {
            if GetForegroundWindow() != window
                || GetCursorPos(&mut original) == 0
                || GetClientRect(window, &mut rect) == 0
                || ClientToScreen(window, &mut origin) == 0
                || GetClipCursor(&mut previous_clip) == 0
            {
                return Err(CaptureError::Unavailable);
            }
            rect.left += origin.x;
            rect.right += origin.x;
            rect.top += origin.y;
            rect.bottom += origin.y;
            if ClipCursor(&rect) == 0 {
                return Err(CaptureError::Unavailable);
            }
            let anchor = Point {
                x: original.x + (offset[0] * scale) as i32,
                y: original.y + (offset[1] * scale) as i32,
            };
            if SetCursorPos(anchor.x, anchor.y) == 0 {
                ClipCursor(&previous_clip);
                return Err(CaptureError::Unavailable);
            }
            let mut hide_calls = 1;
            while ShowCursor(0) >= 0 {
                hide_calls += 1;
            }
            Ok(Self {
                window,
                original,
                anchor,
                previous_clip,
                hide_calls,
                scale,
            })
        }
    }

    pub(super) fn sample(&mut self) -> Result<[f32; 2], CaptureError> {
        let mut point = Point::default();
        // SAFETY: point is writable; window is an opaque HWND. Never warp another foreground app.
        unsafe {
            if GetForegroundWindow() != self.window
                || GetCursorPos(&mut point) == 0
                || SetCursorPos(self.anchor.x, self.anchor.y) == 0
            {
                return Err(CaptureError::Unavailable);
            }
        }
        Ok([
            (point.x - self.anchor.x) as f32 / self.scale,
            (point.y - self.anchor.y) as f32 / self.scale,
        ])
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // SAFETY: restore exactly this guard's clip and ShowCursor increments.
        unsafe {
            ClipCursor(&self.previous_clip);
            if GetForegroundWindow() == self.window {
                SetCursorPos(self.original.x, self.original.y);
            }
            for _ in 0..self.hide_calls {
                ShowCursor(1);
            }
        }
    }
}
