//! Downloads for the launcher and its updater: sources, verification and
//! publication of files, independent of the launcher domain.

mod attempt;
mod budget;
mod cancel;
mod engine;
mod error_text;
mod files;
mod http;
mod partial;
mod request;
pub mod resource_lock;
mod sources;
mod transport;

#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod resume_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod user_agent_tests;

pub use self::budget::ConnectionBudget;
pub use self::cancel::CancellationToken;
pub use self::engine::{TransferBatchReport, TransferEngine};
pub use self::error_text::with_causes;
pub use self::http::{DefaultTransport, FileTransport, HttpTransport};
pub use self::partial::sweep_partials;
pub use self::request::{
    ContentDigest, RetryPolicy, SourceFailure, TransferError, TransferEvent, TransferOutcome,
    TransferRequest,
};
pub use self::sources::{OfficialSource, PrefixMirror, SourceChain, SourceProvider};
pub use self::transport::{
    ByteStream, HttpMethod, HttpRequest, Transport, TransportError, TransportFuture,
    TransportResponse,
};

use std::sync::atomic::AtomicU64;

/// Identifies the launcher to servers; some APIs (Modrinth) reject anonymous
/// clients.
pub const USER_AGENT: &str = concat!("LumilioCL/", env!("CARGO_PKG_VERSION"));

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
