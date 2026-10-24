use lumilio_core::{InstanceRecord, InstanceSettings, Loader, ProjectKind};

use super::DiscoverState;
use crate::features::discover::model::{DiscoverQuery, Provided, SearchStatus};
use crate::live::library_card;

fn record(id: &str, loader: Loader, source: Option<&str>) -> InstanceRecord {
    InstanceRecord {
        id: id.to_owned(),
        name: id.to_owned(),
        game_version: "1.21.1".to_owned(),
        loader,
        loader_version: None,
        favorite: false,
        created_at: 1,
        last_played: None,
        play_seconds: 0,
        installed: true,
        settings: InstanceSettings::default(),
        source_project: source.map(str::to_owned),
    }
}

fn model_with(games: &[(&str, Loader, Option<&str>)]) -> DiscoverState {
    let mut model = DiscoverState::default();
    let cards = games
        .iter()
        .map(|(id, loader, source)| library_card(&record(id, *loader, *source), 100))
        .collect();
    model.library = cards;
    model
}

#[test]
fn browsing_for_a_game_provides_its_filters_and_trims_the_tabs() {
    let mut model = model_with(&[("a", Loader::Fabric, None), ("b", Loader::Vanilla, None)]);
    assert_eq!(
        model.provided(),
        Provided::default(),
        "plain browsing provides nothing"
    );
    assert_eq!(model.discover_kinds().len(), 4);
    model.browsing_for = Some("a".to_owned());
    model.query = DiscoverQuery::new(ProjectKind::Mod);
    assert_eq!(model.provided().loader.as_deref(), Some("fabric"));
    assert_eq!(model.discover_kinds().len(), 3);
    model.browsing_for = Some("b".to_owned());
    assert_eq!(
        model.discover_kinds().len(),
        2,
        "no mods for a vanilla game"
    );
    // A game that was deleted stops being the context.
    model.browsing_for = Some("gone".to_owned());
    assert_eq!(model.provided(), Provided::default());
}

#[test]
fn hide_installed_is_offered_for_packs_and_inside_a_game_and_reads_the_right_list() {
    let mut model = model_with(&[
        ("a", Loader::Fabric, Some("1KVo5zza")),
        ("b", Loader::Fabric, None),
    ]);
    model.query = DiscoverQuery::new(ProjectKind::Mod);
    assert!(!model.can_hide_installed(), "not for plain mod browsing");
    model.query = DiscoverQuery::new(ProjectKind::Modpack);
    assert!(model.can_hide_installed());
    assert!(!model.hiding_installed());
    model.discover_prefs.hide_installed_modpacks = true;
    assert!(
        model.hiding_installed(),
        "the pack tab's choice is remembered"
    );
    assert_eq!(
        model.installed_projects(),
        ["1KVo5zza"],
        "packs the library came from"
    );
    model.browsing_for = Some("a".to_owned());
    model.query = DiscoverQuery::new(ProjectKind::Mod);
    assert!(model.can_hide_installed());
    assert!(
        !model.hiding_installed(),
        "inside a game it is the session's own switch"
    );
    model.query.hide_installed = true;
    assert!(model.hiding_installed());
}

#[test]
fn packs_installed_from_another_source_are_not_this_ones() {
    let mut model = model_with(&[
        ("a", Loader::Fabric, Some("1KVo5zza")),
        ("b", Loader::Fabric, Some("lumilio.curseforge/285109")),
    ]);
    model.query = DiscoverQuery::new(ProjectKind::Modpack);
    assert_eq!(model.installed_projects(), ["1KVo5zza"]);
    model.query.source = "lumilio.curseforge".into();
    assert_eq!(model.installed_projects(), ["285109"]);
}

#[test]
fn pages_follow_the_total_and_the_page_size() {
    let mut model = DiscoverState::default();
    assert_eq!(model.pages(), 0);
    model.search = SearchStatus::Done { total: 41 };
    model.query.page_size = 20;
    assert_eq!(model.pages(), 3);
    model.query.page_size = 50;
    assert_eq!(model.pages(), 1);
    model.search = SearchStatus::Done { total: 0 };
    assert_eq!(model.pages(), 0);
}
