//! Home presentation states, their launch transitions, and their rendering.
//!
//! Home is the fastest route from opening the launcher to playing
//! (design language §5). The hero reflects the state; Continue, Launching,
//! Playing, and Recovery put their foreground directly on the hero, and the
//! launch moment is driven by `lumilio_core::LaunchSession`.

mod buttons;
mod launching;
mod lists;
mod recovery;
mod render;

#[cfg(test)]
mod tests;

pub use presentation::{
    ActHandler, AttentionRow, HomeIntent, HomeIntentHandler, HomeLinks, HomePresentation,
    HoverHandler, OpenHandler, RecoveryDetail, Subject, WorldHint, initial_presentation,
};

pub use self::launching::{
    BAR_SEGMENTS, filled_segments, phase_headline, phase_label, play_minutes,
};
pub use self::recovery::recovery_sentence;
pub use self::render::{ShellHomeColors, body_label, on_art, render_body, render_overlay};

mod presentation;
