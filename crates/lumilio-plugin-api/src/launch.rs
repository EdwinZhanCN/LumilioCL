//! Observation only: these events cannot change the launch result.

use serde::{Deserialize, Serialize};

use crate::{HostContext, PluginError};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LaunchTarget {
    World(String),
    Server(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LaunchOutcome {
    Clean,
    Crashed,
    FailedToStart,
    Stopped,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LaunchEvent {
    Started {
        instance_name: String,
        game_version: String,
        loader: String,
        target: Option<LaunchTarget>,
    },
    Exited {
        outcome: LaunchOutcome,
        played_seconds: u64,
    },
}

pub trait LaunchObserver: Send + Sync {
    fn observe(&self, ctx: &dyn HostContext, event: &LaunchEvent) -> Result<(), PluginError>;
}

/// A typed native operation, never an arbitrary socket path or RPC command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
pub struct DiscordActivity {
    pub application_id: String,
    pub details: Option<String>,
    pub state: Option<String>,
}

impl DiscordActivity {
    #[must_use]
    pub fn new(
        application_id: impl Into<String>,
        details: Option<String>,
        state: Option<String>,
    ) -> Self {
        Self {
            application_id: application_id.into(),
            details,
            state,
        }
    }
}
