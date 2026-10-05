use super::*;
use lumilio_plugin_api::{FetchResponse, LaunchOutcome};

struct Context {
    id: u64,
    revision: u64,
    values: BTreeMap<String, SettingValue>,
    calls: Mutex<Vec<Option<DiscordActivity>>>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            id: 1,
            revision: 0,
            values: Discord::default()
                .manifest()
                .settings
                .into_iter()
                .map(|field| (field.key, field.kind.default_value()))
                .collect(),
            calls: Mutex::default(),
        }
    }
}

impl HostContext for Context {
    fn launch_id(&self) -> Option<u64> {
        Some(self.id)
    }
    fn settings_revision(&self) -> u64 {
        self.revision
    }
    fn setting(&self, key: &str) -> Option<SettingValue> {
        self.values.get(key).cloned()
    }
    fn discord_activity(&self, activity: Option<DiscordActivity>) -> Result<(), PluginError> {
        self.calls.lock().unwrap().push(activity);
        Ok(())
    }
    fn read_file(&self, _: &str) -> Result<Vec<u8>, PluginError> {
        panic!("no file I/O")
    }
    fn list_files(&self, _: &str) -> Result<Vec<String>, PluginError> {
        panic!("no file I/O")
    }
    fn fetch(&self, _: &str) -> Result<FetchResponse, PluginError> {
        panic!("no network I/O")
    }
}

fn started(target: Option<LaunchTarget>) -> LaunchEvent {
    LaunchEvent::Started {
        instance_name: "Private instance".into(),
        game_version: "1.21.1".into(),
        loader: "Fabric".into(),
        target,
    }
}
fn exited() -> LaunchEvent {
    LaunchEvent::Exited {
        outcome: LaunchOutcome::Clean,
        played_seconds: 42,
    }
}
fn configured() -> Context {
    let mut ctx = Context::default();
    ctx.values.insert(
        "application_id".into(),
        SettingValue::Text("123456789012345678".into()),
    );
    ctx
}

#[test]
fn defaults_are_private_and_an_unconfigured_application_sends_no_presence() {
    let plugin = Discord::default();
    assert!(!plugin.manifest().default_enabled);
    assert_eq!(
        plugin.manifest().permissions,
        vec![
            Permission::LaunchEvents,
            Permission::Native(NativeCapability::DiscordIpc)
        ]
    );
    let ctx = Context::default();
    plugin
        .observe(
            &ctx,
            &started(Some(LaunchTarget::Server("private.test".into()))),
        )
        .unwrap();
    assert_eq!(*ctx.calls.lock().unwrap(), [None]);
}

#[test]
fn both_disclosure_settings_are_read_on_each_call_and_exit_clears() {
    let plugin = Discord::default();
    let mut ctx = configured();
    let event = started(Some(LaunchTarget::Server("mc.test:25565".into())));
    plugin.observe(&ctx, &event).unwrap();
    let activity = ctx.calls.lock().unwrap()[0].clone().unwrap();
    assert_eq!(
        activity.details.as_deref(),
        Some("Private instance · Minecraft 1.21.1 · Fabric")
    );
    assert_eq!(activity.state, None);
    assert!(!format!("{activity:?}").contains("mc.test"));
    ctx.values
        .insert("show_game".into(), SettingValue::Toggle(false));
    ctx.values
        .insert("show_target".into(), SettingValue::Toggle(true));
    plugin.observe(&ctx, &event).unwrap();
    let activity = ctx.calls.lock().unwrap()[1].clone().unwrap();
    assert_eq!(activity.details, None);
    assert!(!format!("{activity:?}").contains("Private instance"));
    assert_eq!(activity.state.as_deref(), Some("服务器：mc.test:25565"));
    plugin.observe(&ctx, &exited()).unwrap();
    assert_eq!(ctx.calls.lock().unwrap().last(), Some(&None));
}

#[test]
fn world_names_are_bounded_without_splitting_unicode() {
    let mut ctx = configured();
    ctx.values
        .insert("show_target".into(), SettingValue::Toggle(true));
    let activity = activity(
        &ctx,
        &started(Some(LaunchTarget::World("世界\0".repeat(80)))),
    )
    .unwrap();
    let state = activity.state.unwrap();
    assert!(state.starts_with("世界："));
    assert!(state.len() <= 128 && !state.contains('\0'));
}

#[test]
fn overlapping_sessions_restore_the_previous_game_and_preference_changes_forget_stale_sessions() {
    let plugin = Discord::default();
    let mut ctx = configured();
    plugin.observe(&ctx, &started(None)).unwrap();
    ctx.id = 2;
    plugin.observe(&ctx, &started(None)).unwrap();
    ctx.id = 1;
    plugin.observe(&ctx, &exited()).unwrap();
    assert!(ctx.calls.lock().unwrap().last().unwrap().is_some());
    ctx.id = 2;
    plugin.observe(&ctx, &exited()).unwrap();
    assert_eq!(ctx.calls.lock().unwrap().last(), Some(&None));
    ctx.id = 3;
    plugin.observe(&ctx, &started(None)).unwrap();
    ctx.revision = 1;
    ctx.id = 4;
    plugin.observe(&ctx, &started(None)).unwrap();
    plugin.observe(&ctx, &exited()).unwrap();
    assert_eq!(ctx.calls.lock().unwrap().last(), Some(&None));
}

#[test]
fn an_old_settings_snapshot_cannot_erase_newer_running_sessions() {
    let plugin = Discord::default();
    let mut current = configured();
    current.revision = 2;
    plugin.observe(&current, &started(None)).unwrap();
    current.id = 2;
    plugin.observe(&current, &started(None)).unwrap();
    let mut stale = configured();
    stale.id = 3;
    stale.revision = 1;
    plugin.observe(&stale, &started(None)).unwrap();
    current.id = 1;
    plugin.observe(&current, &exited()).unwrap();
    assert!(
        current.calls.lock().unwrap().last().unwrap().is_some(),
        "the second current game is still running"
    );
    assert!(
        stale.calls.lock().unwrap().is_empty(),
        "an obsolete context must produce no native intent"
    );
    current.id = 2;
    plugin.observe(&current, &exited()).unwrap();
    assert_eq!(current.calls.lock().unwrap().last(), Some(&None));
}
