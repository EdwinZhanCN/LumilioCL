//! The one transport: streamable HTTP on the loopback interface, inside the
//! running launcher. External agents connect by URL with a bearer token;
//! the launcher's own agent will take the same path. Nothing listens on
//! other interfaces, every request must carry the token, and a browser
//! origin is refused outright (DNS rebinding), so a web page cannot reach
//! the library through the person's browser.

use std::convert::Infallible;
use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use bytes::Bytes;
use http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use http::{Request, Response, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use lumilio_core::{LauncherService, Transport};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::access::{self, Connection};
use crate::server::LauncherMcp;

/// The endpoint below the address.
pub const PATH: &str = "/mcp";
/// How many ports above the configured one are tried when it is busy.
const PORT_TRIES: u16 = 20;

/// A listening server. Dropping it stops listening and ends every session;
/// [`shutdown`](Self::shutdown) also waits until the listener is gone.
pub struct Running {
    connection: Connection,
    port: u16,
    cancel: CancellationToken,
    accepting: Option<tokio::task::JoinHandle<()>>,
}

impl Running {
    /// The MCP endpoint, such as `http://127.0.0.1:27315/mcp`.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.connection.url
    }

    /// The bearer token requests must carry.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.connection.token
    }

    /// The port in use: the configured one, or the next free one above it.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Stops listening and waits for the listener to let go of the service,
    /// so the data folder can be opened again right after.
    pub async fn shutdown(mut self) {
        self.cancel.cancel();
        if let Some(accepting) = self.accepting.take() {
            let _ = accepting.await;
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Listens on `127.0.0.1:port`, or the next free port above it; `0` takes
/// any free port. The token is read from, or first written to,
/// `agent/mcp.json` under the service's data root, with the address.
pub async fn serve<T: Transport + Clone>(
    service: Arc<LauncherService<T>>,
    port: u16,
) -> Result<Running, ServeError> {
    let root = service.layout().root().to_path_buf();
    let token = match access::existing_token(&root) {
        Some(token) => token,
        None => access::new_token().map_err(ServeError::Access)?,
    };
    let (listener, address) = bind(port).await?;
    let connection = Connection {
        url: format!("http://{address}{PATH}"),
        token,
    };
    access::write(&root, &connection).map_err(ServeError::Access)?;

    let cancel = CancellationToken::new();
    let config = StreamableHttpServerConfig::default()
        .enforce_origin_validation()
        .with_cancellation_token(cancel.child_token());
    let mcp = StreamableHttpService::new(
        move || Ok(LauncherMcp::new(service.clone())),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let guard = Guard {
        token: Arc::from(connection.token.as_str()),
        mcp,
    };
    let accepting = tokio::spawn(accept(listener, guard, cancel.clone()));
    Ok(Running {
        connection,
        port: address.port(),
        cancel,
        accepting: Some(accepting),
    })
}

async fn bind(port: u16) -> Result<(TcpListener, SocketAddr), ServeError> {
    let last = if port == 0 {
        0
    } else {
        port.saturating_add(PORT_TRIES)
    };
    let mut failure = None;
    for candidate in port..=last {
        match TcpListener::bind((Ipv4Addr::LOCALHOST, candidate)).await {
            Ok(listener) => {
                let address = listener.local_addr().map_err(ServeError::Listen)?;
                return Ok((listener, address));
            }
            Err(error) => failure = Some(error),
        }
    }
    Err(ServeError::Listen(
        failure.unwrap_or_else(|| std::io::Error::other("no port to try")),
    ))
}

async fn accept<T: Transport + Clone>(
    listener: TcpListener,
    guard: Guard<T>,
    cancel: CancellationToken,
) {
    loop {
        let accepted = tokio::select! {
            () = cancel.cancelled() => break,
            accepted = listener.accept() => accepted,
        };
        let Ok((stream, _)) = accepted else {
            continue;
        };
        let guard = guard.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let service = service_fn(move |request| {
                let guard = guard.clone();
                async move { Ok::<_, Infallible>(guard.handle(request).await) }
            });
            let builder = auto::Builder::new(TokioExecutor::new());
            let connection = builder.serve_connection(TokioIo::new(stream), service);
            tokio::select! {
                () = cancel.cancelled() => {}
                _ = connection => {}
            }
        });
    }
}

/// The token check in front of the protocol.
struct Guard<T: Transport + Clone> {
    token: Arc<str>,
    mcp: StreamableHttpService<LauncherMcp<T>, LocalSessionManager>,
}

impl<T: Transport + Clone> Clone for Guard<T> {
    fn clone(&self) -> Self {
        Self {
            token: self.token.clone(),
            mcp: self.mcp.clone(),
        }
    }
}

type Reply = Response<BoxBody<Bytes, Infallible>>;

impl<T: Transport + Clone> Guard<T> {
    async fn handle(&self, request: Request<Incoming>) -> Reply {
        if request.uri().path() != PATH {
            return plain(StatusCode::NOT_FOUND, "not here; the MCP endpoint is /mcp");
        }
        if !self.authorized(&request) {
            let mut reply = plain(StatusCode::UNAUTHORIZED, "a bearer token is required");
            reply.headers_mut().insert(
                WWW_AUTHENTICATE,
                http::HeaderValue::from_static("Bearer realm=\"LumilioCL\""),
            );
            return reply;
        }
        self.mcp.handle(request).await
    }

    fn authorized(&self, request: &Request<Incoming>) -> bool {
        request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .is_some_and(|presented| same_secret(presented.trim(), &self.token))
    }
}

/// Equal without a length-dependent early exit on the content.
fn same_secret(presented: &str, expected: &str) -> bool {
    let (presented, expected) = (presented.as_bytes(), expected.as_bytes());
    if presented.len() != expected.len() {
        return false;
    }
    presented
        .iter()
        .zip(expected)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn plain(status: StatusCode, text: &'static str) -> Reply {
    Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Full::new(Bytes::from_static(text.as_bytes())).boxed())
        .expect("a static response builds")
}

/// Why the server could not start.
#[derive(Debug)]
pub enum ServeError {
    /// No port in the range could be bound, or the address is unknown.
    Listen(std::io::Error),
    /// `agent/mcp.json` could not be read or written.
    Access(std::io::Error),
}

impl fmt::Display for ServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Listen(error) => write!(f, "could not listen on the loopback port: {error}"),
            Self::Access(error) => write!(f, "could not keep the connection file: {error}"),
        }
    }
}

impl std::error::Error for ServeError {}
