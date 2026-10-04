use super::*;

fn entry(name: &str, is_dir: bool) -> FileEntry {
    FileEntry {
        name: name.into(),
        is_dir,
        size: 0,
        modified: 0,
    }
}

fn below() -> BTreeMap<String, Vec<FileEntry>> {
    BTreeMap::from([
        (
            "config".to_owned(),
            vec![entry("sodium.json", false), entry("lithium.toml", false)],
        ),
        ("mods".to_owned(), vec![entry("a.jar", false)]),
        ("saves".to_owned(), vec![entry("w", true)]),
    ])
}

#[test]
fn the_usual_content_is_ticked_with_its_insides_and_worlds_and_logs_are_not_opened() {
    let nodes = tree(
        &[
            entry("config", true),
            entry("logs", true),
            entry("mods", true),
            entry("options.txt", false),
            entry("saves", true),
            entry(".lumilio", true),
        ],
        &below(),
    );
    let names: Vec<_> = nodes.iter().map(|n| (n.name.as_str(), n.on)).collect();
    assert_eq!(
        names,
        [
            ("config", true),
            ("logs", false),
            ("mods", true),
            ("options.txt", false),
            ("saves", false)
        ]
    );
    assert_eq!(nodes[0].children.len(), 2);
    assert!(nodes[0].children.iter().all(|(_, on)| *on));
    assert!(
        nodes[4].children.is_empty(),
        "worlds are not offered one by one"
    );
}

#[test]
fn unticking_one_file_keeps_the_rest_of_the_folder_and_ticking_it_back_restores_the_folder() {
    let mut nodes = tree(&[entry("config", true)], &below());
    assert_eq!(nodes[0].paths(), ["config"]);
    nodes[0].toggle_child(0);
    assert!(!nodes[0].on);
    assert_eq!(nodes[0].paths(), ["config/lithium.toml"]);
    nodes[0].toggle_child(0);
    assert!(nodes[0].on);
    assert_eq!(nodes[0].paths(), ["config"]);
    nodes[0].toggle();
    assert!(nodes[0].paths().is_empty() && nodes[0].children.iter().all(|(_, on)| !*on));
    nodes[0].toggle();
    assert_eq!(nodes[0].paths(), ["config"]);
}

#[test]
fn a_pack_needs_a_name_a_version_and_something_in_it() {
    let nodes = tree(&[entry("mods", true), entry("saves", true)], &below());
    let spec = spec_from(" Pack ", " 1.0 ", "  ", &nodes, PackFormat::Modrinth).unwrap();
    assert_eq!(
        (
            spec.name.as_str(),
            spec.version.as_str(),
            spec.summary.clone()
        ),
        ("Pack", "1.0", None)
    );
    assert_eq!(spec.include, ["mods"]);
    assert_eq!(
        spec_from("P", "1", " hi ", &nodes, PackFormat::Prism)
            .unwrap()
            .summary
            .as_deref(),
        Some("hi")
    );
    assert!(spec_from(" ", "1", "", &nodes, PackFormat::Modrinth).is_err());
    assert!(spec_from("P", " ", "", &nodes, PackFormat::Modrinth).is_err());
    let mut none = nodes;
    none[0].toggle();
    assert!(spec_from("P", "1", "", &none, PackFormat::Modrinth).is_err());
}

#[gpui::test]
fn the_form_starts_from_the_game_and_hands_over_what_is_ticked(cx: &mut gpui::TestAppContext) {
    use std::cell::RefCell;
    cx.update(gpui_component::init);
    let got: Rc<RefCell<Vec<ExportSpec>>> = Rc::default();
    let sink = got.clone();
    let entries = [
        entry("config", true),
        entry("mods", true),
        entry("saves", true),
    ];
    let below = below();
    let (form, cx) = cx.add_window_view(|window, cx| {
        ExportForm::new(
            "生存",
            &entries,
            &below,
            Rc::new(move |spec, _, _| sink.borrow_mut().push(spec)),
            window,
            cx,
        )
    });
    let spec = form.read_with(cx, |form, cx| form.spec(cx)).unwrap();
    assert_eq!(
        (
            spec.name.as_str(),
            spec.version.as_str(),
            spec.include.clone()
        ),
        (
            "生存",
            "1.0.0",
            vec!["config".to_owned(), "mods".to_owned()]
        )
    );

    form.update(cx, |form, _| form.nodes[2].toggle());
    let spec = form.read_with(cx, |form, cx| form.spec(cx)).unwrap();
    assert_eq!(spec.include, ["config", "mods", "saves"]);

    // One file out of config.
    form.update(cx, |form, _| form.nodes[0].toggle_child(1));
    let spec = form.read_with(cx, |form, cx| form.spec(cx)).unwrap();
    assert_eq!(spec.include, ["config/sodium.json", "mods", "saves"]);

    let name = form.read_with(cx, |form, _| form.name.clone());
    cx.update(|window, cx| name.update(cx, |name, cx| name.set_value("  ", window, cx)));
    assert_eq!(
        form.read_with(cx, |form, cx| form.spec(cx)),
        Err("请输入整合包名称")
    );
    assert!(got.borrow().is_empty(), "nothing is handed over by looking");
}
