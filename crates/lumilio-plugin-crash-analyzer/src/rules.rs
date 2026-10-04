use std::sync::OnceLock;

use lumilio_plugin_api::{Finding, Severity};
use regex::Regex;

struct Rule {
    id: &'static str,
    pattern: &'static str,
    title: &'static str,
    advice: &'static str,
    severity: Severity,
}

const fn error(
    id: &'static str,
    pattern: &'static str,
    title: &'static str,
    advice: &'static str,
) -> Rule {
    Rule {
        id,
        pattern,
        title,
        advice,
        severity: Severity::Error,
    }
}

// The first five patterns and their order are unchanged from the launcher's
// former diagnostics::analyze. A repeated match still yields one finding.
const LEGACY: [Rule; 5] = [
    error(
        "out-of-memory",
        r"java\.lang\.OutOfMemoryError|There is insufficient memory",
        "内存不足",
        "试着调高最大内存，或停用一些 Mod",
    ),
    error(
        "java-too-old",
        r"UnsupportedClassVersionError|class file version \d+\.\d+|requires running the game with Java \d+",
        "Java 版本太旧",
        "这个游戏需要更新的 Java",
    ),
    error(
        "bad-jvm-option",
        r"Unrecognized (VM )?option|Could not create the Java Virtual Machine",
        "有无法识别的 Java 参数",
        "检查「设置」里的附加参数",
    ),
    error(
        "mod-conflict",
        r"Mixin apply failed|Incompatible mod set|Mod resolution failed|Duplicate mod",
        "Mod 之间有冲突或缺少依赖",
        "可以逐个停用后再试",
    ),
    error(
        "graphics-failure",
        r"GLFW error|OpenGL error|Pixel format not accelerated",
        "显卡或 OpenGL 出错",
        "更新显卡驱动或关闭光影",
    ),
];

// Adapted rule signatures from
// 3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/CrashReportAnalyzer.java
// Copyright (C) 2021 huangyuhui <huanghongxun2008@126.com> and contributors.
// GPL-3.0-or-later; ADR 0011. Chinese advice and Rust execution are our own.
// Common mod/dependency/configuration failures precede runtime/platform cases.
const ADDITIONAL: [Rule; 16] = [
    error(
        "duplicate-mods",
        r"Found a duplicate mod |Found duplicate mods:",
        "加载器发现重复的 Mod",
        "在「内容」里保留每个 Mod 的一份文件，再重新启动",
    ),
    error(
        "missing-dependency",
        r"Missing or unsupported mandatory dependencies:|ModResolutionException: Could not find required mod:",
        "Mod 缺少依赖或依赖版本不匹配",
        "按日志中的依赖名称安装对应版本，同时确认游戏和加载器版本",
    ),
    error(
        "mod-entrypoint",
        r"LoaderExceptionModCrash: Caught exception from |Failed to create mod instance\. ModID:|Could not execute entrypoint stage '[^']+' due to errors, provided by '[^']+'!",
        "有 Mod 在加载时出错",
        "先更新或停用日志指出的 Mod；若仍失败，检查它的依赖版本",
    ),
    error(
        "config-parse",
        r"Failed loading config file .+ of type .+ for modid |com\.electronwill\.nightconfig\.core\.io\.ParsingException: Not enough data available",
        "Mod 配置文件读不了",
        "先备份日志指出的配置文件，再移开它，让 Mod 重新生成；不要删除世界文件",
    ),
    error(
        "missing-class",
        r"java\.lang\.NoClassDefFoundError:",
        "运行时缺少一个类",
        "检查 Mod 和依赖是否完整、版本是否匹配；游戏文件缺失时可尝试修复",
    ),
    error(
        "missing-method",
        r"java\.lang\.NoSuchMethodError:",
        "Mod 调用了不存在的方法",
        "核对 Mod、依赖和加载器版本，更新或停用最近添加的 Mod 后再试",
    ),
    error(
        "mixin-failure",
        r"MixinApplyError|Mixin prepare failed |mixin\.injection\.throwables\.|Mixin apply for mod .+ failed",
        "Mod 的 Mixin 注入失败",
        "检查日志指出的 Mod 是否适用于当前游戏版本，更新或停用冲突的 Mod",
    ),
    error(
        "native-library",
        r"java\.lang\.UnsatisfiedLinkError: Failed to locate library:",
        "没有找到本机库",
        "尝试修复游戏文件，并确认 Java 的架构与系统一致",
    ),
    error(
        "changed-signature",
        r"java\.lang\.SecurityException: SHA1 digest error for |signer information does not match signer information of other classes in the same package",
        "文件签名校验失败",
        "恢复原始游戏或 Mod 文件，再尝试修复；不要直接修改 jar 文件",
    ),
    error(
        "heap-size",
        r"Could not reserve enough space for .+KB object heap|The specified size exceeds the maximum representable size|Invalid maximum heap size",
        "Java 无法分配指定的内存",
        "调低最大内存，并确认使用 64 位 Java；这个提示也可能来自系统可用内存不足",
    ),
    error(
        "opengl-unsupported",
        r"The driver does not appear to support OpenGL",
        "显卡驱动没有提供所需的 OpenGL",
        "安装显卡厂商的驱动，并确认游戏使用了支持 OpenGL 的显卡",
    ),
    error(
        "java-too-new",
        r"Unable to make protected final java\.lang\.Class java\.lang\.ClassLoader\.defineClass|java\.lang\.NoSuchFieldException: ucp|Unsupported class file major version|because module java\.base does not export|java\.lang\.ClassNotFoundException: jdk\.nashorn\.api\.scripting\.NashornScriptEngineFactory",
        "加载器或 Mod 可能不支持当前 Java",
        "改用这个游戏和加载器推荐的 Java 版本，或更新相关 Mod",
    ),
    error(
        "openj9",
        r"Open J9 is not supported|OpenJ9 is incompatible|\.J9VMInternals\.",
        "当前运行环境不支持 OpenJ9",
        "在「设置」里改用 HotSpot Java，或安装启动器推荐的 Java",
    ),
    error(
        "decompressed-mods",
        r"The directories below appear to be extracted jar files\. Fix this before you continue|Extracted mod jars found, loading will NOT continue",
        "有 Mod 被解压成了文件夹",
        "移开 mods 文件夹中解压后的 Mod，重新放入原始 jar 文件",
    ),
    error(
        "invalid-mod-name",
        r"Invalid module name: '' is not a Java identifier",
        "Mod 文件名无法生成有效的模块名称",
        "保留 jar 后缀，将日志指出的 Mod 文件改成含英文字母的名称后重试",
    ),
    Rule {
        id: "manual-debug-crash",
        pattern: r"Manually triggered debug crash",
        title: "这是手动触发的调试崩溃",
        advice: "日志显示游戏通过调试快捷键主动崩溃；如果是误按，重新启动即可",
        severity: Severity::Info,
    },
];

pub(super) fn analyze(text: &str) -> Vec<Finding> {
    static COMPILED: OnceLock<Vec<(&'static Rule, Regex)>> = OnceLock::new();
    let rules = COMPILED.get_or_init(|| {
        LEGACY
            .iter()
            .chain(ADDITIONAL.iter())
            .map(|rule| {
                (
                    rule,
                    Regex::new(rule.pattern).expect("tested built-in rule"),
                )
            })
            .collect()
    });
    rules
        .iter()
        .filter_map(|(rule, regex)| {
            let matched = regex.find(text)?;
            // Keep evidence small and literal; never treat log paths as commands.
            let start = text[..matched.start()].rfind('\n').map_or(0, |at| at + 1);
            let end = text[matched.end()..]
                .find('\n')
                .map_or(text.len(), |at| matched.end() + at);
            Some(Finding {
                rule: rule.id.into(),
                severity: rule.severity,
                title: rule.title.into(),
                advice: rule.advice.into(),
                evidence: Some(text[start..end].chars().take(512).collect()),
            })
        })
        .collect()
}
