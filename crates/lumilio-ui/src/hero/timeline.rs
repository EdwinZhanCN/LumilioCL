//! The carousel clock: which slide is on screen, how long it has been alive,
//! and whether a chunk-loading transition is in flight.
//!
//! Time only enters through [`Timeline::tick`], so the whole state machine is
//! deterministic under test.

/// Seconds a slide stays on screen before auto-advancing.
pub const DWELL_SECONDS: f32 = 9.5;
/// Seconds the chunk-loading transition takes.
pub const TRANSITION_SECONDS: f32 = 1.2;
/// Longest step accepted from the frame clock; larger gaps (the window was
/// hidden, Home was off screen) resume instead of skipping ahead.
pub const MAX_STEP_SECONDS: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub from: usize,
    /// Scene clock of the outgoing slide, which keeps animating while it leaves.
    pub from_age: f32,
    /// Seconds since the transition started.
    pub elapsed: f32,
}

impl Transition {
    pub fn progress(&self) -> f32 {
        (self.elapsed / TRANSITION_SECONDS).clamp(0., 1.)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    slides: usize,
    dwell_seconds: f32,
    current: usize,
    /// Scene clock of the current slide, starting at zero when it is chosen.
    age: f32,
    dwell: f32,
    transition: Option<Transition>,
    paused: bool,
}

impl Timeline {
    pub fn new(slides: usize) -> Self {
        Self::with_dwell(slides, DWELL_SECONDS)
    }

    /// A timeline with a custom dwell. With a single slide, the dwell ending
    /// replays that slide through the transition instead of advancing.
    pub fn with_dwell(slides: usize, dwell_seconds: f32) -> Self {
        Self {
            slides: slides.max(1),
            dwell_seconds: dwell_seconds.max(TRANSITION_SECONDS),
            current: 0,
            age: 0.,
            dwell: 0.,
            transition: None,
            paused: false,
        }
    }

    pub fn current(&self) -> usize {
        self.current
    }

    pub fn age(&self) -> f32 {
        self.age
    }

    pub fn transition(&self) -> Option<Transition> {
        self.transition
    }

    /// Hovering pauses auto-advance only; the scene itself keeps living.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Fraction of the dwell that has elapsed, for the indicator fill.
    pub fn dwell_progress(&self) -> f32 {
        (self.dwell / self.dwell_seconds).clamp(0., 1.)
    }

    pub fn tick(&mut self, seconds: f32) {
        let step = seconds.clamp(0., MAX_STEP_SECONDS);
        self.age += step;

        if let Some(transition) = self.transition.as_mut() {
            transition.from_age += step;
            transition.elapsed += step;
            if transition.elapsed >= TRANSITION_SECONDS {
                self.transition = None;
            }
            return;
        }

        if !self.paused {
            self.dwell += step;
            if self.dwell >= self.dwell_seconds {
                self.switch((self.current + 1) % self.slides, true);
            }
        }
    }

    /// Selects a slide. With `animate` the outgoing slide streams out through
    /// the chunk transition; without it the change is immediate.
    pub fn go_to(&mut self, slide: usize, animate: bool) {
        let slide = slide % self.slides;
        if slide != self.current {
            self.switch(slide, animate);
        }
    }

    /// Restarts the scene clock without a transition.
    pub fn restart(&mut self) {
        self.transition = None;
        self.age = 0.;
        self.dwell = 0.;
    }

    fn switch(&mut self, slide: usize, animate: bool) {
        self.transition = animate.then_some(Transition {
            from: self.current,
            from_age: self.age,
            elapsed: 0.,
        });
        self.current = slide;
        self.age = 0.;
        self.dwell = 0.;
    }
}

#[cfg(test)]
mod tests;
