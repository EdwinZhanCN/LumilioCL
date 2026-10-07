//! AppKit invokes this scoped local monitor on the main thread. The only unsafe
//! bridge returns the original event pointer and removes the exact monitor token.
#![allow(unsafe_code)]

use super::CaptureError;
use block2::RcBlock;
use core_graphics::{
    display::CGDisplay,
    event::CGEvent,
    event_source::{CGEventSource, CGEventSourceStateID},
    geometry::CGPoint,
};
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSEvent, NSEventMask};
use raw_window_handle::RawWindowHandle;
use std::{cell::Cell, ptr::NonNull, rc::Rc};

pub(super) struct Capture {
    original: CGPoint,
    display: CGDisplay,
    monitor: Retained<AnyObject>,
    motion: Rc<Cell<[f32; 2]>>,
    hidden: bool,
    disassociated: bool,
}

impl Capture {
    pub(super) fn new(
        handle: RawWindowHandle,
        offset: [f32; 2],
        _: f32,
    ) -> Result<Self, CaptureError> {
        if !matches!(handle, RawWindowHandle::AppKit(_)) {
            return Err(CaptureError::Unsupported);
        }
        let _main_thread = MainThreadMarker::new().ok_or(CaptureError::Unavailable)?;
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| CaptureError::Unavailable)?;
        let original = CGEvent::new(source)
            .map_err(|_| CaptureError::Unavailable)?
            .location();
        // Quartz positions are display points, not Retina physical pixels.
        let anchor = CGPoint::new(
            original.x + f64::from(offset[0]),
            original.y + f64::from(offset[1]),
        );
        let display = CGDisplay::main();
        let motion = Rc::new(Cell::new([0.; 2]));
        let pending = motion.clone();
        let handler = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit supplies a live NSEvent for the duration of this
            // main-thread callback. It is never stored or returned as a new object.
            let native = unsafe { event.as_ref() };
            let previous = pending.get();
            pending.set([
                previous[0] + native.deltaX() as f32,
                previous[1] + native.deltaY() as f32,
            ]);
            event.as_ptr()
        });
        // SAFETY: the block returns the same live NSEvent pointer. AppKit copies
        // the block and the guard retains the returned token until removeMonitor.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::MouseMoved
                    | NSEventMask::LeftMouseDragged
                    | NSEventMask::RightMouseDragged
                    | NSEventMask::OtherMouseDragged,
                &handler,
            )
        }
        .ok_or(CaptureError::Unavailable)?;
        let mut capture = Self {
            original,
            display,
            monitor,
            motion,
            hidden: false,
            disassociated: false,
        };
        CGDisplay::warp_mouse_cursor_position(anchor).map_err(|_| CaptureError::Unavailable)?;
        capture
            .display
            .hide_cursor()
            .map_err(|_| CaptureError::Unavailable)?;
        capture.hidden = true;
        CGDisplay::associate_mouse_and_mouse_cursor_position(false)
            .map_err(|_| CaptureError::Unavailable)?;
        capture.disassociated = true;
        Ok(capture)
    }

    pub(super) fn sample(&mut self) -> Result<[f32; 2], CaptureError> {
        Ok(self.motion.replace([0.; 2]))
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // SAFETY: this is the retained token from this guard's local monitor.
        unsafe {
            NSEvent::removeMonitor(&self.monitor);
        }
        if self.disassociated {
            let _ = CGDisplay::associate_mouse_and_mouse_cursor_position(true);
        }
        let _ = CGDisplay::warp_mouse_cursor_position(self.original);
        if self.hidden {
            let _ = self.display.show_cursor();
        }
    }
}
