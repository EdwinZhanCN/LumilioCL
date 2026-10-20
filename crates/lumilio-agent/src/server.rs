//! The MCP server around one [`LauncherService`]: identity, instructions and
//! the tool router. One instance serves one session; the transport makes
//! them.

use std::sync::Arc;

use lumilio_core::{LauncherService, Transport};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool_handler};

use crate::tools;

/// What a connecting agent reads first.
const INSTRUCTIONS: &str = "LumilioCL is a Minecraft launcher. Its tools read the library of \
game instances and the Litematica schematics (.litematic files) each instance holds. Instance \
ids come from list_instances; schematic paths come from list_schematics and are relative to \
the instance's game directory.";

/// The launcher's tools, served to one MCP session.
pub struct LauncherMcp<T: Transport + Clone> {
    service: Arc<LauncherService<T>>,
    router: ToolRouter<Self>,
}

impl<T: Transport + Clone> LauncherMcp<T> {
    #[must_use]
    pub fn new(service: Arc<LauncherService<T>>) -> Self {
        Self {
            service,
            router: tools::router(),
        }
    }

    pub(crate) fn service(&self) -> &LauncherService<T> {
        &self.service
    }
}

#[tool_handler(router = self.router)]
impl<T: Transport + Clone> ServerHandler for LauncherMcp<T> {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("LumilioCL", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}
