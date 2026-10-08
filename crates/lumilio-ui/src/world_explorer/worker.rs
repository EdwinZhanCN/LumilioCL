use lumilio_map_render::{Camera, Error, Frame, Grid, Scene, Tile};
use std::sync::{Arc, Condvar, Mutex};
pub(super) struct Request {
    pub serial: u64,
    pub camera: Camera,
    pub tiles: Vec<Tile>,
    pub grid: Grid,
    pub width: u32,
    pub height: u32,
}
#[derive(Default)]
struct Pending {
    request: Option<Request>,
    closed: bool,
}
#[derive(Default)]
pub(super) struct Mailbox {
    pending: Mutex<Pending>,
    ready: Condvar,
}
impl Mailbox {
    pub fn put(&self, request: Request) {
        let mut pending = self.pending.lock().unwrap();
        if !pending.closed {
            pending.request = Some(request);
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
pub(super) struct Worker {
    pub mailbox: Arc<Mailbox>,
    pub output: async_channel::Receiver<Result<(u64, Frame), Error>>,
}
impl Worker {
    pub fn start() -> std::io::Result<Self> {
        let mailbox = Arc::new(Mailbox::default());
        let input = mailbox.clone();
        let (send, output) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("world-map-render".into())
            .spawn(move || {
                let mut scene = match Scene::new() {
                    Ok(scene) => scene,
                    Err(error) => {
                        let _ = send.send_blocking(Err(error));
                        return;
                    }
                };
                while let Some(request) = input.take() {
                    let frame = scene
                        .render(
                            request.camera,
                            &request.tiles,
                            request.grid,
                            request.width,
                            request.height,
                        )
                        .map(|frame| (request.serial, frame));
                    if send.send_blocking(frame).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self { mailbox, output })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.mailbox.close();
        self.output.close();
    }
}
