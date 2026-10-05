//! The 3D preview window (ADR 0027): a webview that fills its own window and
//! runs the vendored schematic viewer. The page asks for two things over a
//! custom protocol, the schematic and a resource pack, and the launcher
//! answers them from memory. Nothing here knows which plugin asked.

mod assets;
#[cfg(test)]
mod tests;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod window;

/// Whether this system can show the preview. `gpui-wry` supports macOS and
/// Windows; elsewhere the pages hide the key.
pub const SUPPORTED: bool = cfg!(any(target_os = "macos", target_os = "windows"));

/// What one preview window shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRequest {
    pub title: String,
    pub schematic: Vec<u8>,
    /// Built from the game's own jar; `None` when the game is not installed.
    pub pack: Option<ModelPack>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelPack {
    /// Names the pack for the viewer; carries the game version and the jar.
    pub id: String,
    pub bytes: Vec<u8>,
}

/// What the page is told for one request.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Served {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Served {
    fn ok(body: Vec<u8>, content_type: &'static str) -> Self {
        Self {
            status: 200,
            content_type,
            body,
        }
    }

    fn missing() -> Self {
        Self {
            status: 404,
            content_type: "text/plain",
            body: Vec::new(),
        }
    }
}

/// The answer to one request path. Only the viewer's own files, the schematic
/// and the pack are served; nothing is read from the disk.
pub(crate) fn serve(path: &str, request: &ModelRequest) -> Served {
    match path {
        "/" | "/viewer.html" => assets::find("/viewer.html")
            .map(|(body, kind)| Served::ok(body, kind))
            .unwrap_or_else(Served::missing),
        "/schematic" => Served::ok(request.schematic.clone(), "application/octet-stream"),
        "/pack.zip" => match &request.pack {
            Some(pack) => Served::ok(pack.bytes.clone(), "application/zip"),
            None => Served::missing(),
        },
        other => assets::find(other)
            .map(|(body, kind)| Served::ok(body, kind))
            .unwrap_or_else(Served::missing),
    }
}

/// The page address for a request: the pack's name rides in the query so the
/// viewer can tell packs of different versions apart.
pub(crate) fn page_url(origin: &str, request: &ModelRequest) -> String {
    match &request.pack {
        Some(pack) => format!("{origin}/viewer.html?pack={}", pack.id),
        None => format!("{origin}/viewer.html"),
    }
}

/// Opens a preview window for `request`.
pub fn open(request: ModelRequest, cx: &mut gpui::App) -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        window::open(request, cx)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (request, cx);
        Err("这个系统上还不能预览 3D".to_owned())
    }
}
