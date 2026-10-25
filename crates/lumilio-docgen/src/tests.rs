use super::*;

mod structure;

fn path(page: &str, action: &str) -> IaPath {
    IaPath {
        page: page.into(),
        action: action.into(),
        place: "L3".into(),
        result: "ok".into(),
        note: None,
        file: "lumilio-ui/src/a.rs".into(),
    }
}

#[test]
fn an_annotation_has_three_fields_and_an_optional_note() {
    let line = "    // ia[library]: 搜索 | L3 `Input` | 按名称过滤 | 空结果另说";
    let parsed = parse_line(line).unwrap().unwrap();
    assert_eq!(parsed.page, "library");
    assert_eq!(parsed.action, "搜索");
    assert_eq!(parsed.result, "按名称过滤");
    assert_eq!(parsed.note.as_deref(), Some("空结果另说"));
    assert!(parse_line("// an ordinary comment").is_none());
    assert!(parse_line("let x = 1; // ia[library]: a | b | c").is_none());
    assert!(parse_line("// ia[library]: only | two").unwrap().is_err());
    assert!(
        parse_line("// ia[library]: a | b | c | d | e")
            .unwrap()
            .is_err()
    );
    assert!(parse_line("// ia[library]: a | | c").unwrap().is_err());
}

#[test]
fn unknown_pages_and_repeats_are_problems() {
    assert!(validate(&[path("library", "搜索")]).is_empty());
    let problems = validate(&[
        path("nowhere", "a"),
        path("library", "b"),
        path("library", "c"),
        path("library", "c"),
    ]);
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn only_pages_with_paths_get_a_file_and_stale_files_are_found() {
    let dir = tempfile::tempdir().unwrap();
    let files = generate(&[path("library", "搜索")]);
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

/// Windows names the scanned files with `\`; the pages must not change with it.
#[test]
fn implementing_files_are_named_with_slashes_on_every_platform() {
    let joined = Path::new("lumilio-ui").join("src").join("home.rs");
    assert_eq!(ia::slash_path(&joined), "lumilio-ui/src/home.rs");
    assert_eq!(
        ia::slash_path(Path::new(r"lumilio-ui\src\home.rs")),
        "lumilio-ui/src/home.rs"
    );
}

/// The real repository: annotations are well formed and the generated
/// files are current. Regenerate with
/// `cargo run -p lumilio-docgen -- ia`.
#[test]
fn the_repositorys_generated_paths_are_current() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (paths, mut problems) = scan(&root);
    problems.extend(validate(&paths));
    assert!(problems.is_empty(), "{problems:#?}");
    let stale = stale(&root, &generate(&paths));
    assert!(
        stale.is_empty(),
        "docs/ia/paths is stale: {stale:?}; run `cargo run -p lumilio-docgen -- ia`"
    );
}
