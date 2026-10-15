//! The schematics folder's files and what is known about them: content
//! fingerprints, duplicate groups, and the sections the catalogue shows them
//! in (one per folder, plan schematic-studio T29).

use std::collections::{BTreeMap, HashMap};

use lumilio_schematic::fingerprint::Fingerprint;

pub(super) fn is_schematic(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".litematic")
}

pub(super) fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The file's name without `.litematic`: what people know a schematic by.
pub(super) fn stem(path: &str) -> &str {
    let name = file_name(path);
    if is_schematic(name) {
        &name[..name.len() - ".litematic".len()]
    } else {
        name
    }
}

/// The folder's files and what is known about them.
#[derive(Default)]
pub(super) struct Tree {
    /// Game-relative paths of the `.litematic` files below the studio folder.
    files: Vec<String>,
    /// Content fingerprints computed so far.
    fingerprints: HashMap<String, Fingerprint>,
}

impl Tree {
    pub(super) fn set_files(&mut self, folder: &str, files: Vec<String>) {
        let mut files: Vec<String> = files
            .into_iter()
            .filter(|path| is_schematic(path) && relative(folder, path).is_some())
            .collect();
        files.sort();
        files.dedup();
        self.fingerprints.retain(|path, _| files.contains(path));
        self.files = files;
    }

    pub(super) fn files(&self) -> &[String] {
        &self.files
    }

    pub(super) fn set_fingerprint(&mut self, file: &str, fingerprint: Fingerprint) {
        if self.files.iter().any(|path| path == file) {
            self.fingerprints.insert(file.to_owned(), fingerprint);
        }
    }

    pub(super) fn forget_fingerprint(&mut self, file: &str) {
        self.fingerprints.remove(file);
    }

    pub(super) fn has_fingerprint(&self, file: &str) -> bool {
        self.fingerprints.contains_key(file)
    }

    pub(super) fn fingerprint(&self, file: &str) -> Option<Fingerprint> {
        self.fingerprints.get(file).copied()
    }

    /// Groups of files with the same content, two or more each, in the path
    /// order of their first file; a marker's number is its group's place.
    pub(super) fn duplicates(&self) -> Vec<Vec<String>> {
        let mut by_print: BTreeMap<Fingerprint, Vec<String>> = BTreeMap::new();
        for (path, print) in &self.fingerprints {
            by_print.entry(*print).or_default().push(path.clone());
        }
        let mut groups: Vec<Vec<String>> = by_print
            .into_values()
            .filter(|group| group.len() > 1)
            .map(|mut group| {
                group.sort();
                group
            })
            .collect();
        groups.sort();
        groups
    }

    /// The files matching `query` (anywhere in the path, any case), one
    /// section per folder below `folder`: the folder itself first (named
    /// `""`), then the others by path.
    pub(super) fn sections(&self, folder: &str, query: &str) -> Vec<(String, Vec<String>)> {
        let query = query.trim().to_lowercase();
        let mut sections: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for path in &self.files {
            let Some(inner) = relative(folder, path) else {
                continue;
            };
            if !query.is_empty() && !inner.to_lowercase().contains(&query) {
                continue;
            }
            let section = inner.rsplit_once('/').map_or("", |(folder, _)| folder);
            sections
                .entry(section.to_owned())
                .or_default()
                .push(path.clone());
        }
        // BTreeMap orders "" before every folder.
        sections.into_iter().collect()
    }
}

/// `path` relative to `folder`, if it lies inside it.
fn relative<'a>(folder: &str, path: &'a str) -> Option<&'a str> {
    path.strip_prefix(folder)?.strip_prefix('/')
}
