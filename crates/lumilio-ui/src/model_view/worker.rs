//! A scene never crosses threads. One pending request and one pending result
//! bound the queue even when pointer events arrive faster than rendering.

use std::sync::{Arc, Condvar, Mutex};

use lumilio_schematic_render::{Frame, Scene, SceneError, View};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Request {
    pub view: View,
    pub width: u32,
    pub height: u32,
}

#[derive(Default)]
struct Pending {
    latest: Option<Request>,
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
            pending.latest = Some(request);
            self.ready.notify_one();
        }
    }

    pub fn take(&self) -> Option<Request> {
        let mut pending = self.pending.lock().unwrap();
        loop {
            if pending.closed {
                return None;
            }
            if let Some(request) = pending.latest.take() {
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

pub(super) enum Event {
    /// Sent once, before the first frame: block IDs the game's assets cannot draw.
    Loaded {
        undrawable: Vec<String>,
    },
    Frame(Frame),
}

pub(super) type Output = Result<Event, SceneError>;

pub(super) struct Worker {
    pub mailbox: Arc<Mailbox>,
    output: async_channel::Receiver<Output>,
}

impl Worker {
    pub fn start(schematic: Vec<u8>, pack: Vec<u8>) -> std::io::Result<Self> {
        let mailbox = Arc::new(Mailbox::default());
        let requests = mailbox.clone();
        let (send, output) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("schematic-preview".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut scene = Scene::load(&schematic, &pack)?;
                    let undrawable = scene.undrawable_blocks().to_vec();
                    if send
                        .send_blocking(Ok(Event::Loaded { undrawable }))
                        .is_err()
                    {
                        return Ok(());
                    }
                    while let Some(request) = requests.take() {
                        let frame = scene.render(&request.view, request.width, request.height)?;
                        if send.send_blocking(Ok(Event::Frame(frame))).is_err() {
                            break;
                        }
                    }
                    Ok::<_, SceneError>(())
                }));
                let error = match result {
                    Ok(Ok(())) => return,
                    Ok(Err(error)) => error,
                    Err(_) => SceneError::Render("preview worker panicked".into()),
                };
                let _ = send.send_blocking(Err(error));
            })?;
        Ok(Self { mailbox, output })
    }

    pub fn output(&self) -> async_channel::Receiver<Output> {
        self.output.clone()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.mailbox.close();
        self.output.close();
        // Joining here would block the UI during a mesh or a GPU readback.
    }
}
