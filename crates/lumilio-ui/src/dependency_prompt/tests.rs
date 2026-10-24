use lumilio_core::{ReleaseChannel, Version};

use super::*;

fn version() -> Version {
    Version {
        id: "v".into(),
        project_id: "p".into(),
        name: "n".into(),
        number: "1.2".into(),
        channel: ReleaseChannel::Release,
        game_versions: Vec::new(),
        loaders: Vec::new(),
        published: String::new(),
        files: Vec::new(),
        dependencies: Vec::new(),
        downloads: 0,
        changelog: String::new(),
        changelog_format: lumilio_core::TextFormat::Markdown,
    }
}

fn need(id: &str, fits: bool) -> DependencyNeed {
    DependencyNeed {
        project_id: id.into(),
        title: id.to_uppercase(),
        version: fits.then(version),
    }
}

#[test]
fn everything_installable_starts_ticked_and_an_unfit_one_can_never_be_chosen() {
    let prompt = DependencyPrompt::new(
        "Cool".into(),
        DependencyReport {
            needs: vec![need("a", true), need("b", false), need("c", true)],
            optional: vec![need("o", true)],
            conflicts: vec!["X".into()],
        },
        Rc::new(|_, _, _| {}),
    );
    assert_eq!(
        chosen(&prompt.rows),
        ["a", "c"],
        "suggestions start unticked"
    );
    let mut rows = prompt.rows;
    rows[0].1 = false;
    rows[1].1 = true;
    rows[3].1 = true;
    assert_eq!(
        chosen(&rows),
        ["c", "o"],
        "unticked and unfit ones stay out"
    );
    assert!(DependencyPrompt::worth_showing(&DependencyReport {
        conflicts: vec!["X".into()],
        ..DependencyReport::default()
    }));
    assert!(!DependencyPrompt::worth_showing(
        &DependencyReport::default()
    ));
}
