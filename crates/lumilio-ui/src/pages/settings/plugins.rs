use super::super::live::LiveCtx;
use super::rows::send;
use crate::kit::ViewIntent;
use crate::live::{LiveIntent, SettingsView};
use crate::tr;
use crate::{controls::Fader, key::Key, kit, theme};
use gpui::{AnyElement, IntoElement, SharedString, div, prelude::*, px};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{Sizable as _, h_flex, v_flex};
use lumilio_core::{PluginInfo, PluginStatus};
use lumilio_plugin_api::{NativeCapability, Permission, SettingField, SettingKind, SettingValue};

/// The view-state group that remembers the chosen plugin.
pub(super) const PLUGIN_GROUP: u8 = 202;

fn plugin_name(info: &PluginInfo) -> String {
    if info.manifest.id == "lumilio.world-explorer" {
        tr!("map-plugin-name").into()
    } else {
        info.manifest.name.clone()
    }
}

pub(super) fn permission_text(permission: &Permission) -> String {
    match permission {
        Permission::ReadGameFiles { under } => {
            tr!("settings-plugin-read-files", folder = under.as_str())
        }
        Permission::Network { hosts } => tr!(
            "settings-plugin-network",
            hosts = hosts.join(tr!("common-list-separator"))
        ),
        Permission::LaunchEvents => tr!("settings-plugin-launch-events").into(),
        Permission::Native(NativeCapability::DiscordIpc) => tr!("settings-plugin-discord").into(),
    }
}

pub(super) fn setting_value(info: &PluginInfo, field: &SettingField) -> SettingValue {
    info.state
        .values
        .get(&field.key)
        .filter(|value| field.kind.accepts(value))
        .cloned()
        .unwrap_or_else(|| field.kind.default_value())
}

/// Master–detail: the plugin list on the leading side, the chosen plugin's
/// state and settings on the trailing side. Each fills the body's height and
/// scrolls on its own, so the list stays in view however long a plugin's
/// settings are.
pub(super) fn render(view: &SettingsView, ctx: &LiveCtx) -> AnyElement {
    if view.plugins.is_empty() {
        return div()
            .debug_selector(|| "settings-plugin-empty".into())
            .child(kit::empty(
                tr!("settings-plugins-none"),
                tr!("settings-plugins-none-help"),
                ctx.colors,
            ))
            .into_any_element();
    }
    let selected = ctx
        .state
        .choice(PLUGIN_GROUP, 0)
        .min(view.plugins.len() - 1);
    // ia[settings]: 选择插件 | 插件 · 左侧列表 | 右侧显示所选插件的状态、权限和设置
    let list = kit::keep_wheel(
        v_flex()
            .id("settings-plugin-list")
            .w(px(220.))
            .flex_none()
            .h_full()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&ctx.controls.plugin_scroll)
            .pb(theme::BOTTOM_SAFE_AREA)
            .children(view.plugins.iter().enumerate().map(|(index, info)| {
                let emit = ctx.emit.clone();
                let id = format!("settings-plugin-{}", info.manifest.id);
                kit::led_option(
                    SharedString::from(id.clone()),
                    plugin_name(info),
                    index == selected,
                    ctx.colors,
                    move |window, app| emit(ViewIntent::Choose(PLUGIN_GROUP, index), window, app),
                )
                .debug_selector(move || id)
            })),
        &ctx.controls.plugin_scroll,
    )
    .into_any_element();
    h_flex()
        .w_full()
        .h_full()
        .min_h_0()
        .items_start()
        .gap_6()
        .child(list)
        .child(
            div()
                .id("settings-plugin-pane")
                .debug_selector(|| "settings-plugin-pane".into())
                .flex_1()
                .min_w_0()
                .h_full()
                .min_h_0()
                .overflow_y_scroll()
                .child(
                    div()
                        .w_full()
                        .pb(theme::BOTTOM_SAFE_AREA)
                        .child(plugin_detail(&view.plugins[selected], ctx)),
                ),
        )
        .into_any_element()
}

/// One plugin's state and settings (§10 rows) under its name.
fn plugin_detail(info: &PluginInfo, ctx: &LiveCtx) -> AnyElement {
    let id = info.manifest.id.clone();
    let enabled = info.state.enabled.unwrap_or(info.manifest.default_enabled);
    let mut rows = Vec::new();
    let status = match info.status {
        PluginStatus::Enabled => tr!("settings-plugin-enabled"),
        PluginStatus::Disabled => tr!("settings-plugin-disabled"),
        PluginStatus::Failed { .. } => tr!("settings-plugin-failed"),
    };
    // ia[settings]: 插件：启用 / 停用 | 插件详情 · 设置行开关 | 保存开关；停用后贡献消失，运行失败的插件重启后恢复
    let toggle = Fader::new(
        SharedString::from(format!("{id}-enabled")),
        enabled,
        tr!("settings-plugin-enable"),
        send(
            &ctx.handler,
            LiveIntent::SetPluginEnabled {
                id: id.clone(),
                enabled: !enabled,
            },
        ),
    );
    rows.push(
        kit::value_row(
            SharedString::from(format!("{id}-state")),
            tr!("settings-plugin-state"),
            None,
            status,
            Some(toggle.into_any_element()),
            ctx.colors,
        )
        .into_any_element(),
    );
    // ia[settings]: 插件：查看权限 | 插件详情 · 权限行 | 逐条显示可读取的游戏文件夹、可访问的主机和其他能力
    for (index, permission) in info.manifest.permissions.iter().enumerate() {
        rows.push(
            kit::value_row(
                SharedString::from(format!("{id}-permission-{index}")),
                tr!("settings-plugin-permission"),
                None,
                permission_text(permission),
                None,
                ctx.colors,
            )
            .into_any_element(),
        );
    }
    if let PluginStatus::Failed { message } = &info.status {
        // ia[settings]: 插件：查看失败原因 | 插件详情 · [技术详情] | 显示本次调用失败原因；失败状态不会写入设置
        rows.push(
            kit::value_row(
                SharedString::from(format!("{id}-failure")),
                tr!("settings-plugin-failure"),
                None,
                tr!("settings-plugin-failure-value"),
                Some(
                    kit::technical(
                        SharedString::from(format!("{id}-technical")),
                        message.clone(),
                    )
                    .into_any_element(),
                ),
                ctx.colors,
            )
            .into_any_element(),
        );
    }
    for field in &info.manifest.settings {
        let value = setting_value(info, field);
        let key = field.key.clone();
        let control_id = SharedString::from(format!("{id}-{key}-control"));
        let handler = ctx.handler.clone();
        let shown = match &value {
            SettingValue::Toggle(_) | SettingValue::Choice(_) => String::new(),
            SettingValue::Text(text) => text.clone(),
            SettingValue::Number(number) => number.to_string(),
        };
        // ia[settings]: 插件：更改设置 | 插件详情 · 声明式设置行 | 开关和选项立即保存；文字与数字在弹窗中校验并保存，失败保留草稿
        let control = match (&field.kind, value) {
            (SettingKind::Toggle { .. }, SettingValue::Toggle(on)) => Fader::new(
                control_id,
                on,
                tr!("settings-plugin-enable-setting"),
                send(
                    &handler,
                    LiveIntent::SetPluginValue {
                        id: id.clone(),
                        key: key.clone(),
                        value: SettingValue::Toggle(!on),
                    },
                ),
            )
            .into_any_element(),
            (SettingKind::Choice { options, .. }, SettingValue::Choice(current)) => {
                let (id, key, options) = (id.clone(), key.clone(), options.clone());
                Key::new(control_id)
                    .label(current.clone())
                    .white()
                    .small()
                    .dropdown_menu(move |mut menu, _, _| {
                        for option in &options {
                            let intent = LiveIntent::SetPluginValue {
                                id: id.clone(),
                                key: key.clone(),
                                value: SettingValue::Choice(option.clone()),
                            };
                            let handler = handler.clone();
                            menu = menu.item(
                                PopupMenuItem::new(option.clone())
                                    .checked(option == &current)
                                    .on_click(move |_, window, cx| {
                                        handler(intent.clone(), window, cx)
                                    }),
                            );
                        }
                        menu
                    })
                    .into_any_element()
            }
            _ => kit::ghost(
                control_id,
                tr!("common-edit-more"),
                send(
                    &handler,
                    LiveIntent::EditPluginSetting {
                        id: id.clone(),
                        key: key.clone(),
                    },
                ),
            )
            .into_any_element(),
        };
        rows.push(
            kit::value_row(
                SharedString::from(format!("{id}-{key}")),
                field.label.clone(),
                (!field.help.is_empty()).then(|| field.help.clone().into()),
                shown,
                Some(control),
                ctx.colors,
            )
            .into_any_element(),
        );
    }
    // ia[settings]: 插件：恢复默认 | 插件详情 · [恢复默认] | 清除启用状态和设置覆盖，使用清单默认值；本次运行的失败状态保留
    rows.push(
        kit::value_row(
            SharedString::from(format!("{id}-defaults")),
            tr!("settings-plugin-defaults"),
            None,
            "",
            Some(
                kit::ghost(
                    SharedString::from(format!("{id}-reset")),
                    tr!("settings-plugin-reset"),
                    send(&ctx.handler, LiveIntent::ResetPlugin(id.clone())),
                )
                .into_any_element(),
            ),
            ctx.colors,
        )
        .into_any_element(),
    );
    kit::section(
        plugin_name(info),
        ctx.colors,
        v_flex()
            .gap_3()
            .child(div().text_sm().text_color(ctx.colors.muted).child(
                if info.manifest.id == "lumilio.world-explorer" {
                    tr!("map-plugin-description").to_owned()
                } else {
                    info.manifest.description.clone()
                },
            ))
            .child(kit::list(rows, ctx.colors)),
    )
    .into_any_element()
}
