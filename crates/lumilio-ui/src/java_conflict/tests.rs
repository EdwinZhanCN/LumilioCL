use super::*;
use lumilio_core::{JavaChoice, JavaRule, JavaRuntime};

fn runtime(root: &std::path::Path, name: &str, version: &str) -> JavaRuntime {
    let home = root.join(name);
    std::fs::create_dir_all(home.join("bin")).unwrap();
    std::fs::write(home.join("bin/java"), "").unwrap();
    std::fs::write(
        home.join("release"),
        format!("JAVA_VERSION=\"{version}\"\n"),
    )
    .unwrap();
    JavaRuntime::inspect(&home).unwrap()
}

fn picked(runtime: &JavaRuntime, broken: Vec<JavaRule>, chosen_by_user: bool) -> JavaChoice {
    JavaChoice {
        runtime: runtime.clone(),
        broken,
        chosen_by_user,
    }
}

#[test]
fn only_a_chosen_java_that_cannot_run_the_game_is_asked_about() {
    let dir = tempfile::tempdir().unwrap();
    let (java17, java8) = (
        runtime(dir.path(), "j17", "17.0.9"),
        runtime(dir.path(), "j8", "1.8.0_412"),
    );
    let mut java = InstanceJava {
        runtimes: vec![java17.clone(), java8.clone()],
        choice: Some(picked(&java17, vec![JavaRule::LaunchWrapper], true)),
        automatic: Some(picked(&java8, Vec::new(), false)),
        needed: Some(8),
        build: lumilio_core::JavaBuild::Any,
        auto_install: true,
    };
    assert_eq!(
        JavaConflict::of(&java),
        Some(JavaConflict {
            chosen: 17,
            needed: 8,
            switch: Some(JavaSwitch::Installed(8)),
        })
    );

    // Nothing installed fits: the switch downloads, or is not offered.
    java.automatic = None;
    assert_eq!(
        JavaConflict::of(&java).unwrap().switch,
        Some(JavaSwitch::Download(8))
    );
    java.auto_install = false;
    assert_eq!(JavaConflict::of(&java).unwrap().switch, None);

    // A suggestion, or the launcher's own pick, is never asked about.
    java.choice = Some(picked(&java17, vec![JavaRule::ModLoader(8)], true));
    assert_eq!(JavaConflict::of(&java), None);
    java.choice = Some(picked(&java17, vec![JavaRule::LaunchWrapper], false));
    assert_eq!(JavaConflict::of(&java), None);
}
