use std::io::{self, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use lumilio_core::{FileTransport, LauncherService, ServiceError};

#[test]
fn root_lock_probe() {
    let Some(root) = std::env::var_os("LUMILIO_LOCK_TEST_ROOT") else {
        return;
    };
    let result = LauncherService::open(root, FileTransport, Vec::new());
    if std::env::var("LUMILIO_LOCK_TEST_MODE").as_deref() == Ok("busy") {
        assert!(matches!(result, Err(ServiceError::RootBusy(_))));
    } else {
        let _service = result.unwrap();
        std::fs::write(
            std::env::var_os("LUMILIO_LOCK_TEST_MARKER").unwrap(),
            b"held",
        )
        .unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
    }
}

struct Probe(Child);
impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn probe(root: &std::path::Path, mode: &str, marker: &std::path::Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "root_lock_probe", "--nocapture"])
        .env("LUMILIO_LOCK_TEST_ROOT", root)
        .env("LUMILIO_LOCK_TEST_MODE", mode)
        .env("LUMILIO_LOCK_TEST_MARKER", marker);
    command
}

#[test]
fn a_root_has_one_writer_and_process_death_releases_ownership() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let marker = dir.path().join("held");
    let first = LauncherService::open(&root, FileTransport, Vec::new()).unwrap();
    assert!(matches!(
        LauncherService::open(&root, FileTransport, Vec::new()),
        Err(ServiceError::RootBusy(_))
    ));
    let other =
        LauncherService::open(dir.path().join("independent"), FileTransport, Vec::new()).unwrap();
    assert!(probe(&root, "busy", &marker).status().unwrap().success());
    drop(other);
    drop(first);

    let mut holder = Probe(
        probe(&root, "hold", &marker)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() {
        assert!(
            holder.0.try_wait().unwrap().is_none(),
            "probe exited before acquiring the lock"
        );
        assert!(Instant::now() < deadline, "probe never acquired the lock");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(matches!(
        LauncherService::open(&root, FileTransport, Vec::new()),
        Err(ServiceError::RootBusy(_))
    ));
    holder.0.kill().unwrap();
    holder.0.wait().unwrap();
    let reopened = LauncherService::open(&root, FileTransport, Vec::new()).unwrap();
    assert!(root.join("locks/writer.lock").is_file());
    drop(reopened);

    // A cooperative shutdown releases ownership too; lock-file presence is irrelevant.
    std::fs::remove_file(&marker).unwrap();
    let mut holder = Probe(
        probe(&root, "hold", &marker)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() {
        assert!(holder.0.try_wait().unwrap().is_none());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    holder.0.stdin.take().unwrap().write_all(b"exit\n").unwrap();
    assert!(holder.0.wait().unwrap().success());
    LauncherService::open(&root, FileTransport, Vec::new()).unwrap();
}
