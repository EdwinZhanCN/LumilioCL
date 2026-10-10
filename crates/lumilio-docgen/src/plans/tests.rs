use super::*;
use serde_json::{Value, json};

const EXAMPLE: &str = include_str!("../../../../.agents/schemas/examples/proposed.json");

fn example() -> Plan {
    serde_json::from_str(EXAMPLE).unwrap()
}

#[test]
fn invalid_contracts_and_relations_are_rejected() {
    for fixture in [
        include_str!("../../../../.agents/schemas/examples/invalid-status.json"),
        include_str!("../../../../.agents/schemas/examples/missing-acceptance.json"),
    ] {
        assert!(serde_json::from_str::<Plan>(fixture).is_err());
    }
    let original: Value = serde_json::from_str(EXAMPLE).unwrap();
    for (field, value) in [
        ("schemaVersion", json!(2)),
        ("status", json!("shipped")),
        ("title", json!(" ")),
        ("visibility", json!("secret")),
        ("tasks", json!([])),
        ("dependencies", json!(["missing"])),
    ] {
        let mut input = original.clone();
        input[field] = value;
        assert!(
            serde_json::from_value::<Plan>(input)
                .map(|p| validate(&[p]).is_err())
                .unwrap_or(true),
            "{field}"
        );
    }
    let mut input = original.clone();
    input["tasks"][0]
        .as_object_mut()
        .unwrap()
        .remove("acceptance");
    assert!(serde_json::from_value::<Plan>(input).is_err());
    let mut input = original.clone();
    input["unexpected"] = json!(true);
    assert!(serde_json::from_value::<Plan>(input).is_err());
    let mut input = original;
    input["tasks"][0]["unexpected"] = json!(true);
    assert!(serde_json::from_value::<Plan>(input).is_err());
    assert!(validate(&[example(), example()]).is_err());
    let mut p = example();
    p.tasks.push(
        serde_json::from_value(
            json!({"id":"T1","title":"duplicate","status":"proposed","acceptance":"fails"}),
        )
        .unwrap(),
    );
    assert!(validate(&[p]).is_err());
    let mut second = example();
    second.id = "another-plan".into();
    assert!(
        validate(&[example(), second]).is_err(),
        "roadmap IDs must also be unique"
    );
}

#[test]
fn projection_is_a_whitelist_and_visibility_is_independent() {
    let mut p = example();
    p.summary = "PRIVATE_SUMMARY".into();
    p.decisions[0].rationale = "PRIVATE_RATIONALE".into();
    p.roadmap.as_mut().unwrap().enabled = true;
    let files = generate(&[p]);
    assert!(!files["web/src/data/roadmap.generated.json"].contains("example"));
    let mut p = example();
    p.visibility = Visibility::Public;
    assert!(!generate(&[p])["web/src/data/roadmap.generated.json"].contains("example"));
    let mut p = example();
    p.visibility = Visibility::Public;
    p.roadmap.as_mut().unwrap().enabled = true;
    let files = generate(&[p]);
    let public: Value =
        serde_json::from_str(&files["web/src/data/roadmap.generated.json"]).unwrap();
    assert_eq!(
        public["items"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        ["id", "planId", "name", "lede", "version", "status"]
            .into_iter()
            .map(String::from)
            .collect()
    );
    assert!(!files["web/src/data/roadmap.generated.json"].contains("rationale"));
    let mut p = example();
    p.visibility = Visibility::Public;
    p.status = Status::Cancelled;
    p.roadmap.as_mut().unwrap().enabled = true;
    assert!(!generate(&[p])["web/src/data/roadmap.generated.json"].contains("example"));
}

#[test]
fn lifecycle_is_retained_deterministic_and_staleness_fails() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".agents/plans")).unwrap();
    let path = dir.path().join(".agents/plans/example.json");
    fs::write(&path, EXAMPLE).unwrap();
    run(dir.path(), Some("generate")).unwrap();
    let before = fs::read(dir.path().join("docs/plans/example.md")).unwrap();
    run(dir.path(), Some("generate")).unwrap();
    assert_eq!(
        before,
        fs::read(dir.path().join("docs/plans/example.md")).unwrap()
    );
    run(dir.path(), Some("check")).unwrap();
    fs::write(dir.path().join("docs/plans/example.md"), "edited").unwrap();
    assert!(
        run(dir.path(), Some("check"))
            .unwrap_err()
            .contains("stale")
    );
    let mut p = example();
    p.status = Status::Completed;
    assert!(validate(&[p]).is_err());
    let mut p = example();
    p.status = Status::Completed;
    p.tasks[0].status = Status::Completed;
    p.validation[0].result = Some("test passed".into());
    p.outcome = Some("contract demonstrated".into());
    fs::write(&path, serde_json::to_vec_pretty(&p).unwrap()).unwrap();
    run(dir.path(), Some("generate")).unwrap();
    run(dir.path(), Some("check")).unwrap();
    assert!(path.exists());
    fs::write(dir.path().join("docs/plans/orphan.md"), "old").unwrap();
    assert!(
        run(dir.path(), Some("check"))
            .unwrap_err()
            .contains("orphan")
    );
    run(dir.path(), Some("generate")).unwrap();
    assert!(!dir.path().join("docs/plans/orphan.md").exists());
    fs::write(
        dir.path().join("web/src/data/roadmap.generated.json"),
        "edited",
    )
    .unwrap();
    assert!(
        run(dir.path(), Some("check"))
            .unwrap_err()
            .contains("roadmap.generated.json")
    );
    fs::write(&path, "{}").unwrap();
    assert!(run(dir.path(), Some("check")).is_err());
}

#[test]
fn repository_plans_are_current() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    run(&root, Some("check")).unwrap();
}

#[test]
fn schema_is_versioned_and_required_fields_match_the_parser() {
    let schema: Value =
        serde_json::from_str(include_str!("../../../../.agents/schemas/plan.schema.json")).unwrap();
    assert_eq!(schema["properties"]["schemaVersion"]["const"], 1);
    assert_eq!(schema["additionalProperties"], false);
    let serialized = serde_json::to_value(example()).unwrap();
    assert_eq!(
        schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<BTreeSet<_>>(),
        serialized
            .as_object()
            .unwrap()
            .keys()
            .collect::<BTreeSet<_>>()
    );
    for field in ["scope", "tasks", "validation", "decisions", "roadmap"] {
        let mut contract = &schema["properties"][field];
        let mut sample = &serialized[field];
        if let Some(items) = contract.get("items") {
            contract = items;
            sample = &sample[0];
        }
        if let Some(alternatives) = contract.get("anyOf") {
            contract = &alternatives[0];
        }
        assert_eq!(contract["additionalProperties"], false, "{field}");
        assert_eq!(
            contract["properties"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<BTreeSet<_>>(),
            sample.as_object().unwrap().keys().collect::<BTreeSet<_>>(),
            "{field}"
        );
    }
    assert_eq!(
        schema["properties"]["status"]["enum"],
        serde_json::to_value([
            Status::Proposed,
            Status::InProgress,
            Status::Blocked,
            Status::Completed,
            Status::Cancelled
        ])
        .unwrap()
    );
    let input: Value = serde_json::from_str(EXAMPLE).unwrap();
    for field in schema["required"].as_array().unwrap() {
        let mut missing = input.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove(field.as_str().unwrap());
        assert!(
            serde_json::from_value::<Plan>(missing).is_err(),
            "required {field}"
        );
    }
    for status in schema["properties"]["status"]["enum"].as_array().unwrap() {
        assert!(serde_json::from_value::<Status>(status.clone()).is_ok());
    }
}
