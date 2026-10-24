use serde::Deserialize;

use super::*;

#[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
struct Doc {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    value: String,
}

impl Versioned for Doc {
    fn schema(&self) -> u32 {
        self.schema
    }
    fn set_schema(&mut self, schema: u32) {
        self.schema = schema;
    }
}

#[test]
fn missing_files_load_as_the_default() {
    let dir = tempfile::tempdir().unwrap();
    let loaded: Loaded<Doc> = load(&dir.path().join("a.json"), 1).unwrap();
    assert_eq!(loaded.value, Doc::default());
    assert!(!loaded.recovered);
}

#[test]
fn saves_stamp_the_schema_and_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/a.json");
    let mut doc = Doc {
        schema: 0,
        value: "x".into(),
    };
    save(&path, &mut doc, 3).unwrap();
    let loaded: Loaded<Doc> = load(&path, 3).unwrap();
    assert_eq!(loaded.value.value, "x");
    assert_eq!(loaded.value.schema, 3);
    assert!(!dir.path().join("nested/a.json.tmp").exists());
}

#[test]
fn damage_is_preserved_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    fs::write(&path, "{ nope").unwrap();
    let loaded: Loaded<Doc> = load(&path, 1).unwrap();
    assert!(loaded.recovered);
    assert_eq!(
        fs::read_to_string(dir.path().join("a.json.broken")).unwrap(),
        "{ nope"
    );
    assert!(!path.exists());
}

#[test]
fn a_newer_schema_is_refused_and_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    fs::write(&path, r#"{"schema": 9}"#).unwrap();
    assert!(matches!(
        load::<Doc>(&path, 1),
        Err(PersistError::NewerSchema(9))
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), r#"{"schema": 9}"#);
}
