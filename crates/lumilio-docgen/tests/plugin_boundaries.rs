//! Plugins stay independent of the launcher and its UI, including renamed,
//! optional, development, build and target-specific dependencies.

use std::fs;
use std::path::Path;
use toml::Value;

fn check_dependencies(table: &toml::Table, workspace: &toml::Table, problems: &mut Vec<String>) {
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(deps) = table.get(section).and_then(Value::as_table) else {
            continue;
        };
        for (alias, spec) in deps {
            let spec = if spec.get("workspace").and_then(Value::as_bool) == Some(true) {
                workspace.get(alias).unwrap_or(spec)
            } else {
                spec
            };
            let package = spec.get("package").and_then(Value::as_str).unwrap_or(alias);
            if ["lumilio-core", "lumilio-ui", "lumilio-app"].contains(&package)
                || package.starts_with("gpui")
            {
                problems.push(format!("{section}: forbidden dependency {package}"));
            }
        }
    }
    if let Some(targets) = table.get("target").and_then(Value::as_table) {
        for target in targets.values().filter_map(Value::as_table) {
            check_dependencies(target, workspace, problems);
        }
    }
}

fn inspect(manifest: &toml::Table, workspace: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    check_dependencies(manifest, workspace, &mut problems);
    let has_api = manifest
        .get("dependencies")
        .and_then(Value::as_table)
        .is_some_and(|deps| {
            deps.iter().any(|(alias, spec)| {
                let spec = if spec.get("workspace").and_then(Value::as_bool) == Some(true) {
                    workspace.get(alias).unwrap_or(spec)
                } else {
                    spec
                };
                spec.get("package").and_then(Value::as_str).unwrap_or(alias) == "lumilio-plugin-api"
            })
        });
    if !has_api {
        problems.push("missing lumilio-plugin-api dependency".into());
    }
    problems
}

#[test]
fn plugin_crates_obey_dependency_boundaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workspace: Value =
        toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
    let dependencies = workspace["workspace"]["dependencies"].as_table().unwrap();
    let mut problems = Vec::new();
    for entry in fs::read_dir(root.join("crates")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("lumilio-plugin-") || name == "lumilio-plugin-api" {
            continue;
        }
        let manifest: Value =
            toml::from_str(&fs::read_to_string(entry.path().join("Cargo.toml")).unwrap()).unwrap();
        problems.extend(
            inspect(manifest.as_table().unwrap(), dependencies)
                .into_iter()
                .map(|error| format!("{name}: {error}")),
        );
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn renamed_and_target_specific_dependencies_cannot_evade_the_guard() {
    let workspace = toml::Table::from_iter([(
        "window".into(),
        toml::from_str::<Value>("package = \"gpui-pre\"\nversion = \"1\"").unwrap(),
    )]);
    for text in [
        "[dependencies]\nlumilio-core = \"1\"",
        "[dependencies]\nengine = { package = \"lumilio-core\", version = \"1\" }",
        "[dev-dependencies]\nlumilio-ui = \"1\"",
        "[build-dependencies]\nlumilio-app = \"1\"",
        "[target.'cfg(unix)'.dependencies]\nwindow = { workspace = true }",
    ] {
        let manifest: Value = toml::from_str(text).unwrap();
        assert!(
            inspect(manifest.as_table().unwrap(), &workspace)
                .iter()
                .any(|problem| problem.contains("forbidden")),
            "{text}"
        );
    }
    assert!(
        inspect(&toml::Table::new(), &workspace)
            .iter()
            .any(|error| error.contains("missing"))
    );
    let good: Value = toml::from_str(
        "[dependencies]\napi = { package = \"lumilio-plugin-api\", version = \"1\" }",
    )
    .unwrap();
    assert!(inspect(good.as_table().unwrap(), &workspace).is_empty());
}
