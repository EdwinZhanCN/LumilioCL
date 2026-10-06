use super::library::arranged;
use super::library::loader_code;
use super::library::present_loaders;
use crate::live::LibraryCard;

use crate::live::library_card;
use lumilio_core::{InstanceRecord, InstanceSettings};

fn card(id: &str, name: &str, loader: lumilio_core::Loader, created: u64) -> LibraryCard {
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
    use lumilio_core::Loader::{Fabric, Forge, NeoForge, Quilt, Vanilla};
    let codes = [Vanilla, Fabric, Forge, NeoForge, Quilt].map(loader_code);
    assert_eq!(codes, [1, 2, 3, 4, 5]);
}

#[test]
fn the_library_is_ordered_and_filtered_by_the_choices() {
    use lumilio_core::Loader::{Fabric, Forge, Vanilla};
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
fn the_record_reads_hours_minutes_and_counts() {
    use super::record_readings;
    use crate::live::HomePlaces;
    let places = |play_seconds, worlds, servers| HomePlaces {
        instance: "a".to_owned(),
        places: Vec::new(),
        play_seconds,
        worlds,
        servers,
    };
    let fresh: Vec<_> = record_readings(&places(0, 0, 0))
        .iter()
        .map(|reading| (reading.value, reading.unit))
        .collect();
    assert_eq!(
        fresh,
        [(0, Some("MIN")), (0, None), (0, None)],
        "a new game reads zeros instead of hiding the record"
    );
    let long = record_readings(&places(128 * 3600 + 59, 3, 1));
    let shown: Vec<_> = long
        .iter()
        .map(|reading| (reading.label.as_ref(), reading.value, reading.unit))
        .collect();
    assert_eq!(
        shown,
        [
            ("游玩时间", 128, Some("H")),
            ("世界", 3, None),
            ("服务器", 1, None)
        ]
    );
    let short = record_readings(&places(25 * 60, 0, 0));
    assert_eq!((short[0].value, short[0].unit), (25, Some("MIN")));
}
