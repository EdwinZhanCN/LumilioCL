use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::{App, Bounds, Context, FocusHandle, Pixels, Point, RenderImage, Task, Window};
use lumilio_core::{AssetNotice, AssetPlan, ModelPreview};
use lumilio_schematic_render::{Angle, Focus, Frame, Picked, SceneError, Slice, SliceAxis, View};

use super::input::{FlightInput, Mode};
use super::slice::Cells;
use super::worker::{Aim, Event, Job, Request, Worker};
use super::{slice, worker};
use crate::{kit, tr};

pub(super) type Load = Rc<dyn Fn(u64, &mut Window, &mut App)>;
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

/// The height the inline tool plates take at the picture's bottom edge
/// (12 px margin, 55 px of keys with their LEDs), with a gap: what a
/// read-out above them clears.
pub(crate) const TOOL_STRIP_CLEARANCE: f32 = 76.;

/// [`TOOL_STRIP_CLEARANCE`] while slicing: the trailing plate holds the
/// layer stepper over its keys (36 px more).
pub(crate) const SLICING_CLEARANCE: f32 = 112.;

/// A press that moves less than this is a click, not a drag.
pub(super) const CLICK_SLOP: f32 = 4.;

/// The inspected block's outline, in logical pixels (T32).
pub(super) const MARK_WIDTH: f32 = 2.;

/// The outline's colour (the theme's ink in linear RGB) and its width in the
/// frame's pixels.
pub(super) type MarkStyle = ([f32; 4], f32);

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

pub(super) fn physical_size(width: f32, height: f32, scale: f32) -> Option<(u32, u32)> {
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
pub(super) enum State {
    Loading,
    Ready,
    Failed {
        message: &'static str,
        detail: Option<String>,
    },
}

impl State {
    pub(super) fn failed(error: SceneError) -> Self {
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
pub(super) fn undrawable_note(undrawable: &[String]) -> Option<(String, String)> {
    (!undrawable.is_empty()).then(|| {
        (
            tr!("model-undrawable", count = undrawable.len()),
            undrawable.join("\n"),
        )
    })
}

/// The same for entities nothing draws (plan schematic-studio T8).
pub(super) fn undrawable_entities_note(undrawable: &[String]) -> Option<(String, String)> {
    (!undrawable.is_empty()).then(|| {
        (
            tr!("model-undrawable-entities", count = undrawable.len()),
            undrawable.join("\n"),
        )
    })
}

pub(crate) struct Viewer {
    pub(super) load: Load,
    /// Inline in a page (the studio's viewport): fills its column and leaves
    /// focus where the person put it. Otherwise it sits in a modal.
    pub(super) inline: bool,
    pub(super) asked: bool,
    pub(super) state: State,
    /// Block IDs the game's assets cannot draw, reported once per load.
    pub(super) undrawable: Vec<String>,
    /// Entity IDs nothing draws, reported once per load.
    pub(super) undrawable_entities: Vec<String>,
    pub(super) view: View,
    pub(super) worker: Option<Worker>,
    pub(super) task: Option<Task<()>>,
    pub(super) image: Option<Arc<RenderImage>>,
    pub(super) old_images: Vec<Arc<RenderImage>>,
    pub(super) last_request: Option<Request>,
    pub(super) drag: Option<Point<Pixels>>,
    pub(super) focus: FocusHandle,
    pub(super) release_registered: bool,
    pub(super) mode: Mode,
    pub(super) explore_position: [f32; 3],
    pub(super) orbit_view: View,
    pub(super) flight_view: Option<View>,
    pub(super) input: FlightInput,
    pub(super) capture: Option<lumilio_pointer::Capture>,
    pub(super) capture_error: Option<&'static str>,
    pub(super) viewport: Option<Bounds<Pixels>>,
    pub(super) last_tick: Instant,
    pub(super) clock_pending: bool,
    /// Where the blocks come from, once the assets arrived.
    pub(super) plan: Option<AssetPlan>,
    pub(super) on_pointed: Option<OnPointed>,
    /// Where the left button went down, to tell a click from a drag.
    pub(super) press: Option<Point<Pixels>>,
    pub(super) pick_task: Option<Task<()>>,
    /// The block the sight line hits in Explore, for the read-out (D24).
    pub(super) looked: Option<Picked>,
    pub(super) look_task: Option<Task<()>>,
    /// The camera moved since the last sight-line pick.
    pub(super) look_dirty: bool,
    /// The block under the pointer in Orbital (studio only), for the
    /// read-out.
    pub(super) hovered: Option<Picked>,
    /// Where the pointer is, as a fraction of the picture, while it is over
    /// it; one pick at a time follows it.
    pub(super) hover_at: Option<(f32, f32)>,
    pub(super) hover_task: Option<Task<()>>,
    /// Which blocks keep their colour (studio only, T31); sent to the
    /// worker when it changes and again when a new worker starts.
    pub(super) block_focus: Focus,
    /// The block the last click inspected (studio only, T32).
    pub(super) inspected: Option<Inspected>,
    pub(super) mark_style: MarkStyle,
    /// The outline the worker was last told to draw.
    pub(super) mark_sent: Option<([i32; 3], MarkStyle)>,
    /// A block to inspect once the picture is drawn (the review hook).
    pub(super) pending_inspect: Option<[i32; 3]>,
    /// Where the outlined block is in the frame on screen.
    pub(super) mark_rect: Option<[f32; 4]>,
    /// The schematic's cells, both corners included, once loaded (T25).
    pub(super) cells: Option<Cells>,
    /// The layers drawn, while slicing (T25).
    pub(super) slice: Option<Slice>,
    /// The axis 切片 slices along next.
    pub(super) slice_axis: SliceAxis,
    /// The blocks in the slice's layer, by id.
    pub(super) layer_blocks: Vec<(String, u64)>,
    pub(super) slice_task: Option<Task<()>>,
    /// 切片's dropdown and the layer field (studio only), made once a draw
    /// has a window; `asked` while that is on its way.
    pub(super) slice_controls: Option<slice::SliceControls>,
    pub(super) slice_controls_asked: bool,
    /// The selected box of cells (T25).
    pub(super) selection: Option<Cells>,
    /// The selection's colour, the theme's focus colour in linear RGB.
    pub(super) selection_color: [f32; 4],
    pub(super) selection_sent: Option<(Cells, [f32; 4])>,
    /// The blocks in the selection, by id.
    pub(super) selection_blocks: Vec<(String, u64)>,
    pub(super) selection_task: Option<Task<()>>,
    /// A Ctrl-drag selecting a box.
    pub(super) selecting: Option<slice::SelectDrag>,
    pub(super) select_task: Option<Task<()>>,
}

impl Viewer {
    pub(super) fn new(load: Load, cx: &mut Context<Self>) -> Self {
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
            slice_controls: None,
            slice_controls_asked: false,
            selection: None,
            selection_color: [0., 0., 1., 1.],
            selection_sent: None,
            selection_blocks: Vec::new(),
            selection_task: None,
            selecting: None,
            select_task: None,
        }
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

    pub(super) fn frame_arrived(&mut self, result: worker::Output) {
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

    pub(super) fn reset(&mut self, cx: &mut Context<Self>) {
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

    /// 角度's menu in the studio: 复位视角 for the current mode, then the
    /// presets, which turn the orbit camera (Explore gives way to Orbital).
    pub(crate) fn angle_entries(&self, cx: &mut Context<Self>) -> Vec<kit::MenuEntry> {
        let ready = self.state == State::Ready;
        let viewer = cx.weak_entity();
        let mut entries = vec![
            kit::MenuEntry::new(tr!("model-reset-key"), move |_, cx| {
                let _ = viewer.update(cx, |viewer, cx| viewer.reset(cx));
            })
            .disabled(!ready),
        ];
        for (index, (angle, label)) in Angle::ALL.into_iter().zip(angle_labels()).enumerate() {
            let viewer = cx.weak_entity();
            let entry = kit::MenuEntry::new(*label, move |_, cx| {
                let _ = viewer.update(cx, |viewer, cx| viewer.turn_to(angle, cx));
            })
            .disabled(!ready);
            entries.push(if index == 0 { entry.separated() } else { entry });
        }
        entries
    }

    /// The room the studio's floating tools take at the picture's bottom,
    /// which its read-out and an inspect card in the corner clear.
    pub(crate) fn tool_clearance(&self) -> f32 {
        if self.slice.is_some() {
            SLICING_CLEARANCE
        } else {
            TOOL_STRIP_CLEARANCE
        }
    }

    pub(super) fn retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn request(&mut self, width: f32, height: f32, scale: f32) {
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
