//! What the whole launcher is pointed at — the current game and the current
//! account — and which kinds of shared state have changed, published to
//! whoever shows them (plan global-context, D1, D5).
//!
//! The settings and the library each publish from their one save, so no
//! write can forget to. A subscriber keeps the revisions it last saw and reads
//! again what grew; a watch channel keeps only the newest snapshot, so a slow
//! subscriber never acts on an older one after a newer one.

use std::sync::Arc;

use tokio::sync::watch;

use crate::instances::InstanceRecord;
use crate::settings::LauncherSettings;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Context {
    /// The game the launcher plays and installs into.
    pub current_instance: Option<String>,
    /// The key of the account games start with.
    pub current_account: Option<String>,
    /// The launch on its way, the game running, or the launch that failed
    /// last; `None` once a game ended normally or was stopped.
    pub session: Option<Session>,
    pub revisions: Revisions,
}

/// One launch of one game (plan global-context, T6).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub instance: String,
    pub phase: SessionPhase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionPhase {
    /// Checking, downloading and starting, before the game's process exists.
    Preparing,
    /// The game's process runs.
    Playing,
    /// The launch failed or the game crashed: recovering retries this game.
    Failed,
}

impl Session {
    /// Whether a launch is on its way or a game runs.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.phase != SessionPhase::Failed
    }
}

/// How many times each kind of state has changed. Each only grows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Revisions {
    /// Games and collections.
    pub library: u64,
    /// Accounts, the selected one and authentication servers.
    pub accounts: u64,
    /// Where Java is found, which is off, where it is downloaded from,
    /// whether it is, and the runtimes the launcher installed.
    pub java: u64,
    /// Which plugins are on, and their settings.
    pub plugins: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Change {
    Library,
    Java,
}

/// The sending side, shared by the stores that publish.
#[derive(Clone, Debug)]
pub(crate) struct ContextHub(Arc<watch::Sender<Context>>);

impl ContextHub {
    pub(crate) fn new(settings: &LauncherSettings) -> Self {
        let (sender, _) = watch::channel(Context {
            current_instance: settings.current_instance.clone(),
            current_account: settings.selected_account.clone(),
            session: None,
            revisions: Revisions::default(),
        });
        Self(Arc::new(sender))
    }

    pub(crate) fn get(&self) -> Context {
        self.0.borrow().clone()
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<Context> {
        self.0.subscribe()
    }

    pub(crate) fn bump(&self, change: Change) {
        self.0.send_modify(|context| match change {
            Change::Library => context.revisions.library += 1,
            Change::Java => context.revisions.java += 1,
        });
    }

    /// Publishes the session of `instance`: `None` ends it, unless another
    /// game's session has replaced it meanwhile.
    pub(crate) fn session(&self, instance: &str, phase: Option<SessionPhase>) {
        self.0.send_if_modified(|context| {
            let next = phase.map(|phase| Session {
                instance: instance.to_owned(),
                phase,
            });
            let theirs = context
                .session
                .as_ref()
                .is_some_and(|session| session.instance != instance);
            if (next.is_none() && theirs) || context.session == next {
                return false;
            }
            context.session = next;
            true
        });
    }

    /// Publishes what a settings save changed, if anything shown depends on it.
    pub(crate) fn settings_saved(&self, before: &LauncherSettings, after: &LauncherSettings) {
        self.0.send_if_modified(|context| {
            let mut changed = false;
            if context.current_instance != after.current_instance {
                context.current_instance.clone_from(&after.current_instance);
                changed = true;
            }
            if before.accounts != after.accounts
                || before.selected_account != after.selected_account
                || before.auth_servers != after.auth_servers
            {
                context.current_account.clone_from(&after.selected_account);
                context.revisions.accounts += 1;
                changed = true;
            }
            if before.extra_java_roots != after.extra_java_roots
                || before.disabled_java != after.disabled_java
                || before.java_source != after.java_source
                || before.auto_install_java != after.auto_install_java
            {
                context.revisions.java += 1;
                changed = true;
            }
            if before.plugins != after.plugins {
                context.revisions.plugins += 1;
                changed = true;
            }
            changed
        });
    }
}

/// The game that becomes current when there is none, or the current one is
/// gone: the one played last, else the newest.
pub(crate) fn fallback_instance(records: &[InstanceRecord]) -> Option<&str> {
    records
        .iter()
        .max_by(|a, b| {
            (a.last_played, a.created_at)
                .cmp(&(b.last_played, b.created_at))
                .then_with(|| b.id.cmp(&a.id))
        })
        .map(|record| record.id.as_str())
}
