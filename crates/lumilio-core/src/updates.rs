//! Checking installed content for newer versions, and applying them.
//!
//! Behavior notes: `docs/behavior/updates.md`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::Path;

use crate::activity::CancellationToken;
use crate::content::{self, ContentError};
use crate::discover::{
    DiscoverError, IntentError, ModrinthClient, ProjectKind, Version, install_request,
};
use crate::instance::Loader;
use crate::transfer::{TransferEngine, TransferError, Transport};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentUpdate {
    pub kind: ProjectKind,
    /// The installed file that would be replaced.
    pub file_name: String,
    pub current_sha1: String,
    pub latest: Version,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UpdateReport {
    pub updates: Vec<ContentUpdate>,
    /// Files Modrinth knows that are already the newest compatible version.
    pub up_to_date: usize,
    /// Enabled files Modrinth does not know (hand-installed or from elsewhere).
    pub unknown: Vec<String>,
}

#[derive(Debug)]
pub enum UpdateError {
    Content(ContentError),
    Discover(DiscoverError),
    Intent(IntentError),
    Transfer(TransferError),
}

impl Display for UpdateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Content(error) => write!(f, "{error}"),
            Self::Discover(error) => write!(f, "{error}"),
            Self::Intent(error) => write!(f, "{error}"),
            Self::Transfer(error) => write!(f, "{error}"),
        }
    }
}

impl Error for UpdateError {}

impl From<ContentError> for UpdateError {
    fn from(error: ContentError) -> Self {
        Self::Content(error)
    }
}
impl From<DiscoverError> for UpdateError {
    fn from(error: DiscoverError) -> Self {
        Self::Discover(error)
    }
}
impl From<IntentError> for UpdateError {
    fn from(error: IntentError) -> Self {
        Self::Intent(error)
    }
}
impl From<TransferError> for UpdateError {
    fn from(error: TransferError) -> Self {
        Self::Transfer(error)
    }
}

/// Checks the enabled files of one kind against Modrinth.
///
/// Disabled files and folder packs are left out (a disabled file is the user's
/// decision; a folder has no single hash). A file whose newest compatible
/// version lists the file's own hash is up to date. Files that cannot be read
/// are skipped.
pub async fn check<T: Transport>(
    client: &ModrinthClient<T>,
    game_dir: &Path,
    kind: ProjectKind,
    game_version: &str,
    loader: Loader,
) -> Result<UpdateReport, UpdateError> {
    let folder = kind.install_folder().ok_or(ContentError::NotAFileKind)?;
    let mut by_hash: Vec<(String, String)> = Vec::new();
    for item in content::scan(game_dir, kind)? {
        if !item.enabled || item.is_directory {
            continue;
        }
        if let Ok(hash) = content::sha1_hex(&game_dir.join(folder).join(&item.file_name)) {
            by_hash.push((hash, item.file_name));
        }
    }
    let hashes: Vec<String> = by_hash.iter().map(|(hash, _)| hash.clone()).collect();
    let known = client.identify(&hashes).await?;
    let latest = client.latest_for(&hashes, loader, game_version).await?;

    let mut report = UpdateReport::default();
    for (hash, file_name) in by_hash {
        if !known.contains_key(&hash) {
            report.unknown.push(file_name);
            continue;
        }
        match latest.get(&hash) {
            Some(newest)
                if !newest
                    .files
                    .iter()
                    .any(|file| file.sha1.as_deref() == Some(&hash)) =>
            {
                report.updates.push(ContentUpdate {
                    kind,
                    file_name,
                    current_sha1: hash,
                    latest: newest.clone(),
                });
            }
            _ => report.up_to_date += 1,
        }
    }
    Ok(report)
}

/// Downloads the new file (verified), then removes the old one. If the new
/// version has the same file name, the download replaces it in place. Nothing
/// is removed unless the download succeeded. Returns the new file name.
pub async fn apply<T: Transport>(
    engine: &TransferEngine<T>,
    update: &ContentUpdate,
    game_dir: &Path,
    sources: Vec<String>,
    cancel: CancellationToken,
) -> Result<String, UpdateError> {
    let request = install_request(update.kind, &update.latest, game_dir, sources)?;
    let new_name = update
        .latest
        .install_file()
        .map(|file| file.filename.clone())
        .ok_or(IntentError::NoFile)?;
    engine.transfer(request, cancel).await?;
    if new_name != update.file_name {
        content::remove(game_dir, update.kind, &update.file_name)?;
    }
    Ok(new_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::{FileTransport, TransportFuture, TransportResponse};
    use std::fs;
    use std::sync::Mutex;

    /// Answers the two update endpoints from canned JSON and records bodies.
    struct Api {
        identify: String,
        update: String,
        bodies: Mutex<Vec<(String, String)>>,
    }

    impl Transport for Api {
        fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
            Box::pin(async { Ok(TransportResponse::from_bytes(404, Vec::new())) })
        }
        fn post_json<'a>(&'a self, source: &'a str, body: Vec<u8>) -> TransportFuture<'a> {
            self.bodies
                .lock()
                .unwrap()
                .push((source.to_owned(), String::from_utf8(body).unwrap()));
            let answer = if source.ends_with("/v2/version_files/update") {
                self.update.clone()
            } else {
                self.identify.clone()
            };
            Box::pin(async move { Ok(TransportResponse::from_bytes(200, answer.into_bytes())) })
        }
    }

    fn version_json(id: &str, file: &str, url: &str, sha1: &str) -> String {
        format!(
            r#"{{"id":"{id}","project_id":"P","version_number":"{id}","game_versions":["1.21.1"],
                "loaders":["fabric"],"date_published":"2024-01-01T00:00:00Z",
                "files":[{{"url":"{url}","filename":"{file}","primary":true,"size":3,"hashes":{{"sha1":"{sha1}"}}}}]}}"#
        )
    }

    fn sha1_of(text: &str) -> String {
        use sha1::{Digest, Sha1};
        Sha1::digest(text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    struct Setup {
        _dir: tempfile::TempDir,
        game: std::path::PathBuf,
        server: std::path::PathBuf,
    }

    fn setup() -> Setup {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("mods")).unwrap();
        let server = dir.path().join("server");
        fs::create_dir_all(&server).unwrap();
        Setup {
            game,
            server,
            _dir: dir,
        }
    }

    #[tokio::test]
    async fn classifies_outdated_current_and_unknown_files() {
        let s = setup();
        fs::write(s.game.join("mods/outdated.jar"), "old").unwrap();
        fs::write(s.game.join("mods/current.jar"), "cur").unwrap();
        fs::write(s.game.join("mods/mystery.jar"), "???").unwrap();
        fs::write(s.game.join("mods/off.jar.disabled"), "off").unwrap();
        let (old, cur) = (sha1_of("old"), sha1_of("cur"));
        let identify = format!(
            "{{\"{old}\":{},\"{cur}\":{}}}",
            version_json("v1", "outdated.jar", "u1", &old),
            version_json("v9", "current.jar", "u2", &cur)
        );
        let update = format!(
            "{{\"{old}\":{},\"{cur}\":{}}}",
            version_json("v2", "outdated-2.jar", "u3", &sha1_of("new")),
            version_json("v9", "current.jar", "u2", &cur)
        );
        let api = Api {
            identify,
            update,
            bodies: Mutex::default(),
        };
        let client = ModrinthClient::new(api);
        let report = check(&client, &s.game, ProjectKind::Mod, "1.21.1", Loader::Fabric)
            .await
            .unwrap();
        assert_eq!(report.updates.len(), 1);
        assert_eq!(report.updates[0].file_name, "outdated.jar");
        assert_eq!(report.updates[0].latest.id, "v2");
        assert_eq!(report.up_to_date, 1);
        assert_eq!(report.unknown, ["mystery.jar"]);
    }

    #[tokio::test]
    async fn requests_carry_hashes_loader_and_game_version_but_never_disabled_files() {
        let s = setup();
        fs::write(s.game.join("mods/a.jar"), "a").unwrap();
        fs::write(s.game.join("mods/b.jar.disabled"), "b").unwrap();
        let api = Api {
            identify: "{}".into(),
            update: "{}".into(),
            bodies: Mutex::default(),
        };
        let client = ModrinthClient::new(api);
        check(&client, &s.game, ProjectKind::Mod, "1.21.1", Loader::Fabric)
            .await
            .unwrap();
        let bodies = client_bodies(&client);
        let update_body: serde_json::Value = serde_json::from_str(
            &bodies
                .iter()
                .find(|(u, _)| u.ends_with("/update"))
                .unwrap()
                .1,
        )
        .unwrap();
        assert_eq!(update_body["hashes"], serde_json::json!([sha1_of("a")]));
        assert_eq!(update_body["loaders"], serde_json::json!(["fabric"]));
        assert_eq!(update_body["game_versions"], serde_json::json!(["1.21.1"]));
        assert_eq!(update_body["algorithm"], "sha1");
    }

    fn client_bodies(client: &ModrinthClient<Api>) -> Vec<(String, String)> {
        client.transport_for_tests().bodies.lock().unwrap().clone()
    }

    #[tokio::test]
    async fn an_empty_folder_makes_no_requests() {
        let s = setup();
        let api = Api {
            identify: "{}".into(),
            update: "{}".into(),
            bodies: Mutex::default(),
        };
        let client = ModrinthClient::new(api);
        let report = check(&client, &s.game, ProjectKind::Mod, "1.21.1", Loader::Fabric)
            .await
            .unwrap();
        assert_eq!(report, UpdateReport::default());
        assert!(client_bodies(&client).is_empty());
    }

    fn update_for(server: &Path, new_name: &str, new_body: &str, old: &str) -> ContentUpdate {
        fs::write(server.join(new_name), new_body).unwrap();
        let url = url::Url::from_file_path(server.join(new_name))
            .unwrap()
            .to_string();
        let json = version_json("v2", new_name, &url, &sha1_of(new_body));
        let latest = crate::discover::decode_versions(format!("[{json}]").as_bytes())
            .unwrap()
            .remove(0);
        ContentUpdate {
            kind: ProjectKind::Mod,
            file_name: old.to_owned(),
            current_sha1: sha1_of("old"),
            latest,
        }
    }

    #[tokio::test]
    async fn applying_downloads_and_verifies_then_removes_the_old_file() {
        let s = setup();
        fs::write(s.game.join("mods/old.jar"), "old").unwrap();
        let update = update_for(&s.server, "new.jar", "new", "old.jar");
        let engine = TransferEngine::new(FileTransport, 2).unwrap();
        let sources = vec![update.latest.install_file().unwrap().url.clone()];
        let name = apply(&engine, &update, &s.game, sources, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(name, "new.jar");
        assert_eq!(
            fs::read_to_string(s.game.join("mods/new.jar")).unwrap(),
            "new"
        );
        assert!(!s.game.join("mods/old.jar").exists());
    }

    #[tokio::test]
    async fn a_failed_download_keeps_the_old_file() {
        let s = setup();
        fs::write(s.game.join("mods/old.jar"), "old").unwrap();
        let mut update = update_for(&s.server, "new.jar", "new", "old.jar");
        // Corrupt what the "server" holds: the verified hash no longer matches.
        fs::write(s.server.join("new.jar"), "tampered").unwrap();
        update.latest.files[0].size = 0;
        let engine = TransferEngine::new(FileTransport, 2).unwrap();
        let sources = vec![update.latest.install_file().unwrap().url.clone()];
        let result = apply(&engine, &update, &s.game, sources, CancellationToken::new()).await;
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(s.game.join("mods/old.jar")).unwrap(),
            "old"
        );
        assert!(!s.game.join("mods/new.jar").exists());
    }

    #[tokio::test]
    async fn the_same_file_name_is_replaced_in_place() {
        let s = setup();
        fs::write(s.game.join("mods/same.jar"), "old").unwrap();
        let update = update_for(&s.server, "same.jar", "new", "same.jar");
        let engine = TransferEngine::new(FileTransport, 2).unwrap();
        let sources = vec![update.latest.install_file().unwrap().url.clone()];
        apply(&engine, &update, &s.game, sources, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(
            fs::read_to_string(s.game.join("mods/same.jar")).unwrap(),
            "new"
        );
    }
}
