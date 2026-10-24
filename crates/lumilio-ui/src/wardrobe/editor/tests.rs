use super::*;

#[test]
fn draft_changes_do_not_change_its_original_values() {
    let mut draft = Editor::new(Target::Current, SkinModel::Wide, PairedCape::Hidden);
    assert!(!draft.changed());
    draft.edited.model = SkinModel::Slim;
    draft.edited.cape = PairedCape::Cape("owned".into());
    draft.edited.replacement = Some(PathBuf::from("new.png"));
    assert!(draft.changed());
    assert_eq!(draft.original.model, SkinModel::Wide);
    assert_eq!(draft.original.cape, PairedCape::Hidden);
    assert!(draft.original.replacement.is_none());
}
