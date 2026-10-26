//! Query the authoritative JSON through the shipped CLI without generating files.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use lumilio_docgen::plans::{Plan, Status, Task};
use serde_json::{Value, json};
use tempfile::TempDir;

fn checkout() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("crates/lumilio-docgen")).unwrap();
    fs::create_dir_all(dir.path().join(".agents/plans")).unwrap();
    for (index, (id, status)) in [
        ("echo", Status::Blocked),
        ("delta", Status::InProgress),
        ("charlie", Status::Proposed),
        ("bravo", Status::Cancelled),
        ("alpha", Status::Completed),
    ]
    .into_iter()
    .enumerate()
    {
        let mut plan: Plan = serde_json::from_str(include_str!(
            "../../../.agents/schemas/examples/proposed.json"
        ))
        .unwrap();
        plan.id = id.into();
        plan.title = format!("计划 {id}");
        plan.status = status;
        plan.roadmap = None;
        plan.tasks[0].status = Status::Completed;
        plan.tasks.push(Task {
            id: "T2".into(),
            title: "No longer needed".into(),
            status: Status::Cancelled,
            acceptance: "Keep the cancelled task visible".into(),
        });
        plan.validation[0].result = Some("Verified in the fixture".into());
        plan.outcome = Some("Fixture outcome".into());
        fs::write(
            dir.path().join(format!(".agents/plans/file-{index}.json")),
            serde_json::to_vec_pretty(&plan).unwrap(),
        )
        .unwrap();
    }
    // Querying must not rewrite or remove even stale generated artifacts.
    fs::create_dir_all(dir.path().join("docs/plans")).unwrap();
    fs::write(dir.path().join("docs/plans/orphan.md"), "leave me alone").unwrap();
    dir
}

fn query(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lumilio-docgen"))
        .arg("plans")
        .args(args)
        .env("CARGO_MANIFEST_DIR", root.join("crates/lumilio-docgen"))
        .output()
        .unwrap()
}

fn success(root: &Path, args: &[&str]) -> String {
    let output = query(root, args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if path.is_dir() {
            for (child, contents) in files(&path) {
                result.insert(format!("{name}/{child}"), contents);
            }
        } else {
            result.insert(name.into_owned(), fs::read(&path).unwrap());
        }
    }
    result
}

#[test]
fn lists_statuses_in_id_order_without_counting_cancelled_tasks_as_completed() {
    let dir = checkout();
    let rows: Value = serde_json::from_str(&success(dir.path(), &["list", "--json"])).unwrap();
    let ids: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| &row["id"])
        .collect();
    assert_eq!(ids, ["alpha", "bravo", "charlie", "delta", "echo"]);
    assert_eq!(
        rows[0]["tasks"],
        json!({"completed": 1, "cancelled": 1, "total": 2})
    );
    assert_eq!(rows[0]["title"], "计划 alpha");
    assert_eq!(rows[0]["status"], "completed");
    for (status, expected) in [
        ("completed", vec!["alpha"]),
        ("cancelled", vec!["bravo"]),
        ("proposed", vec!["charlie"]),
        ("in_progress", vec!["delta"]),
        ("blocked", vec!["echo"]),
        ("incomplete", vec!["charlie", "delta", "echo"]),
    ] {
        let rows: Value = serde_json::from_str(&success(
            dir.path(),
            &["list", "--status", status, "--json"],
        ))
        .unwrap();
        let ids: Vec<_> = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|row| &row["id"])
            .collect();
        assert_eq!(ids, expected, "{status}");
    }
    let text = success(dir.path(), &["list", "--status", "completed"]);
    assert!(
        text.contains("alpha\tcompleted\t1/2 (+1 cancelled)\t计划 alpha"),
        "{text}"
    );
}

#[test]
fn shows_stable_id_and_full_json_without_touching_files() {
    let dir = checkout();
    let before = files(dir.path());
    let text = success(dir.path(), &["show", "alpha"]);
    for expected in [
        "alpha",
        "计划 alpha",
        "T1",
        "T2",
        "cancelled",
        "Keep the cancelled task visible",
        "Fixture outcome",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    let detail: Value =
        serde_json::from_str(&success(dir.path(), &["show", "alpha", "--json"])).unwrap();
    let source: Value = serde_json::from_slice(&before[".agents/plans/file-4.json"]).unwrap();
    assert_eq!(detail, source);
    success(dir.path(), &["list"]);
    success(dir.path(), &["list", "--json", "--status", "incomplete"]);
    assert_eq!(files(dir.path()), before);
}

#[test]
fn rejects_bad_arguments_and_missing_ids_without_stdout_or_writes() {
    let dir = checkout();
    let before = files(dir.path());
    for args in [
        vec!["list", "--status"],
        vec!["list", "--status", "finished"],
        vec!["list", "--status", "completed", "--status", "blocked"],
        vec!["list", "--unknown"],
        vec!["show"],
        vec!["show", "missing"],
        vec!["show", "alpha", "bravo"],
        vec!["show", "alpha", "--status", "completed"],
        vec!["validate", "--json"],
        vec!["generate", "--unknown"],
    ] {
        let output = query(dir.path(), &args);
        assert!(!output.status.success(), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
    assert_eq!(files(dir.path()), before);
}

#[test]
fn empty_results_are_valid_json_and_queries_report_invalid_source() {
    let dir = checkout();
    for entry in fs::read_dir(dir.path().join(".agents/plans")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    assert_eq!(success(dir.path(), &["list", "--json"]).trim(), "[]");
    assert!(success(dir.path(), &["list"]).contains("No matching plans"));
    fs::write(dir.path().join(".agents/plans/broken.json"), "{}").unwrap();
    let output = query(dir.path(), &["list", "--json"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("broken.json"));
}
