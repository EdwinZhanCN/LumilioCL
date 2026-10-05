use lumilio_core::{FileTransport, LauncherService};

#[test]
fn production_refuses_a_busy_root_before_creating_a_window() {
    let dir = tempfile::tempdir().unwrap();
    let _owner = LauncherService::open(dir.path(), FileTransport, Vec::new()).unwrap();
    let database = std::fs::read(dir.path().join("launcher.db")).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lumilio-app"))
        .env("LUMILIO_PREVIEW", "off")
        .env("LUMILIO_HOME", dir.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("already in use"));
    assert_eq!(
        std::fs::read(dir.path().join("launcher.db")).unwrap(),
        database
    );
}
