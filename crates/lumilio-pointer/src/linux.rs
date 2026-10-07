//! X11 uses a scoped pointer grab and an invisible cursor. Wayland deliberately
//! rejects capture until GPUI exposes its relative-pointer/constraints protocols.
use super::CaptureError;
use raw_window_handle::RawWindowHandle;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use x11rb::{
    CURRENT_TIME, NONE, connection::Connection as _, protocol::xproto::*,
    rust_connection::RustConnection,
};

fn unavailable<T>(_: T) -> CaptureError {
    CaptureError::Unavailable
}

pub(super) struct Capture {
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Motion {
    stopped: bool,
    delta: [f32; 2],
    failed: bool,
}

#[derive(Default)]
struct Shared {
    motion: Mutex<Motion>,
    wake: Condvar,
}

impl Capture {
    pub(super) fn new(
        handle: RawWindowHandle,
        offset: [f32; 2],
        scale: f32,
    ) -> Result<Self, CaptureError> {
        let window = match handle {
            RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).map_err(unavailable)?,
            RawWindowHandle::Xcb(handle) => handle.window.get(),
            _ => return Err(CaptureError::Unsupported),
        };
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        // Connecting and querying X11 can block, particularly over a remote
        // DISPLAY. The UI only consumes accumulated travel under a short lock.
        std::thread::Builder::new()
            .name("schematic-pointer".into())
            .spawn(move || {
                let Ok(mut native) = X11Capture::new(window, offset, scale) else {
                    worker.motion.lock().unwrap().failed = true;
                    return;
                };
                loop {
                    if worker.motion.lock().unwrap().stopped {
                        break;
                    }
                    let delta = native.sample();
                    let mut motion = worker.motion.lock().unwrap();
                    match delta {
                        Ok(delta) => {
                            motion.delta[0] += delta[0];
                            motion.delta[1] += delta[1];
                        }
                        Err(_) => {
                            motion.failed = true;
                            break;
                        }
                    }
                    // Native input sampling, independent of decorative world motion.
                    let (motion, _) = worker
                        .wake
                        .wait_timeout(motion, Duration::from_millis(16))
                        .unwrap();
                    if motion.stopped {
                        break;
                    }
                }
            })
            .map_err(unavailable)?;
        Ok(Self { shared })
    }

    pub(super) fn sample(&mut self) -> Result<[f32; 2], CaptureError> {
        let mut motion = self.shared.motion.lock().unwrap();
        if motion.failed {
            return Err(CaptureError::Unavailable);
        }
        Ok(std::mem::take(&mut motion.delta))
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.shared.motion.lock().unwrap().stopped = true;
        self.shared.wake.notify_one();
    }
}

struct X11Capture {
    connection: RustConnection,
    window: u32,
    root: u32,
    cursor: u32,
    original: [i16; 2],
    anchor: [i16; 2],
    scale: f32,
}

impl X11Capture {
    fn new(window: u32, offset: [f32; 2], scale: f32) -> Result<Self, CaptureError> {
        let (connection, _) = x11rb::connect(None).map_err(unavailable)?;
        let pointer = connection
            .query_pointer(window)
            .map_err(unavailable)?
            .reply()
            .map_err(unavailable)?;
        let (pixmap, gc, cursor) = (
            connection.generate_id().map_err(unavailable)?,
            connection.generate_id().map_err(unavailable)?,
            connection.generate_id().map_err(unavailable)?,
        );
        connection
            .create_pixmap(1, pixmap, window, 1, 1)
            .map_err(unavailable)?;
        connection
            .create_gc(gc, pixmap, &CreateGCAux::new().foreground(0))
            .map_err(unavailable)?;
        connection
            .poly_fill_rectangle(
                pixmap,
                gc,
                &[Rectangle {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                }],
            )
            .map_err(unavailable)?;
        connection
            .create_cursor(cursor, pixmap, pixmap, 0, 0, 0, 0, 0, 0, 0, 0)
            .map_err(unavailable)?;
        connection.free_gc(gc).map_err(unavailable)?;
        connection.free_pixmap(pixmap).map_err(unavailable)?;
        let reply = connection
            .grab_pointer(
                false,
                window,
                EventMask::NO_EVENT,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
                window,
                cursor,
                CURRENT_TIME,
            )
            .map_err(unavailable)?
            .reply()
            .map_err(unavailable)?;
        if reply.status != GrabStatus::SUCCESS {
            return Err(CaptureError::Unavailable);
        }
        let original = [pointer.root_x, pointer.root_y];
        let anchor = [
            original[0].saturating_add((offset[0] * scale) as i16),
            original[1].saturating_add((offset[1] * scale) as i16),
        ];
        let this = Self {
            connection,
            window,
            root: pointer.root,
            cursor,
            original,
            anchor,
            scale,
        };
        this.warp(anchor)?;
        Ok(this)
    }

    fn warp(&self, point: [i16; 2]) -> Result<(), CaptureError> {
        self.connection
            .warp_pointer(NONE, self.root, 0, 0, 0, 0, point[0], point[1])
            .map_err(|_| CaptureError::Unavailable)?;
        self.connection
            .flush()
            .map_err(|_| CaptureError::Unavailable)
    }

    fn sample(&mut self) -> Result<[f32; 2], CaptureError> {
        let point = self
            .connection
            .query_pointer(self.window)
            .map_err(|_| CaptureError::Unavailable)?
            .reply()
            .map_err(|_| CaptureError::Unavailable)?;
        self.warp(self.anchor)?;
        Ok([
            (i32::from(point.root_x) - i32::from(self.anchor[0])) as f32 / self.scale,
            (i32::from(point.root_y) - i32::from(self.anchor[1])) as f32 / self.scale,
        ])
    }
}

impl Drop for X11Capture {
    fn drop(&mut self) {
        let _ = self.connection.ungrab_pointer(CURRENT_TIME);
        let _ = self.warp(self.original);
        let _ = self.connection.free_cursor(self.cursor);
        let _ = self.connection.flush();
    }
}
