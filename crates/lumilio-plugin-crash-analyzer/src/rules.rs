use std::sync::OnceLock;

use lumilio_plugin_api::{Finding, Severity};
use regex::Regex;

struct Rule {
    id: &'static str,
    pattern: &'static str,
    severity: Severity,
}

const fn error(id: &'static str, pattern: &'static str) -> Rule {
    Rule {
        id,
        pattern,
        severity: Severity::Error,
    }
}

// The first five patterns and their order are unchanged from the launcher's
// former diagnostics::analyze. A repeated match still yields one finding.
const LEGACY: [Rule; 5] = [
    error(
        "out-of-memory",
        r"java\.lang\.OutOfMemoryError|There is insufficient memory",
    ),
    error(
        "java-too-old",
        r"UnsupportedClassVersionError|class file version \d+\.\d+|requires running the game with Java \d+",
    ),
    error(
        "bad-jvm-option",
        r"Unrecognized (VM )?option|Could not create the Java Virtual Machine",
    ),
    error(
        "mod-conflict",
        r"Mixin apply failed|Incompatible mod set|Mod resolution failed|Duplicate mod",
    ),
    error(
        "graphics-failure",
        r"GLFW error|OpenGL error|Pixel format not accelerated",
    ),
];

// Adapted rule signatures from
// 3rd-party/HMCL/HMCLCore/src/main/java/org/jackhuang/hmcl/game/CrashReportAnalyzer.java
// Copyright (C) 2021 huangyuhui <huanghongxun2008@126.com> and contributors.
// GPL-3.0-or-later; ADR 0011. The words and Rust execution are our own.
// Common mod/dependency/configuration failures precede runtime/platform cases.
const ADDITIONAL: [Rule; 16] = [
    error(
        "duplicate-mods",
        r"Found a duplicate mod |Found duplicate mods:",
    ),
    error(
        "missing-dependency",
        r"Missing or unsupported mandatory dependencies:|ModResolutionException: Could not find required mod:",
    ),
    error(
        "mod-entrypoint",
        r"LoaderExceptionModCrash: Caught exception from |Failed to create mod instance\. ModID:|Could not execute entrypoint stage '[^']+' due to errors, provided by '[^']+'!",
    ),
    error(
        "config-parse",
        r"Failed loading config file .+ of type .+ for modid |com\.electronwill\.nightconfig\.core\.io\.ParsingException: Not enough data available",
    ),
    error("missing-class", r"java\.lang\.NoClassDefFoundError:"),
    error("missing-method", r"java\.lang\.NoSuchMethodError:"),
    error(
        "mixin-failure",
        r"MixinApplyError|Mixin prepare failed |mixin\.injection\.throwables\.|Mixin apply for mod .+ failed",
    ),
    error(
        "native-library",
        r"java\.lang\.UnsatisfiedLinkError: Failed to locate library:",
    ),
    error(
        "changed-signature",
        r"java\.lang\.SecurityException: SHA1 digest error for |signer information does not match signer information of other classes in the same package",
    ),
    error(
        "heap-size",
        r"Could not reserve enough space for .+KB object heap|The specified size exceeds the maximum representable size|Invalid maximum heap size",
    ),
    error(
        "opengl-unsupported",
        r"The driver does not appear to support OpenGL",
    ),
    error(
        "java-too-new",
        r"Unable to make protected final java\.lang\.Class java\.lang\.ClassLoader\.defineClass|java\.lang\.NoSuchFieldException: ucp|Unsupported class file major version|because module java\.base does not export|java\.lang\.ClassNotFoundException: jdk\.nashorn\.api\.scripting\.NashornScriptEngineFactory",
    ),
    error(
        "openj9",
        r"Open J9 is not supported|OpenJ9 is incompatible|\.J9VMInternals\.",
    ),
    error(
        "decompressed-mods",
        r"The directories below appear to be extracted jar files\. Fix this before you continue|Extracted mod jars found, loading will NOT continue",
    ),
    error(
        "invalid-mod-name",
        r"Invalid module name: '' is not a Java identifier",
    ),
    Rule {
        id: "manual-debug-crash",
        pattern: r"Manually triggered debug crash",
        severity: Severity::Info,
    },
];

/// Every rule id, so a test can check each one has words in `text`.
#[cfg(test)]
pub(super) fn ids() -> impl Iterator<Item = &'static str> {
    LEGACY.iter().chain(ADDITIONAL.iter()).map(|rule| rule.id)
}

pub(super) fn analyze(text: &str, locale: &str) -> Vec<Finding> {
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
                title: super::text::title(rule.id, locale).into(),
                advice: super::text::advice(rule.id, locale).into(),
                evidence: Some(text[start..end].chars().take(512).collect()),
            })
        })
        .collect()
}
