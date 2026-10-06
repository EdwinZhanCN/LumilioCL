//! What every platform's packaging shares: where things are, what the
//! artifacts are called, and the small process and file helpers.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest as _, Sha256};

use crate::version::Version;

pub type Result<T = ()> = std::result::Result<T, String>;

/// Platform identity, fixed once released (assets/icons/PACKAGING.md §0).
pub const APP_ID: &str = "app.lumilio.LumilioCL";
pub const APP_NAME: &str = "LumilioCL";
/// The executable, as `lumilio-app` names it.
pub const BINARY: &str = "lumiliocl";
pub const HOMEPAGE: &str = "https://github.com/EdwinZhanCN/LumilioCL";

pub struct Release {
    pub version: Version,
    pub commit: String,
    /// The architecture as artifact names spell it: `arm64` or `x64`.
    pub arch: &'static str,
    pub root: PathBuf,
    pub target: PathBuf,
    pub dist: PathBuf,
}

impl Release {
    pub fn detect() -> Result<Self> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .map_err(|error| format!("cannot find the workspace root: {error}"))?;
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("target"));
        Ok(Self {
            version: Version::workspace(),
            commit: commit(&root),
            arch: arch(std::env::consts::ARCH)?,
            dist: root.join("dist"),
            root,
            target,
        })
    }

    /// `LumilioCL-<version>-<os>-<arch>`: every artifact carries the version,
    /// the OS and the architecture in its name (PACKAGING.md §5).
    pub fn stem(&self, os: &str) -> String {
        artifact_stem(&self.version, os, self.arch)
    }

    /// A clean staging folder for one platform.
    pub fn stage(&self, os: &str) -> Result<PathBuf> {
        let stage = self.target.join("package").join(os);
        if stage.exists() {
            fs::remove_dir_all(&stage).map_err(|error| io("clear", &stage, error))?;
        }
        create_dir(&stage)?;
        Ok(stage)
    }

    pub fn icons(&self) -> PathBuf {
        self.root.join("assets/icons")
    }

    pub fn license(&self) -> PathBuf {
        self.root.join("LICENSE")
    }

    /// The release executable `cargo build --release` leaves behind.
    pub fn binary(&self) -> PathBuf {
        self.target
            .join("release")
            .join(format!("{BINARY}{}", std::env::consts::EXE_SUFFIX))
    }

    pub fn build(&self, env: &[(&str, &str)]) -> Result {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        let mut command = Command::new(cargo);
        command.current_dir(&self.root).args([
            "build",
            "--release",
            "--locked",
            "-p",
            "lumilio-app",
        ]);
        for (key, value) in env {
            command.env(key, value);
        }
        run(&mut command)
    }

    /// The text of the `BUILD.txt` every package carries, so a downloaded
    /// artifact says what it was built from.
    pub fn build_info(&self, os: &str, signature: &str) -> String {
        format!(
            "{APP_NAME} {version}\nos: {os}\narch: {arch}\ncommit: {commit}\nsignature: {signature}\n{HOMEPAGE}\n",
            version = self.version,
            arch = self.arch,
            commit = self.commit,
        )
    }

    pub fn dist_file(&self, name: &str) -> Result<PathBuf> {
        create_dir(&self.dist)?;
        let path = self.dist.join(name);
        if path.exists() {
            fs::remove_file(&path).map_err(|error| io("replace", &path, error))?;
        }
        Ok(path)
    }
}

pub fn artifact_stem(version: &Version, os: &str, arch: &str) -> String {
    format!("{APP_NAME}-{version}-{os}-{arch}")
}

fn arch(rust_arch: &str) -> Result<&'static str> {
    match rust_arch {
        "aarch64" => Ok("arm64"),
        "x86_64" => Ok("x64"),
        other => Err(format!("no release is built for {other}")),
    }
}

/// The commit being built. CI names it; a local build asks git and marks
/// uncommitted changes, since such a build is not reproducible from a commit.
fn commit(root: &Path) -> String {
    if let Ok(sha) = std::env::var("GITHUB_SHA") {
        return sha;
    }
    let git = |args: &[&str]| {
        Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    match git(&["rev-parse", "HEAD"]) {
        Some(sha) if git(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty()) => {
            format!("{sha}-dirty")
        }
        Some(sha) => sha,
        None => "unknown".to_owned(),
    }
}

/// Fills `{{KEY}}` placeholders, refusing a template that still has one left.
pub fn fill(template: &str, values: &[(&str, &str)]) -> Result<String> {
    let mut text = template.to_owned();
    for (key, value) in values {
        text = text.replace(&format!("{{{{{key}}}}}"), value);
    }
    match text.find("{{") {
        Some(at) => Err(format!(
            "template placeholder left unfilled: {}",
            &text[at..text.len().min(at + 32)]
        )),
        None => Ok(text),
    }
}

/// Writes `<artifact>.sha256` in `sha256sum` format beside the artifact.
pub fn write_checksum(artifact: &Path) -> Result<PathBuf> {
    let mut file = fs::File::open(artifact).map_err(|error| io("open", artifact, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| io("read", artifact, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let name = artifact
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no file name", artifact.display()))?;
    let mut sums = artifact.as_os_str().to_owned();
    sums.push(".sha256");
    let sums = PathBuf::from(sums);
    write(&sums, format!("{digest}  {name}\n"))?;
    Ok(sums)
}

pub fn run(command: &mut Command) -> Result {
    let label = describe(command);
    run_labelled(command, &label)
}

/// Runs a command whose arguments must not reach the log (a signing password),
/// naming it by `label` instead.
pub fn run_labelled(command: &mut Command, label: &str) -> Result {
    eprintln!("$ {label}");
    let status = command
        .status()
        .map_err(|error| format!("cannot run {label}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} failed ({status})"))
    }
}

/// Runs a command and returns its standard output.
pub fn read(command: &mut Command) -> Result<String> {
    let output = command
        .output()
        .map_err(|error| format!("cannot run {}: {error}", describe(command)))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "{} failed ({}): {}",
            describe(command),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Whether `program` is on `PATH`.
pub fn on_path(program: &str) -> bool {
    let name = format!("{program}{}", std::env::consts::EXE_SUFFIX);
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(&name).is_file()))
}

fn describe(command: &Command) -> String {
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| part.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn create_dir(path: &Path) -> Result {
    fs::create_dir_all(path).map_err(|error| io("create", path, error))
}

pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    fs::write(path, contents).map_err(|error| io("write", path, error))
}

pub fn copy(from: &Path, to: &Path) -> Result {
    if let Some(parent) = to.parent() {
        create_dir(parent)?;
    }
    fs::copy(from, to).map(drop).map_err(|error| {
        format!(
            "cannot copy {} to {}: {error}",
            from.display(),
            to.display()
        )
    })
}

/// Copies a folder tree, skipping Finder litter.
pub fn copy_tree(from: &Path, to: &Path) -> Result {
    let entries = fs::read_dir(from).map_err(|error| io("read", from, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io("read", from, error))?;
        let name = entry.file_name();
        if name == ".DS_Store" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            copy_tree(&path, &to.join(&name))?;
        } else {
            copy(&path, &to.join(&name))?;
        }
    }
    Ok(())
}

/// Packaging for macOS and Linux links; Windows packaging never does, so
/// there it is an error rather than a different kind of link.
pub fn symlink(target: &Path, link: &Path) -> Result {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).map_err(|error| io("link", link, error))
    }
    #[cfg(not(unix))]
    {
        Err(format!(
            "cannot link {} to {}: symbolic links are only made on Unix",
            link.display(),
            target.display()
        ))
    }
}

/// `chmod`; a no-op where files have no Unix mode.
pub fn set_mode(path: &Path, mode: u32) -> Result {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|error| io("set the mode of", path, error))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

pub fn io(action: &str, path: &Path, error: std::io::Error) -> String {
    format!("cannot {action} {}: {error}", path.display())
}
