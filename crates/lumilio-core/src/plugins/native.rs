//! Host-owned, cancellable Discord IPC. No plugin supplies paths or raw RPC.
//! Protocol: https://docs.discord.com/developers/topics/rpc

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lumilio_plugin_api::{DiscordActivity, PluginError};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::watch;

const IPC_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_FRAME: usize = 64 * 1024;

#[cfg(all(test, unix))]
#[path = "tests/native.rs"]
mod tests;

#[derive(Default)]
pub(super) struct Native {
    workers: Mutex<BTreeMap<String, watch::Sender<Option<DiscordActivity>>>>,
    #[cfg(test)]
    pub(super) socket: Option<std::path::PathBuf>,
}

impl Native {
    pub(super) fn update(
        &self,
        id: &str,
        activity: Option<DiscordActivity>,
    ) -> Result<(), PluginError> {
        if let Some(activity) = &activity {
            if activity
                .application_id
                .parse::<u64>()
                .ok()
                .is_none_or(|id| id == 0)
                || !activity.application_id.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(PluginError::InvalidInput(
                    "invalid Discord application id".into(),
                ));
            }
            for text in [&activity.details, &activity.state].into_iter().flatten() {
                if text.len() > 128 || text.contains('\0') {
                    return Err(PluginError::InvalidInput(
                        "invalid Discord activity text".into(),
                    ));
                }
            }
        }
        let mut workers = self
            .workers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(sender) = workers.get(id) {
            sender.send_replace(activity);
        } else if activity.is_some() {
            let (sender, receiver) = watch::channel(activity);
            workers.insert(id.to_owned(), sender);
            #[cfg(test)]
            let socket = self.socket.clone();
            tokio::runtime::Handle::current().spawn(async move {
                worker(
                    receiver,
                    #[cfg(test)]
                    socket,
                )
                .await;
            });
        }
        Ok(())
    }

    pub(super) fn clear(&self, id: &str) {
        let _ = self.update(id, None);
    }
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}
type Connection = Box<dyn Stream>;

async fn worker(
    mut receiver: watch::Receiver<Option<DiscordActivity>>,
    #[cfg(test)] socket: Option<std::path::PathBuf>,
) {
    loop {
        let activity = receiver.borrow_and_update().clone();
        // A preference change or exit cancels even a stalled handshake. Keep
        // the accepted connection alive only until the next desired state.
        let mut connection = tokio::select! {
            biased;
            changed = receiver.changed() => {
                if changed.is_err() { return; }
                continue;
            }
            result = tokio::time::timeout(IPC_TIMEOUT, async {
                let activity = activity?;
                let stream = connect(#[cfg(test)] socket.as_deref()).await?;
                publish(stream, &activity).await.ok()
            }) => result.ok().flatten(),
        };
        let changed = loop {
            if let Some(stream) = &mut connection {
                tokio::select! {
                    changed = receiver.changed() => break changed,
                    message = receive(stream) => {
                        if message.is_err() { connection = None; }
                    }
                }
            } else {
                break receiver.changed().await;
            }
        };
        // Disconnecting withdraws activity, including after a failed command.
        // Also send an explicit clear on a healthy connection for clients that
        // defer processing a disconnected pipe.
        if let Some(mut connection) = connection {
            let _ =
                tokio::time::timeout(IPC_TIMEOUT, send(&mut connection, 1, &command(None))).await;
        }
        if changed.is_err() {
            return;
        }
    }
}

async fn connect(#[cfg(test)] socket: Option<&std::path::Path>) -> Option<Connection> {
    #[cfg(all(test, unix))]
    if let Some(socket) = socket {
        return tokio::net::UnixStream::connect(socket)
            .await
            .ok()
            .map(|s| Box::new(s) as Connection);
    }
    #[cfg(all(test, not(unix)))]
    let _ = socket;
    #[cfg(unix)]
    {
        let mut roots: Vec<std::path::PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(Into::into)
            .collect();
        roots.push("/tmp".into());
        for root in roots {
            for prefix in [
                root.clone(),
                root.join("app/com.discordapp.Discord"),
                root.join("snap.discord"),
            ] {
                for n in 0..10 {
                    if let Ok(stream) =
                        tokio::net::UnixStream::connect(prefix.join(format!("discord-ipc-{n}")))
                            .await
                    {
                        return Some(Box::new(stream));
                    }
                }
            }
        }
    }
    #[cfg(windows)]
    for n in 0..10 {
        if let Ok(stream) = tokio::net::windows::named_pipe::ClientOptions::new()
            .open(format!(r"\\?\pipe\discord-ipc-{n}"))
        {
            return Some(Box::new(stream));
        }
    }
    None
}

async fn publish(
    mut stream: Connection,
    activity: &DiscordActivity,
) -> std::io::Result<Connection> {
    send(
        &mut stream,
        0,
        &serde_json::json!({"v": 1, "client_id": activity.application_id}),
    )
    .await?;
    let ready = receive(&mut stream).await?;
    if ready["evt"] != "READY" {
        return Err(std::io::Error::other("Discord handshake refused"));
    }
    let command = command(Some(activity));
    send(&mut stream, 1, &command).await?;
    let reply = receive(&mut stream).await?;
    if reply["evt"] == "ERROR"
        || reply["cmd"] != "SET_ACTIVITY"
        || reply["nonce"] != command["nonce"]
    {
        return Err(std::io::Error::other("Discord activity refused"));
    }
    Ok(stream)
}

fn command(activity: Option<&DiscordActivity>) -> serde_json::Value {
    static NEXT_NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let nonce = NEXT_NONCE
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .to_string();
    let activity = activity.map(|activity| {
        let mut value = serde_json::json!({"type": 0});
        if let Some(details) = &activity.details {
            value["details"] = details.clone().into();
        }
        if let Some(state) = &activity.state {
            value["state"] = state.clone().into();
        }
        value
    });
    serde_json::json!({"cmd": "SET_ACTIVITY", "args": {"pid": std::process::id(), "activity": activity}, "nonce": nonce})
}

async fn send(
    stream: &mut Connection,
    opcode: u32,
    body: &serde_json::Value,
) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(body)?;
    stream.write_u32_le(opcode).await?;
    stream.write_u32_le(bytes.len() as u32).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await
}

async fn receive(stream: &mut Connection) -> std::io::Result<serde_json::Value> {
    loop {
        let opcode = stream.read_u32_le().await?;
        let length = stream.read_u32_le().await? as usize;
        if length > MAX_FRAME {
            return Err(std::io::Error::other("Discord frame too large"));
        }
        let mut bytes = vec![0; length];
        stream.read_exact(&mut bytes).await?;
        match opcode {
            1 => return serde_json::from_slice(&bytes).map_err(std::io::Error::other),
            3 => {
                stream.write_u32_le(4).await?;
                stream.write_u32_le(length as u32).await?;
                stream.write_all(&bytes).await?;
            }
            _ => return Err(std::io::Error::other("Discord closed the connection")),
        }
    }
}

pub(super) struct Access {
    pub(super) native: Arc<Native>,
    pub(super) preferences: Arc<std::sync::RwLock<super::Preferences>>,
    pub(super) entry: Arc<super::Entry>,
    pub(super) revision: u64,
    pub(super) deadline: std::time::Instant,
}

impl Access {
    pub(super) fn update(&self, activity: Option<DiscordActivity>) -> Result<(), PluginError> {
        use lumilio_plugin_api::{NativeCapability, Permission};
        let entry = &self.entry;
        let preferences = self
            .preferences
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = preferences
            .states
            .get(&entry.manifest.id)
            .cloned()
            .unwrap_or_default();
        if !entry.core
            || !entry
                .manifest
                .permissions
                .contains(&Permission::Native(NativeCapability::DiscordIpc))
            || !super::enabled(&entry.manifest, &state)
            || preferences
                .revisions
                .get(&entry.manifest.id)
                .copied()
                .unwrap_or(0)
                != self.revision
            || std::time::Instant::now() >= self.deadline
            || entry
                .failure
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
        {
            return Err(PluginError::PermissionDenied);
        }
        self.native.update(&entry.manifest.id, activity)
    }
}
