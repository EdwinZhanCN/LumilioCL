//! Asking a server how it is (Server List Ping, protocol 1.7+).
//!
//! Adapted from Modrinth App (`packages/app-lib/src/util/server_ping.rs`,
//! GPL-3.0; ADR 0022): the handshake and status exchange and the limits on
//! lengths a server states. Everything a server sends is untrusted, so every
//! length is capped before memory is reserved and the whole exchange has a
//! deadline.

use serde_json::Value;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const DEFAULT_PORT: u16 = 25565;
const TIMEOUT: Duration = Duration::from_secs(5);
/// The status JSON is at most 32 767 characters, up to three bytes each.
const MAX_STATUS_BYTES: usize = 32_767 * 3;
const MAX_VARINT_BYTES: usize = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerStatus {
    /// The message of the day as plain text (colour codes removed).
    pub motd: String,
    pub online: Option<u32>,
    pub max: Option<u32>,
    pub version: Option<String>,
    /// Round-trip time of the ping, when the server answered it.
    pub latency_ms: Option<u64>,
}

#[derive(Debug)]
pub enum PingError {
    /// The address has no host or a port that is not a number.
    BadAddress,
    TimedOut,
    Io(io::Error),
    /// The server answered something that is not a status.
    Protocol(&'static str),
}

impl Display for PingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadAddress => f.write_str("the address is not valid"),
            Self::TimedOut => f.write_str("the server did not answer in time"),
            Self::Io(error) => write!(f, "could not reach the server: {error}"),
            Self::Protocol(why) => write!(f, "unexpected answer from the server: {why}"),
        }
    }
}

impl Error for PingError {}

impl From<io::Error> for PingError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Splits `host`, `host:port` or `[v6]:port`; the port defaults to 25565.
pub fn parse_address(address: &str) -> Result<(String, u16), PingError> {
    let address = address.trim();
    let (host, port) = if let Some(rest) = address.strip_prefix('[') {
        let (host, tail) = rest.split_once(']').ok_or(PingError::BadAddress)?;
        (host, tail.strip_prefix(':'))
    } else if address.matches(':').count() == 1 {
        let (host, port) = address.split_once(':').ok_or(PingError::BadAddress)?;
        (host, Some(port))
    } else {
        // No colon, or a bare IPv6 address.
        (address, None)
    };
    if host.is_empty() {
        return Err(PingError::BadAddress);
    }
    let port = match port {
        Some(text) => text.parse().map_err(|_| PingError::BadAddress)?,
        None => DEFAULT_PORT,
    };
    Ok((host.to_owned(), port))
}

pub async fn probe(address: &str) -> Result<ServerStatus, PingError> {
    let (host, port) = parse_address(address)?;
    probe_at(&host, port).await
}

pub async fn probe_at(host: &str, port: u16) -> Result<ServerStatus, PingError> {
    tokio::time::timeout(TIMEOUT, exchange(host, port))
        .await
        .map_err(|_| PingError::TimedOut)?
}

fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    while value >= 0x80 {
        out.push((value & 0x7f) as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

async fn read_varint<R: AsyncRead + Unpin>(reader: &mut R) -> Result<i32, PingError> {
    let mut result = 0_u32;
    for shift in 0..MAX_VARINT_BYTES {
        let byte = reader.read_u8().await?;
        result |= u32::from(byte & 0x7f) << (shift * 7);
        if byte & 0x80 == 0 {
            return Ok(result as i32);
        }
    }
    Err(PingError::Protocol("a number is too long"))
}

/// A length a server stated, refused when negative or above `max`.
fn capped(length: i32, max: usize, why: &'static str) -> Result<usize, PingError> {
    usize::try_from(length)
        .ok()
        .filter(|length| *length <= max)
        .ok_or(PingError::Protocol(why))
}

fn packet(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + MAX_VARINT_BYTES);
    write_varint(&mut out, body.len() as i32);
    out.extend(body);
    out
}

async fn exchange(host: &str, port: u16) -> Result<ServerStatus, PingError> {
    let mut stream = TcpStream::connect((host, port)).await?;
    stream.set_nodelay(true)?;

    let mut handshake = Vec::new();
    write_varint(&mut handshake, 0);
    // -1: no particular protocol version; servers answer all the same.
    write_varint(&mut handshake, -1);
    write_varint(&mut handshake, host.len() as i32);
    handshake.extend(host.as_bytes());
    handshake.extend(port.to_be_bytes());
    write_varint(&mut handshake, 1);
    stream.write_all(&packet(&handshake)).await?;
    stream.write_all(&[0x01, 0x00]).await?;
    stream.flush().await?;

    let length = capped(
        read_varint(&mut stream).await?,
        MAX_STATUS_BYTES + 2 * MAX_VARINT_BYTES,
        "the status is too long",
    )?;
    let mut body = (&mut stream).take(length as u64);
    if read_varint(&mut body).await? != 0 {
        return Err(PingError::Protocol("not a status packet"));
    }
    let json_length = capped(
        read_varint(&mut body).await?,
        MAX_STATUS_BYTES,
        "the status is too long",
    )?;
    let mut json = vec![0_u8; json_length];
    body.read_exact(&mut json).await?;
    // Whatever is left of the packet is not ours.
    tokio::io::copy(&mut body, &mut tokio::io::sink()).await?;
    let mut status = read_status(&json)?;
    status.latency_ms = latency(&mut stream).await.ok();
    Ok(status)
}

async fn latency(stream: &mut TcpStream) -> Result<u64, PingError> {
    let token = 0x4c75_6d69_6c69_6f21_i64;
    let started = Instant::now();
    let mut request = vec![0x09, 0x01];
    request.extend(token.to_be_bytes());
    stream.write_all(&request).await?;
    stream.flush().await?;
    let mut prefix = [0_u8; 2];
    stream.read_exact(&mut prefix).await?;
    let echoed = stream.read_i64().await?;
    if prefix != [0x09, 0x01] || echoed != token {
        return Err(PingError::Protocol("the ping was not echoed"));
    }
    Ok(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
}

pub(super) fn read_status(json: &[u8]) -> Result<ServerStatus, PingError> {
    let value: Value =
        serde_json::from_slice(json).map_err(|_| PingError::Protocol("the status is not JSON"))?;
    let count = |key: &str| {
        value
            .get("players")
            .and_then(|players| players.get(key))
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    let mut motd = String::new();
    if let Some(description) = value.get("description") {
        flatten(description, &mut motd);
    }
    Ok(ServerStatus {
        motd: strip_codes(motd.trim()),
        online: count("online"),
        max: count("max"),
        version: value
            .get("version")
            .and_then(|version| version.get("name"))
            .and_then(Value::as_str)
            .map(strip_codes),
        latency_ms: None,
    })
}

/// The text of a chat component: its own `text`, then its `extra` parts.
fn flatten(component: &Value, out: &mut String) {
    match component {
        Value::String(text) => out.push_str(text),
        Value::Array(parts) => parts.iter().for_each(|part| flatten(part, out)),
        Value::Object(map) => {
            if let Some(Value::String(text)) = map.get("text") {
                out.push_str(text);
            }
            if let Some(extra) = map.get("extra") {
                flatten(extra, out);
            }
        }
        _ => {}
    }
}

/// Removes `§x` formatting codes.
fn strip_codes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}
