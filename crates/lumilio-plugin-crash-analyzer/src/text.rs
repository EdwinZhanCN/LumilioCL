//! The analyzer's own words, Chinese and English side by side. Rule ids stay
//! stable; only what a person reads changes with the launcher's language.

use lumilio_plugin_api::Words;

pub(super) fn name() -> Words {
    Words::new("崩溃与日志分析", "Crash and log analysis")
}

pub(super) fn description() -> Words {
    Words::new(
        "从游戏日志中找出可能的原因，提供排查建议。",
        "Finds likely causes in game logs and suggests what to check.",
    )
}

/// Picks the pair's English string for an English tag, the Chinese otherwise.
fn pick((zh_cn, en): (&'static str, &'static str), locale: &str) -> &'static str {
    if locale.starts_with("en") && !en.is_empty() {
        en
    } else {
        zh_cn
    }
}

struct Entry {
    title: (&'static str, &'static str),
    advice: (&'static str, &'static str),
}

pub(super) fn title(rule: &str, locale: &str) -> &'static str {
    pick(entry(rule).title, locale)
}

pub(super) fn advice(rule: &str, locale: &str) -> &'static str {
    pick(entry(rule).advice, locale)
}

fn entry(rule: &str) -> Entry {
    match rule {
        "out-of-memory" => Entry {
            title: ("内存不足", "Not enough memory"),
            advice: (
                "试着调高最大内存，或停用一些 Mod",
                "Try raising the maximum memory, or disable some mods",
            ),
        },
        "java-too-old" => Entry {
            title: ("Java 版本太旧", "Java is too old"),
            advice: ("这个游戏需要更新的 Java", "This game needs a newer Java"),
        },
        "bad-jvm-option" => Entry {
            title: ("有无法识别的 Java 参数", "Unrecognized Java option"),
            advice: (
                "检查「设置」里的附加参数",
                "Check the extra arguments in Settings",
            ),
        },
        "mod-conflict" => Entry {
            title: (
                "Mod 之间有冲突或缺少依赖",
                "Mods conflict or a dependency is missing",
            ),
            advice: ("可以逐个停用后再试", "Try disabling them one at a time"),
        },
        "graphics-failure" => Entry {
            title: ("显卡或 OpenGL 出错", "Graphics or OpenGL failed"),
            advice: (
                "更新显卡驱动或关闭光影",
                "Update the graphics driver, or turn off shaders",
            ),
        },
        "duplicate-mods" => Entry {
            title: ("加载器发现重复的 Mod", "The loader found duplicate mods"),
            advice: (
                "在「内容」里保留每个 Mod 的一份文件，再重新启动",
                "Keep one file per mod in Content, then start the game again",
            ),
        },
        "missing-dependency" => Entry {
            title: (
                "Mod 缺少依赖或依赖版本不匹配",
                "A mod is missing a dependency or the versions do not match",
            ),
            advice: (
                "按日志中的依赖名称安装对应版本，同时确认游戏和加载器版本",
                "Install the version named in the log, and check the game and loader versions",
            ),
        },
        "mod-entrypoint" => Entry {
            title: ("有 Mod 在加载时出错", "A mod failed while loading"),
            advice: (
                "先更新或停用日志指出的 Mod；若仍失败，检查它的依赖版本",
                "Update or disable the mod the log names; if it still fails, check its dependency versions",
            ),
        },
        "config-parse" => Entry {
            title: ("Mod 配置文件读不了", "A mod config file cannot be read"),
            advice: (
                "先备份日志指出的配置文件，再移开它，让 Mod 重新生成；不要删除世界文件",
                "Back up the config file the log names, then move it aside so the mod can recreate it; do not delete world files",
            ),
        },
        "missing-class" => Entry {
            title: ("运行时缺少一个类", "A class is missing at runtime"),
            advice: (
                "检查 Mod 和依赖是否完整、版本是否匹配；游戏文件缺失时可尝试修复",
                "Check that the mods and dependencies are complete and match versions; repair the game files if they are missing",
            ),
        },
        "missing-method" => Entry {
            title: (
                "Mod 调用了不存在的方法",
                "A mod called a method that does not exist",
            ),
            advice: (
                "核对 Mod、依赖和加载器版本，更新或停用最近添加的 Mod 后再试",
                "Check the mod, dependency and loader versions; update or disable recently added mods and try again",
            ),
        },
        "mixin-failure" => Entry {
            title: ("Mod 的 Mixin 注入失败", "A mod's Mixin injection failed"),
            advice: (
                "检查日志指出的 Mod 是否适用于当前游戏版本，更新或停用冲突的 Mod",
                "Check whether the mod the log names fits this game version; update or disable the conflicting mod",
            ),
        },
        "native-library" => Entry {
            title: ("没有找到本机库", "A native library was not found"),
            advice: (
                "尝试修复游戏文件，并确认 Java 的架构与系统一致",
                "Try repairing the game files, and make sure Java's architecture matches the system",
            ),
        },
        "changed-signature" => Entry {
            title: ("文件签名校验失败", "A file's signature check failed"),
            advice: (
                "恢复原始游戏或 Mod 文件，再尝试修复；不要直接修改 jar 文件",
                "Restore the original game or mod files and repair; do not edit jar files directly",
            ),
        },
        "heap-size" => Entry {
            title: (
                "Java 无法分配指定的内存",
                "Java cannot allocate the requested memory",
            ),
            advice: (
                "调低最大内存，并确认使用 64 位 Java；这个提示也可能来自系统可用内存不足",
                "Lower the maximum memory and use 64-bit Java; this can also mean the system has too little free memory",
            ),
        },
        "opengl-unsupported" => Entry {
            title: (
                "显卡驱动没有提供所需的 OpenGL",
                "The graphics driver does not offer the required OpenGL",
            ),
            advice: (
                "安装显卡厂商的驱动，并确认游戏使用了支持 OpenGL 的显卡",
                "Install the driver from the graphics vendor, and make sure the game uses a GPU that supports OpenGL",
            ),
        },
        "java-too-new" => Entry {
            title: (
                "加载器或 Mod 可能不支持当前 Java",
                "The loader or a mod may not support this Java",
            ),
            advice: (
                "改用这个游戏和加载器推荐的 Java 版本，或更新相关 Mod",
                "Use the Java version the game and loader recommend, or update the mods",
            ),
        },
        "openj9" => Entry {
            title: ("当前运行环境不支持 OpenJ9", "OpenJ9 is not supported here"),
            advice: (
                "在「设置」里改用 HotSpot Java，或安装启动器推荐的 Java",
                "Switch to HotSpot Java in Settings, or install the Java the launcher recommends",
            ),
        },
        "decompressed-mods" => Entry {
            title: (
                "有 Mod 被解压成了文件夹",
                "A mod was extracted into a folder",
            ),
            advice: (
                "移开 mods 文件夹中解压后的 Mod，重新放入原始 jar 文件",
                "Move the extracted mod out of the mods folder and put the original jar file back",
            ),
        },
        "invalid-mod-name" => Entry {
            title: (
                "Mod 文件名无法生成有效的模块名称",
                "The mod file name cannot form a valid module name",
            ),
            advice: (
                "保留 jar 后缀，将日志指出的 Mod 文件改成含英文字母的名称后重试",
                "Keep the jar extension and rename the mod file the log names to include English letters, then try again",
            ),
        },
        "manual-debug-crash" => Entry {
            title: (
                "这是手动触发的调试崩溃",
                "This crash was triggered manually for debugging",
            ),
            advice: (
                "日志显示游戏通过调试快捷键主动崩溃；如果是误按，重新启动即可",
                "The log shows the game crashed through the debug key; if it was pressed by mistake, just start it again",
            ),
        },
        _ => Entry {
            title: ("", ""),
            advice: ("", ""),
        },
    }
}
