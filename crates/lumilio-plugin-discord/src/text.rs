//! Discord plugin words, Chinese and English side by side. Proper nouns
//! (Discord, Minecraft, LumilioCL) stay as they are.

use lumilio_plugin_api::Words;

pub(crate) fn name() -> Words {
    Words::new("Discord 游戏状态", "Discord game status")
}

pub(crate) fn description() -> Words {
    Words::new(
        "在 Discord 中显示正在玩的游戏，退出后清除状态",
        "Show the game you are playing in Discord, and clear it when you exit",
    )
}

pub(crate) fn application_label() -> Words {
    Words::new("Discord 应用 ID", "Discord application ID")
}

pub(crate) fn application_help() -> Words {
    Words::new(
        "默认使用 LumilioCL 官方 Discord 应用 ID。留空使用内置 ID，也可填写其他 ID 覆盖。",
        "Uses the official LumilioCL Discord application ID by default. Leave empty for the built-in ID, or enter another ID to override it.",
    )
}

pub(crate) fn show_game_label() -> Words {
    Words::new("显示游戏名", "Show game name")
}

pub(crate) fn show_game_help() -> Words {
    Words::new(
        "在状态中显示实例名称、Minecraft 版本和加载器。",
        "Show the instance name, Minecraft version and loader in the status.",
    )
}

pub(crate) fn show_target_label() -> Words {
    Words::new("显示世界或服务器", "Show world or server")
}

pub(crate) fn show_target_help() -> Words {
    Words::new(
        "直接启动世界或服务器时，显示世界名或服务器地址。",
        "When launching a world or server directly, show the world name or server address.",
    )
}

/// The activity line for a directly launched world, in `locale`.
pub(crate) fn world(name: &str, locale: &str) -> String {
    if locale.starts_with("en") {
        format!("World: {name}")
    } else {
        format!("世界：{name}")
    }
}

/// The activity line for a directly joined server, in `locale`.
pub(crate) fn server(address: &str, locale: &str) -> String {
    if locale.starts_with("en") {
        format!("Server: {address}")
    } else {
        format!("服务器：{address}")
    }
}
