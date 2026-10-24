use std::time::Instant;

use gpui::{Context, Pixels, Point, Window};
use lumilio_schematic_render::{Picked, View};

use super::input::Mode;
use super::worker::{Aim, Job};
use super::{State, Viewer};
use crate::{theme, tr};

impl Viewer {
    /// A point in the window as a fraction of the picture, once laid out.
    pub(super) fn fraction(&self, position: Point<Pixels>) -> Option<(f32, f32)> {
        let bounds = self.viewport?;
        let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        (width >= 1. && height >= 1.).then(|| {
            (
                f32::from(position.x - bounds.origin.x) / width,
                f32::from(position.y - bounds.origin.y) / height,
            )
        })
    }

    /// Tells the worker the outline the picture should have, on a change:
    /// the inspected block in the theme's ink, or none.
    pub(super) fn sync_mark(&mut self) {
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

    pub(super) fn tell(&self, cx: &mut Context<Self>) {
        if let Some(on_pointed) = self.on_pointed.clone() {
            on_pointed(cx);
        }
        cx.notify();
    }

    /// Follows the pointer over the picture with one pick at a time; a move
    /// while one is out is picked when it returns.
    pub(super) fn hover(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
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

    pub(super) fn ask_hover(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn leave(&mut self, cx: &mut Context<Self>) {
        self.hover_at = None;
        if self.hovered.take().is_some() {
            self.tell(cx);
        }
    }

    /// Asks the worker to inspect the block under `position`, a point in
    /// the window.
    pub(super) fn pick(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
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
    pub(super) fn inspect_at(&mut self, at: (f32, f32), cx: &mut Context<Self>) {
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

    pub(super) fn inspect(&mut self, aim: Aim, cx: &mut Context<Self>) {
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
    pub(super) fn look(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn ask_pick(
        &self,
        at: (f32, f32),
    ) -> Option<async_channel::Receiver<Option<Picked>>> {
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

    pub(super) fn release_capture(&mut self) {
        self.capture = None;
        self.input.clear();
        self.drag = None;
        self.looked = None;
        self.look_dirty = false;
    }

    pub(super) fn select_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
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

    pub(super) fn capture(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    pub(super) fn schedule_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.clock_pending {
            return;
        }
        self.clock_pending = true;
        let viewer = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = viewer.update(cx, |viewer, cx| viewer.tick(window, cx));
        });
    }

    pub(super) fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clock_pending = false;
        let Some(capture) = self.capture.as_mut() else {
            return;
        };
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
        let delta = match capture.sample() {
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
}
