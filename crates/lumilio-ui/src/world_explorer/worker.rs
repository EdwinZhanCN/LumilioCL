use super::stats::{Stats, add};
use lumilio_map_render::{Camera, Error, Frame, Grid, Scene, Sprite, Tile};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;
pub(super) struct Request {
    pub serial: u64,
    /// When the view asked for this frame, for latency statistics.
    pub issued: Instant,
    pub camera: Camera,
    pub tiles: Vec<Tile>,
    pub sprites: Vec<Sprite>,
    pub grid: Grid,
    pub width: u32,
    pub height: u32,
}
#[derive(Default)]
struct Pending {
    request: Option<Request>,
    closed: bool,
}
pub(super) struct Mailbox {
    pending: Mutex<Pending>,
    ready: Condvar,
    stats: Arc<Stats>,
}
impl Mailbox {
    fn new(stats: Arc<Stats>) -> Self {
        Self {
            pending: Mutex::default(),
            ready: Condvar::new(),
            stats,
        }
    }
    pub fn put(&self, request: Request) {
        let mut pending = self.pending.lock().unwrap();
        if !pending.closed {
            self.stats.requested.fetch_add(1, Relaxed);
            if pending.request.replace(request).is_some() {
                self.stats.coalesced.fetch_add(1, Relaxed);
            }
            self.ready.notify_one();
        }
    }
    fn take(&self) -> Option<Request> {
        let mut pending = self.pending.lock().unwrap();
        loop {
            if pending.closed {
                return None;
            }
            if let Some(request) = pending.request.take() {
                return Some(request);
            }
            pending = self.ready.wait(pending).unwrap();
        }
    }
    pub fn close(&self) {
        self.pending.lock().unwrap().closed = true;
        self.ready.notify_one();
    }
}
/// A finished frame and the request it answers.
pub(super) struct Rendered {
    pub serial: u64,
    pub issued: Instant,
    pub frame: Frame,
}
pub(super) struct Worker {
    pub mailbox: Arc<Mailbox>,
    pub output: async_channel::Receiver<Result<Rendered, Error>>,
    pub stats: Arc<Stats>,
}
impl Worker {
    pub fn start() -> std::io::Result<Self> {
        let stats = Arc::new(Stats::default());
        let mailbox = Arc::new(Mailbox::new(stats.clone()));
        let counted = stats.clone();
        let input = mailbox.clone();
        let (send, output) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("world-map-render".into())
            .spawn(move || {
                let mut scene = match Scene::new() {
                    Ok(scene) => {
                        if std::env::var_os("LUMILIO_MAP_STATS").is_some() {
                            eprintln!("[map-stats] adapter: {}", scene.adapter());
                        }
                        scene
                    }
                    Err(error) => {
                        let _ = send.send_blocking(Err(error));
                        return;
                    }
                };
                while let Some(request) = input.take() {
                    add(&counted.queue_us, request.issued.elapsed());
                    let frame = scene
                        .render(
                            request.camera,
                            &request.tiles,
                            &request.sprites,
                            request.grid,
                            request.width,
                            request.height,
                        )
                        .map(|frame| {
                            let timings = frame.timings;
                            counted.rendered.fetch_add(1, Relaxed);
                            add(&counted.render_us, timings.total);
                            add(&counted.encode_us, timings.encode);
                            add(&counted.wait_us, timings.wait);
                            add(&counted.copy_us, timings.copy);
                            counted.bytes.fetch_add(frame.bgra.len() as u64, Relaxed);
                            Rendered {
                                serial: request.serial,
                                issued: request.issued,
                                frame,
                            }
                        });
                    if send.send_blocking(frame).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            mailbox,
            output,
            stats,
        })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.mailbox.close();
        self.output.close();
    }
}
