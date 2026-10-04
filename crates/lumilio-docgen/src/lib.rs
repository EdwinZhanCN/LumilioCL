//! Generates `docs/ia/paths/*.md` from `// ia[page]:` comments in the code
//! (ADR 0019).
//!
//! ```text
//! // ia[library]: 排序 / 按加载器筛选 | L4 两个下拉 | 视图状态，记在偏好设置里 | H-NAV-04
//! ```
//!
//! The fields are action, layer / component, result, flow ids and an optional
//! note, separated by `|`. A path appears in the generated file if and only if
//! its comment exists, so every generated row means "built".

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// A page whose paths are generated: its key in the comment and the generated
/// file's title.
pub struct Page {
    pub key: &'static str,
    pub title: &'static str,
}

pub const PAGES: &[Page] = &[
    Page {
        key: "navigation",
        title: "全局导航",
    },
    Page {
        key: "home",
        title: "首页",
    },
    Page {
        key: "library",
        title: "游戏库",
    },
    Page {
        key: "discover",
        title: "发现",
    },
    Page {
        key: "activity",
        title: "动态",
    },
    Page {
        key: "accounts",
        title: "账户",
    },
    Page {
        key: "settings",
        title: "设置",
    },
    Page {
        key: "instance",
        title: "游戏页（整体）",
    },
    Page {
        key: "instance.overview",
        title: "游戏页 · 概览",
    },
    Page {
        key: "instance.content",
        title: "游戏页 · 内容",
    },
    Page {
        key: "instance.worlds",
        title: "游戏页 · 世界",
    },
    Page {
        key: "instance.history",
        title: "游戏页 · 历史",
    },
    Page {
        key: "instance.diagnostics",
        title: "游戏页 · 诊断",
    },
    Page {
        key: "instance.settings",
        title: "游戏页 · 设置",
    },
];

/// One annotated path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IaPath {
    pub page: String,
    pub action: String,
    pub place: String,
    pub result: String,
    pub ids: String,
    pub note: Option<String>,
    /// Where it is implemented, relative to `crates/`.
    pub file: String,
}

/// Reads one source line. `None` when it is not an annotation.
pub fn parse_line(line: &str) -> Option<Result<IaPath, String>> {
    let rest = line.trim_start().strip_prefix("//")?.trim_start();
    let rest = rest.strip_prefix("ia[")?;
    let (page, rest) = rest.split_once("]:")?;
    let fields: Vec<&str> = rest.split('|').map(str::trim).collect();
    if !(4..=5).contains(&fields.len()) || fields[..4].iter().any(|field| field.is_empty()) {
        return Some(Err(format!(
            "ia[{page}] needs 操作 | 层 / 组件 | 结果与反馈 | 编号 [| 备注], got {} field(s)",
            fields.len()
        )));
    }
    Some(Ok(IaPath {
        page: page.trim().to_owned(),
        action: fields[0].to_owned(),
        place: fields[1].to_owned(),
        result: fields[2].to_owned(),
        ids: fields[3].to_owned(),
        note: fields
            .get(4)
            .filter(|note| !note.is_empty())
            .map(|n| (*n).to_owned()),
        file: String::new(),
    }))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every annotation under `crates/*/src`, in file then line order, and the
/// problems found while reading them.
pub fn scan(root: &Path) -> (Vec<IaPath>, Vec<String>) {
    let crates = root.join("crates");
    let mut files = Vec::new();
    let mut members: Vec<_> = fs::read_dir(&crates)
        .map(|dir| dir.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    members.sort();
    for member in members {
        // The tool's own tests and docs mention the syntax.
        if member
            .file_name()
            .is_some_and(|name| name == "lumilio-docgen")
        {
            continue;
        }
        rust_files(&member.join("src"), &mut files);
    }
    let mut paths = Vec::new();
    let mut problems = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let relative = file
            .strip_prefix(&crates)
            .unwrap_or(&file)
            .to_string_lossy()
            .into_owned();
        for (index, line) in text.lines().enumerate() {
            match parse_line(line) {
                None => {}
                Some(Ok(mut path)) => {
                    path.file = relative.clone();
                    paths.push(path);
                }
                Some(Err(problem)) => problems.push(format!("{relative}:{}: {problem}", index + 1)),
            }
        }
    }
    (paths, problems)
}

/// The H- and L- flow ids the workflow documents define.
pub fn known_ids(root: &Path) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    let dir = root.join("docs/workflows");
    let Ok(entries) = fs::read_dir(dir) else {
        return ids;
    };
    for entry in entries.flatten() {
        let Ok(text) = fs::read_to_string(entry.path()) else {
            continue;
        };
        for line in text.lines() {
            // `<a id="h-nav-04"></a>H-NAV-04 …` rows and `## L-LIB-01 — …` headings.
            if let Some(at) = line.find("<a id=\"h-") {
                let id = &line[at + 7..];
                if let Some(end) = id.find('"') {
                    ids.insert(id[..end].to_uppercase());
                }
            }
            // `## L-LIB-01 — …`, or a numbered one: `## 4. L-OPS-02 — …`.
            if let Some(heading) = line.strip_prefix("## ") {
                let heading = heading
                    .split_once(". ")
                    .filter(|(number, _)| number.chars().all(|c| c.is_ascii_digit()))
                    .map_or(heading, |(_, rest)| rest);
                if heading.starts_with("L-")
                    && let Some(id) = heading.split_whitespace().next()
                {
                    ids.insert(id.to_owned());
                }
            }
        }
    }
    ids
}

/// The flow ids a `编号` field names: `H-CONTENT-06/07` is two ids. Anything
/// else (`—`, `ARCH …`) names none.
pub fn ids_of(field: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in field.split(['、', ',', '，']) {
        let part = part.trim();
        let mut tokens = part.split('/');
        let Some(first) = tokens.next() else { continue };
        let is_id = (first.starts_with("H-") || first.starts_with("L-"))
            && first
                .rsplit('-')
                .next()
                .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()));
        if !is_id {
            continue;
        }
        let (stem, _) = first.rsplit_once('-').unwrap_or((first, ""));
        out.push(first.to_owned());
        for number in tokens {
            out.push(format!("{stem}-{number}"));
        }
    }
    out
}

/// Problems that make a set of annotations wrong: unknown pages, unknown or
/// misspelt flow ids, the same action written twice on a page.
pub fn validate(paths: &[IaPath], known: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for path in paths {
        if !PAGES.iter().any(|page| page.key == path.page) {
            problems.push(format!("{}: unknown page ia[{}]", path.file, path.page));
        }
        for id in ids_of(&path.ids) {
            if !known.contains(&id) {
                problems.push(format!(
                    "{}: ia[{}] \"{}\" names {id}, which docs/workflows does not define",
                    path.file, path.page, path.action
                ));
            }
        }
        if !seen.insert((path.page.clone(), path.action.clone())) {
            problems.push(format!(
                "{}: ia[{}] \"{}\" is written twice",
                path.file, path.page, path.action
            ));
        }
    }
    problems
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// The generated file for one page.
pub fn render(page: &Page, paths: &[&IaPath]) -> String {
    let mut out = String::new();
    out.push_str(
        "<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；\n     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->\n",
    );
    out.push_str(&format!("# {} · 已实现的用户路径\n\n", page.title));
    out.push_str("表里每一行都有对应的实现；没做的、范围外的见 [../README.md](../README.md)。\n\n");
    out.push_str(
        "| 操作 | 层 / 组件 | 结果与反馈 | 编号 | 备注 | 实现 |\n|---|---|---|---|---|---|\n",
    );
    for path in paths {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | `{}` |\n",
            cell(&path.action),
            cell(&path.place),
            cell(&path.result),
            cell(&path.ids),
            cell(path.note.as_deref().unwrap_or("")),
            path.file
        ));
    }
    out
}

/// Every generated file's relative path (under `docs/ia/paths/`) and content.
pub fn generate(paths: &[IaPath]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for page in PAGES {
        let rows: Vec<&IaPath> = paths.iter().filter(|path| path.page == page.key).collect();
        if !rows.is_empty() {
            out.insert(format!("{}.md", page.key), render(page, &rows));
        }
    }
    out.insert("README.md".to_owned(), render_index(paths));
    out
}

/// The index of the generated pages, with how many paths each holds.
pub fn render_index(paths: &[IaPath]) -> String {
    let mut out = String::from(
        "<!-- 生成文件，不要手改。重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->\n# 用户路径索引\n\n每个页面已实现的用户路径；表里每一行都有对应的实现。没做的、范围外的见 [../README.md](../README.md)。\n\n| 页面 | 路径数 |\n|---|---|\n",
    );
    for page in PAGES {
        let count = paths.iter().filter(|path| path.page == page.key).count();
        if count > 0 {
            out.push_str(&format!(
                "| [{}]({}.md) | {count} |\n",
                page.title, page.key
            ));
        }
    }
    out
}

fn paths_dir(root: &Path) -> PathBuf {
    root.join("docs/ia/paths")
}

/// Writes the generated files and removes ones that no longer have rows.
pub fn write(root: &Path, files: &BTreeMap<String, String>) -> std::io::Result<()> {
    let dir = paths_dir(root);
    fs::create_dir_all(&dir)?;
    for (name, content) in files {
        fs::write(dir.join(name), content)?;
    }
    for entry in fs::read_dir(&dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".md") && !files.contains_key(&name) {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// The generated files that differ from what is on disk (missing, changed, or
/// left over).
pub fn stale(root: &Path, files: &BTreeMap<String, String>) -> Vec<String> {
    let dir = paths_dir(root);
    let mut out = Vec::new();
    for (name, content) in files {
        if fs::read_to_string(dir.join(name)).ok().as_deref() != Some(content.as_str()) {
            out.push(name.clone());
        }
    }
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".md") && !files.contains_key(&name) {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(page: &str, action: &str, ids: &str) -> IaPath {
        IaPath {
            page: page.into(),
            action: action.into(),
            place: "L3".into(),
            result: "ok".into(),
            ids: ids.into(),
            note: None,
            file: "lumilio-ui/src/a.rs".into(),
        }
    }

    #[test]
    fn an_annotation_has_four_fields_and_an_optional_note() {
        let line = "    // ia[library]: 搜索 | L3 `Input` | 按名称过滤 | H-NAV-04 | 空结果另说";
        let parsed = parse_line(line).unwrap().unwrap();
        assert_eq!(parsed.page, "library");
        assert_eq!(parsed.action, "搜索");
        assert_eq!(parsed.ids, "H-NAV-04");
        assert_eq!(parsed.note.as_deref(), Some("空结果另说"));
        assert!(parse_line("// an ordinary comment").is_none());
        assert!(parse_line("let x = 1; // ia[library]: a | b | c | d").is_none());
        assert!(
            parse_line("// ia[library]: only | three | fields")
                .unwrap()
                .is_err()
        );
        assert!(parse_line("// ia[library]: a | | c | d").unwrap().is_err());
    }

    #[test]
    fn a_slash_in_an_id_names_several_ids() {
        assert_eq!(
            ids_of("H-CONTENT-06/07/08、L-CONT-01"),
            ["H-CONTENT-06", "H-CONTENT-07", "H-CONTENT-08", "L-CONT-01"]
        );
        assert!(ids_of("—").is_empty());
        assert!(ids_of("ARCH User Collections").is_empty());
    }

    #[test]
    fn unknown_pages_unknown_ids_and_repeats_are_problems() {
        let known: BTreeSet<String> = ["H-NAV-04".to_owned()].into();
        assert!(validate(&[path("library", "搜索", "H-NAV-04")], &known).is_empty());
        let problems = validate(
            &[
                path("nowhere", "a", "—"),
                path("library", "b", "H-NAV-99"),
                path("library", "c", "—"),
                path("library", "c", "—"),
            ],
            &known,
        );
        assert_eq!(problems.len(), 3, "{problems:?}");
    }

    #[test]
    fn only_pages_with_paths_get_a_file_and_stale_files_are_found() {
        let dir = tempfile::tempdir().unwrap();
        let files = generate(&[path("library", "搜索", "H-NAV-04")]);
        assert_eq!(
            files.keys().collect::<Vec<_>>(),
            ["README.md", "library.md"],
            "an index, and a file only for the page that has paths"
        );
        assert!(files["README.md"].contains("[游戏库](library.md) | 1"));
        assert_eq!(
            stale(dir.path(), &files),
            ["README.md", "library.md"],
            "missing"
        );
        write(dir.path(), &files).unwrap();
        assert!(stale(dir.path(), &files).is_empty());
        let mut edited = files.clone();
        edited.get_mut("library.md").unwrap().push('x');
        assert_eq!(stale(dir.path(), &edited), ["library.md"], "changed");
        assert_eq!(
            stale(dir.path(), &BTreeMap::new()),
            ["README.md", "library.md"],
            "left over"
        );
    }

    /// The real repository: annotations are well formed, name flows that exist,
    /// and the generated files are current. Regenerate with
    /// `cargo run -p lumilio-docgen -- ia`.
    #[test]
    fn the_repositorys_generated_paths_are_current() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (paths, mut problems) = scan(&root);
        problems.extend(validate(&paths, &known_ids(&root)));
        assert!(problems.is_empty(), "{problems:#?}");
        let stale = stale(&root, &generate(&paths));
        assert!(
            stale.is_empty(),
            "docs/ia/paths is stale: {stale:?}; run `cargo run -p lumilio-docgen -- ia`"
        );
    }
}
