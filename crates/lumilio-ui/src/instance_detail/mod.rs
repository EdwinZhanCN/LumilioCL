//! Real Instance Overview and Settings.Performance, bound to a stable ID.

mod actions;
mod content;
mod data;
mod diagnostics;
mod editors;
mod forms;
mod intent;
mod overview;
mod panels;
mod performance;
mod plugin_tabs;
mod render;
mod settings;

#[cfg(test)]
mod tests;

use instance_detail_view::CrashRead;
pub use instance_detail_view::{
    Dropdown, InstanceDetailView, InstanceHandler, Operated, TAB_CONTENT, TAB_DIAGNOSTICS,
    TAB_HISTORY, TAB_OVERVIEW, TAB_SCREENSHOTS, TAB_SETTINGS, TAB_WORLDS, TABS, Tabs,
};

pub use self::intent::{ContentUpdateItem, InstanceIntent, Section};
pub use self::panels::{Arrived, ProblemAction, export_notice, problem_action, problem_text};
pub use self::settings::{
    SETTINGS_ADVANCED, SETTINGS_GAME, SETTINGS_JAVA, SETTINGS_OPTIONS, SETTINGS_PERFORMANCE,
    SETTINGS_RUNTIME,
};

mod instance_detail_view;
