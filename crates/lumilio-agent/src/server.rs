//! The MCP server around one [`LauncherService`]: identity, instructions and
//! the tool router, plus the stdio transport for a child-process host.

use std::fmt;
use std::sync::Arc;

use lumilio_core::{LauncherService, Transport};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, ServiceExt, tool_handler};

use crate::tools;

/// What a connecting agent reads first.
const INSTRUCTIONS: &str = "LumilioCL is a Minecraft launcher. Its tools read the library of \
game instances and the Litematica schematics (.litematic files) each instance holds. Instance \
ids come from list_instances; schematic paths come from list_schematics and are relative to \
the instance's game directory.";

/// The launcher's tools, served to one MCP client.
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

/// Serves `service` over this process's standard streams until the client
/// disconnects. Stdout belongs to the protocol while this runs: nothing else
/// may print there.
pub async fn serve_stdio<T: Transport + Clone>(
    service: Arc<LauncherService<T>>,
) -> Result<(), ServeError> {
    let running = LauncherMcp::new(service)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|error| ServeError::Initialize(error.to_string()))?;
    running
        .waiting()
        .await
        .map_err(|error| ServeError::Stopped(error.to_string()))?;
    Ok(())
}

/// Why a stdio session did not run to a clean close.
#[derive(Debug)]
pub enum ServeError {
    /// The client never completed the MCP handshake.
    Initialize(String),
    /// The session ended with a fault instead of a close.
    Stopped(String),
}

impl fmt::Display for ServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Initialize(reason) => write!(f, "the MCP handshake failed: {reason}"),
            Self::Stopped(reason) => write!(f, "the MCP session failed: {reason}"),
        }
    }
}

impl std::error::Error for ServeError {}
