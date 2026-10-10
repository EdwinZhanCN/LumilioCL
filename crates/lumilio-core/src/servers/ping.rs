//! Asking a server how it is (Server List Ping): the modern 1.7+ handshake and
//! the legacy pre-1.7 one.
//!
//! Adapted from Modrinth App (`packages/app-lib/src/util/server_ping.rs`,
//! GPL-3.0-only; ADR 0022): the handshake and status exchange, the legacy
//! packet, and the limits on lengths a server states. Everything a server sends
//! is untrusted, so every length is capped before memory is reserved and the
//! whole exchange has a deadline.

use super::protocol::ProtocolVersion;
use crate::skin::Pixels;
use base64::Engine as _;
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
/// A legacy status is at most 32 767 UTF-16 units.
const MAX_LEGACY_CHARS: usize = 32_767;
/// A server's `favicon` is a PNG data URI; only this form is a picture.
const FAVICON_PREFIX: &str = "data:image/png;base64,";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerStatus {
    /// The message of the day as plain text (colour codes removed).
    pub motd: String,
    pub online: Option<u32>,
    pub max: Option<u32>,
    pub version: Option<String>,
    /// Round-trip time of the ping, when the server answered it.
    pub latency_ms: Option<u64>,
    /// The server's icon, when it sent one that is a usable PNG.
    pub favicon: Option<Pixels>,
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

/// Asks the server `address` names. An address without a port is first looked
/// up as a Minecraft SRV record; the handshake still carries the name that was
/// typed, as the game does.
pub async fn probe(
    address: &str,
    protocol: Option<ProtocolVersion>,
) -> Result<ServerStatus, PingError> {
    let (name, name_port) = parse_address(address)?;
    tokio::time::timeout(TIMEOUT, async {
        let (host, port) = resolve_server_address(&name, name_port).await;
        dispatch((&host, port), (&name, name_port), protocol).await
    })
    .await
    .map_err(|_| PingError::TimedOut)?
}

/// Asks `host:port` directly (no SRV lookup) with the handshake `protocol`
/// calls for: the legacy one for a pre-1.7 version, otherwise the modern one.
#[cfg(test)]
pub async fn probe_at(
    host: &str,
    port: u16,
    protocol: Option<ProtocolVersion>,
) -> Result<ServerStatus, PingError> {
    tokio::time::timeout(TIMEOUT, dispatch((host, port), (host, port), protocol))
        .await
        .map_err(|_| PingError::TimedOut)?
}

/// Connects to `connect` and asks as `name`; the two differ when an SRV record
/// redirects the connection.
async fn dispatch(
    connect: (&str, u16),
    name: (&str, u16),
    protocol: Option<ProtocolVersion>,
) -> Result<ServerStatus, PingError> {
    match protocol {
        Some(known) if known.legacy => legacy_exchange(connect, name, known.version).await,
        known => exchange(connect, name, known.map(|known| known.version)).await,
    }
}

/// Where a server named `host` (no explicit port) really is: its
/// `_minecraft._tcp.<host>` SRV record, or the address itself when there is no
/// record, the host is an IP literal, or a port was given.
async fn resolve_server_address(host: &str, port: u16) -> (String, u16) {
    if port != DEFAULT_PORT
        || host.parse::<std::net::Ipv4Addr>().is_ok()
        || host.parse::<std::net::Ipv6Addr>().is_ok()
    {
        return (host.to_owned(), port);
    }
    match lookup_srv(host).await {
        Some((target, port)) => (target, port),
        None => (host.to_owned(), port),
    }
}

/// A DNS `_minecraft._tcp.<host>` SRV lookup: its first target and port (the
/// game resolves a bare domain the same way). Any failure is "no record".
async fn lookup_srv(host: &str) -> Option<(String, u16)> {
    use hickory_resolver::TokioResolver;
    let resolver = TokioResolver::builder_tokio().ok()?.build();
    let lookup = resolver
        .srv_lookup(format!("_minecraft._tcp.{host}"))
        .await
        .ok()?;
    let record = lookup.into_iter().next()?;
    Some((
        record.target().to_string().trim_end_matches('.').to_owned(),
        record.port(),
    ))
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

async fn exchange(
    connect: (&str, u16),
    name: (&str, u16),
    protocol: Option<u32>,
) -> Result<ServerStatus, PingError> {
    let mut stream = TcpStream::connect(connect).await?;
    stream.set_nodelay(true)?;

    // The handshake carries the name that was typed, not the SRV target.
    let (host, port) = name;
    let mut handshake = Vec::new();
    write_varint(&mut handshake, 0);
    // The game's protocol, or -1 for "no particular version"; servers answer
    // all the same.
    write_varint(
        &mut handshake,
        protocol.map_or(-1, |version| version as i32),
    );
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

/// The legacy (pre-1.7) ping: a `0xFE` packet carrying the host and port,
/// answered by one UTF-16 string of `§`- or NUL-separated fields. Adapted from
/// Modrinth App `server_ping.rs::legacy` (GPL-3.0-only; ADR 0022). No latency
/// is measured here.
async fn legacy_exchange(
    connect: (&str, u16),
    name: (&str, u16),
    protocol: u32,
) -> Result<ServerStatus, PingError> {
    // The protocol byte gates the packet layout: below 47 there is no hostname
    // field, below 73 no "MC|PingHost" prefix.
    let protocol = u8::try_from(protocol).unwrap_or(74);
    let (host, port) = name;
    let mut request = vec![0xfe];
    if protocol >= 47 {
        request.push(0x01);
    }
    if protocol >= 73 {
        request.push(0xfa);
        write_legacy(&mut request, "MC|PingHost");
        let from = request.len();
        request.push(protocol);
        write_legacy(&mut request, host);
        request.extend_from_slice(&u32::from(port).to_be_bytes());
        let length = u16::try_from(request.len() - from).map_err(|_| PingError::BadAddress)?;
        request.splice(from..from, length.to_be_bytes());
    }

    let mut stream = TcpStream::connect(connect).await?;
    stream.write_all(&request).await?;
    stream.flush().await?;

    if stream.read_u8().await? != 0xff {
        return Err(PingError::Protocol("not a legacy status"));
    }
    let chars = usize::from(stream.read_u16().await?);
    if chars > MAX_LEGACY_CHARS {
        return Err(PingError::Protocol("the status is too long"));
    }
    let mut utf16 = vec![0_u16; chars];
    for unit in &mut utf16 {
        *unit = stream.read_u16().await?;
    }

    let text = String::from_utf16_lossy(&utf16);
    let mut ancient = false;
    let mut parts = text.split('\0');
    // Modern legacy answers start with "§1"; older ones are `§`-separated.
    if parts.next() != Some("§1") {
        ancient = true;
        parts = text.split('§');
    }
    let mut version = None;
    if !ancient {
        let _protocol = parts.next().and_then(|text| text.parse::<u32>().ok());
        version = parts
            .next()
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
    }
    let motd = parts.next().unwrap_or_default();
    let online = parts.next().and_then(|text| text.parse::<u32>().ok());
    let max = parts.next().and_then(|text| text.parse::<u32>().ok());
    Ok(ServerStatus {
        motd: strip_codes(motd.trim()),
        online,
        max,
        version,
        latency_ms: None,
        favicon: None,
    })
}

/// A UTF-16 string with its length in units, big-endian, as the legacy ping
/// writes text.
fn write_legacy(out: &mut Vec<u8>, text: &str) {
    let encoded: Vec<u16> = text.encode_utf16().collect();
    out.extend_from_slice(
        &u16::try_from(encoded.len())
            .unwrap_or(u16::MAX)
            .to_be_bytes(),
    );
    for unit in encoded {
        out.extend_from_slice(&unit.to_be_bytes());
    }
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
        favicon: value
            .get("favicon")
            .and_then(Value::as_str)
            .and_then(favicon),
    })
}

/// The status `favicon` field: `data:image/png;base64,…`, decoded with limits.
/// Anything else is no icon.
fn favicon(value: &str) -> Option<Pixels> {
    let encoded = value.trim().strip_prefix(FAVICON_PREFIX)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    super::decode_icon(&bytes)
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
