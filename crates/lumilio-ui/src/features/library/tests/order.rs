use lumilio_core::{InstanceRecord, InstanceSettings, Loader};

use super::super::LibraryInputs;
use super::super::order::{arranged, loader_code, present_loaders};
use crate::live::{CollectionRow, LibraryCard, library_card};

fn card(id: &str, name: &str, loader: Loader, created: u64) -> LibraryCard {
    library_card(
        &InstanceRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            game_version: "1.21.1".to_owned(),
            loader,
            loader_version: None,
            favorite: false,
            created_at: created,
            last_played: None,
            play_seconds: 0,
            installed: false,
            settings: InstanceSettings::default(),
            source_project: None,
        },
        10,
    )
}

#[test]
fn loaders_are_remembered_by_a_steady_number() {
    use Loader::{Fabric, Forge, NeoForge, Quilt, Vanilla};
    let codes = [Vanilla, Fabric, Forge, NeoForge, Quilt].map(loader_code);
    assert_eq!(codes, [1, 2, 3, 4, 5]);
}

#[test]
fn the_library_is_ordered_and_filtered_by_the_choices() {
    use Loader::{Fabric, Forge, Vanilla};
    let cards = vec![
        card("a", "Zebra", Fabric, 1),
        card("b", "apple", Forge, 3),
        card("c", "Mango", Fabric, 2),
    ];
    let names = |shown: Vec<&LibraryCard>| -> Vec<String> {
        shown.iter().map(|card| card.name.clone()).collect()
    };
    let all = || cards.iter().collect::<Vec<_>>();
    assert_eq!(names(arranged(all(), 0, None)), ["Zebra", "apple", "Mango"]);
    assert_eq!(names(arranged(all(), 1, None)), ["apple", "Mango", "Zebra"]);
    assert_eq!(names(arranged(all(), 2, None)), ["apple", "Mango", "Zebra"]);
    assert_eq!(names(arranged(all(), 1, Some(Fabric))), ["Mango", "Zebra"]);
    assert!(arranged(all(), 0, Some(Vanilla)).is_empty());
    // Only the loaders the library has, in a steady order.
    assert_eq!(present_loaders(&cards), [Fabric, Forge]);
    assert!(present_loaders(&[]).is_empty());
}

#[test]
fn a_collection_lists_its_games_that_still_exist() {
    let inputs = LibraryInputs {
        library: ["a", "b", "c"]
            .map(|id| card(id, id, Loader::Vanilla, 1))
            .to_vec(),
        collections: vec![CollectionRow {
            name: "生存".into(),
            members: vec!["c".into(), "gone".into(), "a".into()],
        }],
        ..LibraryInputs::default()
    };
    let ids: Vec<_> = inputs
        .collection_cards(&inputs.collections[0])
        .iter()
        .map(|card| card.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["c", "a"],
        "kept in the order added; deleted games are skipped"
    );
}
