//! Decides what the hero shows for each Home state (design language §5).
//!
//! The director is pure state: time enters only through [`Director::tick`],
//! so every mode change, the launch unload/reload, and the eased chunk reveal
//! are tested without a window.

use super::FrameSpec;
use super::scenes::Scene;
use super::timeline::{TRANSITION_SECONDS, Timeline};
use super::transition;

/// A focused scene replays itself (through a chunk reload) this often.
const FOCUS_DWELL: f32 = 12.;
/// How long the visible world takes to unload when a launch begins.
pub const UNLOAD_SECONDS: f32 = 0.45;
/// Exponential follow rates (per second) of the displayed chunk reveal.
const LOAD_FOLLOW: f32 = 3.5;
const SETTLE_FOLLOW: f32 = 6.;
const SNAP: f32 = 0.002;
/// Follow rates of the Continue flare: quick to catch, slower to settle.
const FLARE_RISE: f32 = 7.;
const FLARE_FALL: f32 = 3.;
/// Brightness of a dimmed still frame.
pub const DIM: f32 = 0.55;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HeroMode {
    /// Carousel of every scene; for first use and before Home has data.
    Showcase,
    /// One scene, replaying; Home has something to continue.
    Focus(Scene),
    /// The scene's world loads chunk by chunk with launch progress in `[0, 1]`.
    Loading { scene: Scene, progress: f32 },
    /// A composed frame that does not tick (game running, recovery).
    Still { scene: Scene, age: f32, dim: bool },
}

impl HeroMode {
    fn scene(self, showcase: &Timeline) -> Scene {
        match self {
            Self::Showcase => Scene::ALL[showcase.current()],
            Self::Focus(scene) | Self::Loading { scene, .. } | Self::Still { scene, .. } => scene,
        }
    }

    /// Modes that run on the focus clock share scene time with each other.
    fn uses_focus_clock(self) -> bool {
        matches!(self, Self::Focus(_) | Self::Loading { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Entering {
    from: Scene,
    from_age: f32,
    elapsed: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Director {
    mode: HeroMode,
    showcase: Timeline,
    focus: Timeline,
    /// Share of the world that is loaded on screen, in `[0, 1]`.
    reveal: f32,
    /// Seconds into the unload that opens a launch, while it runs.
    unload: Option<f32>,
    entering: Option<Entering>,
    /// Displayed and wanted flare (pointer resting on Continue), in `[0, 1]`.
    flare: f32,
    flare_target: f32,
}

impl Default for Director {
    fn default() -> Self {
        Self::new()
    }
}

impl Director {
    pub fn new() -> Self {
        Self {
            mode: HeroMode::Showcase,
            showcase: Timeline::new(Scene::ALL.len()),
            focus: Timeline::with_dwell(1, FOCUS_DWELL),
            reveal: 1.,
            unload: None,
            entering: None,
            flare: 0.,
            flare_target: 0.,
        }
    }

    /// The pointer rests on (or leaves) Continue. Only a focused scene flares.
    pub fn set_flare(&mut self, on: bool, reduce_motion: bool) -> bool {
        let target = if on && matches!(self.mode, HeroMode::Focus(_)) {
            1.
        } else {
            0.
        };
        if target == self.flare_target {
            return false;
        }
        self.flare_target = target;
        if reduce_motion {
            self.flare = target;
        }
        true
    }

    pub fn mode(&self) -> HeroMode {
        self.mode
    }

    pub fn showcase(&self) -> &Timeline {
        &self.showcase
    }

    #[cfg(test)]
    pub fn reveal(&self) -> f32 {
        self.reveal
    }

    fn scene(&self) -> Scene {
        self.mode.scene(&self.showcase)
    }

    fn age(&self) -> f32 {
        match self.mode {
            HeroMode::Showcase => self.showcase.age(),
            HeroMode::Focus(_) | HeroMode::Loading { .. } => self.focus.age(),
            HeroMode::Still { age, .. } => age,
        }
    }

    /// Switches mode. Returns whether anything changed.
    pub fn set_mode(&mut self, mode: HeroMode, reduce_motion: bool) -> bool {
        if mode == self.mode {
            return false;
        }
        let previous = self.mode;
        let (from, from_age) = (self.scene(), self.age());
        let next_scene = mode.scene(&self.showcase);

        let continuous = match (previous, mode) {
            // Progress updates within one launch keep everything running.
            (HeroMode::Loading { scene: a, .. }, HeroMode::Loading { scene: b, .. }) => a == b,
            (a, b) if a.uses_focus_clock() && b.uses_focus_clock() => from == next_scene,
            (
                HeroMode::Still {
                    scene: a, age: x, ..
                },
                HeroMode::Still {
                    scene: b, age: y, ..
                },
            ) => a == b && x == y,
            _ => false,
        };

        if !continuous {
            if mode.uses_focus_clock() {
                self.focus.restart();
            }
            self.entering = (!reduce_motion).then_some(Entering {
                from,
                from_age,
                elapsed: 0.,
            });
        }

        let launch_begins = matches!(mode, HeroMode::Loading { .. })
            && !matches!(previous, HeroMode::Loading { .. });
        if launch_begins {
            // The visible world unloads, then streams back with progress.
            self.entering = None;
            self.unload = (!reduce_motion).then_some(0.);
        }
        if !matches!(mode, HeroMode::Loading { .. }) {
            self.unload = None;
        }
        self.focus
            .set_paused(matches!(mode, HeroMode::Loading { .. }));
        if !matches!(mode, HeroMode::Focus(_)) {
            // The Continue button is gone, so its hover can no longer end.
            self.flare_target = 0.;
        }
        self.mode = mode;
        if reduce_motion {
            self.reveal = self.reveal_target();
        }
        true
    }

    fn reveal_target(&self) -> f32 {
        match self.mode {
            HeroMode::Loading { progress, .. } => progress.clamp(0., 1.),
            _ => 1.,
        }
    }

    /// Advances clocks by `seconds`.
    pub fn tick(&mut self, seconds: f32, reduce_motion: bool) {
        if reduce_motion {
            self.entering = None;
            self.unload = None;
            self.reveal = self.reveal_target();
            self.flare = self.flare_target;
            return;
        }
        let step = seconds.clamp(0., super::timeline::MAX_STEP_SECONDS);
        let rate = if self.flare_target > self.flare {
            FLARE_RISE
        } else {
            FLARE_FALL
        };
        self.flare += (self.flare_target - self.flare) * (1. - (-step * rate).exp());
        if (self.flare_target - self.flare).abs() < SNAP {
            self.flare = self.flare_target;
        }
        match self.mode {
            HeroMode::Showcase => self.showcase.tick(step),
            HeroMode::Focus(_) | HeroMode::Loading { .. } => self.focus.tick(step),
            HeroMode::Still { .. } => {}
        }

        if let Some(entering) = self.entering.as_mut() {
            entering.elapsed += step;
            entering.from_age += step;
            if entering.elapsed >= TRANSITION_SECONDS {
                self.entering = None;
            }
        }

        if let Some(elapsed) = self.unload.as_mut() {
            *elapsed += step;
            let remaining = (1. - *elapsed / UNLOAD_SECONDS).max(0.);
            self.reveal = self.reveal.min(remaining);
            if remaining == 0. {
                self.unload = None;
            }
            return;
        }

        let target = self.reveal_target();
        let rate = if matches!(self.mode, HeroMode::Loading { .. }) {
            LOAD_FOLLOW
        } else {
            SETTLE_FOLLOW
        };
        self.reveal += (target - self.reveal) * (1. - (-step * rate).exp());
        if (target - self.reveal).abs() < SNAP {
            self.reveal = target;
        }
    }

    /// Whether another frame would look different from this one.
    pub fn animating(&self) -> bool {
        !matches!(self.mode, HeroMode::Still { .. })
            || self.entering.is_some()
            || self.unload.is_some()
            || self.reveal != self.reveal_target()
            || self.flare != self.flare_target
    }

    pub fn frame(&self, reduce_motion: bool) -> FrameSpec {
        let scene = self.scene();
        let age = match self.mode {
            HeroMode::Still { age, .. } => age,
            _ if reduce_motion => scene.still_age(),
            _ => self.age(),
        };
        let leaving = if reduce_motion {
            None
        } else if let Some(entering) = self.entering {
            Some((
                entering.from,
                entering.from_age,
                (entering.elapsed / TRANSITION_SECONDS).clamp(0., 1.),
            ))
        } else if self.mode == HeroMode::Showcase {
            self.showcase
                .transition()
                .map(|t| (Scene::ALL[t.from], t.from_age, t.progress()))
        } else if matches!(self.mode, HeroMode::Focus(_)) {
            // A focused scene replays by reloading itself.
            self.focus
                .transition()
                .map(|t| (scene, t.from_age, t.progress()))
        } else {
            None
        };
        FrameSpec {
            scene,
            age,
            leaving,
            reveal: self.reveal,
            dim: matches!(self.mode, HeroMode::Still { dim: true, .. }),
            flare: self.flare,
        }
    }

    /// The live readout for the current mode, given the texel grid size.
    pub fn hud(&self, size: (i32, i32)) -> Option<String> {
        match self.mode {
            HeroMode::Showcase | HeroMode::Focus(_) => Some(self.scene().hud(self.age(), size)),
            HeroMode::Loading { .. } => {
                let (loaded, total) = transition::loaded_chunks(
                    size.0.max(1) as usize,
                    size.1.max(1) as usize,
                    self.reveal,
                );
                Some(format!("区块 {loaded}/{total}"))
            }
            HeroMode::Still { .. } => None,
        }
    }

    pub fn select(&mut self, slide: usize, reduce_motion: bool) {
        if self.mode == HeroMode::Showcase {
            self.showcase.go_to(slide, !reduce_motion);
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.showcase.set_paused(paused);
    }
}

#[cfg(test)]
mod tests;
