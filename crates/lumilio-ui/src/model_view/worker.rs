//! A scene never crosses threads. One pending request and one pending result
//! bound the queue even when pointer events arrive faster than rendering.
//! Picks, exports (plan schematic-studio T10), block icons (T27) and
//! inspections (T32) queue beside it, each with its own reply, and go before
//! the next viewport frame.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

use lumilio_schematic_render::{
    BakedPack, EntityLayers, Focus, Frame, Picked, Scene, SceneError, Slice, View,
};

use super::Inspected;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Request {
    pub view: View,
    pub width: u32,
    pub height: u32,
}

/// One-off work with its own answer. Dropping the worker drops the senders,
/// so whoever waits learns the viewport is gone.
pub(crate) enum Job {
    /// The block under pixel `(x, y)` of a frame of `request`'s size.
    Pick {
        request: Request,
        x: f32,
        y: f32,
        reply: async_channel::Sender<Option<Picked>>,
    },
    /// A block with what its block entity holds (plan schematic-studio
    /// T32).
    Inspect {
        aim: Aim,
        reply: async_channel::Sender<Option<Inspected>>,
    },
    /// The block to outline, in this linear colour, or none (T32); the last
    /// frame is drawn again with it.
    Mark {
        position: Option<[i32; 3]>,
        color: [f32; 4],
        /// The band's width in pixels of the frame.
        width: f32,
    },
    /// A frame for a file, at its own size and background.
    Export {
        request: Request,
        reply: async_channel::Sender<Result<Frame, SceneError>>,
    },
    /// Which blocks keep their colour (plan schematic-studio T31); the last
    /// frame is drawn again with it.
    Focus(Focus),
    /// One icon per block state, `size` pixels square (plan schematic-studio
    /// T27), from the same pack as the picture.
    Icons {
        states: Vec<String>,
        size: u32,
        reply: async_channel::Sender<Result<Vec<Option<Frame>>, SceneError>>,
    },
    /// Draws only `slice`'s layer and those below, or every layer (T25); the
    /// last frame is drawn again. The reply is the blocks in the layer.
    Slice {
        slice: Option<Slice>,
        reply: async_channel::Sender<Result<Vec<(String, u64)>, SceneError>>,
    },
    /// Shows a box of cells as selected, in this linear colour (T25); the
    /// last frame is drawn again.
    Selection {
        cells: Option<super::slice::Cells>,
        color: [f32; 4],
    },
    /// The blocks in a box of cells, by id, most first (T25).
    BlocksIn {
        min: [i32; 3],
        max: [i32; 3],
        reply: async_channel::Sender<Vec<(String, u64)>>,
    },
    /// The same for items, as a container's slots show them (T32).
    ItemIcons {
        items: Vec<String>,
        size: u32,
        reply: async_channel::Sender<Result<Vec<Option<Frame>>, SceneError>>,
    },
}

/// Which block an inspection is of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Aim {
    /// The one under pixel `(x, y)` of a frame of `request`'s size.
    Pixel { request: Request, x: f32, y: f32 },
    /// The one at this position (the review hook).
    Cell([i32; 3]),
}

/// What the worker does next.
pub(crate) enum Work {
    Frame(Request),
    Job(Job),
}

#[derive(Default)]
struct Pending {
    latest: Option<Request>,
    jobs: VecDeque<Job>,
    closed: bool,
}

#[derive(Default)]
pub(crate) struct Mailbox {
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

    pub fn queue(&self, job: Job) {
        let mut pending = self.pending.lock().unwrap();
        if !pending.closed {
            pending.jobs.push_back(job);
            self.ready.notify_one();
        }
    }

    /// The next job, else the latest frame request; `None` once closed.
    pub fn take(&self) -> Option<Work> {
        let mut pending = self.pending.lock().unwrap();
        loop {
            if pending.closed {
                return None;
            }
            if let Some(job) = pending.jobs.pop_front() {
                return Some(Work::Job(job));
            }
            if let Some(request) = pending.latest.take() {
                return Some(Work::Frame(request));
            }
            pending = self.ready.wait(pending).unwrap();
        }
    }

    /// Stops the worker; waiting jobs are dropped, which ends their replies.
    pub fn close(&self) {
        let mut pending = self.pending.lock().unwrap();
        pending.closed = true;
        pending.jobs.clear();
        self.ready.notify_one();
    }

    /// The next piece of work without waiting.
    #[cfg(test)]
    pub fn try_take(&self) -> Option<Work> {
        let mut pending = self.pending.lock().unwrap();
        if let Some(job) = pending.jobs.pop_front() {
            return Some(Work::Job(job));
        }
        pending.latest.take().map(Work::Frame)
    }

    #[cfg(test)]
    pub fn is_closed(&self) -> bool {
        self.pending.lock().unwrap().closed
    }
}

pub(super) enum Event {
    /// Sent once, before the first frame: block IDs the game's assets cannot draw, and entity IDs
    /// nothing draws (no binding row or no model layer; plan schematic-studio T8).
    Loaded {
        undrawable: Vec<String>,
        undrawable_entities: Vec<String>,
        explore_position: [f32; 3],
        /// The schematic's cells, both corners included (T25).
        cells: super::slice::Cells,
    },
    /// A frame, and where the outlined block is in it (`[left, top, right,
    /// bottom]` as fractions; T32).
    Frame {
        frame: Frame,
        mark: Option<[f32; 4]>,
    },
}

pub(super) type Output = Result<Event, SceneError>;

pub(super) struct Worker {
    pub mailbox: Arc<Mailbox>,
    output: async_channel::Receiver<Output>,
}

impl Worker {
    pub fn start(
        schematic: Vec<u8>,
        pack: BakedPack,
        entities: EntityLayers,
    ) -> std::io::Result<Self> {
        let mailbox = Arc::new(Mailbox::default());
        let requests = mailbox.clone();
        let (send, output) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("schematic-preview".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Icons mesh block entities with the same layers.
                    let icon_layers = entities.clone();
                    let mut scene = Scene::load(&schematic, &pack, entities)?;
                    let undrawable = scene.undrawable_blocks().to_vec();
                    let undrawable_entities = scene.undrawable_entities().to_vec();
                    let explore_position = scene.explore_position();
                    if send
                        .send_blocking(Ok(Event::Loaded {
                            undrawable,
                            undrawable_entities,
                            explore_position,
                            cells: scene.cell_bounds(),
                        }))
                        .is_err()
                    {
                        return Ok(());
                    }
                    // The frame on screen, drawn again when the focus or the
                    // outline changes.
                    let mut shown: Option<Request> = None;
                    while let Some(work) = requests.take() {
                        let draw = match work {
                            Work::Frame(request) => {
                                shown = Some(request);
                                Some(request)
                            }
                            Work::Job(Job::Focus(focus)) => {
                                scene.set_focus(&focus);
                                shown
                            }
                            Work::Job(Job::Mark {
                                position,
                                color,
                                width,
                            }) => {
                                scene.set_mark(position, color, width);
                                shown
                            }
                            Work::Job(Job::Slice { slice, reply }) => {
                                // A new axis meshes again; a failure there is
                                // the slice's to report, the picture stays.
                                let result = scene.set_slice(slice).map(|()| {
                                    slice
                                        .map(|slice| {
                                            let (min, max) = scene.layer_cells(slice);
                                            scene.blocks_in(min, max)
                                        })
                                        .unwrap_or_default()
                                });
                                let _ = reply.try_send(result);
                                shown
                            }
                            Work::Job(Job::Selection { cells, color }) => {
                                scene.set_selection(cells, color);
                                shown
                            }
                            Work::Job(Job::BlocksIn { min, max, reply }) => {
                                let _ = reply.try_send(scene.blocks_in(min, max));
                                None
                            }
                            Work::Job(Job::Pick {
                                request,
                                x,
                                y,
                                reply,
                            }) => {
                                let picked =
                                    scene.pick(&request.view, request.width, request.height, x, y);
                                let _ = reply.try_send(picked);
                                None
                            }
                            Work::Job(Job::Inspect { aim, reply }) => {
                                let picked = match aim {
                                    Aim::Pixel { request, x, y } => scene.pick(
                                        &request.view,
                                        request.width,
                                        request.height,
                                        x,
                                        y,
                                    ),
                                    Aim::Cell(position) => scene.block_at(position),
                                };
                                let inspected = picked.map(|picked| Inspected {
                                    contents: lumilio_schematic::inspect::block_entity(
                                        scene.schematic(),
                                        picked.position,
                                    ),
                                    picked,
                                });
                                let _ = reply.try_send(inspected);
                                None
                            }
                            Work::Job(Job::Export { request, reply }) => {
                                // A failed export is the dialog's to report; the
                                // viewport keeps drawing.
                                let frame =
                                    scene.render(&request.view, request.width, request.height);
                                let _ = reply.try_send(frame);
                                None
                            }
                            Work::Job(Job::Icons {
                                states,
                                size,
                                reply,
                            }) => {
                                let icons = lumilio_schematic_render::icons(
                                    &pack,
                                    icon_layers.clone(),
                                    &states,
                                    size,
                                );
                                let _ = reply.try_send(icons);
                                None
                            }
                            Work::Job(Job::ItemIcons { items, size, reply }) => {
                                let icons = lumilio_schematic_render::item_icons(
                                    &pack,
                                    icon_layers.clone(),
                                    &items,
                                    size,
                                );
                                let _ = reply.try_send(icons);
                                None
                            }
                        };
                        if let Some(request) = draw {
                            let frame =
                                scene.render(&request.view, request.width, request.height)?;
                            let mark =
                                scene.mark_rect(&request.view, request.width, request.height);
                            if send
                                .send_blocking(Ok(Event::Frame { frame, mark }))
                                .is_err()
                            {
                                break;
                            }
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

    /// A worker with no thread, for tests that only need its mailbox.
    #[cfg(test)]
    pub fn idle() -> Self {
        Self {
            mailbox: Arc::new(Mailbox::default()),
            output: async_channel::bounded(1).1,
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.mailbox.close();
        self.output.close();
        // Joining here would block the UI during a mesh or a GPU readback.
    }
}
