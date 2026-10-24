use std::time::Duration;

use gpui::{Pixels, px};

/// Hover, press, and focus feedback.
pub const INSTANT: Duration = Duration::from_millis(90);
/// Route content fade and small state swaps.
pub const QUICK: Duration = Duration::from_millis(160);
/// Panels and morphs such as button → progress bar.
pub const SETTLE: Duration = Duration::from_millis(280);
/// Hero copy entrances and large narrative changes.
pub const SCENE: Duration = Duration::from_millis(700);
/// Frame interval of world-register animation: the game's 20 ticks/s.
pub const WORLD_TICK: Duration = Duration::from_millis(50);
/// Entrance travel distance.
pub const LIFT: Pixels = px(8.);

#[cfg(test)]
mod tests;
