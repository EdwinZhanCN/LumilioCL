//! A `java` the tests can start on every platform: `fake_java_program.rs`,
//! compiled once per change into the target directory and told what to do
//! by a `java.fake` file beside each copy.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use super::{World, sha1_hex};

const PROGRAM: &str = include_str!("fake_java_program.rs");

/// Prints the line the launcher waits for and notes the arguments in
/// `args.txt` of the instance folder.
pub(crate) const ECHO_ARGS: &str = "print Setting user: x\nargs > $INST_DIR/args.txt";

/// The managed Java 21 the launcher picks, acting out `script`.
pub(crate) fn fake_java(world: &World, script: &str) {
    let jdk = world.service.layout().runtimes().join("jdk-21");
    std::fs::create_dir_all(&jdk).unwrap();
    std::fs::write(jdk.join("release"), "JAVA_VERSION=\"21.0.1\"\n").unwrap();
    install_java(&jdk.join("bin"), script);
}

/// `bin/java` in `bin`, acting out `script`; returns the executable.
pub(crate) fn install_java(bin: &Path, script: &str) -> PathBuf {
    std::fs::create_dir_all(bin).unwrap();
    let java = bin.join(format!("java{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(program(), &java).unwrap();
    std::fs::write(bin.join("java.fake"), script).unwrap();
    java
}

/// The compiled program, built by the first test that needs it. Concurrent
/// test processes may each build it; each publishes by an atomic rename.
fn program() -> &'static Path {
    static PROGRAM_PATH: OnceLock<PathBuf> = OnceLock::new();
    PROGRAM_PATH.get_or_init(|| {
        let digest = sha1_hex(PROGRAM.as_bytes());
        // target/<profile>/deps/<test binary> → target/<profile>/fake-java/<digest>
        let dir = std::env::current_exe()
            .unwrap()
            .parent()
            .and_then(Path::parent)
            .expect("the test binary is in target/<profile>/deps")
            .join("fake-java")
            .join(digest);
        let built = dir.join(format!("java{}", std::env::consts::EXE_SUFFIX));
        if built.is_file() {
            return built;
        }
        std::fs::create_dir_all(&dir).unwrap();
        let unique = std::process::id();
        let source = dir.join(format!("program-{unique}.rs"));
        let output = dir.join(format!("java-{unique}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, PROGRAM).unwrap();
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let status = Command::new(rustc)
            .args(["--edition", "2024", "-o"])
            .arg(&output)
            .arg(&source)
            .status()
            .expect("rustc starts");
        assert!(status.success(), "the fake java compiles");
        // Windows refuses to replace a copy another test is running.
        if std::fs::rename(&output, &built).is_err() {
            assert!(built.is_file(), "the fake java is published");
        }
        std::fs::remove_file(&source).ok();
        built
    })
}
