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
        key: "plugin.world-explorer",
        title: "插件 · 世界地图",
    },
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
        key: "instance.screenshots",
        title: "游戏页 · 截图",
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
    Page {
        key: "plugin.litematica",
        title: "插件 · Litematica 投影",
    },
];

/// One annotated path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IaPath {
    pub page: String,
    pub action: String,
    pub place: String,
    pub result: String,
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
    if !(3..=4).contains(&fields.len()) || fields[..3].iter().any(|field| field.is_empty()) {
        return Some(Err(format!(
            "ia[{page}] needs 操作 | 层 / 组件 | 结果与反馈 [| 备注], got {} field(s)",
            fields.len()
        )));
    }
    Some(Ok(IaPath {
        page: page.trim().to_owned(),
        action: fields[0].to_owned(),
        place: fields[1].to_owned(),
        result: fields[2].to_owned(),
        note: fields
            .get(3)
            .filter(|note| !note.is_empty())
            .map(|n| (*n).to_owned()),
        file: String::new(),
    }))
}

pub(super) fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
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

/// Problems that make a set of annotations wrong: unknown pages and the same
/// action written twice on a page.
pub fn validate(paths: &[IaPath]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for path in paths {
        if !PAGES.iter().any(|page| page.key == path.page) {
            problems.push(format!("{}: unknown page ia[{}]", path.file, path.page));
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

pub(super) fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// The generated file for one page.
pub fn render(page: &Page, paths: &[&IaPath]) -> String {
    let mut out = String::new();
    out.push_str(
        "<!-- 生成文件，不要手改。来源：代码里的 `// ia[...]` 注释；\n     重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->\n",
    );
    out.push_str(&format!("# {} · 已实现的用户路径\n\n", page.title));
    out.push_str("表里每一行都有对应的实现；没有的就是没做。\n\n");
    out.push_str("| 操作 | 层 / 组件 | 结果与反馈 | 备注 | 实现 |\n|---|---|---|---|---|\n");
    for path in paths {
        out.push_str(&format!(
            "| {} | {} | {} | {} | `{}` |\n",
            cell(&path.action),
            cell(&path.place),
            cell(&path.result),
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
        "<!-- 生成文件，不要手改。重新生成：cargo run -p lumilio-docgen -- ia。约定见 ADR 0019。 -->\n# 用户路径索引\n\n每个页面已实现的用户路径；表里每一行都有对应的实现，没有的就是没做。\n\n| 页面 | 路径数 |\n|---|---|\n",
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

pub(super) fn paths_dir(root: &Path) -> PathBuf {
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
