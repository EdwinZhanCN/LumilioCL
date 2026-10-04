mod engine;
mod files;
mod http;
mod request;
mod sources;
mod transport;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod user_agent_tests;

pub use self::engine::{TransferBatchReport, TransferEngine};
pub use self::http::{DefaultTransport, FileTransport, HttpTransport};
pub use self::request::{
    RetryPolicy, SourceFailure, TransferError, TransferEvent, TransferOutcome, TransferRequest,
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
