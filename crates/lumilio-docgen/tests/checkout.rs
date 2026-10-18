//! The tool works on the checkout `cargo run` names at run time, not on the
//! one that built it. In October 2026 a binary built in another worktree
//! into the shared `target/` wrote that worktree's docs and left the main
//! checkout's stale while reporting success.

use std::process::Command;

#[test]
fn the_checkout_comes_from_cargo_at_run_time() {
    let checkout = tempfile::tempdir().unwrap();
    let crates = checkout.path().join("crates");
    std::fs::create_dir_all(crates.join("lumilio-docgen")).unwrap();
    std::fs::create_dir_all(crates.join("demo/src")).unwrap();
    std::fs::write(
        crates.join("demo/src/lib.rs"),
        "// ia[nowhere]: 打开 | 首页 | 打开了\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lumilio-docgen"))
        .args(["ia", "--check"])
        .env("CARGO_MANIFEST_DIR", crates.join("lumilio-docgen"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("unknown page ia[nowhere]"), "{stderr}");
}
