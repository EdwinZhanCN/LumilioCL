//! Builds a demo data folder for screenshots and screen recordings, through
//! the same service the launcher uses, so everything in it is real:
//!
//! ```sh
//! cargo run --release -p lumilio-app --example demo_home -- <data folder> <spec.json>
//! LUMILIO_HOME=<data folder> cargo run --release -p lumilio-app
//! ```
//!
//! Start from an empty folder. The spec lists other launchers' folders to
//! import, games to create (with Modrinth content), modpacks and servers:
//!
//! ```json
//! {
//!   "import": ["/path/to/.minecraft"],
//!   "games": [{ "name": "Survival", "version": "26.3", "loader": "fabric",
//!               "mods": ["fabric-api", "sodium"], "shaders": [], "resourcepacks": [],
//!               "favorite": true }],
//!   "modpacks": ["fabulously-optimized"],
//!   "servers": [{ "game": "Survival", "name": "Hub", "address": "play.example.net" }],
//!   "current": "Survival"
//! }
//! ```
//!
//! Every game is installed, so it starts without downloading. Installing
//! needs an account: with none, the tool borrows an offline one and removes it
//! at the end. Sign in from the launcher afterwards.

use std::path::PathBuf;
use std::sync::Arc;

use lumilio_core::{
    CancellationToken, DefaultTransport, LauncherService, Loader, PackPolicy, ProjectKind,
    ServerEntry, ServiceError, recommended_loader,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    #[serde(default)]
    import: Vec<Import>,
    #[serde(default)]
    games: Vec<Game>,
    #[serde(default)]
    modpacks: Vec<String>,
    #[serde(default)]
    servers: Vec<Server>,
    /// The game the home page continues; otherwise the last one made.
    #[serde(default)]
    current: Option<String>,
}

/// Another launcher's folder; `name` renames the game found there (only
/// when it holds one game).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Import {
    path: PathBuf,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Game {
    name: String,
    version: String,
    #[serde(default)]
    loader: Loader,
    #[serde(default)]
    mods: Vec<String>,
    #[serde(default)]
    shaders: Vec<String>,
    #[serde(default)]
    resourcepacks: Vec<String>,
    #[serde(default)]
    favorite: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Server {
    game: String,
    name: String,
    address: String,
}

type Service = LauncherService<DefaultTransport>;

fn main() -> Result<(), String> {
    let [_, home, spec] = <[String; 3]>::try_from(std::env::args().collect::<Vec<_>>())
        .map_err(|_| "usage: demo_home <data folder> <spec.json>".to_owned())?;
    let spec: Spec = serde_json::from_str(
        &std::fs::read_to_string(&spec).map_err(|error| format!("{spec}: {error}"))?,
    )
    .map_err(|error| format!("{spec}: {error}"))?;

    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    let transport = DefaultTransport::new().map_err(|error| error.to_string())?;
    let service = {
        let _guard = runtime.enter();
        // The same plugins as the launcher (src/backend.rs): the Modrinth
        // plugin provides content and modpacks.
        LauncherService::open(
            PathBuf::from(home),
            transport,
            vec![
                Arc::new(lumilio_plugin_crash_analyzer::CrashAnalyzer),
                Arc::new(lumilio_plugin_litematica::Litematica),
                Arc::new(lumilio_plugin_modrinth::Modrinth),
                Arc::new(lumilio_plugin_discord::Discord::default()),
                Arc::new(lumilio_plugin_world_explorer::WorldExplorer),
            ],
        )
        .map_err(|error| error.to_string())?
    };
    runtime
        .block_on(async {
            // Installing needs an account. Borrow an offline one when there
            // is none, and forget it afterwards, whatever happened.
            let borrowed = if service.settings().await.accounts.is_empty() {
                service.add_account("LumilioDemo", None).await?;
                service
                    .settings()
                    .await
                    .accounts
                    .first()
                    .map(|account| account.key())
            } else {
                None
            };
            let built = build(&service, spec).await;
            if let Some(key) = borrowed {
                service.remove_account(&key).await?;
            }
            built
        })
        .map_err(|error| error.to_string())
}

async fn build(service: &Service, spec: Spec) -> Result<(), ServiceError> {
    let mut names = Vec::new();
    for import in &spec.import {
        let found = service.detect_games(&import.path).await?;
        if import.name.is_some() && found.len() != 1 {
            return Err(ServiceError::Install(format!(
                "{} holds {} games; a name needs exactly one",
                import.path.display(),
                found.len()
            )));
        }
        for game in found {
            println!("import {} ({})", game.name, import.path.display());
            let record = service.import_game(game, CancellationToken::new()).await?;
            let name = match &import.name {
                Some(name) => {
                    service.rename(&record.id, name).await?;
                    name.clone()
                }
                None => record.name,
            };
            install(service, &record.id, &name).await?;
            names.push((name, record.id));
        }
    }
    for game in &spec.games {
        // Pick the loader version as the New Game form does: the
        // recommended one of those offered for this game version.
        let versions = service.loader_versions(game.loader, &game.version).await?;
        let loader_version = recommended_loader(&versions).map(|version| version.version.clone());
        println!(
            "create {} {} {:?} {}",
            game.name,
            game.version,
            game.loader,
            loader_version.as_deref().unwrap_or("")
        );
        let record = service
            .create_instance(
                &game.name,
                Some(&game.version),
                game.loader,
                loader_version.as_deref(),
            )
            .await?;
        for (kind, projects) in [
            (ProjectKind::Mod, &game.mods),
            (ProjectKind::Shader, &game.shaders),
            (ProjectKind::ResourcePack, &game.resourcepacks),
        ] {
            for project in projects {
                let file = service
                    .install_content(&record.id, kind, project, CancellationToken::new())
                    .await?;
                println!("  + {file}");
            }
        }
        if game.favorite {
            service.set_favorite(&record.id, true).await?;
        }
        install(service, &record.id, &record.name).await?;
        names.push((record.name, record.id));
    }
    for project in &spec.modpacks {
        println!("modpack {project}");
        let record = service
            .install_modpack(project, CancellationToken::new())
            .await?;
        install(service, &record.id, &record.name).await?;
        names.push((record.name, record.id));
    }
    let id_of = |game: &str| {
        names
            .iter()
            .find(|(name, _)| name == game)
            .map(|(_, id)| id.clone())
            .ok_or_else(|| ServiceError::Install(format!("no game named {game}")))
    };
    for server in spec.servers {
        let id = id_of(&server.game)?;
        println!("server {} → {}", server.name, server.game);
        let entry = ServerEntry {
            name: server.name,
            address: server.address,
            packs: PackPolicy::Ask,
            icon: None,
        };
        service.add_server(&id, entry).await?;
    }
    if let Some(game) = &spec.current {
        println!("current {game}");
        service.set_current_instance(&id_of(game)?).await?;
    }
    Ok(())
}

async fn install(service: &Service, id: &str, name: &str) -> Result<(), ServiceError> {
    println!("  install {name}");
    // Progress also goes to Activity; the tool only waits for the result.
    let (updates, _ignored) = tokio::sync::mpsc::unbounded_channel();
    service
        .install_instance(id, updates, CancellationToken::new())
        .await
}
