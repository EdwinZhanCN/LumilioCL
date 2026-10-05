use super::*;
use crate::live::{LiveModel, library_card};
use lumilio_core::{
    CategoryTag, DiscoverFilters, GameVersionTag, InstanceRecord, InstanceSettings, Loader,
    LoaderTag, Pick, SearchHit, Stance,
};

fn query() -> DiscoverQuery {
    DiscoverQuery::new(ProjectKind::Mod)
}

fn category(kind: ProjectKind, header: &str, name: &str) -> CategoryTag {
    CategoryTag {
        name: name.to_owned(),
        header: header.to_owned(),
        kind,
    }
}

fn version(name: &str, release: bool) -> GameVersionTag {
    GameVersionTag {
        version: name.to_owned(),
        release,
        snapshot: !release,
        published: String::new(),
    }
}

fn loader(name: &str, kinds: &[&str]) -> LoaderTag {
    LoaderTag {
        name: name.to_owned(),
        project_types: kinds.iter().map(|kind| (*kind).to_owned()).collect(),
    }
}

fn filters() -> FilterModel {
    FilterModel::from_core(&DiscoverFilters {
        categories: vec![
            category(ProjectKind::Mod, "categories", "technology"),
            category(ProjectKind::Mod, "categories", "magic"),
            category(ProjectKind::Mod, "features", "multiplayer"),
            category(ProjectKind::ResourcePack, "resolutions", "128x"),
            category(ProjectKind::ResourcePack, "resolutions", "16x"),
            category(ProjectKind::ResourcePack, "resolutions", "8x-"),
            category(ProjectKind::ResourcePack, "categories", "themed"),
            category(ProjectKind::Shader, "performance impact", "high"),
            category(ProjectKind::Shader, "performance impact", "potato"),
            category(ProjectKind::Shader, "performance impact", "medium"),
        ],
        game_versions: vec![
            version("26.4-pre1", false),
            version("26.3", true),
            version("26.2", true),
        ],
        loaders: vec![
            loader("quilt", &["mod", "modpack"]),
            loader("fabric", &["mod", "modpack"]),
            loader("neoforge", &["mod", "modpack"]),
            loader("bukkit", &["mod", "plugin"]),
            loader("iris", &["shader"]),
            loader("vanilla", &["shader"]),
            loader("canvas", &["shader"]),
        ],
        ..Default::default()
    })
}

#[test]
fn the_sidebar_offers_only_what_the_source_can_filter_by() {
    use lumilio_core::{KindAbilities, SortIndex};
    let every = |kind| KindAbilities {
        kind,
        sorts: vec![SortIndex::Relevance],
        game_versions: true,
        categories: true,
        loaders: true,
        environment: true,
        open_source: true,
        hide_installed: true,
        advanced: true,
    };
    let mut model = filters();
    let before = sections(ProjectKind::Mod, &model);
    assert!(before.contains(&Section::Loader) && before.contains(&Section::Environment));

    // A source that filters mods by game version only.
    let mut mods = every(ProjectKind::Mod);
    (
        mods.loaders,
        mods.environment,
        mods.open_source,
        mods.advanced,
    ) = (false, false, false, false);
    mods.hide_installed = false;
    let found = lumilio_core::DiscoverFilters {
        abilities: vec![mods, every(ProjectKind::Shader)],
        ..Default::default()
    };
    model = FilterModel::from_core(&found);
    assert_eq!(sections(ProjectKind::Mod, &model), [Section::Version]);
    assert!(!model.offers_hide_installed(ProjectKind::Mod));
    assert!(model.offers_hide_installed(ProjectKind::Shader));
    assert!(
        sections(ProjectKind::ResourcePack, &model).is_empty(),
        "a kind the source does not serve has no filters"
    );
}

#[test]
fn any_change_but_paging_returns_to_the_first_page() {
    let mut q = query();
    q.page = 7;
    assert_eq!(q.clone().apply(DiscoverChange::Page(8)).page, 8);
    for change in [
        DiscoverChange::Sort(SortIndex::Downloads),
        DiscoverChange::PageSize(50),
        DiscoverChange::ToggleVersion("26.3".to_owned()),
        DiscoverChange::ShowAllVersions(true),
        DiscoverChange::Include(PickGroup::Category, "magic".to_owned()),
        DiscoverChange::Exclude(PickGroup::Loader, "forge".to_owned()),
        DiscoverChange::ToggleSide(Side::Client),
        DiscoverChange::License(Stance::Include),
        DiscoverChange::Advanced("archived".to_owned()),
        DiscoverChange::HideInstalled(true),
        DiscoverChange::Unlock(Lock::Version),
        DiscoverChange::ClearFilters,
        DiscoverChange::Kind(ProjectKind::Shader),
    ] {
        assert_eq!(q.clone().apply(change.clone()).page, 0, "{change:?}");
    }
}

#[test]
fn including_toggles_and_excluding_replaces_then_toggles() {
    let include = |q: DiscoverQuery| {
        q.apply(DiscoverChange::Include(
            PickGroup::Category,
            "magic".to_owned(),
        ))
    };
    let exclude = |q: DiscoverQuery| {
        q.apply(DiscoverChange::Exclude(
            PickGroup::Category,
            "magic".to_owned(),
        ))
    };
    let q = include(query());
    assert_eq!(q.categories, [Pick::include("magic")]);
    assert!(include(q.clone()).categories.is_empty(), "again: dropped");
    // Leaving out an option that was asked for swaps the stance.
    let q = exclude(q);
    assert_eq!(q.categories, [Pick::exclude("magic")]);
    // Asking for a left-out option drops it, as the app's row does.
    assert!(include(q.clone()).categories.is_empty());
    assert!(exclude(q).categories.is_empty(), "left out twice: dropped");
}

#[test]
fn a_new_kind_starts_its_own_search_but_keeps_what_applies_everywhere() {
    let mut q = query()
        .apply(DiscoverChange::Sort(SortIndex::Follows))
        .apply(DiscoverChange::PageSize(50))
        .apply(DiscoverChange::Include(
            PickGroup::Category,
            "magic".to_owned(),
        ))
        .apply(DiscoverChange::Include(
            PickGroup::Loader,
            "fabric".to_owned(),
        ))
        .apply(DiscoverChange::ToggleVersion("26.3".to_owned()))
        .apply(DiscoverChange::License(Stance::Include))
        .apply(DiscoverChange::Advanced("plugin".to_owned()))
        .apply(DiscoverChange::Advanced("archived".to_owned()));
    q.text = "sodium".to_owned();
    let same = q.clone().apply(DiscoverChange::Kind(ProjectKind::Mod));
    assert_eq!(same.categories.len(), 1, "the same kind keeps its picks");
    assert_eq!(same.sort, SortIndex::Follows);
    let packs = q.apply(DiscoverChange::Kind(ProjectKind::ResourcePack));
    assert!(packs.categories.is_empty() && packs.loaders.is_empty());
    assert_eq!(
        (packs.text.as_str(), packs.sort),
        ("", SortIndex::Relevance)
    );
    assert_eq!(packs.versions, ["26.3"], "versions carry over");
    assert_eq!(packs.license, Some(Stance::Include));
    assert_eq!(packs.page_size, 50);
    assert_eq!(
        packs.advanced,
        ["archived"],
        "an exclusion the kind does not offer is dropped"
    );
}

#[test]
fn the_license_toggles_and_sides_are_independent() {
    let q = query()
        .apply(DiscoverChange::ToggleSide(Side::Client))
        .apply(DiscoverChange::ToggleSide(Side::Server))
        .apply(DiscoverChange::ToggleSide(Side::Client));
    assert_eq!((q.client, q.server), (false, true));
    // A second press of the same stance removes it.
    let q = query()
        .apply(DiscoverChange::License(Stance::Include))
        .apply(DiscoverChange::License(Stance::Include));
    assert_eq!(q.license, None);
    let q = q
        .apply(DiscoverChange::License(Stance::Include))
        .apply(DiscoverChange::License(Stance::Exclude));
    assert_eq!(q.license, Some(Stance::Exclude));
}

#[test]
fn clearing_filters_keeps_text_sort_and_the_hide_toggle() {
    let mut q = query()
        .apply(DiscoverChange::Sort(SortIndex::Follows))
        .apply(DiscoverChange::HideInstalled(true));
    q.text = "sodium".to_owned();
    let q = q
        .apply(DiscoverChange::Include(PickGroup::Category, "x".to_owned()))
        .apply(DiscoverChange::ToggleVersion("1.0".to_owned()))
        .apply(DiscoverChange::Advanced("archived".to_owned()))
        .apply(DiscoverChange::ClearFilters);
    assert!(!q.filtered());
    assert_eq!((q.text.as_str(), q.sort), ("sodium", SortIndex::Follows));
    assert!(q.hide_installed);
}

#[test]
fn advanced_options_replace_their_parent_or_children() {
    let q = query().apply(DiscoverChange::Advanced("ai_content_code".to_owned()));
    assert_eq!(q.advanced, ["ai_content_code"]);
    let q = q.apply(DiscoverChange::Advanced("ai_content".to_owned()));
    assert_eq!(q.advanced, ["ai_content"], "the whole replaces the part");
    let q = q.apply(DiscoverChange::Advanced("ai_content_text".to_owned()));
    assert_eq!(q.advanced, ["ai_content_text"], "a part replaces the whole");
    let q = q.apply(DiscoverChange::Advanced("ai_content_text".to_owned()));
    assert!(q.advanced.is_empty());
}

#[test]
fn only_mods_and_modpacks_offer_the_code_disclosures_and_only_mods_other_types() {
    let ids = |kind| -> Vec<&'static str> {
        advanced_options(kind)
            .into_iter()
            .map(|option| option.id)
            .collect()
    };
    assert!(ids(ProjectKind::Mod).contains(&"telemetry"));
    assert!(ids(ProjectKind::Mod).contains(&"plugin"));
    assert!(ids(ProjectKind::Modpack).contains(&"system_interactions"));
    assert!(!ids(ProjectKind::Modpack).contains(&"plugin"));
    assert!(!ids(ProjectKind::Shader).contains(&"telemetry"));
    assert!(ids(ProjectKind::ResourcePack).contains(&EPILEPSY_TRIGGERS));
    assert!(is_type_exclusion("datapack") && !is_type_exclusion("archived"));
}

#[test]
fn a_game_provides_its_version_and_loader_where_they_matter() {
    use ProjectKind::*;
    let provided = |kind, version: &str, loader| Provided::for_game(kind, version, loader);
    assert_eq!(
        provided(Mod, "1.21.1", Loader::Fabric),
        Provided {
            version: Some("1.21.1".to_owned()),
            loader: Some("fabric".to_owned())
        }
    );
    // Resource packs are tied to neither.
    assert_eq!(
        provided(ResourcePack, "1.21.1", Loader::Fabric),
        Provided::default()
    );
    // Shaders: the version, not the loader, unless the game is vanilla.
    assert_eq!(
        provided(Shader, "1.21.1", Loader::Fabric),
        Provided {
            version: Some("1.21.1".to_owned()),
            loader: None
        }
    );
    assert_eq!(
        provided(Shader, "1.21.1", Loader::Vanilla),
        Provided {
            version: None,
            loader: Some("vanilla".to_owned())
        }
    );
    // Packs are installed whole, so only the version could matter.
    assert_eq!(provided(Modpack, "1.21.1", Loader::Forge).loader, None);
}

#[test]
fn tabs_inside_a_game_drop_packs_and_mods_for_a_vanilla_game() {
    assert_eq!(visible_kinds(None).len(), 4);
    assert_eq!(
        visible_kinds(Some(Loader::Fabric)),
        [
            ProjectKind::Mod,
            ProjectKind::ResourcePack,
            ProjectKind::Shader
        ]
    );
    assert_eq!(
        visible_kinds(Some(Loader::Vanilla)),
        [ProjectKind::ResourcePack, ProjectKind::Shader]
    );
}

#[test]
fn the_games_filters_stand_in_until_released_then_the_persons_own_count() {
    let provided = Provided {
        version: Some("1.21.1".to_owned()),
        loader: Some("fabric".to_owned()),
    };
    let filters = filters();
    let mut q = query()
        .apply(DiscoverChange::ToggleVersion("26.3".to_owned()))
        .apply(DiscoverChange::Include(
            PickGroup::Loader,
            "quilt".to_owned(),
        ));
    // Locked: the person's own picks do not count, and are not shown as chips.
    let search = q.to_search(&provided, &filters, Vec::new());
    assert_eq!(search.game_versions, ["1.21.1"]);
    assert_eq!(search.loaders, [Pick::include("fabric").any_of()]);
    // Released: theirs.
    q = q.apply(DiscoverChange::Unlock(Lock::Version));
    let search = q.to_search(&provided, &filters, Vec::new());
    assert_eq!(search.game_versions, ["26.3"]);
    assert_eq!(
        search.loaders,
        [Pick::include("fabric").any_of()],
        "loader still locked"
    );
    // Syncing takes the game's back and forgets the person's.
    let q = q.apply(DiscoverChange::Sync(Lock::Version));
    assert!(q.versions.is_empty() && !q.is_unlocked(Lock::Version));
    assert_eq!(
        q.to_search(&provided, &filters, Vec::new()).game_versions,
        ["1.21.1"]
    );
}

#[test]
fn the_core_query_carries_picks_groups_and_exclusions() {
    let filters = filters();
    let mut q = DiscoverQuery::new(ProjectKind::ResourcePack)
        .apply(DiscoverChange::Include(
            PickGroup::Category,
            "16x".to_owned(),
        ))
        .apply(DiscoverChange::Include(
            PickGroup::Category,
            "themed".to_owned(),
        ))
        .apply(DiscoverChange::License(Stance::Include))
        .apply(DiscoverChange::Advanced("epilepsy_triggers".to_owned()))
        .apply(DiscoverChange::Sort(SortIndex::Updated))
        .apply(DiscoverChange::PageSize(5000));
    q.text = "tech".to_owned();
    let q = q.apply(DiscoverChange::Page(3));
    let search = q.to_search(&Provided::default(), &filters, vec!["P".to_owned()]);
    assert_eq!((search.page_size, search.page), (100, 3));
    assert_eq!(
        (search.text.as_str(), search.sort),
        ("tech", SortIndex::Updated)
    );
    assert_eq!(
        search.categories,
        [Pick::include("16x").any_of(), Pick::include("themed")],
        "resolutions are an any-of group, other categories are not"
    );
    assert_eq!(search.open_source, Some(Stance::Include));
    assert_eq!(search.hidden_projects, ["P"]);
    assert_eq!(search.excluded_disclosures, ["epilepsy_triggers"]);
}

#[test]
fn sections_follow_the_apps_order_per_kind() {
    use Section::*;
    let filters = filters();
    let cat = |header: &str| Category(header.to_owned());
    assert_eq!(
        sections(ProjectKind::Mod, &filters),
        [
            Version,
            Loader,
            cat("categories"),
            cat("features"),
            Environment,
            License,
            Advanced
        ]
    );
    assert_eq!(
        sections(ProjectKind::ResourcePack, &filters),
        [
            cat("categories"),
            cat("resolutions"),
            Version,
            License,
            Advanced
        ]
    );
    assert_eq!(
        sections(ProjectKind::Shader, &filters),
        [
            cat("performance impact"),
            Loader,
            License,
            Version,
            Advanced
        ]
    );
    assert_eq!(
        sections(ProjectKind::Modpack, &FilterModel::default()),
        [Environment, Version, Loader, License, Advanced]
    );
}

#[test]
fn categories_group_by_header_and_sort_like_modrinth() {
    let filters = filters();
    assert_eq!(
        filters.category_groups(ProjectKind::Mod),
        [
            ("categories".to_owned(), vec!["magic", "technology"]),
            ("features".to_owned(), vec!["multiplayer"]),
        ]
    );
    // Numbers sort as numbers, not as text.
    let packs = filters.category_groups(ProjectKind::ResourcePack);
    assert_eq!(
        packs[1],
        ("resolutions".to_owned(), vec!["8x-", "16x", "128x"])
    );
    // Performance impact goes by weight.
    assert_eq!(
        filters.category_groups(ProjectKind::Shader)[0].1,
        ["potato", "medium", "high"]
    );
    assert!(filters.any_of(ProjectKind::ResourcePack, "16x"));
    assert!(!filters.any_of(ProjectKind::ResourcePack, "themed"));
}

#[test]
fn loader_lists_put_the_usual_first_and_fall_back_without_modrinths_list() {
    let filters = filters();
    // A loader that also serves plugins is not a mod loader.
    assert_eq!(
        filters.loader_options(ProjectKind::Mod),
        ["fabric", "neoforge", "quilt"]
    );
    assert_eq!(
        filters.loader_options(ProjectKind::Shader),
        ["iris", "vanilla", "canvas"]
    );
    assert!(filters.loader_options(ProjectKind::ResourcePack).is_empty());
    assert_eq!(
        FilterModel::default().loader_options(ProjectKind::Mod),
        ["fabric", "forge", "neoforge", "quilt"]
    );
    assert_eq!(
        filters.loaders_not_for(ProjectKind::Mod),
        ["iris", "vanilla", "canvas"]
    );
}

#[test]
fn the_version_list_shows_releases_unless_all_are_asked_for() {
    let filters = filters();
    assert_eq!(filters.versions(false), ["26.3", "26.2"]);
    assert_eq!(filters.versions(true), ["26.4-pre1", "26.3", "26.2"]);
    assert!(filters.knows_version("26.4-pre1"));
    assert!(!filters.knows_version("1.0"));
}

fn hit(categories: &[&str], loaders: &[&str]) -> SearchHit {
    SearchHit {
        project_id: "P".to_owned(),
        slug: "fo".to_owned(),
        title: "FO".to_owned(),
        description: "fast".to_owned(),
        author: "me".to_owned(),
        kind: ProjectKind::Mod,
        categories: categories.iter().map(|name| (*name).to_owned()).collect(),
        all_categories: categories.iter().map(|name| (*name).to_owned()).collect(),
        loaders: loaders.iter().map(|name| (*name).to_owned()).collect(),
        downloads: 17_731_400,
        follows: 4886,
        published: "2026-01-01T10:00:00Z".to_owned(),
        updated: "2026-09-27T10:00:00Z".to_owned(),
        environment: Some(Environment::ClientOnly),
        icon_url: Some("https://cdn/x.png".to_owned()),
    }
}

fn names(tags: &CardTags) -> Vec<&str> {
    tags.shown.iter().map(|tag| tag.name.as_str()).collect()
}

#[test]
fn card_tags_list_categories_then_usual_loaders_and_fold_the_rest() {
    let filters = filters();
    let hit = hit(
        &["technology", "magic", "adventure", "storage", "worldgen"],
        &["quilt", "forge", "fabric"],
    );
    let tags = card_tags(&hit, &query(), &filters);
    // Four fit next to the install button (the environment tag takes a place).
    assert_eq!(
        names(&tags),
        ["adventure", "magic", "storage", "technology"]
    );
    assert_eq!(tags.overflow, ["worldgen", "fabric", "forge", "quilt"]);
    // Without an environment tag one more fits.
    let mut bare = hit.clone();
    bare.environment = None;
    assert_eq!(card_tags(&bare, &query(), &filters).shown.len(), 5);
}

#[test]
fn card_tags_skip_what_was_filtered_by_and_hide_loaders_after_a_loader_filter() {
    let filters = filters();
    let hit = hit(&["magic", "technology"], &["fabric", "forge"]);
    let q = query().apply(DiscoverChange::Include(
        PickGroup::Category,
        "magic".to_owned(),
    ));
    let tags = card_tags(&hit, &q, &filters);
    assert_eq!(names(&tags), ["technology", "fabric", "forge"]);
    assert!(tags.shown[1].loader && !tags.shown[0].loader);
    assert_eq!(
        tags.overflow,
        ["magic"],
        "what was left out still counts as more"
    );
    let q = q.apply(DiscoverChange::Include(
        PickGroup::Loader,
        "fabric".to_owned(),
    ));
    assert_eq!(names(&card_tags(&hit, &q, &filters)), ["technology"]);
    // Resource packs never list loaders.
    let mut pack = hit.clone();
    pack.kind = ProjectKind::ResourcePack;
    assert_eq!(
        names(&card_tags(
            &pack,
            &DiscoverQuery::new(ProjectKind::ResourcePack),
            &filters
        )),
        ["magic", "technology"]
    );
}

#[test]
fn rows_show_counts_environment_and_the_date_the_sort_asks_for() {
    let filters = filters();
    let hit = hit(&["magic"], &["fabric"]);
    let now = parse_rfc3339("2026-09-30T10:00:00Z").unwrap();
    let row = search_row(&hit, now, &query(), &filters);
    assert_eq!(row.downloads, "1773.1 万");
    assert_eq!(row.follows, "4886");
    assert_eq!(
        (row.date.as_str(), row.date_kind),
        ("3 天前", DateKind::Updated)
    );
    assert_eq!(row.environment, Some(Environment::ClientOnly));
    assert_eq!(row.icon_url.as_deref(), Some("https://cdn/x.png"));
    assert_eq!(row.page_url, "https://modrinth.com/mod/fo");
    // Sorted by newest, the card says when it came out.
    let newest = query().apply(DiscoverChange::Sort(SortIndex::Newest));
    let row = search_row(&hit, now, &newest, &filters);
    assert_eq!(row.date_kind, DateKind::Published);
    assert_ne!(row.date, "3 天前");
    let unknown = SearchHit {
        updated: String::new(),
        ..hit
    };
    assert_eq!(search_row(&unknown, now, &query(), &filters).date, "");
}

#[test]
fn environments_have_their_own_words() {
    assert_eq!(
        environment_label(Environment::ClientOrServer),
        "客户端或服务端"
    );
    assert_eq!(environment_label(Environment::SingleplayerOnly), "单人游戏");
    assert_eq!(
        environment_label(Environment::DedicatedServerOnly),
        "专用服务器"
    );
}

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

fn model_with(games: &[(&str, Loader, Option<&str>)]) -> LiveModel {
    let mut model = LiveModel::default();
    let cards = games
        .iter()
        .map(|(id, loader, source)| library_card(&record(id, *loader, *source), 100))
        .collect();
    model.set_library(cards, None);
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
