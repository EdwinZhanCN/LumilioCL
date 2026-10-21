//! The dependency table of plan global-context (D6): which change makes
//! which derived result out of date. One test per derived result.

use super::super::context::{Stale, stale};
use lumilio_core::{Context, Session, SessionPhase};

/// Each kind of change core publishes, by name.
fn changes() -> Vec<(&'static str, Context)> {
    let base = Context::default();
    let mut current = base.clone();
    current.current_instance = Some("other".into());
    let mut account = base.clone();
    account.current_account = Some("Alex".into());
    let mut session = base.clone();
    session.session = Some(Session {
        instance: "other".into(),
        phase: SessionPhase::Preparing,
    });
    let with = |bump: fn(&mut Context)| {
        let mut next = base.clone();
        bump(&mut next);
        next
    };
    vec![
        ("current game", current),
        ("current account", account),
        ("library", with(|c| c.revisions.library += 1)),
        ("accounts", with(|c| c.revisions.accounts += 1)),
        ("java", with(|c| c.revisions.java += 1)),
        ("plugins", with(|c| c.revisions.plugins += 1)),
        ("session", session),
    ]
}

/// The changes that make `derived` out of date.
fn made_stale_by(derived: fn(Stale) -> bool) -> Vec<&'static str> {
    let base = Context::default();
    changes()
        .into_iter()
        .filter(|(_, now)| derived(stale(Some(&base), now)))
        .map(|(name, _)| name)
        .collect()
}

#[test]
fn nothing_seen_yet_makes_everything_out_of_date() {
    let all = stale(None, &Context::default());
    assert!(all.home && all.checks && all.attention && all.accounts);
    assert!(all.installed && all.plugins && all.session);
    assert_eq!(
        stale(Some(&Context::default()), &Context::default()),
        Stale::default()
    );
}

#[test]
fn the_chip_continue_and_continue_playing_follow_the_game_and_the_library() {
    assert_eq!(made_stale_by(|s| s.home), ["current game", "library"]);
}

#[test]
fn each_games_checks_follow_the_library_java_and_plugins() {
    assert_eq!(made_stale_by(|s| s.checks), ["library", "java", "plugins"]);
}

#[test]
fn needs_attention_follows_the_checks_and_the_account() {
    assert_eq!(
        made_stale_by(|s| s.attention),
        ["current account", "library", "accounts", "java", "plugins"]
    );
}

#[test]
fn the_account_list_follows_the_accounts() {
    assert_eq!(
        made_stale_by(|s| s.accounts),
        ["current account", "accounts"]
    );
}

#[test]
fn what_discovers_target_has_installed_follows_the_game_and_the_library() {
    assert_eq!(made_stale_by(|s| s.installed), ["current game", "library"]);
}

#[test]
fn sources_and_plugin_tabs_follow_the_plugins() {
    assert_eq!(made_stale_by(|s| s.plugins), ["plugins"]);
}

#[test]
fn starting_games_restart_and_studio_writes_follow_the_session() {
    assert_eq!(made_stale_by(|s| s.session), ["session"]);
}
