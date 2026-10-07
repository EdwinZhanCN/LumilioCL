use super::{Catalogs, DOMAIN, Locale, loader, text};
use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use i18n_embed::fluent::FluentLanguageLoader;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Every message id of one catalog, with the arguments it uses.
fn catalog(locale: Locale) -> BTreeMap<String, BTreeSet<String>> {
    let path = format!("{}/{DOMAIN}.ftl", locale.tag());
    let file = Catalogs::get(&path).unwrap_or_else(|| panic!("{path} is compiled in"));
    let source = std::str::from_utf8(&file.data).expect("catalogs are UTF-8");
    let resource = fluent_syntax::parser::parse(source)
        .unwrap_or_else(|(_, errors)| panic!("{path} does not parse: {errors:?}"));
    let mut messages = BTreeMap::new();
    for entry in resource.body {
        if let Entry::Message(message) = entry {
            let mut arguments = BTreeSet::new();
            if let Some(pattern) = &message.value {
                pattern_arguments(pattern, &mut arguments);
            }
            let id = message.id.name.to_owned();
            assert!(
                messages.insert(id.clone(), arguments).is_none(),
                "{path} defines {id} twice"
            );
        }
    }
    messages
}

fn pattern_arguments(pattern: &Pattern<&str>, into: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            expression_arguments(expression, into);
        }
    }
}

fn expression_arguments(expression: &Expression<&str>, into: &mut BTreeSet<String>) {
    match expression {
        Expression::Select { selector, variants } => {
            inline_arguments(selector, into);
            for variant in variants {
                pattern_arguments(&variant.value, into);
            }
        }
        Expression::Inline(inline) => inline_arguments(inline, into),
    }
}

fn inline_arguments(inline: &InlineExpression<&str>, into: &mut BTreeSet<String>) {
    match inline {
        InlineExpression::VariableReference { id } => {
            into.insert(id.name.to_owned());
        }
        InlineExpression::Placeable { expression } => expression_arguments(expression, into),
        InlineExpression::FunctionReference { arguments, .. } => {
            for argument in &arguments.positional {
                inline_arguments(argument, into);
            }
            for argument in &arguments.named {
                inline_arguments(&argument.value, into);
            }
        }
        _ => {}
    }
}

fn english() -> FluentLanguageLoader {
    loader(Locale::English)
}

#[test]
fn english_has_every_chinese_message_with_the_same_arguments() {
    let chinese = catalog(Locale::SimplifiedChinese);
    let english = catalog(Locale::English);
    let missing: Vec<_> = chinese
        .keys()
        .filter(|id| !english.contains_key(*id))
        .collect();
    let extra: Vec<_> = english
        .keys()
        .filter(|id| !chinese.contains_key(*id))
        .collect();
    assert!(missing.is_empty(), "missing in English: {missing:?}");
    assert!(extra.is_empty(), "only in English: {extra:?}");
    for (id, arguments) in &chinese {
        assert_eq!(
            arguments, &english[id],
            "{id} takes different arguments in English"
        );
    }
}

#[test]
fn a_count_chooses_its_english_plural() {
    let english = english();
    let rules = |count: usize| {
        english.get_args_concrete(
            "settings-mirrors-count",
            [("count", count.into())].into_iter().collect(),
        )
    };
    assert_eq!(rules(1), "1 rule");
    assert_eq!(rules(3), "3 rules");
    let chinese = loader(Locale::SimplifiedChinese);
    assert_eq!(
        chinese.get_args_concrete(
            "settings-mirrors-count",
            [("count", 3.into())].into_iter().collect()
        ),
        "3 条"
    );
}

#[test]
fn arguments_carry_no_isolation_marks() {
    let shown = english().get_args_concrete(
        "settings-java-disabled",
        [("title", "Java 21".into())].into_iter().collect(),
    );
    assert_eq!(shown, "Java 21 (turned off)");
}

#[test]
fn a_spaced_separator_keeps_its_space() {
    assert_eq!(english().get("common-list-separator"), ", ");
}

#[test]
fn the_system_language_is_the_first_one_the_launcher_has() {
    let ids = |tags: &[&str]| -> Vec<_> { tags.iter().map(|tag| tag.parse().unwrap()).collect() };
    assert_eq!(
        Locale::first_known(&ids(&["ja-JP", "en-US"])),
        Locale::English
    );
    assert_eq!(
        Locale::first_known(&ids(&["zh-Hant-TW", "en-US"])),
        Locale::SimplifiedChinese
    );
    assert_eq!(
        Locale::first_known(&ids(&["ja-JP"])),
        Locale::SimplifiedChinese
    );
    assert_eq!(Locale::first_known(&[]), Locale::SimplifiedChinese);
}

#[test]
fn a_message_is_formatted_once_and_kept() {
    let first = text("settings-title");
    assert_eq!(first, "设置");
    assert!(std::ptr::eq(first, text("settings-title")));
}

/// Files under `src` that may still hold Chinese string literals, with how
/// many lines do; see `hardcoded.txt`.
#[test]
fn hardcoded_chinese_only_shrinks() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = BTreeMap::new();
    for root in ["crates/lumilio-ui/src", "crates/lumilio-app/src"] {
        for file in rust_files(&workspace.join(root)) {
            let source = std::fs::read_to_string(&file).expect("source is readable");
            let lines = chinese_literal_lines(&source);
            if lines > 0 {
                let relative = file.strip_prefix(&workspace).expect("under the workspace");
                found.insert(relative.to_string_lossy().replace('\\', "/"), lines);
            }
        }
    }
    let list_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/i18n/hardcoded.txt");
    let listed: BTreeMap<String, usize> = std::fs::read_to_string(&list_path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (path, count) = line.rsplit_once(' ').expect("`path count`");
            (path.to_owned(), count.parse().expect("a count"))
        })
        .collect();
    if found == listed {
        return;
    }
    if std::env::var_os("LUMILIO_BLESS_HARDCODED").is_some() {
        let mut list = String::from(
            "# Chinese string literals still in code, per file (lines). Move them into the\n\
             # catalogs under crates/lumilio-ui/i18n; this list may only shrink.\n\
             # Regenerate with `just hardcoded-chinese`.\n",
        );
        for (path, count) in &found {
            list.push_str(&format!("{path} {count}\n"));
        }
        std::fs::write(&list_path, list).expect("the list is writable");
        return;
    }
    let grown: Vec<_> = found
        .iter()
        .filter(|(path, count)| listed.get(*path).is_none_or(|listed| *count > listed))
        .collect();
    assert!(
        grown.is_empty(),
        "new Chinese string literals; put them in the catalogs and use `tr!`: {grown:?}"
    );
    panic!(
        "fewer Chinese string literals than hardcoded.txt lists; run `just hardcoded-chinese` \
         so the list shrinks with them"
    );
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name == "tests" || name == "tests.rs" {
            continue;
        }
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files
}

/// How many lines start or hold a string literal with Chinese in it.
/// Comments are skipped, so `// ia[...]` declarations do not count.
fn chinese_literal_lines(source: &str) -> usize {
    let chars: Vec<char> = source.chars().collect();
    let mut lines = BTreeSet::new();
    let (mut index, mut line) = (0, 0);
    let mut literal: Option<usize> = None;
    let mut block_comment = 0;
    while index < chars.len() {
        let c = chars[index];
        let next = chars.get(index + 1).copied();
        if c == '\n' {
            line += 1;
            index += 1;
            continue;
        }
        if block_comment > 0 {
            if c == '*' && next == Some('/') {
                block_comment -= 1;
                index += 2;
            } else if c == '/' && next == Some('*') {
                block_comment += 1;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(start) = literal {
            match c {
                '\\' => index += 2,
                '"' => {
                    literal = None;
                    index += 1;
                }
                _ => {
                    if ('\u{4e00}'..='\u{9fff}').contains(&c) {
                        lines.insert(start);
                    }
                    index += 1;
                }
            }
            continue;
        }
        match (c, next) {
            ('/', Some('/')) => {
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
            }
            ('/', Some('*')) => {
                block_comment = 1;
                index += 2;
            }
            ('"', _) => {
                literal = Some(line);
                index += 1;
            }
            // A character literal such as '"' or '\'' is not a string.
            ('\'', Some('\\')) => {
                // Past the quote, the backslash and the escaped character.
                index += 3;
                while index < chars.len() && chars[index] != '\'' {
                    index += 1;
                }
                index += 1;
            }
            ('\'', Some(_)) if chars.get(index + 2) == Some(&'\'') => index += 3,
            _ => index += 1,
        }
    }
    lines.len()
}

#[test]
fn the_scanner_counts_literals_not_comments() {
    let source = r#"
// 注释里的「中文」不算
let a = "中文"; // 行尾注释的"中文"也不算
let b = '"'; let c = "plain"; let e = '\''; let f = "\"中文\"";
/* 块注释 "中文" */
let d = format!("{} 个", 1);
"#;
    assert_eq!(chinese_literal_lines(source), 3);
}
