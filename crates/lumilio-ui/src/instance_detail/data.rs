use super::intent::{InstanceIntent, Section};
use super::panels::{Arrived, Confirm};
use super::{
    CrashRead, InstanceDetailView, Operated, TAB_CONTENT, TAB_DIAGNOSTICS, TAB_HISTORY,
    TAB_OVERVIEW, TAB_WORLDS, TABS, panels, settings,
};
use crate::toast::Toast;
use gpui::{Context, Window};
use lumilio_core::{InstanceRecord, LauncherSettings};

impl InstanceDetailView {
    /// A response for a different target never becomes this view's data.
    pub fn loaded(
        &mut self,
        result: Result<(InstanceRecord, LauncherSettings), String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok((record, defaults)) if record.id == self.id => {
                self.record = Some(record);
                self.defaults = defaults;
                self.sync_fields = 3;
                self.error = None;
                self.load_technical = None;
            }
            Ok(_) => return,
            Err(detail) => {
                self.error = Some("没有读到这个游戏，可以重试".into());
                self.load_technical = Some(detail);
            }
        }
        self.busy = false;
        cx.notify();
    }

    pub fn saved(
        &mut self,
        memory: bool,
        result: Result<(InstanceRecord, LauncherSettings), String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(data) => {
                self.loaded(Ok(data), cx);
                self.sync_fields = if memory { 2 } else { 1 };
                self.editor_result(None, cx);
                self.toast(
                    Toast::success(if memory {
                        "内存已保存，下次启动时生效"
                    } else {
                        "名称已保存"
                    }),
                    cx,
                );
            }
            Err(detail) => {
                self.busy = false;
                const FAILED: &str = "没有保存成功，输入已保留，可以重试";
                if !self.editor_result(Some(FAILED.into()), cx) {
                    self.toast(Toast::error(FAILED).technical(detail), cx);
                }
                cx.notify();
            }
        }
    }

    /// What was saved from a Settings dialog (which closes itself).
    pub fn settings_saved(
        &mut self,
        result: Result<(InstanceRecord, LauncherSettings), String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(data) => {
                self.loaded(Ok(data), cx);
                self.toast(Toast::success("已保存，下次启动时生效"), cx);
            }
            Err(detail) => {
                self.busy = false;
                self.toast(Toast::error("没有保存成功，可以重试").technical(detail), cx);
                cx.notify();
            }
        }
    }

    /// What the game is on now: game version, loader, loader version.
    #[must_use]
    pub fn runtime(&self) -> Option<(String, lumilio_core::Loader, Option<String>)> {
        let record = self.record.as_ref()?;
        Some((
            record.game_version.clone(),
            record.loader,
            record.loader_version.clone(),
        ))
    }

    /// Shows one group of the Settings tab (dev review and tests).
    pub fn select_settings_group(&mut self, group: usize, cx: &mut Context<Self>) {
        self.settings_sub = group.min(settings::SUBTABS.len() - 1);
        cx.notify();
    }

    /// The machine's memory, for the hint on the memory rows.
    pub fn set_machine_memory(&mut self, total_mb: Option<u64>, cx: &mut Context<Self>) {
        self.machine_memory_mb = total_mb;
        cx.notify();
    }

    pub fn select_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.tab = tab.min(TABS.len() - 1);
        self.confirm = None;
        cx.notify();
    }

    /// Selecting from the page also loads what the new tab shows.
    pub(super) fn open_tab(&mut self, tab: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.select_tab(tab, cx);
        match self.tab {
            TAB_OVERVIEW => {
                self.ensure(Section::Problems, window, cx);
                self.ensure(Section::Size, window, cx);
                self.ensure(Section::History, window, cx);
            }
            TAB_CONTENT => {
                let kind = panels::CONTENT_KINDS[self.content_kind.min(2)];
                self.ensure(Section::Content(kind), window, cx);
            }
            TAB_WORLDS => {
                self.ensure(Section::Worlds, window, cx);
                if self.worlds_sub == 1 {
                    self.ensure(Section::Servers, window, cx);
                }
            }
            TAB_DIAGNOSTICS => self.open_diagnostics(window, cx),
            TAB_HISTORY => {
                let section = if self.history_sub == 2 {
                    Section::Snapshots
                } else {
                    Section::History
                };
                self.ensure(section, window, cx);
            }
            _ => {}
        }
    }

    /// Loads a section once; a section already loaded or on its way is left alone.
    pub fn ensure(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        if !self.data.has(section) {
            self.request(section, window, cx);
        }
    }

    /// Asks for fresh data, unless that is already under way.
    pub fn request(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        if self.data.pending.contains(&section) {
            return;
        }
        self.data.pending.push(section);
        (self.handler)(InstanceIntent::Load(section), window, cx);
    }

    /// A section's data arrived. A failure is page state: the section shows
    /// it in place, with its own 技术详情 (§11).
    pub fn arrived(&mut self, arrived: Arrived, cx: &mut Context<Self>) {
        if matches!(arrived, Arrived::Servers(Ok(_))) {
            self.ping_servers = true;
        }
        self.data.store(arrived);
        cx.notify();
    }

    /// Shows worlds or servers; the servers are read the first time.
    pub(super) fn open_worlds_sub(
        &mut self,
        sub: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.worlds_sub = sub.min(panels::WORLD_SUBS.len() - 1);
        if self.worlds_sub == 1 {
            if self.data.has(Section::Servers) {
                self.ping_servers = true;
            } else {
                self.ensure(Section::Servers, window, cx);
            }
        }
        cx.notify();
    }

    /// Asks every listed server how it is; the answers come to
    /// [`InstanceDetailView::server_status_arrived`].
    pub(super) fn ping_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Ok(servers)) = &self.data.servers else {
            return;
        };
        let mut addresses: Vec<String> = Vec::new();
        for server in servers {
            if !addresses.contains(&server.address) {
                addresses.push(server.address.clone());
            }
        }
        // A list is short, but a pasted file could be long: ask for the first few.
        addresses.truncate(32);
        for address in addresses {
            self.server_status
                .insert(address.clone(), panels::ServerState::Checking);
            (self.handler)(InstanceIntent::PingServer(address), window, cx);
        }
        cx.notify();
    }

    /// One server answered (or did not).
    pub fn server_status_arrived(
        &mut self,
        address: String,
        result: Result<lumilio_core::ServerStatus, String>,
        cx: &mut Context<Self>,
    ) {
        self.server_status.insert(
            address,
            match result {
                Ok(status) => panels::ServerState::Online(status),
                Err(_) => panels::ServerState::Offline,
            },
        );
        cx.notify();
    }

    /// Tells what just happened, as a toast over the page.
    pub fn toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.toasts.push(toast);
        cx.notify();
    }

    /// Toasts not shown yet (a window without the framework `Root` keeps them).
    pub fn pending_toasts(&self) -> &[Toast] {
        &self.toasts
    }

    /// Asks the question `intent` stands for (copy or delete) as soon as the
    /// page has its game.
    pub fn ask_later(&mut self, intent: InstanceIntent, cx: &mut Context<Self>) {
        self.ask_later = Some(intent);
        cx.notify();
    }

    /// What the game has printed since it started, while it runs. When it
    /// stops, the log file has the whole story, so it is read again.
    pub fn game_output(&mut self, lines: Vec<String>, running: bool, cx: &mut Context<Self>) {
        if running {
            if self.live_output.is_none() {
                self.game_started_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok());
            }
            self.live_output = Some(lines);
            // A world opens some time after the start: look again now and then.
            let due = self
                .last_worlds_refresh
                .is_none_or(|at| at.elapsed() >= std::time::Duration::from_secs(5));
            if self.tab == TAB_WORLDS && due {
                self.last_worlds_refresh = Some(std::time::Instant::now());
                self.refresh_worlds = true;
            }
        } else if self.live_output.take().is_some() {
            self.refresh_logs = true;
            self.game_started_ms = None;
            if self.data.has(Section::Worlds) {
                self.refresh_worlds = true;
            }
        }
        cx.notify();
    }

    /// A crash report arrived (or failed to); shown under the list.
    pub fn crash_arrived(&mut self, file: String, result: CrashRead, cx: &mut Context<Self>) {
        // Only the report last asked for is shown.
        if self.crash.as_ref().map(|crash| &crash.0) == Some(&file) {
            self.crash = Some((file, Some(result)));
            cx.notify();
        }
    }

    /// A write finished: show what happened. Returns what it made stale and
    /// has to be read again; the caller loads those and answers with `arrived`.
    pub fn operated(&mut self, done: Operated, cx: &mut Context<Self>) -> Vec<Section> {
        self.busy = false;
        self.confirm = None;
        let failure = done.technical.is_some().then(|| done.notice.clone());
        if let Some(switch) = self.switch.clone() {
            let message = failure.clone();
            switch.update(cx, |switch, cx| switch.finished(message, cx));
            if failure.is_none() {
                self.switch = None;
            }
            self.toasts.push(match &done.technical {
                Some(detail) => Toast::error(done.notice).technical(detail.clone()),
                None => Toast::success(done.notice),
            });
        } else if !self.editor_result(failure, cx) {
            let toast = match done.technical {
                Some(detail) => Toast::error(done.notice).technical(detail),
                None => Toast::success(done.notice),
            };
            self.toasts.push(toast);
        }
        // Writes feed the change history and can change what is wrong.
        let mut stale = done.refresh;
        stale.push(Section::History);
        stale.push(Section::Problems);
        let mut reload = Vec::new();
        for section in stale {
            // Only what has been looked at is worth re-reading.
            if self.data.has(section)
                && !self.data.pending.contains(&section)
                && !reload.contains(&section)
            {
                self.data.pending.push(section);
                reload.push(section);
            }
        }
        cx.notify();
        reload
    }

    /// Sends a write, once: the buttons are disabled while it runs.
    pub(super) fn send(
        &mut self,
        intent: InstanceIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.confirm = None;
        cx.notify();
        (self.handler)(intent, window, cx);
    }

    /// The second click of a destructive action.
    pub(super) fn confirmed(
        &mut self,
        what: &Confirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let intent = match what.clone() {
            Confirm::DeleteWorld(folder) => InstanceIntent::DeleteWorld(folder),
            Confirm::DeleteServer { index, entry } => InstanceIntent::DeleteServer {
                index,
                expected: entry,
            },
            Confirm::DeleteSnapshot(id) => InstanceIntent::DeleteSnapshot(id),
            Confirm::RestoreSnapshot(id) => InstanceIntent::RestoreSnapshot(id),
        };
        self.send(intent, window, cx);
    }
}
