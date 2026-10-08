use super::LiveModel;
use super::accounts::{account_failure, account_rows};
use super::activity::{
    ActivityRow, ActivityState, active_row, activity_in_tab, activity_rows, eta_text, next_sample,
    rate_text, recovery_message,
};
use super::discover::{PageItem, SearchStatus, count_label, page_items, parse_rfc3339, tag_label};
use super::home::home_presentation;
use super::library::{
    CollectionRow, attention_rows, library_card, library_cards, relative_time, seed_of, world_of,
};
use super::settings::settings_view;
use crate::home::HomePresentation;
use lumilio_core::{
    ActiveTask, ActivityView, FinishedTask, HomeSummary, InstanceRecord, LauncherSettings, Loader,
    TaskCategory, TaskLabel, TaskOutcome,
};
use std::path::{Path, PathBuf};

use lumilio_core::{AttentionItem, InstanceSettings};

fn record(id: &str, favorite: bool, last_played: Option<u64>, created_at: u64) -> InstanceRecord {
    InstanceRecord {
        id: id.to_owned(),
        name: id.to_uppercase(),
        game_version: "1.21.1".to_owned(),
        loader: Loader::Fabric,
        loader_version: Some("0.16.0".to_owned()),
        favorite,
        created_at,
        last_played,
        play_seconds: 0,
        installed: false,
        settings: InstanceSettings::default(),
        source_project: None,
    }
}

const NOW: u64 = 10_000_000;

#[test]
fn recovery_leads_with_the_worst_news_and_counts_the_rest() {
    use lumilio_core::RecoveryNote::*;
    assert_eq!(recovery_message(&[]), None);
    let note = recovery_message(&[
        DeleteCompleted {
            instance_id: "a".into(),
        },
        LibraryRecovered {
            preserved: "/x".into(),
            candidates: vec!["old".into(), "older".into()],
        },
        SessionInterrupted {
            instance_id: "b".into(),
            started: 1,
        },
    ])
    .unwrap();
    assert!(note.starts_with("游戏库文件无法读取"));
    assert!(note.contains("2 个可能的游戏目录") && note.contains("另有 2 项"));
    let single = recovery_message(&[ActivityLogSkipped { count: 3 }]).unwrap();
    assert!(single.contains('3') && !single.contains("另有"));
    assert!(!note.contains("/x"), "paths stay out of the sentence");
}

#[test]
fn relative_time_steps_through_the_units() {
    assert_eq!(relative_time(NOW, NOW), "刚刚");
    assert_eq!(relative_time(NOW - 59, NOW), "刚刚");
    assert_eq!(relative_time(NOW - 60, NOW), "1 分钟前");
    assert_eq!(relative_time(NOW - 7200, NOW), "2 小时前");
    assert_eq!(relative_time(NOW - 90_000, NOW), "昨天");
    assert_eq!(relative_time(NOW - 4 * 86_400, NOW), "4 天前");
    assert_eq!(relative_time(NOW - 70 * 86_400, NOW), "2 个月前");
    // A clock that ran backwards never panics or goes negative.
    assert_eq!(relative_time(NOW + 500, NOW), "刚刚");
}

#[test]
fn covers_are_stable_per_instance_and_differ_between_them() {
    assert_eq!(seed_of("survival"), seed_of("survival"));
    assert_ne!(seed_of("survival"), seed_of("creative"));
    assert_eq!(world_of("survival"), world_of("survival"));
}

#[test]
fn the_library_lists_favorites_then_recent_then_newest() {
    let records = [
        record("old", false, None, 1),
        record("new", false, None, 9),
        record("played", false, Some(500), 2),
        record("star", true, None, 3),
    ];
    let order: Vec<_> = library_cards(&records, NOW)
        .into_iter()
        .map(|card| card.id)
        .collect();
    assert_eq!(order, ["star", "played", "new", "old"]);
}

#[test]
fn cards_say_when_an_instance_was_never_played() {
    let never = library_card(&record("a", false, None, 1), NOW);
    assert_eq!(never.played, "还没玩过");
    assert_eq!(never.meta, "1.21.1 · Fabric");
    let played = library_card(&record("b", false, Some(NOW - 3600), 1), NOW);
    assert_eq!(played.played, "上次游玩 1 小时前");
}

#[test]
fn home_is_first_use_with_no_instances() {
    let (home, id) = home_presentation(&[], &HomeSummary::default(), None, NOW);
    assert_eq!(home, HomePresentation::FirstUse);
    assert_eq!(id, None);
}

#[test]
fn home_offers_the_newest_instance_until_something_was_played() {
    let records = [record("a", false, None, 1), record("b", false, None, 5)];
    let (home, id) = home_presentation(&records, &HomeSummary::default(), None, NOW);
    assert_eq!(id.as_deref(), Some("b"));
    let HomePresentation::Continue { subject, recent } = home else {
        panic!("expected Continue");
    };
    assert_eq!(subject.title, "B");
    assert!(subject.metadata.contains("还没玩过"));
    assert!(recent.is_empty());
}

#[test]
fn home_follows_the_summary() {
    let records = [
        record("a", false, Some(100), 1),
        record("b", false, Some(900_000), 2),
        record("c", false, Some(500_000), 3),
    ];
    let summary = HomeSummary {
        continue_with: Some("b".to_owned()),
        recent: vec!["c".to_owned(), "a".to_owned(), "ghost".to_owned()],
        needs_attention: Vec::<AttentionItem>::new(),
    };
    let (home, id) = home_presentation(&records, &summary, None, NOW);
    assert_eq!(id.as_deref(), Some("b"));
    let HomePresentation::Continue { subject, recent } = home else {
        panic!("expected Continue");
    };
    assert!(subject.metadata.contains("上次游玩于"));
    // A stale id in the summary is skipped, not shown.
    let titles: Vec<_> = recent.iter().map(|entry| entry.title.as_str()).collect();
    assert_eq!(titles, ["C", "A"]);
}

#[test]
fn home_continues_with_the_current_instance_and_does_not_list_it_twice() {
    let records = [
        record("a", false, Some(100), 1),
        record("b", false, Some(900_000), 2),
        record("c", false, Some(500_000), 3),
    ];
    let summary = HomeSummary {
        continue_with: Some("b".to_owned()),
        recent: vec!["c".to_owned(), "a".to_owned()],
        needs_attention: Vec::<AttentionItem>::new(),
    };
    let (home, id) = home_presentation(&records, &summary, Some("c"), NOW);
    assert_eq!(id.as_deref(), Some("c"));
    let HomePresentation::Continue { subject, recent } = home else {
        panic!("expected Continue");
    };
    assert_eq!(subject.title, "C");
    let titles: Vec<_> = recent.iter().map(|entry| entry.title.as_str()).collect();
    assert_eq!(titles, ["A"]);
    // A current instance that no longer exists is ignored.
    let (_, id) = home_presentation(&records, &summary, Some("ghost"), NOW);
    assert_eq!(id.as_deref(), Some("b"));
}

#[test]
fn account_rows_show_the_selection_and_custom_ids() {
    use lumilio_core::{AccountEntry, LauncherSettings, ProfileId};
    let mut settings = LauncherSettings::default();
    settings.accounts = vec![
        AccountEntry {
            name: "Steve".into(),
            ..AccountEntry::default()
        },
        AccountEntry {
            name: "Alex".into(),
            uuid: Some("123e4567e89b12d3a456426614174000".into()),
            ..AccountEntry::default()
        },
        AccountEntry {
            name: "broken name".into(),
            uuid: Some("not an id".into()),
            ..AccountEntry::default()
        },
        AccountEntry {
            name: "Edwin_Zhan".into(),
            uuid: Some("00000000000000000000000000000abc".into()),
            kind: lumilio_core::AccountKind::Microsoft,
            needs_sign_in: true,
            ..AccountEntry::default()
        },
    ];
    settings.selected_account = Some("Alex".into());
    let rows = account_rows(&settings);
    // An unreadable entry is left out rather than shown wrong.
    assert_eq!(rows.len(), 3);
    let microsoft = &rows[2];
    assert!(microsoft.microsoft && microsoft.needs_sign_in && !microsoft.custom_id);
    assert!(!microsoft.third_party && microsoft.signed_in());
    assert_eq!(microsoft.key, "msa:00000000000000000000000000000abc");
    assert_eq!(microsoft.kind_label(), "Microsoft");
    assert_eq!(rows[0].kind_label(), "离线账户");
    assert_eq!(rows[0].key, "Steve");
    assert!(!rows[0].selected && !rows[0].custom_id);
    assert_eq!(rows[0].uuid, ProfileId::offline("Steve").to_string());
    assert!(rows[1].selected && rows[1].custom_id);
    assert_eq!(rows[1].uuid, "123e4567-e89b-12d3-a456-426614174000");
}

#[test]
fn the_settings_view_reads_the_saved_values_and_describes_each_java() {
    let mut settings = LauncherSettings::default();
    settings.default_max_memory_mb = Some(4096);
    settings.download_concurrency = Some(6);
    settings.prefer_mirrors = true;
    settings.extra_java_roots = vec!["/opt/jdks".into()];
    let view = settings_view(&settings, &[], None, Path::new("/data"), Some(16_384));
    assert_eq!(view.max_memory_mb, Some(4096));
    assert_eq!(view.min_memory_mb, None);
    assert_eq!(view.download_concurrency, Some(6));
    assert_eq!(
        view.download_source,
        lumilio_core::DownloadSourcePreference::MirrorFirst
    );
    assert_eq!(view.java_roots, [PathBuf::from("/opt/jdks")]);
    assert_eq!(view.data_dir, PathBuf::from("/data"));
    assert!(view.java.is_empty() && view.storage.is_none());
    assert_eq!(view.total_memory_mb, Some(16_384));
}

#[test]
fn third_party_accounts_show_their_server_and_offline_ones_their_skin() {
    use lumilio_core::{AccountEntry, AccountKind, AuthServerEntry, LauncherSettings, SkinChoice};
    let little = lumilio_core::LITTLE_SKIN_URL;
    let mut settings = LauncherSettings::default();
    settings.auth_servers = vec![
        AuthServerEntry {
            url: "https://auth.example/api/".into(),
            name: Some("Example Skins".into()),
            non_email_login: true,
        },
        AuthServerEntry {
            url: "https://noname.example/".into(),
            name: None,
            non_email_login: false,
        },
    ];
    let third = |name: &str, id: &str, server: &str| AccountEntry {
        name: name.into(),
        uuid: Some(id.into()),
        kind: AccountKind::ThirdParty,
        server: Some(server.into()),
        login: Some("me@example.com".into()),
        ..AccountEntry::default()
    };
    settings.accounts = vec![
        third("Edwin", "123e4567e89b12d3a456426614174000", little),
        third(
            "Wen",
            "223e4567e89b12d3a456426614174000",
            "https://auth.example/api/",
        ),
        third(
            "Lost",
            "323e4567e89b12d3a456426614174000",
            "https://noname.example/",
        ),
        AccountEntry {
            name: "Steve".into(),
            skin: Some(SkinChoice::LittleSkin),
            ..AccountEntry::default()
        },
    ];
    settings.selected_account = Some(format!("ali:123e4567e89b12d3a456426614174000@{little}"));
    let rows = account_rows(&settings);
    assert_eq!(rows.len(), 4);
    assert!(rows[0].third_party && rows[0].signed_in() && rows[0].selected && !rows[0].custom_id);
    assert_eq!(rows[0].kind_label(), "LittleSkin");
    assert_eq!(rows[1].kind_label(), "Example Skins");
    assert_eq!(
        rows[2].kind_label(),
        "https://noname.example/",
        "an unnamed server shows its address"
    );
    assert_eq!(rows[3].skin_text(), Some("LittleSkin 皮肤"));
    assert!(!rows[3].signed_in());
    assert_eq!(rows[0].skin_text(), None);
}

#[test]
fn authentication_failures_are_told_in_words() {
    use lumilio_core::{ServiceError, SkinError, YggdrasilError};
    let message = |error: ServiceError| account_failure(&error).0;
    assert!(message(ServiceError::Yggdrasil(YggdrasilError::InvalidCredentials)).contains("密码"));
    assert!(
        message(ServiceError::Yggdrasil(YggdrasilError::Network("x".into())))
            .contains("无法连接认证服务器")
    );
    assert!(message(ServiceError::Yggdrasil(YggdrasilError::NoCharacter)).contains("没有角色"));
    assert!(
        message(ServiceError::Yggdrasil(YggdrasilError::Remote {
            kind: "ForbiddenOperationException".into(),
            message: Some("Invalid token.".into()),
        }))
        .contains("失效")
    );
    assert!(
        message(ServiceError::Yggdrasil(YggdrasilError::Remote {
            kind: "Other".into(),
            message: Some("Banned until tomorrow".into()),
        }))
        .contains("Banned until tomorrow"),
        "an unknown refusal keeps the server's own words"
    );
    assert!(message(ServiceError::Skin(SkinError::Picture("x".into()))).contains("PNG"));
    assert!(message(ServiceError::SignInRequired("Edwin".into())).contains("Edwin"));
    let (_, technical) = account_failure(&ServiceError::Yggdrasil(YggdrasilError::Network(
        "refused".into(),
    )));
    assert!(
        technical.contains("refused"),
        "the cause stays behind 技术详情"
    );
}

#[test]
fn account_failures_say_what_to_change() {
    use lumilio_core::{ServiceError, SettingsError};
    let (message, technical) = account_failure(&ServiceError::Settings(
        SettingsError::DuplicateAccount("Steve".into()),
    ));
    assert!(message.contains("Steve") && message.contains("已经有"));
    assert!(technical.contains("already exists"));
    let (message, _) = account_failure(&ServiceError::Settings(SettingsError::DuplicateUuid(
        "x".into(),
    )));
    assert!(message.contains("UUID"));
    let (message, _) = account_failure(&ServiceError::Cancelled);
    assert!(message.contains("没能保存"));
}

#[test]
fn download_counts_are_short() {
    assert_eq!(count_label(999), "999");
    assert_eq!(count_label(12_345), "1.2 万");
    assert_eq!(count_label(250_000_000), "2.5 亿");
}

#[test]
fn activity_shows_running_first_and_maps_outcomes() {
    let view = ActivityView {
        active: vec![ActiveTask {
            id: 1,
            category: TaskCategory::Download,
            label: TaskLabel::Text("安装 sodium".to_owned()),
            instance_id: Some("a".to_owned()),
            started: 0,
            progress: Some((50, 200)),
            unit: lumilio_core::ProgressUnit::Items,
            retry: None,
        }],
        cancellable: [1].into(),
        finished: vec![
            FinishedTask {
                category: TaskCategory::Install,
                label: TaskLabel::Text("ok".to_owned()),
                instance_id: None,
                started: 0,
                finished: NOW - 120,
                outcome: TaskOutcome::Succeeded,
                retry: None,
            },
            FinishedTask {
                category: TaskCategory::Download,
                label: TaskLabel::Text("bad".to_owned()),
                instance_id: None,
                started: 0,
                finished: NOW,
                outcome: TaskOutcome::Failed("no route".to_owned()),
                retry: Some(lumilio_core::RetryAction::InstallModpack {
                    project: "pack".into(),
                }),
            },
        ],
    };
    let rows = activity_rows(&view, NOW);
    assert_eq!(rows[0].state, ActivityState::Running);
    assert_eq!(rows[0].fraction, Some(0.25));
    assert_eq!(rows[0].cancel, Some(1));
    assert!(rows[1..].iter().all(|row| row.cancel.is_none()));
    assert_eq!(rows[1].state, ActivityState::Done);
    assert_eq!(rows[1].detail, "2 分钟前");
    assert_eq!(rows[2].state, ActivityState::Failed("no route".to_owned()));
    let model = LiveModel {
        activity: rows,
        ..LiveModel::default()
    };
    assert_eq!(model.active_tasks(), 1);
}

#[test]
fn home_attention_names_the_game_its_worst_problem_and_the_first_remedy() {
    use lumilio_core::{Problem, ProblemKind, Severity};
    let item = |id: &str, total| AttentionItem {
        instance_id: id.to_owned(),
        headline: Problem {
            severity: Severity::Error,
            kind: ProblemKind::DamagedFiles { count: 2 },
        },
        total,
    };
    let card = |id: &str| library_card(&record(id, false, None, 1), NOW);
    let rows = attention_rows(&[item("a", 3), item("gone", 1)], &[card("a")]);
    assert_eq!(rows.len(), 1, "a deleted game is skipped");
    assert_eq!(rows[0].name, "A");
    assert_eq!(rows[0].more, 2);
    assert!(rows[0].title.contains("损坏"));
    assert_eq!(
        rows[0].action.map(|(action, _)| action),
        Some(crate::instance_detail::ProblemAction::Repair)
    );
}

#[test]
fn a_finished_row_keeps_what_it_needs_to_be_retried_or_opened() {
    let view = ActivityView {
        active: Vec::new(),
        cancellable: Default::default(),
        finished: vec![FinishedTask {
            category: TaskCategory::Install,
            label: TaskLabel::Text("bad".to_owned()),
            instance_id: Some("a".to_owned()),
            started: 0,
            finished: NOW,
            outcome: TaskOutcome::Failed("no route".to_owned()),
            retry: Some(lumilio_core::RetryAction::RepairInstance {
                instance: "a".into(),
            }),
        }],
    };
    let rows = activity_rows(&view, NOW);
    assert_eq!(rows[0].instance.as_deref(), Some("a"));
    assert_eq!(
        rows[0].retry,
        Some(lumilio_core::RetryAction::RepairInstance {
            instance: "a".into()
        })
    );
}

#[test]
fn speed_is_the_smoothed_change_between_readings_and_ignores_noise() {
    let first = next_sample(None, 100, 1_000);
    assert_eq!(first.per_sec, None, "one reading is no speed");
    // 100 more bytes in half a second.
    let second = next_sample(Some(first), 200, 1_500);
    assert_eq!(second.per_sec, Some(200.));
    // A reading too soon after, or one that went backwards, changes nothing.
    assert_eq!(next_sample(Some(second), 900, 1_550), second);
    assert_eq!(next_sample(Some(second), 150, 3_000), second);
    // Smoothing: a burst moves the number only part of the way.
    let third = next_sample(Some(second), 1_200, 2_500);
    let rate = third.per_sec.unwrap();
    assert!(rate > 200. && rate < 1_000., "{rate}");
}

#[test]
fn speed_and_time_left_read_naturally() {
    use lumilio_core::ProgressUnit::{Bytes, Items};
    assert_eq!(rate_text(Bytes, 3.2 * 1024. * 1024.), "3.2 MB/秒");
    assert_eq!(rate_text(Bytes, 900. * 1024.), "900 KB/秒");
    assert_eq!(rate_text(Items, 42.4), "42 个文件/秒");
    assert_eq!(rate_text(Items, 2.46), "2.5 个文件/秒");
    assert_eq!(eta_text(100, 10.).as_deref(), Some("不到 1 分钟"));
    assert_eq!(eta_text(900, 10.).as_deref(), Some("约 2 分钟"));
    assert_eq!(eta_text(36_000, 5.).as_deref(), Some("约 2 小时 0 分钟"));
    assert_eq!(eta_text(10, 0.), None);
}

#[test]
fn set_activity_gives_running_rows_a_speed_names_the_game_and_forgets_finished_tasks() {
    let running = |done| ActivityRow {
        task: Some(7),
        amount: Some((done, 1_000)),
        unit: lumilio_core::ProgressUnit::Bytes,
        rate: None,
        instance: Some("a".into()),
        retry: None,
        category: TaskCategory::Download,
        title: "t".into(),
        detail: "a".into(),
        fraction: None,
        state: ActivityState::Running,
        cancel: None,
    };
    let card = |id: &str| library_card(&record(id, false, None, 1), NOW);
    let mut model = LiveModel::default();
    model.set_library(vec![card("a")], None);
    model.set_activity(vec![running(100)], 1_000);
    assert_eq!(model.activity[0].rate, None);
    assert_eq!(model.activity[0].detail, "A", "the game's name, not its id");
    model.set_activity(vec![running(300)], 2_000);
    assert_eq!(model.activity[0].rate, Some(200.));
    model.set_activity(Vec::new(), 3_000);
    model.set_activity(vec![running(900)], 4_000);
    assert_eq!(
        model.activity[0].rate, None,
        "a task that ended starts over"
    );
}

#[test]
fn an_activity_tab_shows_only_its_category_and_all_shows_everything() {
    let row = |category, title: &str| ActivityRow {
        task: None,
        amount: None,
        unit: lumilio_core::ProgressUnit::Items,
        rate: None,
        instance: None,
        retry: None,
        category,
        title: title.to_owned(),
        detail: String::new(),
        fraction: None,
        state: ActivityState::Done,
        cancel: None,
    };
    let rows = vec![
        row(TaskCategory::Download, "d"),
        row(TaskCategory::Install, "i"),
        row(TaskCategory::Repair, "r"),
        row(TaskCategory::Install, "i2"),
    ];
    assert_eq!(activity_in_tab(&rows, 0).len(), 4);
    let installs: Vec<_> = activity_in_tab(&rows, 2)
        .iter()
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(installs, ["i", "i2"]);
    assert!(activity_in_tab(&rows, 3).is_empty());
    assert_eq!(activity_in_tab(&rows, 99).len(), 4, "unknown tab = all");
}

#[test]
fn a_collection_lists_its_games_that_still_exist_and_marks_membership() {
    let card = |id: &str| library_card(&record(id, false, None, 1), NOW);
    let mut model = LiveModel::default();
    model.set_library(vec![card("a"), card("b"), card("c")], None);
    model.collections = vec![
        CollectionRow {
            name: "生存".into(),
            members: vec!["c".into(), "gone".into(), "a".into()],
        },
        CollectionRow {
            name: "空".into(),
            members: Vec::new(),
        },
    ];
    let ids: Vec<_> = model
        .collection_cards(&model.collections[0])
        .iter()
        .map(|card| card.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["c", "a"],
        "kept in the order added; deleted games are skipped"
    );
    assert_eq!(
        model.memberships_of("a"),
        [("生存".to_owned(), true), ("空".to_owned(), false)]
    );
    assert_eq!(
        model.memberships_of("b"),
        [("生存".to_owned(), false), ("空".to_owned(), false)]
    );
}

#[test]
fn unknown_progress_is_not_a_fraction() {
    let task = ActiveTask {
        id: 1,
        category: TaskCategory::Install,
        label: TaskLabel::Text(String::new()),
        instance_id: None,
        started: 0,
        progress: Some((0, 0)),
        unit: lumilio_core::ProgressUnit::Items,
        retry: None,
    };
    assert_eq!(active_row(&task, false).fraction, None);
}

#[test]
fn the_install_target_survives_refreshes_and_falls_back_when_deleted() {
    let card = |id: &str| library_card(&record(id, false, None, 1), NOW);
    let mut model = LiveModel::default();
    model.set_library(vec![card("a"), card("b")], Some("b".to_owned()));
    assert_eq!(model.install_target.as_deref(), Some("b"));
    // The person's own choice wins over later hints.
    model.install_target = Some("a".to_owned());
    model.set_library(vec![card("a"), card("b")], Some("b".to_owned()));
    assert_eq!(model.install_target.as_deref(), Some("a"));
    // The chosen instance was deleted: fall back rather than dangle.
    model.set_library(vec![card("b")], None);
    assert_eq!(model.install_target.as_deref(), Some("b"));
    model.set_library(Vec::new(), None);
    assert_eq!(model.install_target, None);
}

fn shown(current: u32, pages: u32) -> String {
    page_items(current, pages)
        .iter()
        .map(|item| match item {
            PageItem::Page(page) => (page + 1).to_string(),
            PageItem::Gap => "…".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn the_pager_shows_ends_and_neighbours() {
    assert_eq!(shown(0, 928), "1 2 … 928");
    assert_eq!(shown(5, 928), "1 … 5 6 7 … 928");
    assert_eq!(shown(927, 928), "1 … 927 928");
    assert_eq!(shown(1, 928), "1 2 3 … 928");
    assert_eq!(
        shown(2, 928),
        "1 2 3 4 … 928",
        "a single hidden page is shown, not elided"
    );
    assert_eq!(shown(0, 1), "1");
    assert_eq!(shown(0, 3), "1 2 3");
    assert_eq!(shown(9, 4), "1 2 3 4", "an out-of-range page is clamped");
    assert_eq!(shown(0, 0), "");
}

#[test]
fn pages_follow_the_total_and_the_page_size() {
    let mut model = LiveModel::default();
    assert_eq!(model.pages(), 0);
    model.search = SearchStatus::Done { total: 41 };
    model.query.page_size = 20;
    assert_eq!(model.pages(), 3);
    model.query.page_size = 50;
    assert_eq!(model.pages(), 1);
    model.search = SearchStatus::Done { total: 0 };
    assert_eq!(model.pages(), 0);
}

#[test]
fn timestamps_parse_with_fractions_and_offsets() {
    assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_rfc3339("2000-03-01T00:00:00Z"), Some(951_868_800));
    assert_eq!(
        parse_rfc3339("2024-02-29T12:30:15.123456Z"),
        Some(1_709_209_815)
    );
    assert_eq!(
        parse_rfc3339("2024-02-29T12:30:15+00:00"),
        Some(1_709_209_815)
    );
    assert_eq!(parse_rfc3339("yesterday"), None);
    assert_eq!(parse_rfc3339("2024-13-01T00:00:00Z"), None);
    assert_eq!(parse_rfc3339(""), None);
}

#[test]
fn tag_labels_are_chinese_when_known_and_readable_when_not() {
    assert_eq!(tag_label("optimization"), "优化");
    assert_eq!(tag_label("fabric"), "Fabric");
    assert_eq!(tag_label("some-new-thing"), "Some new thing");
    assert_eq!(tag_label(""), "");
}

#[test]
fn home_places_are_the_latest_readable_worlds_then_the_first_servers() {
    use super::home::{PLACE_SERVERS, PLACE_WORLDS, PlaceTarget, home_places};
    use lumilio_core::{PackPolicy, ServerEntry, WorldInfo};
    let world = |folder: &str, played: Option<i64>, damaged: bool| WorldInfo {
        folder: folder.to_owned(),
        name: folder.to_uppercase(),
        last_played_ms: played,
        game_version: None,
        hardcore: folder == "hard",
        has_icon: false,
        damaged,
        lock_touched_ms: None,
    };
    let worlds = [
        world("old", Some(1_000), false),
        world("never", None, false),
        world("broken", Some(9_000_000_000), true),
        world("hard", Some((NOW - 3 * 86_400) as i64 * 1000), false),
        world("new", Some((NOW - 7200) as i64 * 1000), false),
    ];
    let servers: Vec<_> = ["one", "two", "three"]
        .iter()
        .map(|name| ServerEntry {
            name: (*name).to_owned(),
            address: format!("{name}.example"),
            packs: PackPolicy::Ask,
        })
        .collect();
    let mut game = record("a", false, None, 1);
    game.play_seconds = 4000;
    let places = home_places(&game, &worlds, &servers, NOW);
    assert_eq!(places.instance, "a");
    assert_eq!(
        (places.play_seconds, places.worlds, places.servers),
        (4000, 5, 3)
    );
    let targets: Vec<_> = places.places.iter().map(|place| &place.target).collect();
    // Newest first, the damaged one skipped, then servers in the player's order.
    assert_eq!(
        targets,
        [
            &PlaceTarget::World("new".to_owned()),
            &PlaceTarget::World("hard".to_owned()),
            &PlaceTarget::World("old".to_owned()),
            &PlaceTarget::Server("one.example".to_owned()),
            &PlaceTarget::Server("two.example".to_owned()),
        ]
    );
    assert_eq!(places.places.len(), PLACE_WORLDS + PLACE_SERVERS);
    assert_eq!(places.places[0].detail, "世界 · 上次游玩 2 小时前");
    assert_eq!(places.places[1].detail, "世界 · 上次游玩 3 天前 · 极限模式");
    assert_eq!(places.places[3].detail, "服务器 · one.example");

    // A version that cannot go straight into a world lists nothing to enter,
    // but keeps its record.
    game.game_version = "1.19.4".to_owned();
    let old = home_places(&game, &worlds, &servers, NOW);
    assert!(old.places.is_empty());
    assert_eq!(old.worlds, 5);
}
