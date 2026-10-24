//! The Settings tab of an Instance (`docs/ia/instance/settings.md`): six
//! groups of rows. Each shows the value in effect and where it comes from;
//! anything typed is edited in a dialog, and "恢复默认" removes the override
//! instead of copying the launcher default into the instance. 选项 holds the
//! game's own options (`options.txt`), edited while the game is closed.
//!
//! A child module of `instance_detail`, so it shares the view's private state.

mod groups;
mod java;
mod options;
mod saves;
mod texts;

#[cfg(test)]
mod tests;

use choices::{AFTER_LAUNCH, FOLLOW_OR_OWN, FULLSCREEN, QUICK_PLAY};
pub use choices::{
    SETTINGS_ADVANCED, SETTINGS_GAME, SETTINGS_JAVA, SETTINGS_OPTIONS, SETTINGS_PERFORMANCE,
    SETTINGS_RUNTIME, SUBTABS,
};

pub(super) use self::java::JavaPick;

mod choices;
