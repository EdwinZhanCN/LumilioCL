use super::*;
use lumilio_plugin_api::{AnalysisSource, FetchResponse, GameFacts, SettingValue};

struct NoIo;
impl HostContext for NoIo {
    fn setting(&self, _: &str) -> Option<SettingValue> {
        None
    }
    fn read_file(&self, _: &str) -> Result<Vec<u8>, PluginError> {
        panic!("analysis needs no I/O")
    }
    fn fetch(&self, _: &str) -> Result<FetchResponse, PluginError> {
        panic!("analysis needs no network")
    }
}

fn analyze(text: &str) -> Vec<Finding> {
    CrashAnalyzer
        .analyze(
            &NoIo,
            &AnalysisInput {
                text: text.into(),
                source: AnalysisSource::CrashReport,
                game: GameFacts::default(),
            },
        )
        .unwrap()
}

fn ids(text: &str) -> Vec<String> {
    analyze(text)
        .into_iter()
        .map(|finding| finding.rule)
        .collect()
}

// Migrated from lumilio-core/src/diagnostics/tests.rs, with stable rule ids
// replacing the removed CrashHint enum. The log and ordered behavior remain.
#[test]
fn known_failures_are_recognized_once_each_in_order() {
    let text = "Exception in thread main java.lang.OutOfMemoryError: Java heap space\n\
                java.lang.UnsupportedClassVersionError: bad\n\
                Mixin apply failed x\nMixin apply failed y\n";
    assert_eq!(ids(text), ["out-of-memory", "java-too-old", "mod-conflict"]);
    assert!(analyze("all good").is_empty());
    assert_eq!(ids("Unrecognized option: -XX:Foo"), ["bad-jvm-option"]);
}

macro_rules! sample {
    ($name:ident, $id:literal, $positive:literal, $negative:literal) => {
        #[test]
        fn $name() {
            let findings = analyze($positive);
            let finding = findings
                .iter()
                .find(|finding| finding.rule == $id)
                .unwrap_or_else(|| panic!("missing {} in {:?}", $id, findings));
            assert!(!finding.title.is_empty() && !finding.advice.is_empty());
            let evidence = finding.evidence.as_deref().unwrap();
            assert!(!evidence.is_empty() && $positive.contains(evidence));
            assert!(!ids($negative).iter().any(|id| id == $id));
            assert_eq!(
                ids(concat!($positive, "\n", $positive))
                    .iter()
                    .filter(|id| *id == $id)
                    .count(),
                1
            );
        }
    };
}

sample!(
    heap_legacy,
    "out-of-memory",
    "java.lang.OutOfMemoryError: Java heap space",
    "Java heap space available: 2048 MB"
);
sample!(
    java_legacy,
    "java-too-old",
    "requires running the game with Java 21",
    "running the game with Java 21"
);
sample!(
    jvm_legacy,
    "bad-jvm-option",
    "Unrecognized VM option 'BadFlag'",
    "Recognized VM option 'UseG1GC'"
);
sample!(
    mods_legacy,
    "mod-conflict",
    "Incompatible mod set!",
    "Compatible mod set loaded"
);
sample!(
    graphics_legacy,
    "graphics-failure",
    "Pixel format not accelerated",
    "Pixel format accelerated"
);
sample!(
    duplicate_mods,
    "duplicate-mods",
    "Found duplicate mods:\n\tsodium: a.jar, b.jar",
    "Found mods:\n\tsodium: a.jar"
);
sample!(
    missing_dependency,
    "missing-dependency",
    "Missing or unsupported mandatory dependencies:\n\tfabric-api required by example",
    "All mandatory dependencies were found"
);
sample!(
    mod_entrypoint,
    "mod-entrypoint",
    "Could not execute entrypoint stage 'main' due to errors, provided by 'example'!",
    "Executing entrypoint stage 'main', provided by 'example'"
);
sample!(
    config_parse,
    "config-parse",
    "Failed loading config file example.toml of type COMMON for modid example",
    "Loaded config file example.toml of type COMMON for modid example"
);
sample!(
    missing_class,
    "missing-class",
    "java.lang.NoClassDefFoundError: example/Dependency",
    "Loaded class example/Dependency"
);
sample!(
    missing_method,
    "missing-method",
    "java.lang.NoSuchMethodError: 'void example.Class.oldMethod()'",
    "Called example.Class.oldMethod()"
);
sample!(
    mixin_failure,
    "mixin-failure",
    "org.spongepowered.asm.mixin.throwables.MixinApplyError: example.mixins.json",
    "Applied mixin example.mixins.json"
);
sample!(
    native_library,
    "native-library",
    "java.lang.UnsatisfiedLinkError: Failed to locate library: lwjgl",
    "Located library: lwjgl"
);
sample!(
    changed_signature,
    "changed-signature",
    "java.lang.SecurityException: SHA1 digest error for net/minecraft/Main.class",
    "SHA1 digest verified for net/minecraft/Main.class"
);
sample!(
    heap_size,
    "heap-size",
    "Could not reserve enough space for 2097152KB object heap",
    "Reserved 2097152KB object heap"
);
sample!(
    opengl_unsupported,
    "opengl-unsupported",
    "The driver does not appear to support OpenGL",
    "The driver supports OpenGL 4.6"
);
sample!(
    java_too_new,
    "java-too-new",
    "java.lang.NoSuchFieldException: ucp",
    "Found field: ucp"
);
sample!(
    openj9,
    "openj9",
    "OpenJ9 is incompatible with this loader",
    "Java vendor: Eclipse Adoptium HotSpot"
);
sample!(
    decompressed_mods,
    "decompressed-mods",
    "Extracted mod jars found, loading will NOT continue",
    "Found mod jars, loading will continue"
);
sample!(
    invalid_mod_name,
    "invalid-mod-name",
    "Invalid module name: '' is not a Java identifier",
    "Module name: example.mod is a Java identifier"
);
sample!(
    manual_debug_crash,
    "manual-debug-crash",
    "Description: Manually triggered debug crash",
    "Description: Ticking entity"
);

#[test]
fn a_debug_crash_is_information_and_evidence_is_bounded() {
    assert_eq!(
        analyze("Manually triggered debug crash")[0].severity,
        lumilio_plugin_api::Severity::Info
    );
    let findings = analyze(&format!(
        "java.lang.OutOfMemoryError: {}",
        "界".repeat(10000)
    ));
    assert_eq!(findings[0].evidence.as_ref().unwrap().chars().count(), 512);
}

#[test]
fn the_manifest_requires_no_permissions_and_enables_only_the_analyzer() {
    let plugin = CrashAnalyzer;
    let manifest = plugin.manifest();
    assert_eq!(manifest.id, ID);
    assert!(manifest.default_enabled && manifest.permissions.is_empty());
    assert!(plugin.analyzer().is_some());
    assert!(
        plugin.instance_tab().is_none()
            && plugin.content_source().is_none()
            && plugin.launch_observer().is_none()
    );
}
