//! Tabs that plugins contribute to the instance page.
//!
//! A plugin only describes content (a view tree); this module is the one
//! renderer that turns it into kit components. Actions go out as intents, and
//! the application runs the plugin and answers with the new view.

mod images;

mod plugin_pages;

#[cfg(test)]
pub(super) use plugin_pages::PluginPage;
pub(super) use plugin_pages::PluginPages;
use plugin_pages::{COVER, THUMB};
