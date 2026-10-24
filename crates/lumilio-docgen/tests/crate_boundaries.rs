//! Production dependencies follow `lumilio-app → lumilio-ui → lumilio-core`,
//! and `lumilio-minecraft` stays pure metadata. Development dependencies are
//! exempt: core tests may assemble real plugins.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use toml::Value;

/// The only crates allowed to depend on each launcher layer.
const LAYER_USERS: &[(&str, &[&str])] = &[
    ("lumilio-app", &[]),
    ("lumilio-ui", &["lumilio-app"]),
    ("lumilio-core", &["lumilio-ui", "lumilio-app"]),
];

/// Crates that never draw: no GPUI underneath them.
const WITHOUT_UI: &[&str] = &["lumilio-core", "lumilio-minecraft"];

/// Everything `lumilio-minecraft` may use: parsing only, no I/O.
const MINECRAFT_DEPENDENCIES: &[&str] = &["regex", "serde", "serde_json"];

fn production_dependencies(table: &toml::Table, workspace: &toml::Table, found: &mut Vec<String>) {
    for section in ["dependencies", "build-dependencies"] {
        let Some(deps) = table.get(section).and_then(Value::as_table) else {
            continue;
        };
        for (alias, spec) in deps {
            let spec = if spec.get("workspace").and_then(Value::as_bool) == Some(true) {
                workspace.get(alias).unwrap_or(spec)
            } else {
                spec
            };
            found.push(
                spec.get("package")
                    .and_then(Value::as_str)
                    .unwrap_or(alias)
                    .to_owned(),
            );
        }
    }
    if let Some(targets) = table.get("target").and_then(Value::as_table) {
        for target in targets.values().filter_map(Value::as_table) {
            production_dependencies(target, workspace, found);
        }
    }
}

fn problems(crates: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut problems = Vec::new();
    for (name, deps) in crates {
        for dep in deps {
            if let Some((_, users)) = LAYER_USERS.iter().find(|(layer, _)| layer == dep)
                && !users.contains(&name.as_str())
            {
                problems.push(format!("{name}: must not depend on {dep}"));
            }
            if WITHOUT_UI.contains(&name.as_str()) && dep.starts_with("gpui") {
                problems.push(format!("{name}: must not depend on {dep}"));
            }
            if name == "lumilio-minecraft" && !MINECRAFT_DEPENDENCIES.contains(&dep.as_str()) {
                problems.push(format!(
                    "{name}: {dep} is outside its pure metadata dependencies"
                ));
            }
        }
    }
    problems
}

fn workspace_crates() -> BTreeMap<String, Vec<String>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workspace: Value =
        toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
    let dependencies = workspace["workspace"]["dependencies"].as_table().unwrap();
    let mut crates = BTreeMap::new();
    for member in workspace["workspace"]["members"].as_array().unwrap() {
        let path = root.join(member.as_str().unwrap()).join("Cargo.toml");
        let manifest: Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        let mut deps = Vec::new();
        production_dependencies(manifest.as_table().unwrap(), dependencies, &mut deps);
        let name = manifest["package"]["name"].as_str().unwrap().to_owned();
        crates.insert(name, deps);
    }
    crates
}

#[test]
fn production_dependencies_follow_the_layers() {
    let problems = problems(&workspace_crates());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn reverse_and_impure_dependencies_are_reported() {
    let crates = BTreeMap::from([
        ("lumilio-core".to_owned(), vec!["lumilio-ui".to_owned()]),
        ("lumilio-ui".to_owned(), vec!["lumilio-core".to_owned()]),
        (
            "lumilio-minecraft".to_owned(),
            vec!["serde".to_owned(), "tokio".to_owned(), "gpui".to_owned()],
        ),
        (
            "lumilio-transfer".to_owned(),
            vec!["lumilio-core".to_owned()],
        ),
    ]);
    assert_eq!(
        problems(&crates),
        [
            "lumilio-core: must not depend on lumilio-ui",
            "lumilio-minecraft: tokio is outside its pure metadata dependencies",
            "lumilio-minecraft: must not depend on gpui",
            "lumilio-minecraft: gpui is outside its pure metadata dependencies",
            "lumilio-transfer: must not depend on lumilio-core",
        ]
    );
}
