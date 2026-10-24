//! Guards the measurable parts of the workspace's Rust style contract.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, Item, ItemMod, ItemUse, Meta, UseTree};

fn test_only(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<Meta>()
                .is_ok_and(|meta| requires_test(&meta))
    })
}

fn requires_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) => {
            let Ok(conditions) = list.parse_args_with(
                syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
            ) else {
                return false;
            };
            if list.path.is_ident("all") {
                conditions.iter().any(requires_test)
            } else if list.path.is_ident("any") {
                !conditions.is_empty() && conditions.iter().all(requires_test)
            } else {
                false
            }
        }
        _ => false,
    }
}

fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("workspace sources are readable")
        .map(|entry| entry.expect("source entry is readable").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

fn module_dir(path: &Path) -> PathBuf {
    match path.file_stem().and_then(|name| name.to_str()) {
        Some("lib" | "main" | "mod") => path.parent().unwrap().to_owned(),
        _ => path.with_extension(""),
    }
}

fn is_test_file(path: &Path, test_modules: &BTreeSet<PathBuf>) -> bool {
    path.components().any(|part| part.as_os_str() == "tests")
        || path.file_stem().is_some_and(|stem| {
            let stem = stem.to_string_lossy();
            stem == "tests" || stem.ends_with("_tests")
        })
        || test_modules
            .iter()
            .any(|module| path == module.with_extension("rs") || path.starts_with(module))
}

struct Style {
    problems: Vec<String>,
    block_depth: usize,
}

impl Style {
    fn problem(&mut self, span: proc_macro2::Span, message: &str) {
        self.problems
            .push(format!("{}: {message}", span.start().line));
    }
}

fn nested_group(tree: &UseTree, in_group: bool) -> bool {
    match tree {
        UseTree::Path(path) => in_group || nested_group(&path.tree, false),
        UseTree::Group(group) => group.items.iter().any(|tree| nested_group(tree, true)),
        _ => false,
    }
}

fn forbidden_glob(tree: &UseTree, path: &mut Vec<String>, local: bool) -> bool {
    match tree {
        UseTree::Path(next) => {
            path.push(next.ident.to_string());
            let forbidden = forbidden_glob(&next.tree, path, local);
            path.pop();
            forbidden
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .any(|tree| forbidden_glob(tree, path, local)),
        UseTree::Glob(_) => {
            path != &["gpui", "prelude"]
                && !(local
                    && path
                        .last()
                        .is_some_and(|name| name.starts_with(char::is_uppercase)))
        }
        _ => false,
    }
}

impl<'ast> Visit<'ast> for Style {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.block_depth += 1;
        syn::visit::visit_block(self, block);
        self.block_depth -= 1;
    }

    fn visit_item_mod(&mut self, module: &'ast ItemMod) {
        if test_only(&module.attrs) {
            if module.content.is_some() {
                self.problem(
                    module.span(),
                    "move unit tests into an adjacent tests.rs or tests/ file",
                );
            }
            return;
        }
        syn::visit::visit_item_mod(self, module);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if !test_only(&item.attrs) {
            syn::visit::visit_item_fn(self, item);
        }
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if !test_only(&item.attrs) {
            syn::visit::visit_impl_item_fn(self, item);
        }
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        if test_only(&item.attrs) {
            return;
        }
        if nested_group(&item.tree, false) {
            self.problem(
                item.span(),
                "a use group must contain items from one module",
            );
        }
        // GPUI explicitly supports its fluent-trait prelude (Google's prelude exception).
        if forbidden_glob(&item.tree, &mut Vec::new(), self.block_depth > 0)
            && matches!(item.vis, syn::Visibility::Inherited)
        {
            self.problem(
                item.span(),
                "production imports must name their items explicitly",
            );
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "unwrap" {
            self.problem(
                call.span(),
                "match recoverable failures or explain the invariant with expect",
            );
        }
        if call.method == "unwrap_or_else" && call.args.iter().any(|argument| {
            matches!(argument, syn::Expr::Path(path) if path.path.segments.iter().any(|s| s.ident == "PoisonError") && path.path.segments.last().is_some_and(|s| s.ident == "into_inner"))
        }) {
            self.problem(call.span(), "do not continue with state from a poisoned lock");
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}

fn problems(source: &str) -> Vec<String> {
    let file = syn::parse_file(source).expect("source must parse as Rust");
    let mut style = Style {
        problems: Vec::new(),
        block_depth: 0,
    };
    style.visit_file(&file);
    let test_lines: BTreeSet<_> = file
        .items
        .iter()
        .filter_map(|item| {
            let attrs = match item {
                Item::Mod(item) => &item.attrs,
                Item::Fn(item) => &item.attrs,
                Item::Use(item) => &item.attrs,
                _ => return None,
            };
            test_only(attrs).then(|| item.span().start().line..=item.span().end().line)
        })
        .flatten()
        .collect();
    let lines = source
        .lines()
        .enumerate()
        .filter(|(ix, line)| {
            let line = line.trim();
            !test_lines.contains(&(ix + 1)) && !line.is_empty() && !line.starts_with("//")
        })
        .count();
    if lines > 800 {
        style.problems.push(format!(
            "{lines} production lines: split responsibilities before 800 lines"
        ));
    }
    let children = file
        .items
        .iter()
        .any(|item| matches!(item, Item::Mod(module) if !test_only(&module.attrs)));
    if children {
        let implementation: usize = file
            .items
            .iter()
            .filter(|item| matches!(item, Item::Fn(_) | Item::Impl(_)))
            .map(|item| item.span().end().line - item.span().start().line + 1)
            .sum();
        if implementation > 40 {
            style.problems.push("module boundaries contain declarations and re-exports; move implementation into leaf modules".into());
        }
    }
    style.problems
}

#[test]
fn malformed_structure_is_rejected_without_matching_strings_or_comments() {
    let invalid = [
        "use std::{fs, path::Path};",
        "use std::collections::*;",
        "use RecoveryNote::*;",
        "fn value() -> u8 { Some(1).unwrap() }",
        "#[cfg(test)] mod tests { #[test] fn value() {} }",
        "fn value() { state.lock().unwrap_or_else(std::sync::PoisonError::into_inner); }",
    ];
    for source in invalid {
        assert!(
            !problems(source).is_empty(),
            "missed structural violation in {source}"
        );
    }
    assert!(
        problems("use std::path::{Path, PathBuf};\nuse gpui::prelude::*;\n#[cfg(test)] mod tests;")
            .is_empty()
    );
    assert!(problems("const TEXT: &str = \"use foo::*; Some(1).unwrap()\";").is_empty());
    assert!(problems("mod child;\nfn composition() {}\n").is_empty());
    assert!(problems("fn describe() { use RecoveryNote::*; }\n").is_empty());
    let oversized = "const X: &[u8] = &[\n".to_owned() + &"    1,\n".repeat(800) + "];\n";
    assert!(
        problems(&oversized)
            .iter()
            .any(|p| p.contains("production lines"))
    );
}

#[test]
fn all_crates_follow_the_structure_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let crates = root.join("crates");
    let mut files = Vec::new();
    rust_files(&crates, &mut files);
    let mut test_modules = BTreeSet::new();
    for path in &files {
        let source = fs::read_to_string(path).unwrap();
        let file = syn::parse_file(&source).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for item in &file.items {
            if let Item::Mod(module) = item
                && test_only(&module.attrs)
                && module.content.is_none()
            {
                test_modules.insert(module_dir(path).join(module.ident.to_string()));
            }
        }
    }
    let mut violations = Vec::new();
    for path in files {
        if is_test_file(&path, &test_modules) {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        for problem in problems(&source) {
            // Binary roots compose startup/CLI dispatch; that glue is their responsibility.
            if path.file_name().is_some_and(|name| name == "main.rs")
                && problem.starts_with("module boundaries")
            {
                continue;
            }
            violations.push(format!(
                "{}:{problem}",
                path.strip_prefix(&root).unwrap_or(&path).display()
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "Rust structure/style violations:\n{}",
        violations.join("\n")
    );
}
