//! Follows what core publishes about the current game, the current account
//! and which shared state changed, and works out again what depends on it
//! (plan global-context, T4). Handlers only ask core to change something;
//! what the pages show of it arrives here, whoever made the change.

use gpui::App;
use lumilio_core::{Context, HomeChecks, InstanceRecord, Session, unix_now};

use super::Feed;
use super::lists::{installed_changed, refresh_installed};
use crate::home::HomePresentation;
use crate::live::{account_rows, attention_rows, home_places, home_presentation, library_cards};

/// What a change makes out of date: the dependency table of decision D6,
/// as code. A new derived result is added here with its own test.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Stale {
    /// The library cards, the chip, Continue and 接着玩 ← the current game,
    /// the library.
    pub home: bool,
    /// Each game's own checks, read from disk ← the library, Java, plugins,
    /// a launch that ended.
    pub checks: bool,
    /// 需要留意 ← the checks and the account.
    pub attention: bool,
    /// The account list and chip ← the accounts.
    pub accounts: bool,
    /// What Discover's target game has installed ← the current game, the
    /// library.
    pub installed: bool,
    /// Discover's sources and the open game's plugin tabs ← plugins.
    pub plugins: bool,
    /// Whether Home can start a game, the navigation's restart, and the
    /// open studio windows' writes ← the session.
    pub session: bool,
}

/// What `now` makes out of date after `before`; everything when nothing was
/// seen yet.
pub(super) fn stale(before: Option<&Context>, now: &Context) -> Stale {
    let Some(before) = before else {
        return Stale {
            home: true,
            checks: true,
            attention: true,
            accounts: true,
            installed: true,
            plugins: true,
            session: true,
        };
    };
    let (was, is) = (before.revisions, now.revisions);
    let current = before.current_instance != now.current_instance;
    let library = was.library != is.library;
    let accounts = was.accounts != is.accounts || before.current_account != now.current_account;
    // A launch that ended wrote how it went to the game's history.
    let active = |context: &Context| context.session.as_ref().is_some_and(Session::is_active);
    let ended = active(before) && !active(now);
    let checks = library || ended || was.java != is.java || was.plugins != is.plugins;
    Stale {
        home: current || library,
        checks,
        attention: checks || accounts,
        accounts,
        installed: current || library,
        plugins: was.plugins != is.plugins,
        session: before.session != now.session,
    }
}

/// Each game's checks, with the context they were made in.
pub(super) struct Checked {
    records: Vec<InstanceRecord>,
    checks: HomeChecks,
    made_for: Context,
}

/// Starts following core: what it publishes now, then every change. The
/// watch keeps only the newest snapshot, so a burst of changes is followed
/// once.
pub(in crate::shell) fn follow(feed: &Feed, cx: &mut App) {
    let mut changes = feed.worker.service().subscribe_context();
    let feed = feed.clone();
    cx.spawn(async move |cx| {
        let mut seen: Option<Context> = None;
        loop {
            let now = changes.borrow_and_update().clone();
            let what = stale(seen.as_ref(), &now);
            let mirrored = now.clone();
            cx.update(|cx| {
                mirror(&feed, mirrored, cx);
                apply(&feed, what, cx);
                if what.session {
                    session_moved(&feed, seen.as_ref(), &now, cx);
                }
            });
            seen = Some(now);
            if changes.changed().await.is_err() {
                return;
            }
        }
    })
    .detach();
}

/// Writes the interface's copy first, so everything worked out below reads
/// the new choice. Discover ends browsing for a game that is no longer
/// current and searches again without its locks (D3).
fn mirror(feed: &Feed, context: Context, cx: &mut App) {
    let _ = feed
        .shell
        .update(cx, |shell, cx| shell.follow_context(context, cx));
}

fn apply(feed: &Feed, what: Stale, cx: &mut App) {
    if what.installed {
        installed_changed(feed, cx);
    }
    if what.home {
        refresh_home(feed, cx);
    }
    if what.checks {
        recheck(feed, cx);
    } else if what.attention {
        summarize(feed, cx);
    }
    if what.accounts {
        refresh_accounts(feed, cx);
    }
    if what.plugins {
        refresh_plugins(feed, cx);
    }
}

/// The game whose session became active, or stopped being active: its
/// studio windows shut their writes, or list their folder again.
fn session_moved(feed: &Feed, before: Option<&Context>, now: &Context, cx: &mut App) {
    let active = |context: Option<&Context>| {
        context
            .and_then(|context| context.session.as_ref())
            .filter(|session| session.is_active())
            .map(|session| session.instance.clone())
    };
    let (was, is) = (active(before), active(Some(now)));
    if was == is {
        return;
    }
    let Ok(game) = feed.shell.read_with(cx, |shell, _| shell.game().clone()) else {
        return;
    };
    game.update(cx, |game, cx| {
        if let Some(id) = was {
            game.game_changed(&id, false, cx);
        }
        if let Some(id) = is {
            game.game_changed(&id, true, cx);
        }
    });
}

/// Whether the context still says what a piece of work was asked for.
fn still(feed: &Feed, same: impl Fn(&Context) -> bool) -> bool {
    same(&feed.worker.service().context())
}

/// The library cards, the chip, Continue and the worlds and servers of the
/// game Home continues, from saved records.
fn refresh_home(feed: &Feed, cx: &mut App) {
    let asked = feed.worker.service().context();
    let service = feed.worker.service().clone();
    let handle = feed
        .worker
        .spawn(async move { service.home_snapshot().await });
    let feed = feed.clone();
    cx.spawn(async move |cx| {
        let Some((records, summary)) = crate::runtime::answer(handle, cx).await else {
            return;
        };
        // A newer change is followed on its own.
        if !still(&feed, |now| {
            now.current_instance == asked.current_instance
                && now.revisions.library == asked.revisions.library
        }) {
            return;
        }
        let now = unix_now();
        let current = asked.current_instance.as_deref();
        let (home, continued) = home_presentation(&records, &summary, current, now);
        let continued = continued
            .and_then(|id| records.iter().find(|record| record.id == id))
            .cloned();
        let cards = library_cards(&records, now);
        let _ = feed.shell.update(cx, |shell, cx| {
            // A launch in progress keeps Home. A failure is explained only
            // while its game is current: choosing another one leaves it to
            // 需要留意.
            let keep = match shell.home() {
                HomePresentation::Launching { .. } | HomePresentation::Playing { .. } => true,
                HomePresentation::Recovery { subject, .. } => subject.id.as_deref() == current,
                _ => false,
            };
            if !keep {
                shell.set_home(home, cx);
            }
            shell.update_live(
                |model| {
                    model.set_library(cards);
                    let before = model.attention.len();
                    model
                        .attention
                        .retain(|row| records.iter().any(|record| record.id == row.instance));
                    if model.attention.len() != before {
                        model.attention_revision += 1;
                    }
                    model.home_places = None;
                },
                cx,
            );
        });
        cx.update(|cx| {
            if let Some(record) = continued {
                refresh_places(&feed, record, asked.revisions.library, cx);
            }
            refresh_installed(&feed, cx);
        });
    })
    .detach();
}

/// Checks each game again from disk; meanwhile Home says it is checking
/// instead of showing the earlier answer.
fn recheck(feed: &Feed, cx: &mut App) {
    let asked = feed.worker.service().context();
    if let Some(task) = feed.state.borrow_mut().home_diagnostics.take() {
        task.abort();
    }
    let _ = feed.shell.update(cx, |shell, cx| {
        shell.update_live(|model| model.attention_checking = true, cx);
    });
    let service = feed.worker.service().clone();
    let handle = feed
        .worker
        .spawn(async move { service.home_checks().await });
    feed.state.borrow_mut().home_diagnostics = Some(handle.abort_handle());
    let feed = feed.clone();
    cx.spawn(async move |cx| {
        let Some((records, checks)) = crate::runtime::answer(handle, cx).await else {
            return;
        };
        let same = |was: &Context, now: &Context| {
            let (was, now) = (was.revisions, now.revisions);
            was.library == now.library && was.java == now.java && was.plugins == now.plugins
        };
        if !still(&feed, |now| same(&asked, now)) {
            return;
        }
        {
            let mut state = feed.state.borrow_mut();
            state.home_diagnostics = None;
            state.home_checks = Some(Checked {
                records,
                checks,
                made_for: asked,
            });
        }
        cx.update(|cx| summarize(&feed, cx));
    })
    .detach();
}

/// 需要留意 from the checks already made and the account now; reads no disk.
fn summarize(feed: &Feed, cx: &mut App) {
    let Some((records, checks, made_for)) =
        feed.state.borrow().home_checks.as_ref().map(|checked| {
            (
                checked.records.clone(),
                checked.checks.clone(),
                checked.made_for.revisions,
            )
        })
    else {
        // The first checks are still on their way and summarize when done.
        return;
    };
    let accounts = feed.worker.service().context().revisions.accounts;
    let service = feed.worker.service().clone();
    let handle = feed.worker.spawn(async move {
        let summary = service.home_from(&records, &checks).await;
        (records, summary)
    });
    let feed = feed.clone();
    cx.spawn(async move |cx| {
        let Some((records, summary)) = crate::runtime::answer(handle, cx).await else {
            return;
        };
        // Newer checks or another account change summarize again.
        let current_checks = feed
            .state
            .borrow()
            .home_checks
            .as_ref()
            .map(|checked| checked.made_for.revisions);
        if current_checks != Some(made_for)
            || !still(&feed, |now| now.revisions.accounts == accounts)
        {
            return;
        }
        let cards = library_cards(&records, unix_now());
        let rows = attention_rows(&summary.needs_attention, &cards);
        let _ = feed.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    if model.attention != rows || model.attention_checking {
                        model.attention_revision += 1;
                    }
                    model.attention = rows;
                    model.attention_checking = false;
                },
                cx,
            );
        });
    })
    .detach();
}

fn refresh_accounts(feed: &Feed, cx: &mut App) {
    let service = feed.worker.service().clone();
    let handle = feed.worker.spawn(async move { service.settings().await });
    let feed = feed.clone();
    cx.spawn(async move |cx| {
        let Some(settings) = crate::runtime::answer(handle, cx).await else {
            return;
        };
        let rows = account_rows(&settings);
        let _ = feed.shell.update(cx, |shell, cx| {
            shell.update_live(|model| model.set_accounts(rows), cx);
        });
    })
    .detach();
}

/// Discover's sources, once it has shown them, and the open game's plugin
/// tabs.
fn refresh_plugins(feed: &Feed, cx: &mut App) {
    let Ok((discover, game, open_game)) = feed.shell.read_with(cx, |shell, _| {
        (
            shell.discover().clone(),
            shell.game().clone(),
            shell.live_instance().cloned(),
        )
    }) else {
        return;
    };
    // Which content sources are on may have changed.
    discover.update(cx, |discover, cx| discover.sources_changed(cx));
    if let Some(view) = open_game {
        game.update(cx, |game, cx| game.reload_plugin_tabs(&view, cx));
    }
}

/// Reads the worlds and servers of the game Home continues. A game that
/// cannot be read just lists nothing; an answer for a game Home has moved
/// away from meanwhile is dropped.
fn refresh_places(feed: &Feed, record: InstanceRecord, library: u64, cx: &mut App) {
    let service = feed.worker.service().clone();
    let id = record.id.clone();
    let handle = feed.worker.spawn(async move {
        (
            service.worlds(&id).await.unwrap_or_default(),
            service.servers(&id).await.unwrap_or_default(),
        )
    });
    let feed = feed.clone();
    cx.spawn(async move |cx| {
        let Some((worlds, servers)) = crate::runtime::answer(handle, cx).await else {
            return;
        };
        let current = |now: &Context| {
            now.current_instance.as_ref() == Some(&record.id) && now.revisions.library == library
        };
        if !still(&feed, current) {
            return;
        }
        let places = home_places(&record, &worlds, &servers);
        let addresses = places.server_addresses();
        let _ = feed.shell.update(cx, |shell, cx| {
            shell.update_live(|model| model.home_places = Some(places), cx)
        });
        for address in addresses {
            cx.update(|cx| ask_place(&feed, record.id.clone(), address, library, cx));
        }
    })
    .detach();
}

/// Asks one of Home's servers how it is, as the Worlds tab does. An answer
/// for places Home has read again meanwhile is dropped.
fn ask_place(feed: &Feed, id: String, address: String, library: u64, cx: &mut App) {
    let service = feed.worker.service().clone();
    let (instance, asked) = (id.clone(), address.clone());
    let handle = feed
        .worker
        .spawn(async move { service.server_status(&instance, &asked).await });
    let feed = feed.clone();
    cx.spawn(async move |cx| {
        let answer = crate::runtime::answer(handle, cx)
            .await
            .and_then(Result::ok)
            .ok_or(());
        if !still(&feed, |now| now.revisions.library == library) {
            return;
        }
        let _ = feed.shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    if let Some(places) = model
                        .home_places
                        .as_mut()
                        .filter(|places| places.instance == id)
                    {
                        places.server_answered(&address, answer);
                    }
                },
                cx,
            );
        });
    })
    .detach();
}

/// Checks every game again, as if what they depend on had changed: the
/// person asked for it.
pub(in crate::shell) fn check_again(feed: &Feed, cx: &mut App) {
    recheck(feed, cx);
}
