use super::*;
use lumilio_plugin_api::{ActionId, Effect, GameFacts, InstanceTab, TabState, View, Words};

struct TabPlugin {
    id: &'static str,
    shown: bool,
    panic: bool,
}

impl Plugin for TabPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: self.id.into(),
            name: Words::new(self.id, self.id),
            description: Words::default(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: vec![Permission::ReadGameFiles {
                under: "schematics".into(),
            }],
            settings: Vec::new(),
        }
    }
    fn instance_tab(&self) -> Option<&dyn InstanceTab> {
        Some(self)
    }
}

impl InstanceTab for TabPlugin {
    fn title(&self) -> Words {
        Words::new("投影", "Schematics")
    }
    fn appears(&self, _: &GameFacts, _: &dyn HostContext) -> bool {
        self.shown
    }
    fn view(&self, ctx: &dyn HostContext, state: &TabState) -> Result<View, PluginError> {
        assert!(!self.panic, "boom");
        let files = ctx.list_files("schematics")?;
        Ok(View::Text {
            text: format!("{} {state}", files.join(",")),
            tone: lumilio_plugin_api::Tone::Body,
        })
    }
    fn update(
        &self,
        ctx: &dyn HostContext,
        state: TabState,
        action: ActionId,
    ) -> Result<(TabState, Vec<Effect>), PluginError> {
        let count = state.as_u64().unwrap_or(0) + 1;
        let effects = match action.0.as_str() {
            "reveal" => vec![Effect::RevealGameFile {
                path: "schematics/a.litematic".into(),
            }],
            "escape" => vec![
                Effect::RevealGameFile {
                    path: "schematics/../options.txt".into(),
                },
                Effect::RevealGameFile {
                    path: "mods/x.jar".into(),
                },
                Effect::Toast("hi".into()),
            ],
            "read-outside" => {
                ctx.read_file("options.txt")?;
                Vec::new()
            }
            "save" => vec![Effect::SaveAs {
                suggested_name: "../../evil.csv".into(),
                bytes: b"a".to_vec(),
            }],
            _ => Vec::new(),
        };
        Ok((TabState::from(count), effects))
    }
}

fn plugin(shown: bool, panic: bool) -> PluginHost {
    PluginHost::new(
        vec![Arc::new(TabPlugin {
            id: "test.tab",
            shown,
            panic,
        })],
        BTreeMap::new(),
    )
}

fn game_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("schematics/sub")).unwrap();
    std::fs::write(dir.path().join("schematics/a.litematic"), b"x").unwrap();
    std::fs::write(dir.path().join("schematics/sub/b.litematic"), b"y").unwrap();
    std::fs::write(dir.path().join("options.txt"), b"secret").unwrap();
    dir
}

fn disabled() -> BTreeMap<String, PluginState> {
    BTreeMap::from([(
        "test.tab".to_owned(),
        PluginState {
            enabled: Some(false),
            ..PluginState::default()
        },
    )])
}

#[tokio::test]
async fn tab_appears_only_when_its_condition_holds_and_the_plugin_is_on() {
    let dir = game_dir();
    let tabs = plugin(true, false)
        .tabs(dir.path().to_owned(), GameFacts::default())
        .await;
    assert_eq!(
        tabs,
        vec![PluginTab {
            plugin: "test.tab".into(),
            title: Words::new("投影", "Schematics")
        }]
    );
    assert!(
        plugin(false, false)
            .tabs(dir.path().to_owned(), GameFacts::default())
            .await
            .is_empty()
    );
    let host = PluginHost::new(
        vec![Arc::new(TabPlugin {
            id: "test.tab",
            shown: true,
            panic: false,
        })],
        disabled(),
    );
    assert!(
        host.tabs(dir.path().to_owned(), GameFacts::default())
            .await
            .is_empty()
    );
    assert!(
        host.tab_view("i", dir.path().to_owned(), "test.tab")
            .await
            .is_none()
    );
}

#[tokio::test]
async fn host_keeps_tab_state_per_instance_and_lists_granted_files() {
    let dir = game_dir();
    let host = plugin(true, false);
    host.tab_action("one", dir.path().to_owned(), "test.tab", ActionId::new("x"))
        .await
        .unwrap();
    host.tab_action("one", dir.path().to_owned(), "test.tab", ActionId::new("x"))
        .await
        .unwrap();
    let View::Text { text, .. } = host
        .tab_view("one", dir.path().to_owned(), "test.tab")
        .await
        .unwrap()
    else {
        panic!("text view");
    };
    assert_eq!(text, "schematics/a.litematic,schematics/sub/b.litematic 2");
    let View::Text { text, .. } = host
        .tab_view("two", dir.path().to_owned(), "test.tab")
        .await
        .unwrap()
    else {
        panic!("text view");
    };
    assert!(text.ends_with(" null"), "{text}");
}

#[tokio::test]
async fn effects_outside_the_grant_are_dropped_and_save_names_are_plain() {
    let dir = game_dir();
    let host = plugin(true, false);
    let root = dir.path().to_owned();
    let effects = host
        .tab_action("i", root.clone(), "test.tab", ActionId::new("reveal"))
        .await
        .unwrap();
    assert_eq!(
        effects,
        vec![PluginEffect::Reveal(root.join("schematics/a.litematic"))]
    );
    let effects = host
        .tab_action("i", root.clone(), "test.tab", ActionId::new("escape"))
        .await
        .unwrap();
    assert_eq!(effects, vec![PluginEffect::Toast("hi".into())]);
    let effects = host
        .tab_action("i", root, "test.tab", ActionId::new("save"))
        .await
        .unwrap();
    assert_eq!(
        effects,
        vec![PluginEffect::SaveAs {
            suggested_name: "evil.csv".into(),
            bytes: b"a".to_vec()
        }]
    );
}

#[tokio::test]
async fn reading_outside_the_grant_fails_the_action_not_the_launcher() {
    let dir = game_dir();
    let host = plugin(true, false);
    let result = host
        .tab_action(
            "i",
            dir.path().to_owned(),
            "test.tab",
            ActionId::new("read-outside"),
        )
        .await;
    assert!(result.is_none());
    assert!(matches!(
        host.list().await[0].status,
        PluginStatus::Failed { .. }
    ));
}

#[tokio::test]
async fn a_panicking_tab_only_disables_itself() {
    let dir = game_dir();
    let host = plugin(true, true);
    assert!(
        host.tab_view("i", dir.path().to_owned(), "test.tab")
            .await
            .is_none()
    );
    assert!(matches!(
        host.list().await[0].status,
        PluginStatus::Failed { .. }
    ));
    assert!(
        host.tabs(dir.path().to_owned(), GameFacts::default())
            .await
            .is_empty()
    );
}

#[test]
fn file_access_refuses_traversal_absolute_paths_and_symlinks() {
    let dir = game_dir();
    let manifest = TabPlugin {
        id: "test.tab",
        shown: true,
        panic: false,
    }
    .manifest();
    let root = dir.path();
    assert!(access::read(root, &manifest, "schematics/a.litematic").is_ok());
    for bad in [
        "options.txt",
        "schematics/../options.txt",
        "/etc/passwd",
        "../x",
    ] {
        assert_eq!(
            access::read(root, &manifest, bad),
            Err(PluginError::PermissionDenied),
            "{bad}"
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("options.txt"), root.join("schematics/link")).unwrap();
        assert_eq!(
            access::read(root, &manifest, "schematics/link"),
            Err(PluginError::PermissionDenied)
        );
        std::os::unix::fs::symlink(root, root.join("schematics/loop")).unwrap();
        assert!(
            !access::list(root, &manifest, "schematics")
                .unwrap()
                .iter()
                .any(|file| file.contains("loop") || file.contains("link"))
        );
    }
    std::fs::write(
        root.join("schematics/big"),
        vec![0u8; usize::try_from(access::MAX_FILE_BYTES).unwrap() + 1],
    )
    .unwrap();
    assert!(matches!(
        access::read(root, &manifest, "schematics/big"),
        Err(PluginError::Unavailable(_))
    ));
}
