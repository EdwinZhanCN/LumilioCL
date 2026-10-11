//! Builds a demo data folder for screenshots and screen recordings, through
//! the same service the launcher uses, so everything in it is real:
//!
//! ```sh
//! cargo run --release -p lumilio-app --example demo_home -- <data folder> <spec.json>
//! LUMILIO_HOME=<data folder> cargo run --release -p lumilio-app
//! ```
//!
//! The spec lists other launchers' folders to import, games to create (with
//! Modrinth content and copied files), modpacks and servers. Running it again
//! only adds what is missing, matching games by name, so a folder someone has
//! signed in to can be extended:
//!
//! ```json
//! {
//!   "import": [{ "path": "/path/to/.minecraft", "name": "Imported" }],
//!   "games": [{ "name": "Survival", "version": "26.3", "loader": "fabric",
//!               "mods": ["fabric-api", "sodium"], "shaders": [], "resourcepacks": [],
//!               "copy": [{ "from": "/path/to/World", "into": "saves" }],
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

use std::path::{Path, PathBuf};
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
    /// Files or folders copied into the game folder, e.g. worlds into
    /// `saves` or Litematica projections into `schematics`.
    #[serde(default)]
    copy: Vec<Copy>,
    #[serde(default)]
    favorite: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Copy {
    from: PathBuf,
    into: PathBuf,
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
            let name = import.name.clone().unwrap_or_else(|| game.name.clone());
            if game_named(service, &name).await.is_some() {
                println!("import {name}: already there");
                continue;
            }
            println!("import {name} ({})", import.path.display());
            let record = service.import_game(game, CancellationToken::new()).await?;
            if record.name != name {
                service.rename(&record.id, &name).await?;
            }
            install(service, &record.id, &name).await?;
        }
    }
    for game in &spec.games {
        let id = match game_named(service, &game.name).await {
            Some(id) => {
                println!("game {}: already there", game.name);
                id
            }
            None => create(service, game).await?,
        };
        for (kind, projects) in [
            (ProjectKind::Mod, &game.mods),
            (ProjectKind::Shader, &game.shaders),
            (ProjectKind::ResourcePack, &game.resourcepacks),
        ] {
            let installed = service.installed_projects(&id, kind).await?;
            for project in projects {
                let detail = service.project_detail(project).await?;
                if installed.contains_key(&detail.project.id) {
                    continue;
                }
                let file = service
                    .install_content(&id, kind, project, CancellationToken::new())
                    .await?;
                println!("  + {file}");
            }
        }
        for copy in &game.copy {
            copy_into(&service.layout().game(&id).join(&copy.into), &copy.from)?;
        }
        if game.favorite {
            service.set_favorite(&id, true).await?;
        }
        install(service, &id, &game.name).await?;
    }
    for project in &spec.modpacks {
        let id = service.project_detail(project).await?.project.id;
        let made = service.library().await.instances.into_iter().any(|game| {
            game.source_project
                .is_some_and(|source| source == id || source == *project)
        });
        if made {
            println!("modpack {project}: already there");
            continue;
        }
        println!("modpack {project}");
        let record = service
            .install_modpack(project, CancellationToken::new())
            .await?;
        install(service, &record.id, &record.name).await?;
    }
    for server in spec.servers {
        let id = id_of(service, &server.game).await?;
        if service
            .servers(&id)
            .await?
            .iter()
            .any(|entry| entry.address == server.address)
        {
            println!("server {}: already there", server.name);
            continue;
        }
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
        service
            .set_current_instance(&id_of(service, game).await?)
            .await?;
    }
    Ok(())
}

async fn game_named(service: &Service, name: &str) -> Option<String> {
    service
        .library()
        .await
        .instances
        .into_iter()
        .find(|game| game.name == name)
        .map(|game| game.id)
}

async fn id_of(service: &Service, name: &str) -> Result<String, ServiceError> {
    game_named(service, name)
        .await
        .ok_or_else(|| ServiceError::Install(format!("no game named {name}")))
}

async fn create(service: &Service, game: &Game) -> Result<String, ServiceError> {
    // Pick the loader version as the New Game form does: the recommended one
    // of those offered for this game version.
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
    Ok(record.id)
}

/// Copies a file or a folder into `folder`, keeping what is already there.
fn copy_into(folder: &Path, from: &Path) -> Result<(), ServiceError> {
    let name = from
        .file_name()
        .ok_or_else(|| ServiceError::Install(format!("{} has no name", from.display())))?;
    let target = folder.join(name);
    if target.exists() {
        println!("  = {}", target.display());
        return Ok(());
    }
    std::fs::create_dir_all(folder)?;
    if from.is_dir() {
        for entry in std::fs::read_dir(from)? {
            copy_into(&target, &entry?.path())?;
        }
    } else {
        std::fs::copy(from, &target)?;
    }
    println!("  + {}", target.display());
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
