//! Entity geometry for `bake-blocks` (plan schematic-studio, T6/D4).
//!
//! For every release from 1.17 on, the dumper in `tools/entity-dump` runs
//! once with the client jar and its libraries on the class path and a Java
//! runtime the release accepts. Obfuscated releases (1.17–1.21.x) get a names
//! file built from Mojang's `client_mappings`; the mappings stay in the
//! ignored cache. 1.13.2–1.16.5 have no `LayerDefinitions` and borrow 1.17's
//! geometry. The dumps go into `entities.dat.z`
//! ([`lumilio_block_assets::write_entities`]).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use lumilio_block_assets::{
    DUMPER_JAVA, EntityBundle, EntityDump, EntitySource, dumper_arguments, dumper_failure,
    install_dumper, read_dump, write_entities,
};
use serde_json::Value;

use crate::bake_blocks::{fetch, read_json};
use crate::proguard::{Mappings, dumper_names};
use crate::release::Result;

pub const ENTITIES: &str = "entities.dat.z";
/// The first release with `LayerDefinitions`.
pub const FIRST_DUMPED: &str = "1.17";
const WORKERS: usize = 4;

/// A release whose geometry is wanted, oldest first.
pub struct EntityJar<'a> {
    pub id: &'a str,
    pub jar: &'a Path,
}

/// What the entity bake made.
pub struct EntityBake {
    pub bytes: Vec<u8>,
    /// Distinct layers stored.
    pub layers: usize,
    /// Versions in the file, with the version they borrow from.
    pub versions: Vec<(String, Option<String>)>,
    /// Versions left out, with the reason.
    pub failed: Vec<(String, String)>,
    /// Version → the Java runtime that dumped it.
    pub runtimes: Vec<(String, String)>,
    pub elapsed: Duration,
}

/// A Java runtime on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runtime {
    pub java: PathBuf,
    pub major: u32,
}

/// The major version in `java -version` output: `"25.0.2"` → 25,
/// `"1.8.0_402"` → 8.
pub fn java_major(output: &str) -> Option<u32> {
    let quoted = output.split('"').nth(1)?;
    let mut parts = quoted.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// The smallest runtime at least `needed`: the release's own `javaVersion`
/// and the dumper's class file version, whichever is newer.
pub fn pick_runtime(runtimes: &[Runtime], needed: u32) -> Option<&Runtime> {
    runtimes
        .iter()
        .filter(|runtime| runtime.major >= needed)
        .min_by_key(|runtime| runtime.major)
}

/// Whether a library entry applies on `os` (`osx`, `linux` or `windows`).
/// Rules with `features` describe launch options, not the class path.
pub fn library_allowed(library: &Value, os: &str) -> bool {
    let Some(rules) = library["rules"].as_array() else {
        return true;
    };
    let mut allowed = false;
    for rule in rules {
        if rule.get("features").is_some() {
            continue;
        }
        let applies = rule["os"]["name"].as_str().is_none_or(|name| name == os);
        if applies {
            allowed = rule["action"] == "allow";
        }
    }
    allowed
}

fn mojang_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "osx",
        "windows" => "windows",
        _ => "linux",
    }
}

/// The release's libraries in the cache, each checked against its sha1.
fn libraries(meta: &Value, work: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for library in meta["libraries"].as_array().into_iter().flatten() {
        let artifact = &library["downloads"]["artifact"];
        let (Some(path), Some(url), Some(sha1)) = (
            artifact["path"].as_str(),
            artifact["url"].as_str(),
            artifact["sha1"].as_str(),
        ) else {
            continue;
        };
        if !library_allowed(library, mojang_os()) || !seen.insert(path.to_owned()) {
            continue;
        }
        let to = work.join("libraries").join(path);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fetch(url, &to, sha1)?;
        paths.push(to);
    }
    Ok(paths)
}

/// Java runtimes on this machine: `JAVA_HOME`, the system's registered
/// JDKs, the launcher's own managed runtimes and `java` on `PATH`.
pub fn find_runtimes() -> Vec<Runtime> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let mut homes: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        homes.push(home.into());
    }
    if cfg!(target_os = "macos")
        && let Ok(out) = Command::new("/usr/libexec/java_home").arg("-V").output()
    {
        let text = String::from_utf8_lossy(&out.stderr).into_owned()
            + &String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(at) = line.find(" /") {
                homes.push(PathBuf::from(line[at + 1..].trim()));
            }
        }
    }
    let mut roots: Vec<PathBuf> = vec![
        "/usr/lib/jvm".into(),
        "/Library/Java/JavaVirtualMachines".into(),
    ];
    if let Some(data) = launcher_data() {
        roots.push(data.join("runtimes"));
    }
    for root in roots {
        for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let dir = entry.path();
            homes.push(dir.join("Contents/Home"));
            homes.push(dir.clone());
        }
    }
    let mut javas: Vec<PathBuf> = homes
        .iter()
        .map(|home| home.join("bin").join(exe))
        .collect();
    javas.push(PathBuf::from(exe));
    let mut runtimes: Vec<Runtime> = Vec::new();
    for java in javas {
        let canonical = std::fs::canonicalize(&java).unwrap_or_else(|_| java.clone());
        if runtimes
            .iter()
            .any(|known| std::fs::canonicalize(&known.java).ok().as_ref() == Some(&canonical))
        {
            continue;
        }
        let Ok(out) = Command::new(&java).arg("-version").output() else {
            continue;
        };
        if let Some(major) = java_major(&String::from_utf8_lossy(&out.stderr)) {
            runtimes.push(Runtime { java, major });
        }
    }
    runtimes
}

/// The launcher's data folder (`Layout`'s root) for the current user.
fn launcher_data() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match std::env::consts::OS {
        "macos" => home.map(|home| home.join("Library/Application Support/LumilioCL")),
        "windows" => std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("LumilioCL")),
        _ => std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|home| home.join(".local/share")))
            .map(|dir| dir.join("LumilioCL")),
    }
}

/// Everything one dump needs, gathered before the JVMs start.
struct Job {
    id: String,
    java: Runtime,
    classpath: Vec<PathBuf>,
    names: Option<PathBuf>,
    out: PathBuf,
}

fn prepare(jar: &EntityJar, work: &Path, runtimes: &[Runtime]) -> Result<Job> {
    let dir = jar.jar.parent().ok_or("the client jar has no folder")?;
    let meta = read_json(&dir.join("version.json"))?;
    let required = meta["javaVersion"]["majorVersion"].as_u64().unwrap_or(8) as u32;
    let needed = required.max(DUMPER_JAVA);
    let java = pick_runtime(runtimes, needed)
        .ok_or_else(|| format!("no Java {needed} or newer on this machine"))?
        .clone();
    let names = match meta["downloads"]["client_mappings"].as_object() {
        Some(mappings) => {
            let (url, sha1) = (
                mappings["url"].as_str().ok_or("no mappings url")?,
                mappings["sha1"].as_str().ok_or("no mappings sha1")?,
            );
            let file = dir.join("client_mappings.txt");
            fetch(url, &file, sha1)?;
            let text = std::fs::read_to_string(&file).map_err(|error| error.to_string())?;
            let names = dir.join("entity_names.txt");
            std::fs::write(&names, dumper_names(&Mappings::parse(&text)?)?)
                .map_err(|error| error.to_string())?;
            Some(names)
        }
        None => None,
    };
    let mut classpath = vec![jar.jar.to_path_buf()];
    classpath.extend(libraries(&meta, work)?);
    Ok(Job {
        id: jar.id.to_owned(),
        java,
        classpath,
        names,
        out: dir.join("entities.json"),
    })
}

/// Runs the dumper with `java` and keeps its canonical JSON in `out`
/// (the same bytes the launcher keeps for a runtime dump, see
/// [`lumilio_block_assets::read_dump`]).
pub fn run_dumper(
    java: &Path,
    dumper: &Path,
    classpath: &[PathBuf],
    names: Option<&Path>,
    out: &Path,
) -> Result<(EntityDump, Vec<u8>)> {
    let _ = std::fs::remove_file(out);
    // The game's logger writes `logs/` into the working folder.
    let folder = out.parent().ok_or("the dump has no folder")?;
    let output = Command::new(java)
        .current_dir(folder)
        .args(dumper_arguments(dumper, classpath, out, names))
        .output()
        .map_err(|error| format!("cannot run {}: {error}", java.display()))?;
    if !output.status.success() {
        return Err(format!(
            "the dumper failed ({}): {}",
            output.status,
            dumper_failure(&String::from_utf8_lossy(&output.stderr))
        ));
    }
    let (dump, canonical) = read_dump(out).map_err(|error| error.to_string())?;
    std::fs::write(out, &canonical).map_err(|error| error.to_string())?;
    Ok((dump, canonical))
}

/// Dumps every release from 1.17 on and writes the entity file. A release
/// that cannot be dumped is left out and named in [`EntityBake::failed`].
/// `has(version, texture)`: whether that release's baked pack has the
/// texture (`minecraft:textures/entity/bee/bee.png`), for looks that borrow
/// a newer release's textures (T34).
pub fn bake_entities(
    work: &Path,
    jars: &[EntityJar],
    has: &dyn Fn(&str, &str) -> bool,
) -> Result<EntityBake> {
    let started = Instant::now();
    let dumper = install_dumper(&work.join("dumper")).map_err(|error| error.to_string())?;
    let first = jars
        .iter()
        .position(|jar| jar.id == FIRST_DUMPED)
        .ok_or_else(|| format!("no release {FIRST_DUMPED} to dump"))?;
    let runtimes = find_runtimes();
    eprintln!(
        "Java runtimes: {}",
        runtimes
            .iter()
            .map(|runtime| format!("{} ({})", runtime.major, runtime.java.display()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut failed = Vec::new();
    let mut jobs = Vec::new();
    for jar in &jars[first..] {
        match prepare(jar, work, &runtimes) {
            Ok(job) => jobs.push(job),
            Err(error) => failed.push((jar.id.to_owned(), error)),
        }
    }

    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Result<EntityDump>>>> =
        Mutex::new(jobs.iter().map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(at) else { break };
                    let result = run_dumper(
                        &job.java.java,
                        &dumper,
                        &job.classpath,
                        job.names.as_deref(),
                        &job.out,
                    )
                    .map(|(dump, _)| dump);
                    match &result {
                        Ok(dump) => eprintln!(
                            "dumped {}: {} layers, {} renderers{} (Java {})",
                            job.id,
                            dump.layers.len(),
                            dump.entities.len(),
                            dump.entities_error
                                .as_ref()
                                .map(|why| format!(" ({why})"))
                                .unwrap_or_default(),
                            job.java.major
                        ),
                        Err(error) => eprintln!("dump failed for {}: {error}", job.id),
                    }
                    if let Ok(mut results) = results.lock() {
                        results[at] = Some(result);
                    }
                }
            });
        }
    });
    let results = results.into_inner().map_err(|error| error.to_string())?;

    let mut dumps = Vec::new();
    let mut runtimes_used = Vec::new();
    for (job, result) in jobs.iter().zip(results) {
        match result {
            Some(Ok(dump)) => {
                runtimes_used.push((
                    job.id.clone(),
                    format!("Java {} ({})", job.java.major, job.java.java.display()),
                ));
                dumps.push((job.id.clone(), dump));
            }
            Some(Err(error)) => failed.push((job.id.clone(), error)),
            None => failed.push((job.id.clone(), "the dump did not run".to_owned())),
        }
    }
    // Renderers that drew with code take the layers their newer versions
    // draw the same model from; renderers before 1.21.2 cannot say their
    // textures: newer ones lend theirs where the release's own pack has them.
    lumilio_block_assets::borrow_layers(&mut dumps);
    lumilio_block_assets::borrow_textures(&mut dumps, has);
    let borrowed = dumps.iter().find(|(id, _)| id == FIRST_DUMPED);
    // Release order: the borrowed versions first, then the dumped ones.
    let mut sources: Vec<(&str, EntitySource)> = Vec::new();
    for jar in &jars[..first] {
        match borrowed {
            Some((from, dump)) => sources.push((
                jar.id,
                EntitySource::SameAs(
                    from,
                    lumilio_block_assets::borrowed_looks(dump, jar.id, has),
                ),
            )),
            None => failed.push((jar.id.to_owned(), format!("{FIRST_DUMPED} was not dumped"))),
        }
    }
    for (id, dump) in &dumps {
        sources.push((id.as_str(), EntitySource::Dumped(dump)));
    }
    let bytes = write_entities(sources).map_err(|error| error.to_string())?;
    let bundle = EntityBundle::read(&bytes).map_err(|error| error.to_string())?;
    Ok(EntityBake {
        layers: bundle.layers.len(),
        versions: bundle
            .versions
            .iter()
            .map(|version| (version.id.clone(), version.geometry_from.clone()))
            .collect(),
        failed,
        runtimes: runtimes_used,
        elapsed: started.elapsed(),
        bytes,
    })
}
