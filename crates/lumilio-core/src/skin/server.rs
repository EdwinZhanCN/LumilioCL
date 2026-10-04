//! A Yggdrasil server on this machine that knows one player. authlib-injector
//! points the game at it, so the game fetches that player's skin from here.
//!
//! The routes and answers are adapted from HMCL
//! (`HMCLCore/src/main/java/org/jackhuang/hmcl/auth/offline/YggdrasilServer.java`,
//! Copyright (C) 2021 huangyuhui and contributors, GPL-3.0-or-later; ADR
//! 0011). Only what the game asks for is served, only to this
//! machine, and every request is bounded.

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use super::{LoadedSkin, Signer, SkinModel};
use crate::account::ProfileId;

const HEAD_LIMIT: usize = 16 * 1024;
const BODY_LIMIT: usize = 64 * 1024;
const CONNECTION_TIME: Duration = Duration::from_secs(10);

/// The one player the server answers for.
#[derive(Clone, Debug)]
pub struct Character {
    pub id: ProfileId,
    pub name: String,
    pub skin: LoadedSkin,
}

struct State {
    character: Character,
    signer: Arc<Signer>,
    root: String,
}

/// A running server; dropping it stops it.
pub struct LocalSkinServer {
    port: u16,
    task: JoinHandle<()>,
}

impl Drop for LocalSkinServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Response {
    status: u16,
    content_type: &'static str,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Response {
    fn json(value: &Value) -> Self {
        Self {
            status: 200,
            content_type: "application/json; charset=utf-8",
            headers: Vec::new(),
            body: value.to_string().into_bytes(),
        }
    }

    const fn empty(status: u16) -> Self {
        Self {
            status,
            content_type: "text/plain",
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Error",
    }
}

impl LocalSkinServer {
    /// Starts listening on a free port of this machine.
    pub async fn start(character: Character, signer: Arc<Signer>) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = listener.local_addr()?.port();
        let state = Arc::new(State {
            character,
            signer,
            root: format!("http://localhost:{port}"),
        });
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let state = state.clone();
                tokio::spawn(async move {
                    let _ = tokio::time::timeout(CONNECTION_TIME, serve(stream, &state)).await;
                });
            }
        });
        Ok(Self { port, task })
    }

    /// The address authlib-injector is given.
    #[must_use]
    pub fn api_root(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    #[cfg(test)]
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }
}

/// Reads one request, answers it, and closes.
async fn serve(mut stream: TcpStream, state: &State) -> io::Result<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 2048];
    let head_end = loop {
        if let Some(at) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break at;
        }
        if buffer.len() > HEAD_LIMIT {
            return respond(&mut stream, Response::empty(400)).await;
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap_or_default().split(' ');
    let (method, target) = (
        request_line.next().unwrap_or_default().to_owned(),
        request_line.next().unwrap_or_default().to_owned(),
    );
    let length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if length > BODY_LIMIT {
        return respond(&mut stream, Response::empty(400)).await;
    }
    let mut body = buffer[head_end + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);
    respond(&mut stream, route(state, &method, &target, &body)).await
}

async fn respond(stream: &mut TcpStream, response: Response) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        reason(response.status),
        response.content_type,
        response.body.len()
    );
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&response.body).await?;
    stream.shutdown().await
}

fn route(state: &State, method: &str, target: &str, body: &[u8]) -> Response {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query: BTreeMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    match (method, path) {
        ("GET", "/") => Response::json(&json!({
            "signaturePublickey": state.signer.public_key_pem(),
            "skinDomains": ["127.0.0.1", "localhost"],
            "meta": {
                "serverName": "LumilioCL",
                "implementationName": "LumilioCL",
                "implementationVersion": "1.0",
                "feature.non_email_login": true,
            },
        })),
        ("GET", "/status") => Response::json(&json!({
            "user.count": 1,
            "token.count": 0,
            "pendingAuthentication.count": 0,
        })),
        ("POST", "/api/profiles/minecraft") => {
            let Ok(names) = serde_json::from_slice::<Vec<String>>(body) else {
                return Response::empty(400);
            };
            let known = names.contains(&state.character.name);
            Response::json(&Value::Array(if known {
                vec![json!({"id": state.character.id.compact(), "name": state.character.name})]
            } else {
                Vec::new()
            }))
        }
        ("GET", "/sessionserver/session/minecraft/hasJoined") => match query.get("username") {
            None => Response::empty(400),
            Some(name) if *name == state.character.name => Response::json(&complete_profile(state)),
            Some(_) => Response::empty(204),
        },
        ("POST", "/sessionserver/session/minecraft/join") => Response::empty(204),
        ("GET", path) if path.starts_with("/sessionserver/session/minecraft/profile/") => {
            let id = &path["/sessionserver/session/minecraft/profile/".len()..];
            let wanted = ProfileId::parse(id).ok().filter(|_| id.len() == 32);
            if wanted == Some(state.character.id) {
                Response::json(&complete_profile(state))
            } else {
                Response::empty(204)
            }
        }
        ("GET", path) if path.starts_with("/textures/") => {
            texture(state, &path["/textures/".len()..])
        }
        _ => Response::empty(404),
    }
}

fn texture(state: &State, hash: &str) -> Response {
    let skin = &state.character.skin;
    let found = [skin.skin.as_ref(), skin.cape.as_ref()]
        .into_iter()
        .flatten()
        .find(|texture| texture.hash == hash);
    match found {
        Some(texture) => Response {
            status: 200,
            content_type: "image/png",
            headers: vec![
                ("Etag", format!("\"{hash}\"")),
                ("Cache-Control", "max-age=2592000, public".to_owned()),
            ],
            body: texture.png.clone(),
        },
        None => Response::empty(404),
    }
}

/// The character with its signed `textures` property.
fn complete_profile(state: &State) -> Value {
    let character = &state.character;
    let mut textures = serde_json::Map::new();
    if let Some(skin) = &character.skin.skin {
        let mut entry = json!({"url": format!("{}/textures/{}", state.root, skin.hash)});
        if character.skin.model == SkinModel::Slim {
            entry["metadata"] = json!({"model": "slim"});
        }
        textures.insert("SKIN".to_owned(), entry);
    }
    if let Some(cape) = &character.skin.cape {
        textures.insert(
            "CAPE".to_owned(),
            json!({"url": format!("{}/textures/{}", state.root, cape.hash)}),
        );
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    let payload = json!({
        "timestamp": u64::try_from(timestamp).unwrap_or(0),
        "profileId": character.id.compact(),
        "profileName": character.name,
        "textures": textures,
    });
    let value = base64::engine::general_purpose::STANDARD.encode(payload.to_string());
    json!({
        "id": character.id.compact(),
        "name": character.name,
        "properties": [{
            "name": "textures",
            "signature": state.signer.sign(&value),
            "value": value,
        }],
    })
}
