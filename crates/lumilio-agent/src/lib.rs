//! The launcher as a Model Context Protocol server (plan agent-mcp).
//!
//! Every capability an agent may use goes through MCP, whether the agent is
//! the launcher's own or an external one such as Claude Code: there is no
//! private tool channel, so both see the same tools and the same results.
//! The server wraps one [`LauncherService`](lumilio_core::LauncherService)
//! and exposes read-only tools over its instances and schematics. The one
//! transport is streamable HTTP on the loopback interface ([`serve`]), which
//! the running launcher hosts.

mod access;
mod http;
mod server;
#[cfg(test)]
mod tests;
mod tools;

pub use access::Connection;
pub use http::{PATH, Running, ServeError, serve};
pub use server::LauncherMcp;
