//! Presence policy only; the host owns the Discord connection.

use std::collections::BTreeMap;
use std::sync::Mutex;

use lumilio_plugin_api::{
    API_VERSION, DiscordActivity, HostContext, LaunchEvent, LaunchObserver, LaunchTarget, Manifest,
    NativeCapability, Permission, Plugin, PluginError, SettingField, SettingKind, SettingValue,
};

pub const ID: &str = "lumilio.discord";

#[derive(Default)]
pub struct Discord {
    sessions: Mutex<(u64, BTreeMap<u64, LaunchEvent>)>,
}

impl Plugin for Discord {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: ID.into(),
            name: "Discord 游戏状态".into(),
            description: "在 Discord 中显示正在玩的游戏，退出后清除状态".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            api: API_VERSION,
            default_enabled: false,
            permissions: vec![
                Permission::LaunchEvents,
                Permission::Native(NativeCapability::DiscordIpc),
            ],
            settings: vec![
                SettingField {
                    key: "application_id".into(),
                    label: "Discord 应用 ID".into(),
                    help: "填写为 LumilioCL 注册的 Discord 应用 ID。留空时不发送状态。".into(),
                    kind: SettingKind::Text {
                        default: String::new(),
                    },
                },
                SettingField {
                    key: "show_game".into(),
                    label: "显示游戏名".into(),
                    help: "在状态中显示实例名称、Minecraft 版本和加载器。".into(),
                    kind: SettingKind::Toggle { default: true },
                },
                SettingField {
                    key: "show_target".into(),
                    label: "显示世界或服务器".into(),
                    help: "直接启动世界或服务器时，显示世界名或服务器地址。".into(),
                    kind: SettingKind::Toggle { default: false },
                },
            ],
        }
    }

    fn launch_observer(&self) -> Option<&dyn LaunchObserver> {
        Some(self)
    }
}

impl LaunchObserver for Discord {
    fn observe(&self, ctx: &dyn HostContext, event: &LaunchEvent) -> Result<(), PluginError> {
        let id = ctx
            .launch_id()
            .ok_or_else(|| PluginError::InvalidInput("missing launch id".into()))?;
        let mut saved = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Another process can have observed newer saved preferences while
        // this worker was waiting. Never let its old snapshot erase those
        // sessions or submit a native intent under obsolete disclosure rules.
        if ctx.settings_revision() < saved.0 {
            return Ok(());
        }
        if saved.0 != ctx.settings_revision() {
            saved.0 = ctx.settings_revision();
            saved.1.clear();
        }
        let sessions = &mut saved.1;
        match event {
            LaunchEvent::Started { .. } => {
                sessions.insert(id, event.clone());
            }
            LaunchEvent::Exited { .. } => {
                sessions.remove(&id);
            }
        }
        // The newest remaining process owns the status; an older exit cannot
        // erase the status of a game that is still running.
        let activity = sessions
            .last_key_value()
            .and_then(|(_, event)| activity(ctx, event));
        ctx.discord_activity(activity)
    }
}

fn activity(ctx: &dyn HostContext, event: &LaunchEvent) -> Option<DiscordActivity> {
    let SettingValue::Text(application_id) = ctx.setting("application_id")? else {
        return None;
    };
    let application_id = application_id.trim().to_owned();
    if application_id.is_empty() {
        return None;
    }
    let LaunchEvent::Started {
        instance_name,
        game_version,
        loader,
        target,
        ..
    } = event
    else {
        return None;
    };
    let details = (ctx.setting("show_game") == Some(SettingValue::Toggle(true)))
        .then(|| format!("{instance_name} · Minecraft {game_version} · {loader}"));
    let state = if ctx.setting("show_target") == Some(SettingValue::Toggle(true)) {
        target.as_ref().map(|target| match target {
            LaunchTarget::World(name) => format!("世界：{name}"),
            LaunchTarget::Server(address) => format!("服务器：{address}"),
        })
    } else {
        None
    };
    Some(DiscordActivity::new(
        application_id,
        details.map(bounded),
        state.map(bounded),
    ))
}

fn bounded(mut text: String) -> String {
    text = text.replace('\0', "");
    let mut end = text.len().min(128);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}

#[cfg(test)]
mod tests;
