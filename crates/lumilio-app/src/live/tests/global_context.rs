//! Acceptance of plan global-context (T1): the current game and the current
//! account have one owner, and what Home and Discover show of them follows
//! every change, whichever way it was made. Each test drives the shipped
//! wiring against a real service in a temporary folder. No Java is found
//! (an empty search root), so a game's Java problem depends only on the
//! automatic-download setting; game versions that do not exist keep a
//! launch from downloading anything.

use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::{AppContext as _, Entity, TestAppContext, VisualTestContext};
use lumilio_core::{HistoryEvent, Loader, ProjectKind, SessionOutcome};
use lumilio_ui::home::{HomeIntent, HomePresentation};
use lumilio_ui::instance_detail::InstanceIntent;
use lumilio_ui::live::{LiveIntent, attention_rows, library_cards};
use lumilio_ui::route::Route;
use lumilio_ui::{LauncherShell, ShellIntent};

use super::super::context::follow;
use super::super::dispatch::{on_live_intent, on_shell_intent};
use super::super::instance::open_instance;
use super::super::launch::play;
use super::super::{State, Wiring};
use crate::backend::{Backend, Service};

const WAIT: Duration = Duration::from_secs(10);

/// The game the corner chip names: the one installs go into.
fn chip(wiring: &Wiring, cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| {
        wiring
            .shell
            .read_with(cx, |shell, _| shell.install_target().map(str::to_owned))
            .ok()
            .flatten()
    })
}

/// The game Discover browses for, if any.
fn browsing_for(shell: &LauncherShell, cx: &gpui_kit::App) -> Option<String> {
    shell.discover().read(cx).state().browsing_for.clone()
}

/// The shell, wired to the shipped handlers, in a window that can show
/// dialogs. Searches and Discover's first search are not sent: these tests
/// are about which game is meant, and must not reach the network for results
/// nobody looks at.
pub(super) fn open(
    cx: &mut TestAppContext,
) -> (Entity<LauncherShell>, Wiring, &mut VisualTestContext) {
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let java = dir.path().join("no-java");
    std::fs::create_dir_all(&java).unwrap();
    let backend = Backend::open_with(dir.path().join("root"), |service| {
        service.with_runtime_roots(vec![java])
    })
    .unwrap();
    // The folder outlives the test's service; the process ends with the test.
    std::mem::forget(dir);
    cx.update(gpui_kit::init);
    let cell: Rc<RefCell<Option<Wiring>>> = Rc::default();
    let live = {
        let cell = cell.clone();
        Rc::new(
            move |intent: LiveIntent, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App| {
                if let Some(wiring) = cell.borrow().clone() {
                    on_live_intent(&wiring, intent, window, cx);
                }
            },
        )
    };
    let shell_intents = {
        let cell = cell.clone();
        Rc::new(
            move |intent: ShellIntent, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App| {
                if matches!(intent, ShellIntent::Navigate(Route::Discover)) {
                    return;
                }
                if let Some(wiring) = cell.borrow().clone() {
                    on_shell_intent(&wiring, intent, window, cx);
                }
            },
        )
    };
    let worker = backend.worker();
    let slot: Rc<RefCell<Option<Entity<LauncherShell>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| {
            LauncherShell::new(cx)
                .with_home(HomePresentation::Loading, cx)
                .with_live(live)
                .with_intent_handler(shell_intents)
                .with_worker(worker, cx)
        });
        *keep.borrow_mut() = Some(shell.clone());
        gpui_kit::component::Root::new(shell, window, cx)
    });
    let shell = slot.borrow().clone().expect("shell");
    let wiring = Wiring {
        backend: Rc::new(backend),
        shell: shell.downgrade(),
        state: Rc::new(RefCell::new(State::default())),
    };
    *cell.borrow_mut() = Some(wiring.clone());
    (shell, wiring, cx)
}

/// Runs `work` on the service and waits for its answer.
pub(super) fn ask<R, F>(wiring: &Wiring, work: impl FnOnce(Arc<Service>) -> F) -> R
where
    F: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let service = wiring.backend.service.clone();
    futures::executor::block_on(wiring.backend.spawn(work(service))).unwrap()
}

pub(super) fn until(
    cx: &mut VisualTestContext,
    what: &str,
    done: impl Fn(&mut VisualTestContext) -> bool,
) {
    let deadline = Instant::now() + WAIT;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn game(wiring: &Wiring, name: &'static str, version: &'static str) -> String {
    ask(wiring, move |service| async move {
        service
            .create_instance(name, Some(version), Loader::Vanilla, None)
            .await
            .unwrap()
            .id
    })
}

fn saved_current(wiring: &Wiring) -> Option<String> {
    ask(
        wiring,
        |service| async move { service.current_instance().await },
    )
}

/// Home's 需要留意 rows, as (game, title).
fn attention(shell: &Entity<LauncherShell>, cx: &mut VisualTestContext) -> Vec<(String, String)> {
    shell.read_with(cx, |shell, _| {
        let rows = &shell.live().unwrap().attention;
        rows.iter()
            .map(|row| (row.instance.clone(), row.title.clone()))
            .collect()
    })
}

/// What a fresh background check says Home should list now.
fn attention_now(wiring: &Wiring) -> Vec<(String, String)> {
    let (records, summary) = ask(wiring, |service| async move { service.home().await });
    attention_rows(
        &summary.needs_attention,
        &library_cards(&records, lumilio_core::unix_now()),
    )
    .into_iter()
    .map(|row| (row.instance, row.title))
    .collect()
}

pub(super) fn continue_subject(
    shell: &Entity<LauncherShell>,
    cx: &mut VisualTestContext,
) -> Option<String> {
    shell.read_with(cx, |shell, _| match shell.home() {
        HomePresentation::Continue { subject, .. } => subject.id.clone(),
        _ => None,
    })
}

pub(super) fn failed_attempts(wiring: &Wiring, id: &str) -> usize {
    let id = id.to_owned();
    let history = ask(
        wiring,
        move |service| async move { service.history(&id).await },
    );
    history
        .map(|read| {
            read.events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        HistoryEvent::Session {
                            outcome: SessionOutcome::FailedToPrepare,
                            ..
                        }
                    )
                })
                .count()
        })
        .unwrap_or(0)
}

/// #55: switching away from, or removing, the current account changes what
/// Home's 需要留意 says, without waiting for a restart.
#[gpui_kit::test]
fn home_attention_follows_the_current_account(cx: &mut TestAppContext) {
    let (shell, wiring, cx) = open(cx);
    let id = game(&wiring, "A", "1.20.1");
    ask(&wiring, |service| async move {
        service.add_account("Steve", None).await
    })
    .unwrap();
    cx.update(|_, cx| follow(&wiring, cx));
    until(cx, "Home's first check", |cx| {
        shell.read_with(cx, |shell, _| shell.live().unwrap().library_loaded)
            && attention(&shell, cx) == attention_now(&wiring)
    });
    assert!(
        !attention(&shell, cx).iter().any(|(game, _)| game == &id),
        "with an account and Java to download, the game needs nothing"
    );

    cx.update(|window, cx| {
        on_live_intent(
            &wiring,
            LiveIntent::RemoveAccount("Steve".into()),
            window,
            cx,
        )
    });
    until(cx, "the account list without Steve", |cx| {
        shell.read_with(cx, |shell, _| {
            let model = shell.live().unwrap();
            model.accounts_loaded && model.accounts.is_empty()
        })
    });
    until(cx, "Home to say the game needs an account", |cx| {
        attention(&shell, cx).iter().any(|(game, _)| game == &id)
    });

    ask(&wiring, |service| async move {
        service.add_account("Alex", None).await
    })
    .unwrap();
    cx.update(|window, cx| {
        on_live_intent(
            &wiring,
            LiveIntent::SelectAccount("Alex".into()),
            window,
            cx,
        )
    });
    until(cx, "Home to drop the account problem", |cx| {
        !attention(&shell, cx).iter().any(|(game, _)| game == &id)
    });
}

/// A change to how Java is found or fetched is a change to every game's
/// Java problem: Home checks again.
#[gpui_kit::test]
fn home_attention_follows_the_java_settings(cx: &mut TestAppContext) {
    let (shell, wiring, cx) = open(cx);
    let id = game(&wiring, "A", "1.20.1");
    ask(&wiring, |service| async move {
        service.add_account("Steve", None).await
    })
    .unwrap();
    cx.update(|_, cx| follow(&wiring, cx));
    until(cx, "Home's first check", |cx| {
        shell.read_with(cx, |shell, _| shell.live().unwrap().library_loaded)
            && attention(&shell, cx) == attention_now(&wiring)
    });
    assert!(!attention(&shell, cx).iter().any(|(game, _)| game == &id));

    // No Java anywhere, and none may be downloaded: the game cannot start.
    cx.update(|window, cx| {
        on_live_intent(&wiring, LiveIntent::SetAutoInstallJava(false), window, cx)
    });
    until(cx, "the setting to be saved", |_| {
        !ask(&wiring, |service| async move {
            service.settings().await.auto_install_java
        })
    });
    until(cx, "Home to say the game has no Java", |cx| {
        attention(&shell, cx).iter().any(|(game, _)| game == &id)
    });
}

/// Browsing for a game from its Content tab makes it the current game
/// (decision D3): the chip, Continue and the saved choice agree, and stay
/// so after going back to Discover from the navigation.
#[gpui_kit::test]
fn browsing_for_a_game_makes_it_the_current_game(cx: &mut TestAppContext) {
    let (shell, wiring, cx) = open(cx);
    let a = game(&wiring, "A", "1.20.1");
    let x = game(&wiring, "X", "1.20.1");
    ask(&wiring, {
        let a = a.clone();
        move |service| async move { service.set_current_instance(&a).await }
    })
    .unwrap();
    cx.update(|_, cx| follow(&wiring, cx));
    until(cx, "Home to continue A", |cx| {
        continue_subject(&shell, cx).as_deref() == Some(a.as_str())
    });

    cx.update(|window, cx| {
        open_instance(
            &wiring,
            x.clone(),
            Some(InstanceIntent::BrowseContent(ProjectKind::Mod)),
            window,
            cx,
        )
    });
    until(cx, "Discover to browse for X", |cx| {
        shell.read_with(cx, |shell, cx| {
            browsing_for(shell, cx).as_deref() == Some(x.as_str())
        })
    });
    until(cx, "X to be the saved current game", |_| {
        saved_current(&wiring).as_deref() == Some(x.as_str())
    });

    shell.update_in(cx, |shell, window, cx| {
        shell.go_to(Route::Discover, window, cx)
    });
    until(
        cx,
        "the chip, Continue and the saved choice to agree on X",
        |cx| {
            let chip = chip(&wiring, cx);
            chip.as_deref() == Some(x.as_str())
                && continue_subject(&shell, cx).as_deref() == Some(x.as_str())
                && saved_current(&wiring).as_deref() == Some(x.as_str())
        },
    );
}

/// Choosing another game in the chip while browsing for one ends that
/// browsing (decision D3): the version and loader locks of the old game
/// must not apply to installs into the new one.
#[gpui_kit::test]
fn choosing_another_game_while_browsing_ends_the_browsing(cx: &mut TestAppContext) {
    let (shell, wiring, cx) = open(cx);
    let x = game(&wiring, "X", "1.20.1");
    let z = game(&wiring, "Z", "1.21.1");
    cx.update(|_, cx| follow(&wiring, cx));
    until(cx, "the library", |cx| {
        shell.read_with(cx, |shell, _| shell.live().unwrap().library.len() == 2)
    });
    cx.update(|window, cx| {
        open_instance(
            &wiring,
            x.clone(),
            Some(InstanceIntent::BrowseContent(ProjectKind::Mod)),
            window,
            cx,
        )
    });
    until(cx, "Discover to browse for X", |cx| {
        shell.read_with(cx, |shell, cx| {
            browsing_for(shell, cx).as_deref() == Some(x.as_str())
        })
    });

    // As choosing it in the corner chip.
    shell.update(cx, |shell, cx| shell.choose_current(z.clone(), cx));
    until(cx, "Z to be the saved current game", |_| {
        saved_current(&wiring).as_deref() == Some(z.as_str())
    });
    // The page follows once core publishes the choice (decision D1).
    until(
        cx,
        "browsing for X to end: its version and loader must not apply to Z",
        |cx| shell.read_with(cx, |shell, cx| browsing_for(shell, cx).is_none()),
    );
    let target = chip(&wiring, cx);
    assert_eq!(target.as_deref(), Some(z.as_str()));
}

/// A game started from its Library card becomes the current game (decision
/// D4); when its launch fails, 恢复并继续 tries that game again, not the
/// one that was current before.
#[gpui_kit::test]
fn recovering_a_failed_launch_retries_the_same_game(cx: &mut TestAppContext) {
    let (shell, wiring, cx) = open(cx);
    // Versions that do not exist: every launch fails while preparing.
    let a = game(&wiring, "A", "lumilio-test-none-a");
    let b = game(&wiring, "B", "lumilio-test-none-b");
    ask(&wiring, {
        let a = a.clone();
        move |service| async move {
            service.add_account("Steve", None).await?;
            service.set_current_instance(&a).await
        }
    })
    .unwrap();
    cx.update(|_, cx| follow(&wiring, cx));
    until(cx, "Home to continue A", |cx| {
        continue_subject(&shell, cx).as_deref() == Some(a.as_str())
    });

    cx.update(|window, cx| play(&wiring, b.clone(), window, cx));
    until(cx, "B's launch to fail", |_| {
        failed_attempts(&wiring, &b) == 1
    });
    until(cx, "the launch to be over", |_| {
        wiring.state.borrow().cancel.is_none()
    });
    until(cx, "B to be the saved current game", |_| {
        saved_current(&wiring).as_deref() == Some(b.as_str())
    });
    until(cx, "the chip to show B", |cx| {
        chip(&wiring, cx) == Some(b.clone())
    });

    cx.update(|window, cx| {
        on_shell_intent(&wiring, ShellIntent::Home(HomeIntent::Recover), window, cx)
    });
    until(cx, "the second attempt", |_| {
        failed_attempts(&wiring, &a) + failed_attempts(&wiring, &b) == 2
    });
    assert_eq!(failed_attempts(&wiring, &a), 0, "恢复并继续 started A");
    assert_eq!(failed_attempts(&wiring, &b), 2);
}

/// Deleting the current game saves the game that replaces it, so the next
/// start and everything showing the current game agree on it.
#[gpui_kit::test]
fn deleting_the_current_game_saves_its_replacement(cx: &mut TestAppContext) {
    let (_shell, wiring, _cx) = open(cx);
    let a = game(&wiring, "A", "1.20.1");
    let b = game(&wiring, "B", "1.20.1");
    let saved = ask(&wiring, {
        let a = a.clone();
        move |service| async move {
            service.set_current_instance(&a).await.unwrap();
            service.delete_instance(&a).await.unwrap();
            service.settings().await.current_instance
        }
    });
    assert_eq!(
        saved.as_deref(),
        Some(b.as_str()),
        "settings.json still names A"
    );
}

/// Home explains a failed launch only while its game is current: choosing
/// another game makes Home continue that one, and the failure stays in
/// 需要留意, which checks the games again once a launch ends.
#[gpui_kit::test]
fn choosing_another_game_leaves_a_failed_launch_to_needs_attention(cx: &mut TestAppContext) {
    let (shell, wiring, cx) = open(cx);
    let a = game(&wiring, "A", "lumilio-test-none-a");
    let b = game(&wiring, "B", "lumilio-test-none-b");
    ask(&wiring, |service| async move {
        service.add_account("Steve", None).await
    })
    .unwrap();
    cx.update(|_, cx| follow(&wiring, cx));
    until(cx, "Home's first check", |cx| {
        shell.read_with(cx, |shell, _| shell.live().unwrap().library_loaded)
            && attention_in_full(&shell, cx) == attention_now_in_full(&wiring)
    });
    let before = attention_in_full(&shell, cx);

    cx.update(|window, cx| play(&wiring, b.clone(), window, cx));
    until(cx, "Home to explain B's failure", |cx| {
        shell.read_with(cx, |shell, _| match shell.home() {
            HomePresentation::Recovery { subject, .. } => subject.id.as_deref() == Some(&b),
            _ => false,
        })
    });
    until(cx, "需要留意 to list B's failed launch", |cx| {
        let now = attention_in_full(&shell, cx);
        now != before && now == attention_now_in_full(&wiring)
    });

    // As choosing it in the corner chip.
    shell.update(cx, |shell, cx| shell.choose_current(a.clone(), cx));
    until(cx, "Home to continue A instead of explaining B", |cx| {
        continue_subject(&shell, cx).as_deref() == Some(a.as_str())
    });
    assert!(
        attention(&shell, cx).iter().any(|(game, _)| game == &b),
        "B's failure left 需要留意"
    );
}

/// Home's 需要留意 rows, with how many more problems each game has.
fn attention_in_full(
    shell: &Entity<LauncherShell>,
    cx: &mut VisualTestContext,
) -> Vec<lumilio_ui::home::AttentionRow> {
    shell.read_with(cx, |shell, _| shell.live().unwrap().attention.clone())
}

fn attention_now_in_full(wiring: &Wiring) -> Vec<lumilio_ui::home::AttentionRow> {
    let (records, summary) = ask(wiring, |service| async move { service.home().await });
    attention_rows(
        &summary.needs_attention,
        &library_cards(&records, lumilio_core::unix_now()),
    )
}
