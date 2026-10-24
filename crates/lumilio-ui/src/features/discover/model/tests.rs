use lumilio_core::{
    CategoryTag, DiscoverFilters, GameVersionTag, Loader, LoaderTag, Pick, SearchHit, Stance,
};

use super::*;

fn query() -> DiscoverQuery {
    DiscoverQuery::new(ProjectKind::Mod)
}

fn category(kind: ProjectKind, header: &str, name: &str) -> CategoryTag {
    CategoryTag {
        id: name.to_owned(),
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
        exclusions: true,
        browse_url: None,
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
fn the_filter_count_is_each_pick_once_and_not_the_sort_or_the_hide_toggle() {
    let q = query()
        .apply(DiscoverChange::Sort(SortIndex::Follows))
        .apply(DiscoverChange::HideInstalled(true))
        .apply(DiscoverChange::Include(PickGroup::Category, "x".to_owned()))
        .apply(DiscoverChange::Exclude(PickGroup::Category, "y".to_owned()))
        .apply(DiscoverChange::Include(
            PickGroup::Loader,
            "fabric".to_owned(),
        ))
        .apply(DiscoverChange::ToggleVersion("1.0".to_owned()))
        .apply(DiscoverChange::ToggleSide(Side::Client))
        .apply(DiscoverChange::License(Stance::Include))
        .apply(DiscoverChange::Advanced("archived".to_owned()));
    assert_eq!(q.filter_count(), 7);
    assert_eq!(q.apply(DiscoverChange::ClearFilters).filter_count(), 0);
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
    let names = |groups: Vec<(String, Vec<(&str, &str)>)>| -> Vec<(String, Vec<String>)> {
        groups
            .into_iter()
            .map(|(header, options)| {
                (
                    header,
                    options
                        .into_iter()
                        .map(|(_, name)| name.to_owned())
                        .collect(),
                )
            })
            .collect()
    };
    assert_eq!(
        names(filters.category_groups(ProjectKind::Mod)),
        [
            (
                "categories".to_owned(),
                vec!["magic".to_owned(), "technology".to_owned()]
            ),
            ("features".to_owned(), vec!["multiplayer".to_owned()]),
        ]
    );
    // Numbers sort as numbers, not as text.
    let packs = names(filters.category_groups(ProjectKind::ResourcePack));
    assert_eq!(packs[1].1, ["8x-", "16x", "128x"]);
    // Performance impact goes by weight.
    assert_eq!(
        names(filters.category_groups(ProjectKind::Shader))[0].1,
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
        follows: Some(4886),
        published: "2026-01-01T10:00:00Z".to_owned(),
        updated: "2026-09-27T10:00:00Z".to_owned(),
        environment: Some(Environment::ClientOnly),
        icon_url: Some("https://cdn/x.png".to_owned()),
        page_url: "https://modrinth.com/mod/fo".to_owned(),
        manual_download: false,
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
    assert_eq!(
        (row.count.as_str(), row.count_kind),
        ("1773.1 万", CountKind::Downloads)
    );
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
    // Sorted by followers, the card counts them instead of downloads.
    let by_follows = query().apply(DiscoverChange::Sort(SortIndex::Follows));
    let row = search_row(&hit, now, &by_follows, &filters);
    assert_eq!(
        (row.count.as_str(), row.count_kind),
        ("4886", CountKind::Follows)
    );
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

#[test]
fn another_source_keeps_the_words_kind_and_versions_and_drops_its_picks() {
    let browsed = DiscoverQuery::new(ProjectKind::ResourcePack)
        .apply(DiscoverChange::ToggleVersion("1.21.1".into()))
        .apply(DiscoverChange::Include(PickGroup::Category, "16x".into()))
        .apply(DiscoverChange::Sort(SortIndex::Follows))
        .apply(DiscoverChange::Page(3));
    let mut browsed = browsed;
    browsed.text = "faithful".into();
    let moved = browsed
        .clone()
        .apply(DiscoverChange::Source("lumilio.curseforge".into()));
    assert_eq!(moved.source, "lumilio.curseforge");
    assert_eq!(moved.kind, ProjectKind::ResourcePack);
    assert_eq!(moved.text, "faithful");
    assert_eq!(moved.versions, ["1.21.1"]);
    assert!(
        moved.categories.is_empty(),
        "another source's categories mean other things"
    );
    assert_eq!(moved.sort, SortIndex::Relevance);
    assert_eq!(moved.page, 0);
    let same = browsed
        .clone()
        .apply(DiscoverChange::Source(lumilio_core::DEFAULT_SOURCE.into()));
    assert_eq!(
        same.categories, browsed.categories,
        "the same source changes nothing"
    );
    let kind = moved.apply(DiscoverChange::Kind(ProjectKind::Mod));
    assert_eq!(
        kind.source, "lumilio.curseforge",
        "a new kind stays in the source"
    );
}

/// A CurseForge-like answer: categories by number, named in the interface
/// language, and fewer sorts, no exclusions.
fn curseforge_filters() -> FilterModel {
    use lumilio_core::KindAbilities;
    FilterModel::from_core(&DiscoverFilters {
        abilities: vec![KindAbilities {
            kind: ProjectKind::Mod,
            sorts: vec![
                SortIndex::Relevance,
                SortIndex::Downloads,
                SortIndex::Updated,
            ],
            game_versions: true,
            categories: true,
            loaders: true,
            environment: false,
            open_source: false,
            hide_installed: false,
            advanced: false,
            exclusions: false,
            browse_url: Some("https://www.curseforge.com/minecraft/mc-mods".into()),
        }],
        categories: vec![
            CategoryTag {
                id: "423".into(),
                name: "地图与信息".into(),
                header: "categories".into(),
                kind: ProjectKind::Mod,
            },
            CategoryTag {
                id: "412".into(),
                name: "科技".into(),
                header: "categories".into(),
                kind: ProjectKind::Mod,
            },
        ],
        ..Default::default()
    })
}

#[test]
fn a_source_offers_its_own_sorts_exclusions_and_listing() {
    let filters = curseforge_filters();
    assert_eq!(
        filters.sorts(ProjectKind::Mod),
        [
            SortIndex::Relevance,
            SortIndex::Downloads,
            SortIndex::Updated
        ],
        "menu order, only what the source has"
    );
    assert!(!filters.offers_exclusions(ProjectKind::Mod));
    assert_eq!(
        filters.browse_url(ProjectKind::Mod),
        Some("https://www.curseforge.com/minecraft/mc-mods")
    );
    assert_eq!(
        filters.category_name(ProjectKind::Mod, "423"),
        Some("地图与信息")
    );
    let groups = filters.category_groups(ProjectKind::Mod);
    assert!(
        groups[0].1.contains(&("423", "地图与信息")),
        "a search asks for the ID, the row reads the name"
    );
    assert_eq!(
        FilterModel::default().sorts(ProjectKind::Mod),
        SORTS,
        "everything until the source has said"
    );
}

#[test]
fn a_card_leaves_out_a_category_picked_by_id() {
    let filters = curseforge_filters();
    let query = query().apply(DiscoverChange::Include(PickGroup::Category, "423".into()));
    let tags = card_tags(&hit(&["地图与信息", "科技"], &["forge"]), &query, &filters);
    assert_eq!(names(&tags), ["科技", "forge"]);
}

#[test]
fn a_row_without_followers_counts_downloads_and_says_where_to_download() {
    let mut found = hit(&[], &[]);
    found.follows = None;
    found.manual_download = true;
    let by_follows = query().apply(DiscoverChange::Sort(SortIndex::Follows));
    let row = search_row(&found, 0, &by_follows, &filters());
    assert_eq!(row.count_kind, CountKind::Downloads);
    assert!(row.manual_download);
    assert_eq!(row.source, lumilio_core::DEFAULT_SOURCE);
}
