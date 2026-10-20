//! The tool surface, one file per domain. Each domain is one `ToolRouter`
//! and [`router`] adds them together, so a new domain is a new file and one
//! more term here.
//!
//! Tools answer with structured JSON so an agent reads fields rather than
//! prose. A domain failure (no such instance, an unreadable file) comes back
//! as the tool's error result, which the agent sees and can react to; only a
//! malformed request is a protocol fault.

mod instances;
mod schematics;

use lumilio_core::Transport;
use rmcp::handler::server::router::tool::ToolRouter;

use crate::LauncherMcp;

pub(crate) fn router<T: Transport + Clone>() -> ToolRouter<LauncherMcp<T>> {
    LauncherMcp::<T>::instance_tools() + LauncherMcp::<T>::schematic_tools()
}

/// A domain failure as the agent reads it.
fn failed(error: impl std::fmt::Display) -> String {
    error.to_string()
}
