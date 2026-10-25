//! Root shell: route state, Home snapshot, and presentation-only intents.

mod account_view;
mod chips;
mod chrome;
mod controls;
mod current;
mod detail;
mod features;
mod home_page;
mod instance;
mod locations;
mod memory;
mod mirror;
mod queries;
mod render;
mod routing;

#[cfg(test)]
mod tests;

pub use launcher_shell::{ActivitySummary, IntentHandler, LauncherShell, ShellIntent};

pub(crate) use self::chrome::sync_appearance;
pub use self::chrome::{build_root, follow_system_appearance, window_titlebar};
#[cfg(test)]
use crate::route::Route;

gpui::actions!(lumilio, [Quit]);

mod launcher_shell;
