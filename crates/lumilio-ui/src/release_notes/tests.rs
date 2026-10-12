use super::text::{Notes, Section, parse};
use super::*;
use gpui::{Context, Modifiers, Render, Window, div, prelude::*, px, size};

#[test]
fn notes_split_into_a_summary_and_sections_of_items() {
    let notes = parse(
        "<!-- 发布前删除本注释 -->\n0.2.0 让启动更快，\n并修复登录。\n\n## 新功能\n\n- 世界地图\n  显示矿脉。\n- 支持 `Java 25`。\n\n## 空的\n\n## Fixes\n\n- Sign-in no longer\n  fails.\nA paragraph.\n",
    );
    assert_eq!(
        notes,
        Notes {
            summary: "0.2.0 让启动更快，并修复登录。".into(),
            sections: vec![
                Section {
                    title: "新功能".into(),
                    items: vec!["世界地图显示矿脉。".into(), "支持 `Java 25`。".into()],
                },
                Section {
                    title: "Fixes".into(),
                    items: vec!["Sign-in no longer fails. A paragraph.".into()],
                },
            ],
        }
    );
    // Items before any heading still show, under no title.
    assert_eq!(parse("- a\n- b").sections[0].items, ["a", "b"]);
    assert!(parse("#hashtag").sections.is_empty());
}

fn notes(version: &'static str) -> ReleaseNotes {
    ReleaseNotes {
        version,
        chinese: "",
        english: None,
    }
}

fn versions(picked: &[&ReleaseNotes]) -> Vec<&'static str> {
    picked.iter().map(|notes| notes.version).collect()
}

#[test]
fn a_start_shows_the_releases_since_the_last_shown_newest_first() {
    let all = [
        notes("0.1.3"),
        notes("0.1.10"),
        notes("0.1.9"),
        notes("0.2.0-beta.1"),
        notes("0.2.0"),
    ];
    assert_eq!(
        versions(&pick(&all, Some("0.1.3"), "0.2.0")),
        ["0.2.0", "0.2.0-beta.1", "0.1.10", "0.1.9"]
    );
    assert_eq!(versions(&pick(&all, Some("0.1.9"), "0.1.10")), ["0.1.10"]);
    // From before release notes, or unreadable: only the running release.
    assert_eq!(versions(&pick(&all, None, "0.1.10")), ["0.1.10"]);
    assert_eq!(versions(&pick(&all, Some("dev"), "0.1.10")), ["0.1.10"]);
    // Nothing new, a downgrade, or a release without notes shows nothing.
    assert!(pick(&all, Some("0.2.0"), "0.2.0").is_empty());
    assert!(pick(&all, Some("0.2.0"), "0.1.9").is_empty());
    assert!(pick(&all, None, "0.1.4").is_empty());
}

/// The release workflow publishes `release-notes/<version>.md`; a version
/// bump without it fails here, before a tag is pushed.
#[test]
fn the_running_release_has_notes_and_every_file_reads() {
    assert!(
        current().is_some(),
        "write release-notes/{}.md (assets/icons/PACKAGING.md §5)",
        running()
    );
    for release in RELEASES {
        let chinese = parse(release.chinese);
        assert!(!chinese.summary.is_empty(), "{} summary", release.version);
        assert!(
            chinese.sections.iter().all(|s| !s.title.is_empty()),
            "{} sections have titles",
            release.version
        );
        if let Some(english) = release.english {
            let shape = |notes: &Notes| {
                notes
                    .sections
                    .iter()
                    .map(|section| section.items.len())
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                shape(&parse(english)),
                shape(&chinese),
                "{}.en.md has the same sections and items",
                release.version
            );
        }
    }
}

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

static TWO: [ReleaseNotes; 2] = [
    ReleaseNotes {
        version: "0.2.0",
        chinese: "0.2.0 的概述。\n\n## 新功能\n\n- 第一项\n- 第二项\n\n## 修复\n\n- 第三项\n",
        english: None,
    },
    ReleaseNotes {
        version: "0.1.9",
        chinese: "0.1.9 的概述。\n\n## 修复\n\n- 第四项\n",
        english: None,
    },
];

#[gpui::test]
fn the_dialog_fits_the_smallest_window_and_closes_with_its_key(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    cx.simulate_resize(size(px(720.), px(480.)));
    cx.update(|window, cx| show(TWO.iter().collect(), Occasion::Updated, window, cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let window = size(px(720.), px(480.));
    for selector in [
        "release-notes-body",
        "release-notes-all",
        "release-notes-done",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector}");
    }
    // Both releases are there, and the footer keys stay inside the window
    // while the body scrolls.
    assert!(cx.debug_bounds("release-notes-0.2.0").is_some());
    assert!(cx.debug_bounds("release-notes-0.1.9").is_some());
    let done = cx.debug_bounds("release-notes-done").unwrap();
    assert!(
        done.bottom() <= window.height && done.right() <= window.width,
        "{done:?}"
    );

    cx.simulate_click(done.center(), Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
}
