//! Switching the language is process-wide, so it is tested in its own test
//! binary, away from the unit tests that read Chinese. The tests here take
//! turns through `LANGUAGE`.

use std::sync::{Mutex, MutexGuard, PoisonError};

use gpui::{Context, Entity, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use gpui_component::IndexPath;
use lumilio_ui::i18n::{Locale, generation, locale, set_locale};
use lumilio_ui::live::{count_label, tag_label};
use lumilio_ui::pages::live::LiveControls;
use lumilio_ui::pages::settings::{list_text, memory_text, tabs};

static LANGUAGE: Mutex<()> = Mutex::new(());

fn language_turn() -> MutexGuard<'static, ()> {
    LANGUAGE.lock().unwrap_or_else(PoisonError::into_inner)
}

#[test]
fn switching_language_changes_the_launcher_and_component_words() {
    let _turn = language_turn();
    set_locale(Locale::English);
    assert_eq!(locale(), Locale::English);
    assert_eq!(tabs()[0], "General");
    assert_eq!(memory_text(None), "Not set");
    assert_eq!(
        list_text(&["-Da".into(), "-Db".into(), "-Dc".into()]),
        "3 items: -Da, …"
    );
    assert_eq!(&*gpui_component::locale(), "en");
    // Counts group by thousands in English, by ten thousands in Chinese.
    assert_eq!(count_label(999), "999");
    assert_eq!(count_label(12_345), "12.3K");
    assert_eq!(count_label(2_500_000), "2.5M");
    // A tag the source sends is looked up at run time; proper nouns stay.
    assert_eq!(tag_label("kitchen-sink"), "Kitchen sink");
    assert_eq!(tag_label("fabric"), "Fabric");
    assert_eq!(tag_label("brand-new-tag"), "Brand new tag");

    set_locale(Locale::SimplifiedChinese);
    assert_eq!(count_label(12_345), "1.2 万");
    assert_eq!(tag_label("kitchen-sink"), "大杂烩");
    assert_eq!(tabs()[0], "通用");
    assert_eq!(memory_text(None), "未设置");
    assert_eq!(&*gpui_component::locale(), "zh-CN");
}

struct Host(Entity<LiveControls>);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[gpui::test]
fn a_language_switch_relabels_controls_and_keeps_the_choice(cx: &mut TestAppContext) {
    let _turn = language_turn();
    set_locale(Locale::SimplifiedChinese);
    cx.update(gpui_component::init);
    let (host, cx) =
        cx.add_window_view(|window, cx| Host(cx.new(|cx| LiveControls::new(window, cx))));
    host.update_in(cx, |host, window, cx| {
        host.0.update(cx, |controls, cx| {
            controls.library_sort.update(cx, |sort, cx| {
                sort.set_selected_index(Some(IndexPath::new(1)), window, cx)
            });
        });
    });

    set_locale(Locale::English);
    host.read_with(cx, |host, cx| {
        assert_ne!(
            host.0.read(cx).language,
            generation(),
            "the switch is noticed"
        );
    });
    host.update_in(cx, |host, window, cx| {
        host.0
            .update(cx, |controls, cx| controls.relabel(window, cx));
    });
    host.read_with(cx, |host, cx| {
        let controls = host.0.read(cx);
        assert_eq!(controls.language, generation());
        assert_eq!(
            controls
                .library_filter
                .read(cx)
                .presentation()
                .placeholder()
                .as_ref(),
            "Search games"
        );
        let sort = controls.library_sort.read(cx);
        assert_eq!(sort.selected_index(cx), Some(IndexPath::new(1)));
        assert_eq!(sort.selected_value().map(String::as_str), Some("Name"));
    });
    set_locale(Locale::SimplifiedChinese);
}

struct Pane(Entity<lumilio_ui::schematic_studio::StudioView>);

impl Render for Pane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // The studio window at its 720 px minimum: only the window's inset
        // comes off the width (plan schematic-studio, D22).
        div().size_full().p(gpui::px(12.)).child(self.0.clone())
    }
}

#[gpui::test]
fn in_english_the_studios_tabs_and_menu_are_not_cut_off(cx: &mut TestAppContext) {
    use gpui::{Modifiers, px, size};
    use std::rc::Rc;
    let _turn = language_turn();
    set_locale(Locale::English);
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let studio = cx.new(|cx| {
            lumilio_ui::schematic_studio::StudioView::new(
                "lumilio.litematica".into(),
                "schematics".into(),
                lumilio_ui::schematic_studio::Placement::Window,
                Rc::new(|_, _, _| {}),
                Rc::new(|_, _, _, _| {}),
                Rc::new(|_, _, _| {}),
                cx,
            )
        });
        let pane = cx.new(|_| Pane(studio));
        gpui_component::Root::new(pane, window, cx)
    });
    cx.simulate_resize(size(px(720.), px(480.)));
    cx.run_until_parked();
    assert_eq!(
        lumilio_ui::schematic_studio::tab_labels(),
        ["Materials", "Info", "History"]
    );
    let inspector = cx.debug_bounds("studio-inspector").expect("inspector");
    for index in 0..3 {
        let selector: &'static str =
            Box::leak(format!("studio-inspector-tab-{index}").into_boxed_str());
        let tab = cx.debug_bounds(selector).expect("tab");
        assert!(
            tab.left() >= inspector.left() && tab.right() <= inspector.right(),
            "tab {index} fits: {tab:?} in {inspector:?}"
        );
        assert_eq!(tab.size.height, px(30.), "one line, never squeezed");
    }
    // The head's switcher, orange key and More sit on one row, inside.
    let switcher = cx.debug_bounds("studio-switcher").expect("switcher");
    let replace = cx.debug_bounds("studio-replace").expect("replace key");
    let more = cx.debug_bounds("studio-more").expect("more key");
    assert!(
        switcher.right() <= replace.left() && replace.right() <= more.left(),
        "{switcher:?} {replace:?} {more:?}"
    );
    assert!(more.right() <= px(720.) && replace.size.height < px(40.));
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    // With no file chosen every entry is shown disabled with its reason.
    for index in 0..6 {
        let selector: &'static str = Box::leak(format!("menu-entry-{index}").into_boxed_str());
        let entry = cx.debug_bounds(selector).expect("menu entry");
        assert!(
            entry.size.height < px(24.),
            "entry {index} stays on one line: {entry:?}"
        );
        assert!(
            entry.left() >= px(0.) && entry.right() <= px(720.),
            "entry {index} stays in the window: {entry:?}"
        );
    }
    set_locale(Locale::SimplifiedChinese);
}

#[gpui::test]
fn in_english_the_studios_operation_dialogs_fit_the_minimum_window(cx: &mut TestAppContext) {
    use gpui::{Bounds, Modifiers, Pixels, px, size};
    use lumilio_schematic::{BlockState, Region, UniversalSchematic};
    use lumilio_ui::schematic_studio::{Command, Connection, Event, StudioView};
    use std::rc::Rc;
    let _turn = language_turn();
    set_locale(Locale::English);
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let slot: Rc<std::cell::RefCell<Option<Entity<StudioView>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let studio = cx.new(|cx| {
            StudioView::new(
                "lumilio.litematica".into(),
                "schematics".into(),
                lumilio_ui::schematic_studio::Placement::Window,
                Rc::new(|_, _, _| {}),
                Rc::new(|_, _, _, _| {}),
                Rc::new(|_, _, _| {}),
                cx,
            )
        });
        *keep.borrow_mut() = Some(studio.clone());
        let pane = cx.new(|_| Pane(studio));
        gpui_component::Root::new(pane, window, cx)
    });
    cx.simulate_resize(size(px(720.), px(480.)));
    let studio = slot.borrow().clone().unwrap();
    let (send, commands) = async_channel::unbounded();
    let (events, receive) = async_channel::unbounded();
    studio.update(cx, |studio, cx| {
        studio.connect(Connection { send, receive }, cx)
    });
    cx.run_until_parked();

    let file = "schematics/tower.litematic";
    let mut schematic = UniversalSchematic::new("Tower".to_owned());
    let mut region = Region::new("Main".to_owned(), (0, 0, 0), (2, 1, 1));
    let stairs = BlockState::from_block_string("minecraft:oak_stairs[facing=north,half=top]");
    region.set_block(0, 0, 0, &stairs.unwrap());
    schematic.add_region(region);
    let bytes = lumilio_schematic::to_litematic_bytes(&schematic).unwrap();
    let info = lumilio_plugin_api::FileInfo {
        len: bytes.len() as u64,
        modified_ms: 0,
        sha256: "0".repeat(64),
    };
    let answer = |event: Event, cx: &mut gpui::VisualTestContext| {
        events.try_send(event).unwrap();
        cx.run_until_parked();
    };
    let reads = |commands: &async_channel::Receiver<Command>| {
        std::iter::from_fn(|| commands.try_recv().ok())
            .filter_map(|command| match command {
                Command::Read { file } => Some(file),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    answer(Event::Listed(Ok(vec![file.into()])), cx);
    answer(Event::CanWrite(true), cx);
    let click = |selector: &'static str, cx: &mut gpui::VisualTestContext| {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector}"));
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    };
    click("studio-switcher", cx);
    click("studio-file-schematics/tower.litematic", cx);
    for _ in 0..2 {
        for read in reads(&commands) {
            answer(
                Event::Read {
                    file: read,
                    result: Ok((bytes.clone(), info.clone())),
                },
                cx,
            );
        }
    }
    let inside = |what: &str, bounds: Bounds<Pixels>| {
        assert!(
            bounds.left() >= px(0.) && bounds.right() <= px(720.) && bounds.bottom() <= px(480.),
            "{what} stays in the window: {bounds:?}"
        );
    };
    // 替换方块… is the head's orange key; 转换版本… is More's first entry.
    for (entry, extra) in [
        ("studio-replace", "studio-replace-target-0"),
        ("menu-entry-0", "studio-convert-picker"),
    ] {
        if entry == "menu-entry-0" {
            click("studio-more", cx);
        }
        click(entry, cx);
        if entry == "menu-entry-0" {
            answer(
                Event::Versions(Ok(vec![("26.3".into(), 4790), ("1.13.2".into(), 1631)])),
                cx,
            );
        }
        let footer: &[&str] = if entry == "studio-replace" {
            &[
                "studio-dialog-cancel",
                "studio-dialog-commit",
                "studio-replace-scheme-save",
            ]
        } else {
            &["studio-dialog-cancel", "studio-dialog-commit"]
        };
        for key in footer {
            let bounds = cx.debug_bounds(key).expect("dialog key");
            inside(key, bounds);
            assert!(
                bounds.size.height < px(40.),
                "{key} is one line: {bounds:?}"
            );
        }
        inside(extra, cx.debug_bounds(extra).expect("field"));
        click("studio-dialog-cancel", cx);
    }
    // The property chips of the stairs: a name and a dropdown each, on one
    // line, once "Any properties" is off (the head opens on any state).
    click("studio-replace", cx);
    click("studio-replace-any-0", cx);
    // facing and half.
    for property in [0, 1] {
        let selector: &'static str =
            Box::leak(format!("studio-replace-chip-0-{property}").into_boxed_str());
        let chip = cx.debug_bounds(selector).expect("a property chip");
        inside(selector, chip);
        assert!(
            chip.size.height < px(40.),
            "{selector} is one line: {chip:?}"
        );
    }
    set_locale(Locale::SimplifiedChinese);
}
