//! Shared behavior of the small JSON files the launcher keeps.
//!
//! - Writes go through a sibling temp file and a rename, so a crash leaves the
//!   old or the new file, never a torn one.
//! - A file that cannot be parsed is kept next to itself as `<name>.broken` and
//!   the caller starts from the default, so one bad write never locks the user
//!   out.
//! - A file written by a newer launcher (higher `schema`) is refused untouched,
//!   because saving would destroy data this build cannot represent.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Documents that carry a schema number.
pub trait Versioned {
    fn schema(&self) -> u32;
    fn set_schema(&mut self, schema: u32);
}

#[derive(Debug)]
pub enum PersistError {
    Io(io::Error),
    Encode(String),
    NewerSchema(u32),
}

impl Display for PersistError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "storage failed: {error}"),
            Self::Encode(message) => write!(f, "could not be encoded: {message}"),
            Self::NewerSchema(version) => {
                write!(f, "written by a newer launcher (schema {version})")
            }
        }
    }
}

impl Error for PersistError {}

impl From<io::Error> for PersistError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// A loaded document and whether it had to be recovered from damage.
#[derive(Debug)]
pub struct Loaded<T> {
    pub value: T,
    pub recovered: bool,
    /// Where the unreadable original was kept, when it had to be set aside.
    pub preserved: Option<std::path::PathBuf>,
}

/// Moves a damaged file next to itself as `<name>.broken`, or `.broken.1`,
/// `.broken.2`… when an earlier original is already kept: a second failure
/// never overwrites the first evidence.
pub fn set_aside(path: &Path) -> io::Result<std::path::PathBuf> {
    let mut index = 0u32;
    loop {
        let mut name = path.as_os_str().to_owned();
        name.push(".broken");
        if index > 0 {
            name.push(format!(".{index}"));
        }
        let target = std::path::PathBuf::from(name);
        if !target.exists() {
            fs::rename(path, &target)?;
            return Ok(target);
        }
        index += 1;
    }
}

/// Loads `path`, or the default when it does not exist yet.
pub fn load<T>(path: &Path, current: u32) -> Result<Loaded<T>, PersistError>
where
    T: DeserializeOwned + Default + Versioned,
{
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Loaded {
                value: T::default(),
                recovered: false,
                preserved: None,
            });
        }
        Err(error) => return Err(error.into()),
    };
    match serde_json::from_str::<T>(&text) {
        Ok(value) if value.schema() > current => Err(PersistError::NewerSchema(value.schema())),
        Ok(value) => Ok(Loaded {
            value,
            recovered: false,
            preserved: None,
        }),
        Err(_) => {
            let preserved = set_aside(path)?;
            Ok(Loaded {
                value: T::default(),
                recovered: true,
                preserved: Some(preserved),
            })
        }
    }
}

/// Atomically writes `value`, stamping the current schema.
pub fn save<T>(path: &Path, value: &mut T, current: u32) -> Result<(), PersistError>
where
    T: Serialize + Versioned,
{
    value.set_schema(current);
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| PersistError::Encode(error.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = std::path::PathBuf::from(temporary);
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

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
}
