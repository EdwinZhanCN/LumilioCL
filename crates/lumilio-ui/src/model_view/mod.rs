//! Schematic viewer: in a modal (`ModelView`) or inline in the schematic
//! studio's viewport. Meshing and GPU readback stay on the worker; a frame
//! clock runs only while first-person input is captured.

mod input;
mod modal;
mod slice;

pub(crate) use slice::Cells;
#[cfg(test)]
mod tests;
mod worker;

pub use modal::ModelView;

use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::prelude::*;
use gpui::{
    App, Bounds, Context, FocusHandle, MouseButton, ObjectFit, Pixels, Point, RenderImage,
    ScrollDelta, Task, Window, canvas, div, img, px,
};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{Sizable as _, h_flex, v_flex};
use lumilio_core::{AssetNotice, AssetPlan, ModelPreview};
use lumilio_schematic_render::{Angle, Focus, Frame, Picked, SceneError, Slice, SliceAxis, View};

use crate::{
    controls::Segments,
    key::Key,
    kit,
    theme::{self, ShellColors},
    tr,
};
use input::{FlightInput, Mode};
#[cfg(test)]
pub(crate) use worker::Job as WorkerJob;
#[cfg(test)]
pub(crate) use worker::Work;
use worker::{Aim, Event, Job, Request, Worker};

type Load = Rc<dyn Fn(u64, &mut Window, &mut App)>;
/// Told that what the picture points at changed: the block under the
/// pointer, or the inspected one (studio only).
pub(crate) type OnPointed = Rc<dyn Fn(&mut App)>;

/// The block a click inspects (plan schematic-studio T32): that one block,
/// not its kind. The picture outlines it; the studio says what it is and
/// what its block entity holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Inspected {
    pub picked: Picked,
    pub contents: Option<lumilio_schematic::inspect::Contents>,
}

/// The height the inline tool strip takes at the picture's bottom edge
/// (12 px margin, 55 px of segment keys with their LEDs), with a gap: what
/// a read-out above it clears.
pub(crate) const TOOL_STRIP_CLEARANCE: f32 = 76.;

/// A press that moves less than this is a click, not a drag.
const CLICK_SLOP: f32 = 4.;

/// The inspected block's outline, in logical pixels (T32).
const MARK_WIDTH: f32 = 2.;

/// The outline's colour (the theme's ink in linear RGB) and its width in the
/// frame's pixels.
type MarkStyle = ([f32; 4], f32);

/// The words for an angle preset, in [`Angle::ALL`]'s order.
pub(crate) fn angle_labels() -> &'static [&'static str] {
    crate::tr_all![
        "model-angle-north",
        "model-angle-east",
        "model-angle-south",
        "model-angle-west",
        "model-angle-top",
        "model-angle-isometric",
    ]
}

/// The base's words: its game version, from the downloaded bundle, for the
/// studio's 资源来源 list (T17, T21). The viewport itself carries no
/// readout: the list is the one place that names the sources (D22).
pub(crate) fn base_label(version: &str) -> String {
    tr!("model-source-bundle", version = version)
}

/// An overlay's file name as the readout and the source list show it.
pub(crate) fn file_label(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn physical_size(width: f32, height: f32, scale: f32) -> Option<(u32, u32)> {
    if width < 1. || height < 1. {
        return None;
    }
    let fit = (4096. / (width * scale).max(height * scale)).min(1.);
    Some((
        (width * scale * fit).round().max(1.) as u32,
        (height * scale * fit).round().max(1.) as u32,
    ))
}

#[derive(Debug, PartialEq)]
enum State {
    Loading,
    Ready,
    Failed {
        message: &'static str,
        detail: Option<String>,
    },
}

impl State {
    fn failed(error: SceneError) -> Self {
        let message = match &error {
            SceneError::NoGpu => tr!("model-error-no-gpu"),
            SceneError::Parse(_) => tr!("model-error-parse"),
            SceneError::Pack(_) => tr!("model-error-pack"),
            SceneError::Mesh(_) => tr!("model-error-mesh"),
            SceneError::Render(_) => tr!("model-error-render"),
        };
        let detail = (error != SceneError::NoGpu).then(|| error.to_string());
        Self::Failed { message, detail }
    }
}

/// One plain sentence about blocks left out of the picture; the IDs go to 技术详情.
fn undrawable_note(undrawable: &[String]) -> Option<(String, String)> {
    (!undrawable.is_empty()).then(|| {
        (
            tr!("model-undrawable", count = undrawable.len()),
            undrawable.join("\n"),
        )
    })
}

/// The same for entities nothing draws (plan schematic-studio T8).
fn undrawable_entities_note(undrawable: &[String]) -> Option<(String, String)> {
    (!undrawable.is_empty()).then(|| {
        (
            tr!("model-undrawable-entities", count = undrawable.len()),
            undrawable.join("\n"),
        )
    })
}

pub(crate) struct Viewer {
    load: Load,
    /// Inline in a page (the studio's viewport): fills its column and leaves
    /// focus where the person put it. Otherwise it sits in a modal.
    inline: bool,
    asked: bool,
    state: State,
    /// Block IDs the game's assets cannot draw, reported once per load.
    undrawable: Vec<String>,
    /// Entity IDs nothing draws, reported once per load.
    undrawable_entities: Vec<String>,
    view: View,
    worker: Option<Worker>,
    task: Option<Task<()>>,
    image: Option<Arc<RenderImage>>,
    old_images: Vec<Arc<RenderImage>>,
    last_request: Option<Request>,
    drag: Option<Point<Pixels>>,
    focus: FocusHandle,
    release_registered: bool,
    mode: Mode,
    explore_position: [f32; 3],
    orbit_view: View,
    flight_view: Option<View>,
    input: FlightInput,
    capture: Option<lumilio_pointer::Capture>,
    capture_error: Option<&'static str>,
    viewport: Option<Bounds<Pixels>>,
    last_tick: Instant,
    clock_pending: bool,
    /// Where the blocks come from, once the assets arrived.
    plan: Option<AssetPlan>,
    on_pointed: Option<OnPointed>,
    /// Where the left button went down, to tell a click from a drag.
    press: Option<Point<Pixels>>,
    pick_task: Option<Task<()>>,
    /// The block the sight line hits in Explore, for the read-out (D24).
    looked: Option<Picked>,
    look_task: Option<Task<()>>,
    /// The camera moved since the last sight-line pick.
    look_dirty: bool,
    /// The block under the pointer in Orbital (studio only), for the
    /// read-out.
    hovered: Option<Picked>,
    /// Where the pointer is, as a fraction of the picture, while it is over
    /// it; one pick at a time follows it.
    hover_at: Option<(f32, f32)>,
    hover_task: Option<Task<()>>,
    /// Which blocks keep their colour (studio only, T31); sent to the
    /// worker when it changes and again when a new worker starts.
    block_focus: Focus,
    /// The block the last click inspected (studio only, T32).
    inspected: Option<Inspected>,
    mark_style: MarkStyle,
    /// The outline the worker was last told to draw.
    mark_sent: Option<([i32; 3], MarkStyle)>,
    /// A block to inspect once the picture is drawn (the review hook).
    pending_inspect: Option<[i32; 3]>,
    /// Where the outlined block is in the frame on screen.
    mark_rect: Option<[f32; 4]>,
    /// The schematic's cells, both corners included, once loaded (T25).
    cells: Option<Cells>,
    /// The layers drawn, while slicing (T25).
    slice: Option<Slice>,
    /// The axis 切片 slices along next.
    slice_axis: SliceAxis,
    /// The blocks in the slice's layer, by id.
    layer_blocks: Vec<(String, u64)>,
    slice_task: Option<Task<()>>,
    /// The selected box of cells (T25).
    selection: Option<Cells>,
    /// The selection's colour, the theme's focus colour in linear RGB.
    selection_color: [f32; 4],
    selection_sent: Option<(Cells, [f32; 4])>,
    /// The blocks in the selection, by id.
    selection_blocks: Vec<(String, u64)>,
    selection_task: Option<Task<()>>,
    /// A Ctrl-drag selecting a box.
    selecting: Option<slice::SelectDrag>,
    select_task: Option<Task<()>>,
}

impl Viewer {
    fn new(load: Load, cx: &mut Context<Self>) -> Self {
        Self {
            load,
            inline: false,
            asked: false,
            state: State::Loading,
            undrawable: Vec::new(),
            undrawable_entities: Vec::new(),
            view: View::new(),
            worker: None,
            task: None,
            image: None,
            old_images: Vec::new(),
            last_request: None,
            drag: None,
            focus: cx.focus_handle(),
            release_registered: false,
            mode: Mode::Orbital,
            explore_position: [0., 1.62, 3.],
            orbit_view: View::new(),
            flight_view: None,
            input: FlightInput::default(),
            capture: None,
            capture_error: None,
            viewport: None,
            last_tick: Instant::now(),
            clock_pending: false,
            plan: None,
            on_pointed: None,
            press: None,
            pick_task: None,
            looked: None,
            look_task: None,
            look_dirty: false,
            hovered: None,
            hover_at: None,
            hover_task: None,
            block_focus: Focus::All,
            inspected: None,
            mark_style: ([0., 0., 0., 1.], MARK_WIDTH),
            mark_sent: None,
            pending_inspect: None,
            mark_rect: None,
            cells: None,
            slice: None,
            slice_axis: SliceAxis::Y,
            layer_blocks: Vec::new(),
            slice_task: None,
            selection: None,
            selection_color: [0., 0., 1., 1.],
            selection_sent: None,
            selection_blocks: Vec::new(),
            selection_task: None,
            selecting: None,
            select_task: None,
        }
    }

    /// A point in the window as a fraction of the picture, once laid out.
    fn fraction(&self, position: Point<Pixels>) -> Option<(f32, f32)> {
        let bounds = self.viewport?;
        let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        (width >= 1. && height >= 1.).then(|| {
            (
                f32::from(position.x - bounds.origin.x) / width,
                f32::from(position.y - bounds.origin.y) / height,
            )
        })
    }

    /// Keeps blocks of one kind in colour and fades or hides the rest
    /// (plan schematic-studio T31, D30); nothing is meshed again.
    pub(crate) fn set_focus(&mut self, focus: Focus) {
        if self.block_focus == focus {
            return;
        }
        self.block_focus = focus.clone();
        if let Some(worker) = &self.worker {
            worker.mailbox.queue(Job::Focus(focus));
        }
    }

    #[cfg(test)]
    pub(crate) fn block_focus(&self) -> &Focus {
        &self.block_focus
    }

    /// A viewer that fills the column it sits in (the studio's viewport).
    pub(crate) fn inline(load: Load, on_pointed: OnPointed, cx: &mut Context<Self>) -> Self {
        Self {
            inline: true,
            on_pointed: Some(on_pointed),
            ..Self::new(load, cx)
        }
    }

    /// The block the last click inspected, outlined in the picture.
    pub(crate) fn inspected(&self) -> Option<&Inspected> {
        self.inspected.as_ref()
    }

    /// Lets the inspected block go: its outline goes too.
    pub(crate) fn let_go(&mut self, cx: &mut Context<Self>) {
        if self.inspected.take().is_some() {
            self.sync_mark();
            self.tell(cx);
        }
    }

    /// Tells the worker the outline the picture should have, on a change:
    /// the inspected block in the theme's ink, or none.
    fn sync_mark(&mut self) {
        let mark = self
            .inspected
            .as_ref()
            .map(|inspected| (inspected.picked.position, self.mark_style));
        if mark == self.mark_sent {
            return;
        }
        if let Some(worker) = &self.worker {
            let (color, width) = self.mark_style;
            worker.mailbox.queue(Job::Mark {
                position: mark.map(|(position, _)| position),
                color,
                width,
            });
            self.mark_sent = mark;
        }
    }

    fn tell(&self, cx: &mut Context<Self>) {
        if let Some(on_pointed) = self.on_pointed.clone() {
            on_pointed(cx);
        }
        cx.notify();
    }

    /// Whether the scene is (being) loaded on a worker, so an export can be
    /// queued; it is drawn once the scene is there.
    pub(crate) fn is_ready(&self) -> bool {
        self.worker.is_some()
    }

    /// The camera as the picture shows it now.
    pub(crate) fn view(&self) -> View {
        self.view
    }

    /// Blocks the pack cannot draw, drawn as missing-texture placeholders.
    pub(crate) fn undrawable(&self) -> &[String] {
        &self.undrawable
    }

    /// What the read-out names: the block the sight line hits while flying,
    /// else the one under the pointer.
    pub(crate) fn pointed(&self) -> Option<&Picked> {
        if self.capture.is_some() {
            self.looked.as_ref()
        } else {
            self.hovered.as_ref()
        }
    }

    /// The controls for the current mode, for the read-out when the
    /// pointer is on nothing; or why the cursor could not be captured.
    pub(crate) fn hint(&self) -> &'static str {
        if let Some(error) = self.capture_error {
            return error;
        }
        match self.mode {
            Mode::Orbital => tr!("model-hint-orbital-pick"),
            Mode::Explore if self.capture.is_some() => tr!("model-hint-explore-captured"),
            Mode::Explore => tr!("model-hint-explore"),
        }
    }

    /// Whether the picture is drawn (not loading or failed).
    pub(crate) fn is_drawn(&self) -> bool {
        self.state == State::Ready
    }

    /// Entities nothing draws, for the studio's technical facts.
    pub(crate) fn undrawable_entities(&self) -> &[String] {
        &self.undrawable_entities
    }

    /// One icon per block state, drawn on the worker from the picture's
    /// pack (plan schematic-studio T27); `None` before the scene is loading.
    pub(crate) fn icons(
        &self,
        states: Vec<String>,
        size: u32,
    ) -> Option<async_channel::Receiver<Result<Vec<Option<Frame>>, SceneError>>> {
        let worker = self.worker.as_ref()?;
        let (reply, answer) = async_channel::bounded(1);
        worker.mailbox.queue(Job::Icons {
            states,
            size,
            reply,
        });
        Some(answer)
    }

    /// The same for items, as a container's slots show them (T32).
    pub(crate) fn item_icons(
        &self,
        items: Vec<String>,
        size: u32,
    ) -> Option<async_channel::Receiver<Result<Vec<Option<Frame>>, SceneError>>> {
        let worker = self.worker.as_ref()?;
        let (reply, answer) = async_channel::bounded(1);
        worker.mailbox.queue(Job::ItemIcons { items, size, reply });
        Some(answer)
    }

    /// Where the inspected block is in the picture on screen: `[left, top,
    /// right, bottom]` as fractions of it; `None` while it is not in view.
    pub(crate) fn mark_rect(&self) -> Option<[f32; 4]> {
        self.inspected.as_ref()?;
        self.mark_rect
    }

    /// The picture's size on screen, once it is laid out.
    pub(crate) fn picture_size(&self) -> Option<gpui::Size<Pixels>> {
        self.viewport.map(|bounds| bounds.size)
    }

    /// Stands in for where the inspected block is (tests have no worker).
    #[cfg(test)]
    pub(crate) fn set_mark_rect_for_test(&mut self, rect: Option<[f32; 4]>) {
        self.mark_rect = rect;
    }

    /// Follows the pointer over the picture with one pick at a time; a move
    /// while one is out is picked when it returns.
    fn hover(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(bounds) = self.viewport else {
            return;
        };
        let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        if width < 1. || height < 1. {
            return;
        }
        self.hover_at = Some((
            f32::from(position.x - bounds.origin.x) / width,
            f32::from(position.y - bounds.origin.y) / height,
        ));
        self.ask_hover(cx);
    }

    fn ask_hover(&mut self, cx: &mut Context<Self>) {
        if self.hover_task.is_some() {
            return;
        }
        let Some(at) = self.hover_at else {
            return;
        };
        let Some(answer) = self.ask_pick(at) else {
            return;
        };
        self.hover_task = Some(cx.spawn(async move |this, cx| {
            let hovered = answer.recv().await.ok().flatten();
            let _ = this.update(cx, |this, cx| {
                this.hover_task = None;
                let moved = this.hover_at != Some(at);
                if this.hover_at.is_some() && this.hovered != hovered {
                    this.hovered = hovered;
                    this.tell(cx);
                }
                if moved {
                    this.ask_hover(cx);
                }
            });
        }));
    }

    fn leave(&mut self, cx: &mut Context<Self>) {
        self.hover_at = None;
        if self.hovered.take().is_some() {
            self.tell(cx);
        }
    }

    /// Renders `view` at `width` × `height` on the worker for a file; `None`
    /// before the scene is loaded. The answer ends when the viewport goes.
    pub(crate) fn export(
        &self,
        view: View,
        width: u32,
        height: u32,
    ) -> Option<async_channel::Receiver<Result<Frame, SceneError>>> {
        let worker = self.worker.as_ref()?;
        let (reply, answer) = async_channel::bounded(1);
        worker.mailbox.queue(Job::Export {
            request: Request {
                view,
                width,
                height,
            },
            reply,
        });
        Some(answer)
    }

    /// Turns the orbit camera to a preset angle; Explore gives way to Orbital.
    pub(crate) fn turn_to(&mut self, angle: Angle, cx: &mut Context<Self>) {
        if self.mode == Mode::Explore {
            self.select_mode(Mode::Orbital, cx);
        }
        self.input.clear();
        self.view = self.view.at(angle);
        cx.notify();
    }

    /// Asks the worker to inspect the block under `position`, a point in
    /// the window.
    fn pick(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(bounds) = self.viewport else {
            return;
        };
        let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        if width < 1. || height < 1. {
            return;
        }
        self.inspect_at(
            (
                f32::from(position.x - bounds.origin.x) / width,
                f32::from(position.y - bounds.origin.y) / height,
            ),
            cx,
        );
    }

    /// Inspects the block at a fraction of the viewport: the pointer's in
    /// Orbital, the crosshair's in Explore (D24).
    fn inspect_at(&mut self, at: (f32, f32), cx: &mut Context<Self>) {
        let Some(request) = self.last_request else {
            return;
        };
        self.inspect(
            Aim::Pixel {
                request,
                x: at.0 * request.width as f32,
                y: at.1 * request.height as f32,
            },
            cx,
        );
    }

    /// Inspects the block at `position` once the picture is drawn (the
    /// application's review hook).
    pub(crate) fn inspect_cell(&mut self, position: [i32; 3], cx: &mut Context<Self>) {
        if self.state == State::Ready {
            self.inspect(Aim::Cell(position), cx);
        } else {
            self.pending_inspect = Some(position);
        }
    }

    fn inspect(&mut self, aim: Aim, cx: &mut Context<Self>) {
        let Some(worker) = self.worker.as_ref() else {
            return;
        };
        if self.state != State::Ready {
            return;
        }
        let (reply, answer) = async_channel::bounded(1);
        worker.mailbox.queue(Job::Inspect { aim, reply });
        self.pick_task = Some(cx.spawn(async move |this, cx| {
            if let Ok(inspected) = answer.recv().await {
                let _ = this.update(cx, |this, cx| this.inspected_arrived(inspected, cx));
            }
        }));
    }

    /// Keeps the Explore read-out current: one sight-line pick at a time,
    /// asked again when the camera moved while one was out.
    fn look(&mut self, cx: &mut Context<Self>) {
        if self.look_task.is_some() || !self.look_dirty {
            return;
        }
        let Some(answer) = self.ask_pick((0.5, 0.5)) else {
            return;
        };
        self.look_dirty = false;
        self.look_task = Some(cx.spawn(async move |this, cx| {
            let looked = answer.recv().await.ok().flatten();
            let _ = this.update(cx, |this, cx| {
                this.look_task = None;
                if this.capture.is_some() {
                    let changed = this.looked != looked;
                    this.looked = looked;
                    if changed {
                        this.tell(cx);
                    }
                    this.look(cx);
                } else {
                    this.looked = None;
                }
                cx.notify();
            });
        }));
    }

    /// Queues a pick at a fraction of the viewport; the answer arrives on the
    /// returned channel.
    fn ask_pick(&self, at: (f32, f32)) -> Option<async_channel::Receiver<Option<Picked>>> {
        let (worker, request) = (self.worker.as_ref()?, self.last_request?);
        if self.state != State::Ready {
            return None;
        }
        let (reply, answer) = async_channel::bounded(1);
        worker.mailbox.queue(Job::Pick {
            request,
            x: at.0 * request.width as f32,
            y: at.1 * request.height as f32,
            reply,
        });
        Some(answer)
    }

    /// A click inspected this block, or empty space, which lets the last
    /// one go (T32). Its kind stays as the materials chose it.
    pub(crate) fn inspected_arrived(
        &mut self,
        inspected: Option<Inspected>,
        cx: &mut Context<Self>,
    ) {
        if self.inspected == inspected {
            return;
        }
        self.inspected = inspected;
        self.sync_mark();
        self.tell(cx);
    }

    /// Stands in for the block under the pointer (tests have no worker).
    #[cfg(test)]
    pub(crate) fn hovered_for_test(&mut self, hovered: Option<Picked>, cx: &mut Context<Self>) {
        self.hovered = hovered;
        self.tell(cx);
    }

    #[cfg(test)]
    pub(crate) fn set_undrawable_for_test(&mut self, names: Vec<String>) {
        self.undrawable = names;
    }

    /// A drawn picture of one pixel, for tests without a GPU.
    #[cfg(test)]
    pub(crate) fn set_drawn_for_test(&mut self, cx: &mut Context<Self>) {
        let pixel = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255]));
        self.image = Some(Arc::new(RenderImage::new([image::Frame::new(pixel)])));
        self.state = State::Ready;
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn start_idle_worker(&mut self) -> Arc<worker::Mailbox> {
        let worker = Worker::idle();
        let mailbox = worker.mailbox.clone();
        self.worker = Some(worker);
        mailbox
    }

    /// Called only for this entity's asset request; replacement views have new IDs.
    pub(crate) fn assets(&mut self, result: Result<ModelPreview, String>, cx: &mut Context<Self>) {
        let preview = match result {
            Ok(preview) => {
                self.plan = Some(preview.plan.clone());
                preview
            }
            Err(detail) => {
                self.state = State::Failed {
                    message: tr!("model-read-failed"),
                    detail: Some(detail),
                };
                cx.notify();
                return;
            }
        };
        let Some(pack) = preview.pack else {
            self.state = State::Failed {
                message: match preview.plan.notice {
                    Some(AssetNotice::Offline) => tr!("model-assets-offline"),
                    Some(AssetNotice::Unavailable) => tr!("model-assets-unavailable"),
                    Some(AssetNotice::DownloadDisabled) => tr!("model-assets-download-off"),
                    Some(AssetNotice::Unreadable) => tr!("model-error-pack"),
                    None => tr!("model-assets-missing"),
                },
                detail: None,
            };
            cx.notify();
            return;
        };
        match Worker::start(preview.schematic, pack, preview.entities) {
            Ok(worker) => {
                if self.block_focus != Focus::All {
                    worker.mailbox.queue(Job::Focus(self.block_focus.clone()));
                }
                let output = worker.output();
                self.worker = Some(worker);
                self.mark_sent = None;
                self.sync_mark();
                self.selection_sent = None;
                self.sync_selection();
                if self.slice.is_some() {
                    self.send_slice(cx);
                }
                self.task = Some(cx.spawn(async move |this, cx| {
                    while let Ok(result) = output.recv().await {
                        if this
                            .update(cx, |this, cx| {
                                this.frame_arrived(result);
                                if this.state == State::Ready
                                    && let Some(position) = this.pending_inspect.take()
                                {
                                    this.inspect(Aim::Cell(position), cx);
                                }
                                cx.notify();
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }));
            }
            Err(error) => self.state = State::failed(SceneError::Render(error.to_string())),
        }
        cx.notify();
    }

    fn frame_arrived(&mut self, result: worker::Output) {
        match result {
            Ok(Event::Loaded {
                undrawable,
                undrawable_entities,
                explore_position,
                cells,
            }) => {
                self.cells = Some(cells);
                self.undrawable = undrawable;
                self.undrawable_entities = undrawable_entities;
                self.explore_position = explore_position;
                self.flight_view = None;
                if self.mode == Mode::Explore {
                    self.view.position = Some(explore_position);
                    self.view.yaw_deg = 0.;
                    self.view.pitch_deg = 0.;
                }
            }
            Ok(Event::Frame { frame, mark }) => {
                if let Some(buffer) =
                    image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
                {
                    let image = Arc::new(RenderImage::new([image::Frame::new(buffer)]));
                    if let Some(old) = self.image.replace(image) {
                        self.old_images.push(old);
                    }
                    self.state = State::Ready;
                    // With the picture it belongs to, so a card over the
                    // block moves with the block.
                    self.mark_rect = mark;
                }
            }
            Err(error) => {
                self.release_capture();
                self.state = State::failed(error);
                self.worker = None;
            }
        }
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.input.clear();
        self.view = View {
            background: self.view.background,
            ..View::new()
        };
        if self.mode == Mode::Explore {
            self.view.position = Some(self.explore_position);
            self.view.yaw_deg = 0.;
            self.view.pitch_deg = 0.;
        }
        cx.notify();
    }

    fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.release_capture();
        self.capture_error = None;
        self.worker = None;
        self.task = None;
        self.last_request = None;
        self.state = State::Loading;
        self.inspected = None;
        self.mark_sent = None;
        self.undrawable.clear();
        self.undrawable_entities.clear();
        (self.load)(cx.entity_id().as_u64(), window, cx);
        cx.notify();
    }

    fn release_capture(&mut self) {
        self.capture = None;
        self.input.clear();
        self.drag = None;
        self.looked = None;
        self.look_dirty = false;
    }

    fn select_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.release_capture();
        self.capture_error = None;
        if mode == self.mode {
            cx.notify();
            return;
        }
        match self.mode {
            Mode::Orbital => self.orbit_view = self.view,
            Mode::Explore => self.flight_view = Some(self.view),
        }
        self.mode = mode;
        self.view = match mode {
            Mode::Orbital => self.orbit_view,
            Mode::Explore => self.flight_view.unwrap_or(View {
                yaw_deg: 0.,
                pitch_deg: 0.,
                position: Some(self.explore_position),
                background: self.view.background,
                ..View::new()
            }),
        };
        cx.notify();
    }

    fn capture(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.capture.is_some() || self.state != State::Ready {
            return;
        }
        let Some(bounds) = self.viewport else {
            return;
        };
        let offset = bounds.center() - position;
        let result = raw_window_handle::HasWindowHandle::window_handle(window)
            .map_err(|_| lumilio_pointer::CaptureError::Unsupported)
            .and_then(|handle| {
                lumilio_pointer::Capture::new(
                    handle.as_raw(),
                    [offset.x.into(), offset.y.into()],
                    window.scale_factor(),
                )
            });
        match result {
            Ok(capture) => {
                self.capture = Some(capture);
                self.capture_error = None;
                self.input.clear();
                self.input.shift = window.modifiers().shift;
                self.last_tick = Instant::now();
                self.schedule_tick(window, cx);
            }
            Err(lumilio_pointer::CaptureError::Unsupported) => {
                self.capture_error = Some(tr!("model-capture-unsupported"));
            }
            Err(lumilio_pointer::CaptureError::Unavailable) => {
                self.capture_error = Some(tr!("model-capture-failed"));
            }
        }
        cx.notify();
    }

    fn schedule_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.clock_pending {
            return;
        }
        self.clock_pending = true;
        let viewer = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = viewer.update(cx, |viewer, cx| viewer.tick(window, cx));
        });
    }

    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clock_pending = false;
        if self.capture.is_none() {
            return;
        }
        if !window.is_window_active() || !self.focus.is_focused(window) {
            self.release_capture();
            cx.notify();
            return;
        }
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.last_tick)
            .min(theme::motion::WORLD_TICK);
        self.last_tick = now;
        let delta = match self.capture.as_mut().unwrap().sample() {
            Ok(delta) => delta,
            Err(_) => {
                self.release_capture();
                self.capture_error = Some(tr!("model-capture-ended"));
                cx.notify();
                return;
            }
        };
        let previous = self.view;
        self.view = self
            .input
            .advance(self.view.look(delta[0] * 0.15, delta[1] * 0.15), elapsed);
        if self.view != previous {
            self.look_dirty = true;
            cx.notify();
        }
        // The first look after capture, and each one after a move.
        if self.looked.is_none() && self.look_task.is_none() {
            self.look_dirty = true;
        }
        self.look(cx);
        // Input response is intentional motion, including under reduced motion.
        self.schedule_tick(window, cx);
    }

    fn request(&mut self, width: f32, height: f32, scale: f32) {
        // Bound readback memory and stay below the baseline GPU texture limit.
        let Some((width, height)) = physical_size(width, height, scale) else {
            return;
        };
        let request = Request {
            view: self.view,
            width,
            height,
        };
        if let Some(worker) = &self.worker
            && self.last_request != Some(request)
        {
            worker.mailbox.put(request);
            self.last_request = Some(request);
        }
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.old_images.drain(..) {
            window.drop_image(old).ok();
        }
        if !self.release_registered {
            if !self.inline {
                let focus = self.focus.clone();
                window.defer(cx, move |window, cx| window.focus(&focus, cx));
            }
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.release_capture();
                    cx.notify();
                }
            })
            .detach();
            cx.observe_window_bounds(window, |this, _, cx| {
                // A resize/move invalidates the capture anchor and native clip.
                this.release_capture();
                cx.notify();
            })
            .detach();
            cx.on_blur(&self.focus.clone(), window, |this, _, cx| {
                this.release_capture();
                cx.notify();
            })
            .detach();
            cx.on_release_in(window, |this, window, _| {
                this.release_capture();
                for image in this.old_images.drain(..).chain(this.image.take()) {
                    window.drop_image(image).ok();
                }
            })
            .detach();
            self.release_registered = true;
        }
        if !self.asked {
            self.asked = true;
            let (load, id) = (self.load.clone(), cx.entity_id().as_u64());
            window.defer(cx, move |window, cx| load(id, window, cx));
        }
        let colors = ShellColors::current(cx);
        let rgb = colors.surface.to_rgb();
        // Nucleation clears in linear RGB; GPUI's theme colours are sRGB.
        self.view.background = Some([linear(rgb.r), linear(rgb.g), linear(rgb.b), 1.]);
        let focus = colors.focus.to_rgb();
        self.selection_color = [linear(focus.r), linear(focus.g), linear(focus.b), 1.];
        self.sync_selection();
        let ink = colors.foreground.to_rgb();
        self.mark_style = (
            [linear(ink.r), linear(ink.g), linear(ink.b), 1.],
            MARK_WIDTH * window.scale_factor(),
        );
        self.sync_mark();
        let this = cx.weak_entity();
        let measure = canvas(
            move |bounds, window, cx| {
                let _ = this.update(cx, |this, _| {
                    this.viewport = Some(bounds);
                    this.request(
                        bounds.size.width.into(),
                        bounds.size.height.into(),
                        window.scale_factor(),
                    )
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let content = match &self.state {
            State::Loading => div()
                .debug_selector(|| "model-loading".into())
                .child(tr!("model-loading"))
                .into_any_element(),
            State::Ready => img(self.image.clone().unwrap())
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            State::Failed { message, detail } => v_flex()
                .debug_selector(|| "model-error".into())
                .items_center()
                .gap_3()
                .child(*message)
                .children(
                    detail
                        .as_ref()
                        .map(|detail| kit::technical("model-technical", detail.clone())),
                )
                .into_any_element(),
        };
        // ia[plugin.litematica]: Explore 准星与视线读数 | 工作室视口 · Explore 捕获中 | 画面中心有准星，读数板实时写视线中心方块的图标、名字、属性与坐标；单击检视准星下的那一个方块（描边与检视卡） | 读数在视角变化后更新（D27、T32）
        let crosshair =
            (self.inline && self.mode == Mode::Explore && self.capture.is_some()).then(|| {
                let bar = |w: f32, h: f32| {
                    div()
                        .absolute()
                        .w(px(w))
                        .h(px(h))
                        .left(px((14. - w) / 2.))
                        .top(px((14. - h) / 2.))
                        .bg(colors.foreground)
                        .border_1()
                        .border_color(colors.surface)
                };
                div()
                    .debug_selector(|| "model-crosshair".into())
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .relative()
                            .size(px(14.))
                            .child(bar(14., 3.))
                            .child(bar(3., 14.)),
                    )
            });
        // ia[instance]: 观察 3D 投影 | 3D 预览弹窗画面 | Orbital 拖拽旋转、滚轮缩放；Explore 点击捕获鼠标后第一人称转向，WASD 移动、空格上升、Shift 下降；Esc 先释放鼠标；失焦自动释放 | 自由飞行，不含重力与碰撞；动画方块暂时静止
        let viewport = div()
            .id("model-viewport")
            .role(gpui::Role::Image)
            .aria_label(tr!("model-aria"))
            .debug_selector(|| "model-viewport".into())
            .relative()
            .w_full()
            .map(|viewport| {
                if self.inline {
                    // The studio's picture fills its column, a stage with no
                    // frame of its own (D27).
                    viewport.h_full().min_h(px(96.)).rounded(px(6.))
                } else {
                    viewport.h(px(
                        (f32::from(window.viewport_size().height) - 240.).clamp(160., 640.)
                    ))
                }
            })
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .bg(colors.surface)
            .border_1()
            .border_color(if self.focus.is_focused(window) {
                colors.focus
            } else if self.inline {
                gpui::transparent_black()
            } else {
                colors.border
            })
            .track_focus(&self.focus)
            .tab_stop(true)
            .on_action(
                cx.listener(|this, _: &gpui_component::dialog::Confirm, window, cx| {
                    if this.mode == Mode::Explore && this.capture.is_none() {
                        this.capture(window.mouse_position(), window, cx);
                    }
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    if this.mode == Mode::Explore {
                        if this.capture.is_some() && this.inline {
                            this.inspect_at((0.5, 0.5), cx);
                        } else {
                            this.capture(event.position, window, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    // ia[plugin.litematica]: 框选 | 工作室视口 Orbital 下按住 Ctrl（或 ⌘）从一个方块拖到另一个方块 | 两块之间的长方体被选中：画面里描出它的边、略染上焦点色，材料面板列出选区的尺寸与材料；Esc 清除 | 选区可作替换的范围（T25）
                    if this.inline
                        && (event.modifiers.control || event.modifiers.platform)
                        && this.state == State::Ready
                    {
                        this.start_selecting(event.position, cx);
                        cx.stop_propagation();
                        return;
                    }
                    this.drag = Some(event.position);
                    this.press = Some(event.position);
                    if event.click_count == 2 {
                        this.reset(cx);
                    }
                    cx.stop_propagation();
                }),
            )
            // ia[plugin.litematica]: 检视一个方块 | 工作室视口 Orbital 下单击画面 | 在后台沿视线逐格找到第一个方块，画面里只给这一个方块描边，视口左下出现它的检视卡；材料面板的选中不变；点到空处放下 | 只检视方块，不检视实体；「只看这种」时点不到被藏起的方块（T32、D31）
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, _, cx| {
                    this.drag = None;
                    if this.selecting.is_some() {
                        this.finish_selecting(event.position, cx);
                        return;
                    }
                    let Some(press) = this.press.take() else {
                        return;
                    };
                    let moved = event.position - press;
                    if this.inline
                        && this.mode == Mode::Orbital
                        && this.state == State::Ready
                        && f32::from(moved.x).abs() < CLICK_SLOP
                        && f32::from(moved.y).abs() < CLICK_SLOP
                    {
                        this.pick(event.position, cx);
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.drag = None;
                    this.press = None;
                }),
            )
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !hovered {
                    this.leave(cx);
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if this.mode != Mode::Orbital || this.selecting.is_some() {
                    return;
                }
                if !event.dragging() {
                    this.drag = None;
                    // ia[plugin.litematica]: 指着方块看是什么 | 工作室视口 · Orbital 下移动鼠标 | 读数板写出指针下方块的图标、名字、属性与坐标；移出画面恢复操作提示 | 一次只问一个拾取，跟着指针走（D27）
                    if this.inline && this.state == State::Ready {
                        this.hover(event.position, cx);
                    }
                    return;
                }
                if let Some(previous) = this.drag {
                    this.drag = Some(event.position);
                    this.view = this.view.orbit(
                        f32::from(event.position.x - previous.x) * 0.4,
                        f32::from(event.position.y - previous.y) * 0.4,
                    );
                    cx.notify();
                }
            }))
            .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, _, cx| {
                cx.stop_propagation();
                if this.mode != Mode::Orbital {
                    return;
                }
                let delta = match event.delta {
                    ScrollDelta::Pixels(point) => f32::from(point.y),
                    ScrollDelta::Lines(point) => point.y * 24.,
                };
                this.view = this.view.zoomed((delta * 0.01).clamp(-1., 1.).exp());
                cx.stop_propagation();
                cx.notify();
            }))
            .on_key_up(cx.listener(|this, event: &gpui::KeyUpEvent, _, cx| {
                if this.capture.is_some() && this.input.key(&event.keystroke.key, false) {
                    cx.stop_propagation();
                }
            }))
            .on_modifiers_changed(
                cx.listener(|this, event: &gpui::ModifiersChangedEvent, _, _| {
                    if this.capture.is_some() {
                        if event.modifiers.control
                            || event.modifiers.platform
                            || event.modifiers.alt
                        {
                            this.input.clear();
                        } else {
                            this.input.shift = event.modifiers.shift;
                        }
                    }
                }),
            )
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                // ia[plugin.litematica]: 放下检视的方块 | 工作室视口聚焦时按 Esc（Explore 捕获中先释放鼠标） | 描边与检视卡消失，读数板回到跟着指针 | 也可点检视卡的关闭键或点画面空处（T32）
                if event.keystroke.key == "escape"
                    && this.capture.is_none()
                    && this.inspected.is_some()
                {
                    this.let_go(cx);
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.key == "escape"
                    && this.capture.is_none()
                    && this.selection.is_some()
                {
                    this.set_selection(None, cx);
                    cx.stop_propagation();
                    return;
                }
                if this.capture.is_none() && this.slice.is_some() {
                    let by = match event.keystroke.key.as_str() {
                        "[" => -1,
                        "]" => 1,
                        _ => 0,
                    };
                    if by != 0 {
                        this.step_layer(by, cx);
                        cx.stop_propagation();
                        return;
                    }
                }
                if this.mode == Mode::Explore {
                    if event.keystroke.key == "enter" && this.capture.is_none() {
                        this.capture(window.mouse_position(), window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    if this.capture.is_some() {
                        // ia[plugin.litematica]: 退出 Explore 的鼠标捕获 | 工作室视口 · Explore 捕获中按 Esc | 释放鼠标，停止移动，视线读数清空；再点画面或按 Enter 重新进入 | 只释放鼠标，不切换模式（D24）
                        if event.keystroke.key == "escape" {
                            this.release_capture();
                            cx.notify();
                            cx.stop_propagation();
                            return;
                        }
                        if event.keystroke.modifiers.control
                            || event.keystroke.modifiers.platform
                            || event.keystroke.modifiers.alt
                        {
                            this.input.clear();
                            return;
                        }
                        this.input.shift = event.keystroke.modifiers.shift;
                        if this.input.key(&event.keystroke.key, true) {
                            cx.stop_propagation();
                            return;
                        }
                        if matches!(event.keystroke.key.as_str(), "r" | "R") {
                            this.reset(cx);
                        }
                    }
                    return;
                }
                let (dx, dy, zoom) = match event.keystroke.key.as_str() {
                    "r" | "R" => {
                        this.reset(cx);
                        cx.stop_propagation();
                        return;
                    }
                    "left" => (-10., 0., 1.),
                    "right" => (10., 0., 1.),
                    "up" => (0., -10., 1.),
                    "down" => (0., 10., 1.),
                    "+" | "=" => (0., 0., 1.1),
                    "-" => (0., 0., 1. / 1.1),
                    _ => return,
                };
                this.view = this.view.orbit(dx, dy).zoomed(zoom);
                cx.stop_propagation();
                cx.notify();
            }))
            .child(content)
            .child(measure)
            .children(crosshair);
        // ia[instance]: 复位 3D 投影视角 | 弹窗「复位视角」 | 当前模式回到初始视角；Orbital 可双击画面，聚焦画面后可按 R
        let reset = Key::new("model-reset")
            .label(tr!("model-reset"))
            .ghost()
            .disabled(self.state != State::Ready)
            .on_click(cx.listener(|this, _, _, cx| this.reset(cx)));
        // ia[instance]: 重试 3D 投影预览 | 弹窗出错后的「重试」 | 重新读取当前投影与游戏贴图并生成预览；失败仍显示说明
        let retry = matches!(self.state, State::Failed { .. }).then(|| {
            Key::new("model-retry")
                .label(tr!("common-retry"))
                .ghost()
                .on_click(cx.listener(|this, _, window, cx| this.retry(window, cx)))
        });
        let note = (self.state == State::Ready)
            .then(|| undrawable_note(&self.undrawable))
            .flatten()
            .map(|(sentence, ids)| {
                h_flex()
                    .debug_selector(|| "model-undrawable".into())
                    .w_full()
                    .gap_2()
                    .child(sentence)
                    .child(kit::technical("model-undrawable-technical", ids))
            });
        let entity_note = (self.state == State::Ready)
            .then(|| undrawable_entities_note(&self.undrawable_entities))
            .flatten()
            .map(|(sentence, ids)| {
                h_flex()
                    .debug_selector(|| "model-undrawable-entities".into())
                    .w_full()
                    .gap_2()
                    .child(sentence)
                    .child(kit::technical("model-undrawable-entities-technical", ids))
            });
        let viewer = cx.weak_entity();
        // ia[instance]: 切换 3D 投影观察模式 | 预览弹窗 Orbital / Explore 分段 | 两种模式分别保留视角；切换时释放鼠标并停止移动
        let modes = Segments::new(
            "model-mode",
            &["Orbital", "Explore"],
            usize::from(self.mode == Mode::Explore),
            move |index, _, cx| {
                let _ = viewer.update(cx, |viewer, cx| {
                    viewer.select_mode(
                        if index == 0 {
                            Mode::Orbital
                        } else {
                            Mode::Explore
                        },
                        cx,
                    )
                });
            },
        );
        let angles = self.inline.then(|| {
            let viewer = cx.weak_entity();
            // ia[plugin.litematica]: 换到预设角度 | 工作室视口「角度」下拉：北、东、南、西、顶、等轴 | 转到环绕视角的该方向并重新框住整个投影；在 Explore 里会先切回 Orbital | 等轴是透视下的等轴方向，不是正交投影（T10）
            Key::new("model-angle")
                .label(tr!("model-angle"))
                .white()
                .small()
                .disabled(self.state != State::Ready)
                .debug_selector(|| "model-angle".into())
                .dropdown_menu(move |mut menu, _, _| {
                    for (angle, label) in Angle::ALL.into_iter().zip(angle_labels()) {
                        let viewer = viewer.clone();
                        menu = menu.item(PopupMenuItem::new(*label).on_click(move |_, _, cx| {
                            let _ = viewer.update(cx, |viewer, cx| viewer.turn_to(angle, cx));
                        }));
                    }
                    menu
                })
        });
        if self.inline {
            // The studio's controls float on the picture's bottom edge; its
            // read-out (what the pointer is on, else these controls' hint)
            // is the studio's, above them (D27).
            let tools = h_flex()
                .debug_selector(|| "model-tools".into())
                .id("model-tools")
                .absolute()
                .left_3()
                .bottom_3()
                .max_w(gpui::relative(0.92))
                .flex_wrap()
                .items_end()
                .gap_2()
                .p(px(6.))
                .rounded(px(6.))
                .bg(colors.body.panel)
                .border_1()
                .border_color(colors.border)
                .shadow_sm()
                // Pressing a key must not turn the model or take the cursor.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(modes)
                .children(angles)
                .child(self.slice_controls(colors, cx))
                .child(reset)
                .children(retry);
            return viewport.child(tools).into_any_element();
        }
        let hint = match self.mode {
            Mode::Orbital => tr!("model-hint-orbital"),
            Mode::Explore if self.capture.is_some() => tr!("model-hint-explore-captured"),
            Mode::Explore => tr!("model-hint-explore"),
        };
        v_flex()
            .w_full()
            .gap_2()
            .text_sm()
            .text_color(colors.muted)
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .child(modes)
                    .child(h_flex().gap_2().children(retry).child(reset)),
            )
            .child(viewport)
            .children(note)
            .children(entity_note)
            .child(div().text_xs().child(hint))
            .children(
                self.capture_error
                    .map(|message| div().text_sm().child(message)),
            )
            .into_any_element()
    }
}
