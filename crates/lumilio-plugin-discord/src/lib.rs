//! Presence policy only; the host owns the Discord connection.

mod text;

#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::sync::Mutex;

#[cfg(test)]
use discord::activity;
pub use discord::{DEFAULT_APPLICATION_ID, Discord, ID};
#[cfg(test)]
use lumilio_plugin_api::{
    DiscordActivity, HostContext, LaunchEvent, LaunchObserver, LaunchTarget, NativeCapability,
    Permission, Plugin, PluginError, SettingValue,
};

#[cfg(test)]
mod tests;

mod discord;
