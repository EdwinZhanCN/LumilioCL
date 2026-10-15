//! The schematic studio (plan schematic-studio, T9, T20; D6, D22): the host's
//! surface for a plugin's `View::Studio { folder }`. It has two placements
//! of one view:
//!
//! - In the game page's plugin pane ([`Placement::Pane`]) it is the folder's
//!   tree beside a summary of the chosen file, with one key that opens the
//!   studio window. No 3D viewport and no operations live there.
//! - In a window of its own ([`Placement::Window`], [`StudioWindow`]) it is
//!   the workbench (D27): a head with the file switcher and the operations,
//!   the resident 3D viewport, and the materials column. A tile chooses a
//!   block kind; a click in the picture inspects one block (D31).
//!
//! The studio never touches the disk. It asks the application through a
//! [`Connection`] (list, read, write, move to backup, history, restore) and
//! computes statistics, fingerprints and edited bytes with
//! `lumilio-schematic` on the background executor (T13–T16).

mod dialogs;
mod export;
mod inspect_card;
mod inspector;
mod materials;
mod ops;
mod pane;
mod sources;
mod switcher;
#[cfg(test)]
mod tests;
mod thumbnails;
mod tree;
mod versions;
mod viewport;
mod window;
mod words;
mod writes;

pub use ops::StudioAction;
pub use thumbnails::{THUMBNAIL_SIZE, render_thumbnail};
pub use window::StudioWindow;

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    App, Context, Entity, Pixels, RenderImage, ScrollHandle, Task, WeakEntity, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::{WindowExt as _, h_flex, v_flex};
use lumilio_core::{BlockCatalog, BlockNames, SchematicLogEntry, SchematicOperation};
use lumilio_plugin_api::FileInfo;
use lumilio_schematic::fingerprint::Fingerprint;
use lumilio_schematic::metadata::Info;
use lumilio_schematic::stats::SchematicStats;

use crate::model_view::Viewer;
use crate::theme::ShellColors;
use crate::toast::Toast;
use crate::tr;

/// What the studio asks the application for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// The files below the studio folder, game-relative.
    List,
    /// One file's bytes.
    Read { file: String },
    /// The instance's schematic operation log, newest first.
    History,
    /// Whether a write could start now (no game running, nothing else holding
    /// the instance).
    CanWrite,
    /// Put back the file as it was before log line `index`.
    Restore { index: usize },
    /// Replace `file` with edited bytes if it still is what `expected` says;
    /// without `expected`, write `file` new, refused if one is there (T23).
    Apply {
        file: String,
        bytes: Vec<u8>,
        expected: Option<FileInfo>,
        operation: SchematicOperation,
    },
    /// Move files into the launcher's backups, each if it is still what was
    /// read: deleting and "keep only this" never delete (D9).
    Remove {
        files: Vec<(String, FileInfo)>,
        operation: SchematicOperation,
    },
    /// The game versions a schematic can be converted to, newest first.
    Versions,
    /// The instance's game version (T23).
    Game,
    /// The saved replacement tables and recent targets (T24).
    Schemes,
    /// Keep these as the saved tables and recent targets.
    SaveSchemes(lumilio_core::ReplaceSchemes),
    /// The block states bundle version `target` knows, and those of the
    /// version nearest `source_data_version`: what a backward conversion is
    /// checked against (T14).
    Catalogs {
        target: String,
        source_data_version: u32,
    },
    /// Add the resource-pack zip at `path` to the instance's studio sources;
    /// only the path is kept (T17).
    AddPack { path: std::path::PathBuf },
    /// Take an added pack off the sources; the file stays.
    RemovePack { path: std::path::PathBuf },
    /// Switch the mod or resource pack at `path` on or off for this studio
    /// folder; every one is off until switched on (T21, D23).
    SetSource { path: std::path::PathBuf, on: bool },
    /// The game's names for blocks, items and entities (T26).
    Names,
    /// `file`'s thumbnail, kept under `key` (its content fingerprint) once
    /// drawn (T28).
    Thumbnail { file: String, key: String },
}

/// Why a write did not happen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteFailure {
    /// The game or another operation holds the instance.
    Busy,
    /// The file changed since the studio read it.
    Conflict,
    Other(String),
}

/// What the application answers.
#[derive(Debug)]
pub enum Event {
    Listed(Result<Vec<String>, String>),
    /// A file's bytes and what a write of it must present.
    Read {
        file: String,
        result: Result<(Vec<u8>, FileInfo), String>,
    },
    History(Result<Vec<SchematicLogEntry>, String>),
    CanWrite(bool),
    Restored(Result<SchematicLogEntry, String>),
    Applied {
        file: String,
        result: Result<SchematicLogEntry, WriteFailure>,
    },
    Removed(Result<Vec<SchematicLogEntry>, WriteFailure>),
    /// Game versions with their data versions, newest first.
    Versions(Result<Vec<(String, u32)>, String>),
    /// The instance's game version (`26.3`), [`Command::Game`]'s answer.
    Game(Result<String, String>),
    /// The saved replacement tables, as read or as just kept.
    Schemes(Result<lumilio_core::ReplaceSchemes, String>),
    /// The catalogs [`Command::Catalogs`] asked for.
    Catalogs {
        target: String,
        result: Result<(BlockCatalog, Option<BlockCatalog>), String>,
    },
    /// An added pack was registered or taken off.
    PacksChanged(Result<(), String>),
    /// The switch [`Command::SetSource`] asked for was kept, or why not.
    SourceSwitched {
        path: std::path::PathBuf,
        on: bool,
        result: Result<(), String>,
    },
    /// The names [`Command::Names`] asked for; empty when there are none.
    Names(Result<BlockNames, String>),
    /// A thumbnail as PNG bytes, or why there is none.
    Thumbnail {
        file: String,
        result: Result<Vec<u8>, String>,
    },
}

/// The studio's line to the application, made per studio.
pub struct Connection {
    pub send: async_channel::Sender<Command>,
    pub receive: async_channel::Receiver<Event>,
}

/// Asks for a [`Connection`]; the argument is the studio's entity ID, answered
/// through [`StudioView::connect`].
pub type Load = Rc<dyn Fn(u64, &mut Window, &mut App)>;
/// Asks for a viewport's assets: the file and the viewer's request ID,
/// answered through [`StudioView::model_arrived`].
pub type LoadModel = Rc<dyn Fn(String, u64, &mut Window, &mut App)>;
/// Hands an action that is not the studio's own to the page.
pub type Act = Rc<dyn Fn(StudioAction, &mut Window, &mut App)>;
/// Opens (or brings forward) the studio window for this folder, on the
/// given file when there is one (D22).
pub type OpenWindow = Rc<dyn Fn(Option<String>, &mut Window, &mut App)>;

/// Where the studio is drawn, which decides what it draws (D22).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placement {
    /// The game page's plugin pane: the tree, a summary and the key that
    /// opens the window. No viewport, no operations.
    Pane,
    /// Its own window: tree, viewport and inspector, with More.
    Window,
}

/// What one read of a file gave.
#[derive(Debug)]
pub(crate) struct Analysis {
    pub(crate) stats: SchematicStats,
    pub(crate) info: Info,
}

#[derive(Debug, Default)]
pub(crate) enum Inspection {
    /// No file chosen.
    #[default]
    Empty,
    Loading,
    Ready(Box<Analysis>),
    Failed(String),
}

#[derive(Debug, Default)]
pub(crate) enum Loaded<T> {
    #[default]
    Loading,
    Ready(T),
    Failed(String),
}

/// The materials column's port tabs, in order (D27).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InspectorTab {
    /// The block tiles and the chosen block's detail.
    Materials,
    /// Metadata, entities, duplicates and the folded technical facts.
    Info,
    History,
}

impl InspectorTab {
    pub(crate) const ALL: [Self; 3] = [Self::Materials, Self::Info, Self::History];
}

pub fn tab_labels() -> &'static [&'static str] {
    crate::tr_all![
        "studio-tab-materials",
        "studio-tab-info",
        "studio-tab-history"
    ]
}

pub struct StudioView {
    this: WeakEntity<Self>,
    plugin: String,
    folder: String,
    placement: Placement,
    load: Load,
    load_model: LoadModel,
    act: Act,
    /// How the pane opens the window; a studio in its window has none.
    open_window: Option<OpenWindow>,
    /// A file to choose as soon as the listing names it (the window opened
    /// on it from the pane).
    open_on_list: Option<String>,
    asked: bool,
    connection: Option<async_channel::Sender<Command>>,
    listener: Option<Task<()>>,
    listing: Loaded<()>,
    tree: tree::Tree,
    selected: Option<String>,
    inspection: Inspection,
    viewer: Option<Entity<Viewer>>,
    tab: InspectorTab,
    history: Loaded<Vec<SchematicLogEntry>>,
    /// `false` while the game runs or another operation holds the instance.
    can_write: bool,
    restoring: bool,
    search: Option<Entity<InputState>>,
    block_search: Option<Entity<InputState>>,
    /// The block kind chosen in the materials, by id: its tile is ringed and
    /// its detail shows (D27). A click in the picture inspects one block
    /// instead (D31); its card's 选中这种 chooses its kind.
    chosen: Option<String>,
    /// The tile under the pointer: its kind keeps its colour in the
    /// picture while the rest fade (T31).
    tile_hover: Option<String>,
    /// 只看这种: the picture shows the chosen kind alone (T31).
    isolate: bool,
    /// A kind chosen before the open file was read.
    pending_block: Option<String>,
    /// A block to inspect once the open file has a viewport (review hook).
    pending_inspect: Option<[i32; 3]>,
    /// A dialog to open once the open file is read (review hook).
    pending_dialog: Option<ops::ReviewDialog>,
    /// Each file's thumbnail, by path (T28).
    thumbnails: HashMap<String, Arc<RenderImage>>,
    /// Files whose thumbnails are still to be asked for, one at a time.
    thumb_queue: VecDeque<String>,
    thumb_asking: Option<String>,
    /// The file being drawn when the sources changed: its answer was drawn
    /// with the old ones, so it is asked for again (T21).
    thumb_redo: Option<String>,
    /// Files that have no thumbnail (no assets, or not drawable).
    thumb_failed: HashSet<String>,
    /// What each file's own metadata says, for the catalogue's cards.
    summaries: HashMap<String, Info>,
    /// Each block kind's icon for the open file, by id (T27).
    icons: HashMap<String, Arc<RenderImage>>,
    /// The open file's icons were asked of its viewport.
    icons_asked: bool,
    /// Item pictures for the inspect card's slots, by item id (T32).
    item_icons: HashMap<String, Arc<RenderImage>>,
    item_icons_asked: HashSet<String>,
    /// Icons of a file no longer open, dropped from the window's cache on
    /// the next draw.
    old_icons: Vec<Arc<RenderImage>>,
    /// 技术信息 under 信息 is unfolded.
    technical_open: bool,
    /// The language the search fields' placeholders were set in: they are
    /// kept in the inputs, so a switch sets them again.
    inputs_language: u32,
    /// Files whose fingerprints are still to be computed, one read at a time.
    scan: VecDeque<String>,
    scanning: Option<String>,
    catalogue_scroll: ScrollHandle,
    inspector_scroll: ScrollHandle,
    /// The inspect card's, over the picture (T32).
    inspect_scroll: ScrollHandle,
    toasts: Vec<Toast>,
    /// The chosen file as last read: what an operation edits.
    opened: Option<writes::Opened>,
    /// What each file looked like when it was last read; a move to the
    /// backups presents it.
    infos: HashMap<String, FileInfo>,
    /// A write or move was sent and is not answered yet.
    writing: bool,
    pending: Option<writes::Pending>,
    /// The operation dialog to close on the next draw.
    close_dialog: bool,
    dialog: Option<dialogs::OpenDialog>,
    /// The convert targets; `None` until first asked for.
    versions: Option<Loaded<Vec<(String, u32)>>>,
    /// The instance's game version; `None` until first asked for (T23).
    game: Option<Loaded<String>>,
    /// The open file's blocks against the game version (T23).
    compat: versions::Compat,
    /// 材料 lists only the blocks the game version lacks.
    only_incompatible: bool,
    /// The saved replacement tables and recent targets (T24).
    schemes: lumilio_core::ReplaceSchemes,
    /// The version whose block table the replace dialog was promised.
    replace_catalog: Option<String>,
    /// The open 导出 PNG dialog (T10).
    #[cfg_attr(not(test), allow(dead_code))]
    export: Option<WeakEntity<export::ExportDialog>>,
    /// The sources the viewport's assets came from and what each provides,
    /// for 概览; `None` until they arrive (T17).
    sources: Option<Vec<lumilio_core::ProvidedSource>>,
    /// The game's names for blocks and entities; empty until they arrive,
    /// and when there are none (T26).
    names: Rc<BlockNames>,
}

impl StudioView {
    pub fn new(
        plugin: String,
        folder: String,
        placement: Placement,
        load: Load,
        load_model: LoadModel,
        act: Act,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            this: cx.weak_entity(),
            plugin,
            folder,
            placement,
            load,
            load_model,
            act,
            open_window: None,
            open_on_list: None,
            asked: false,
            connection: None,
            listener: None,
            listing: Loaded::Loading,
            tree: tree::Tree::default(),
            selected: None,
            inspection: Inspection::Empty,
            viewer: None,
            tab: InspectorTab::Materials,
            history: Loaded::Loading,
            can_write: false,
            restoring: false,
            search: None,
            block_search: None,
            chosen: None,
            tile_hover: None,
            isolate: false,
            pending_block: None,
            pending_inspect: None,
            pending_dialog: None,
            thumbnails: HashMap::new(),
            thumb_queue: VecDeque::new(),
            thumb_asking: None,
            thumb_redo: None,
            thumb_failed: HashSet::new(),
            summaries: HashMap::new(),
            icons: HashMap::new(),
            icons_asked: false,
            item_icons: HashMap::new(),
            item_icons_asked: HashSet::new(),
            old_icons: Vec::new(),
            technical_open: false,
            inputs_language: crate::i18n::generation(),
            scan: VecDeque::new(),
            scanning: None,
            catalogue_scroll: ScrollHandle::new(),
            inspector_scroll: ScrollHandle::new(),
            inspect_scroll: ScrollHandle::new(),
            toasts: Vec::new(),
            opened: None,
            infos: HashMap::new(),
            writing: false,
            pending: None,
            close_dialog: false,
            dialog: None,
            versions: None,
            game: None,
            compat: versions::Compat::Unknown,
            only_incompatible: false,
            schemes: lumilio_core::ReplaceSchemes::default(),
            replace_catalog: None,
            export: None,
            sources: None,
            names: Rc::default(),
        }
    }

    pub fn folder(&self) -> &str {
        &self.folder
    }

    pub fn plugin(&self) -> &str {
        &self.plugin
    }

    pub fn placement(&self) -> Placement {
        self.placement
    }

    /// Offers the pane the key (and double-click) that opens the window.
    pub fn on_open_window(mut self, open: OpenWindow) -> Self {
        self.open_window = Some(open);
        self
    }

    /// The file the pane shows, for the window to open on.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Chooses `file` now if the listing has it, otherwise as soon as the
    /// listing arrives: the window opened on it from the pane.
    pub fn select_file(&mut self, file: String, cx: &mut Context<Self>) {
        if self.tree.files().contains(&file) {
            self.open_on_list = None;
            if self.selected.as_deref() != Some(file.as_str()) {
                self.open_file(file, cx);
            }
        } else {
            self.open_on_list = Some(file);
        }
        cx.notify();
    }

    /// Shows a toast in this studio's window.
    pub fn toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.toasts.push(toast);
        cx.notify();
    }

    /// Opens the window on the chosen file (the pane's key and double-click).
    pub(crate) fn open_in_window(&self, window: &mut Window, cx: &mut App) {
        if let Some(open) = &self.open_window {
            open(self.selected.clone(), window, cx);
        }
    }

    /// The application's answer to [`Load`]: start listing.
    pub fn connect(&mut self, connection: Connection, cx: &mut Context<Self>) {
        let receive = connection.receive;
        self.connection = Some(connection.send);
        self.listener = Some(cx.spawn(async move |this, cx| {
            while let Ok(event) = receive.recv().await {
                if this.update(cx, |this, cx| this.event(event, cx)).is_err() {
                    break;
                }
            }
        }));
        self.send(Command::List);
        self.send(Command::History);
        self.send(Command::CanWrite);
        if self.placement == Placement::Window {
            self.send(Command::Names);
            self.send(Command::Schemes);
        }
        cx.notify();
    }

    fn send(&self, command: Command) {
        if let Some(send) = &self.connection {
            let _ = send.try_send(command);
        }
    }

    /// The request ID of the viewport's asset load, if a file is open.
    pub fn viewport_request(&self) -> Option<u64> {
        self.viewer
            .as_ref()
            .map(|viewer| viewer.entity_id().as_u64())
    }

    /// The viewport's assets arrived; late answers for a replaced viewport
    /// are dropped. Returns whether this studio asked.
    pub fn model_arrived(
        &mut self,
        request: u64,
        result: Result<lumilio_core::ModelPreview, String>,
        cx: &mut Context<Self>,
    ) -> bool {
        match self.viewer.clone() {
            Some(viewer) if viewer.entity_id().as_u64() == request => {
                self.sources = result
                    .as_ref()
                    .ok()
                    .map(|preview| preview.plan.provided.clone());
                viewer.update(cx, |viewer, cx| viewer.assets(result, cx));
                self.want_icons(cx);
                true
            }
            _ => false,
        }
    }

    /// The game started: until the application answers, nothing may write.
    pub fn write_access_changed(&mut self, cx: &mut Context<Self>) {
        self.can_write = false;
        self.send(Command::CanWrite);
        cx.notify();
    }

    /// The game ended and may have saved schematics: list again, and write
    /// actions follow whether the instance is free.
    pub fn files_changed(&mut self, cx: &mut Context<Self>) {
        self.send(Command::CanWrite);
        self.send(Command::List);
        cx.notify();
    }

    pub(crate) fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Listed(Ok(files)) => {
                self.tree.set_files(&self.folder, files);
                self.listing = Loaded::Ready(());
                if let Some(selected) = &self.selected
                    && !self.tree.files().contains(selected)
                {
                    self.selected = None;
                    self.viewer = None;
                    self.inspection = Inspection::Empty;
                    self.opened = None;
                }
                let files = self.tree.files().to_vec();
                self.infos.retain(|file, _| files.contains(file));
                self.scan = self
                    .tree
                    .files()
                    .iter()
                    .filter(|file| !self.tree.has_fingerprint(file))
                    .cloned()
                    .collect();
                if let Some(file) = self.open_on_list.take()
                    && files.contains(&file)
                {
                    self.select_file(file, cx);
                }
                self.scan_next();
            }
            Event::Listed(Err(detail)) => self.listing = Loaded::Failed(detail),
            Event::Read { file, result } => self.read_arrived(file, result, cx),
            Event::History(result) => {
                self.history = match result {
                    Ok(entries) => Loaded::Ready(entries),
                    Err(detail) => Loaded::Failed(detail),
                };
            }
            Event::CanWrite(can) => self.can_write = can,
            Event::Applied { file, result } => self.applied(file, result, cx),
            Event::Removed(result) => self.removed(result),
            Event::Versions(result) => self.versions_arrived(result, cx),
            Event::Schemes(result) => self.schemes_arrived(result, cx),
            Event::Game(result) => {
                self.game = Some(match result {
                    Ok(version) => Loaded::Ready(version),
                    Err(detail) => Loaded::Failed(detail),
                });
                self.game_to_dialog(cx);
                self.want_compat(cx);
            }
            Event::Catalogs { target, result } => self.catalogs_arrived(target, result, cx),
            Event::PacksChanged(result) => self.packs_changed(result, cx),
            Event::SourceSwitched { path, on, result } => {
                self.source_switched(&path, on, result, cx);
            }
            // Without names the studio writes IDs; nothing to report.
            Event::Names(Ok(names)) => self.names = Rc::new(names),
            Event::Names(Err(_)) => {}
            Event::Thumbnail { file, result } => self.thumbnail_arrived(file, result, cx),
            Event::Restored(result) => {
                self.restoring = false;
                match result {
                    Ok(entry) => {
                        self.toasts.push(Toast::success(tr!(
                            "studio-restored",
                            file = tree::stem(&entry.file)
                        )));
                        self.reread(&entry.file, cx);
                        self.send(Command::List);
                    }
                    Err(detail) => self
                        .toasts
                        .push(Toast::error(tr!("studio-restore-failed")).technical(detail)),
                }
                self.send(Command::History);
                self.send(Command::CanWrite);
            }
        }
        cx.notify();
    }

    fn scan_next(&mut self) {
        if self.scanning.is_some() {
            return;
        }
        while let Some(file) = self.scan.pop_front() {
            if !self.tree.has_fingerprint(&file) {
                self.send(Command::Read { file: file.clone() });
                self.scanning = Some(file);
                return;
            }
        }
    }

    fn read_arrived(
        &mut self,
        file: String,
        result: Result<(Vec<u8>, FileInfo), String>,
        cx: &mut Context<Self>,
    ) {
        let inspect = self.selected.as_deref() == Some(file.as_str())
            && matches!(self.inspection, Inspection::Loading);
        if self.scanning.as_deref() == Some(file.as_str()) {
            self.scanning = None;
        }
        let bytes = match result {
            Ok((bytes, info)) => {
                self.infos.insert(file.clone(), info.clone());
                let bytes = Arc::new(bytes);
                if inspect {
                    self.opened = Some(writes::Opened {
                        file: file.clone(),
                        bytes: bytes.clone(),
                        info,
                    });
                }
                bytes
            }
            Err(detail) => {
                if inspect {
                    self.inspection = Inspection::Failed(detail);
                }
                self.scan_next();
                return;
            }
        };
        let work = cx
            .background_executor()
            .spawn(async move { analyze(&bytes, inspect) });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((print, info, stats)) => {
                        // A write forgets the fingerprint, so an unknown
                        // one counts as a change too.
                        let changed = this.tree.fingerprint(&file) != Some(print);
                        this.tree.set_fingerprint(&file, print);
                        this.summaries.insert(file.clone(), info.clone());
                        this.want_thumbnail(&file, changed);
                        if let Some(stats) = stats
                            && this.selected.as_deref() == Some(file.as_str())
                        {
                            this.inspection = Inspection::Ready(Box::new(Analysis { stats, info }));
                            this.want_icons(cx);
                            this.want_compat(cx);
                            if let Some(id) = this.pending_block.take() {
                                this.choose_block(&id, cx);
                            }
                        }
                    }
                    Err(detail) => {
                        if inspect && this.selected.as_deref() == Some(file.as_str()) {
                            this.inspection = Inspection::Failed(detail);
                        }
                    }
                }
                this.scan_next();
                cx.notify();
            });
        })
        .detach();
    }

    /// Choose a file: the old viewport goes (its worker and frames with it),
    /// a new one starts, and the inspector reads the file.
    pub(crate) fn select(&mut self, file: String, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected.as_deref() == Some(file.as_str()) {
            return;
        }
        self.open_file(file, cx);
    }

    pub(super) fn open_file(&mut self, file: String, cx: &mut Context<Self>) {
        self.chosen = None;
        self.tile_hover = None;
        self.isolate = false;
        self.old_icons
            .extend(self.icons.drain().map(|(_, icon)| icon));
        self.icons_asked = false;
        // Another file may be drawn from another game version's pack.
        self.old_icons
            .extend(self.item_icons.drain().map(|(_, icon)| icon));
        self.item_icons_asked.clear();
        self.opened = None;
        // The pane has no viewport: the model is drawn in the window (D22).
        self.viewer = (self.placement == Placement::Window)
            .then(|| viewport::viewer(&file, self.load_model.clone(), cx));
        if let (Some(viewer), Some(position)) = (self.viewer.clone(), self.pending_inspect.take()) {
            viewer.update(cx, |viewer, cx| viewer.inspect_cell(position, cx));
        }
        self.sources = None;
        self.inspection = Inspection::Loading;
        self.send(Command::Read { file: file.clone() });
        self.selected = Some(file);
        cx.notify();
    }

    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.is_some() {
            return;
        }
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("studio-search-placeholder")));
        let blocks =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("studio-blocks-placeholder")));
        for input in [&search, &blocks] {
            cx.subscribe_in(input, window, |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
        }
        self.search = Some(search);
        self.block_search = Some(blocks);
        self.inputs_language = crate::i18n::generation();
    }

    /// Sets the search fields' placeholders again after a language switch.
    fn relabel_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.inputs_language == crate::i18n::generation() {
            return;
        }
        self.inputs_language = crate::i18n::generation();
        for (input, text) in [
            (&self.search, tr!("studio-search-placeholder")),
            (&self.block_search, tr!("studio-blocks-placeholder")),
        ] {
            if let Some(input) = input {
                input.update(cx, |input, cx| input.set_placeholder(text, window, cx));
            }
        }
    }
}

/// The materials column's width in a studio window this wide, less its
/// padding: a share within bounds that keeps four tiles a row, while the
/// viewport keeps the rest (D27). At the 720 px minimum the viewport still
/// has about 400 px.
pub(crate) fn materials_width(window_width: f32) -> Pixels {
    let inner = (window_width - 2. * window::PADDING).max(0.);
    px((inner * 0.32).clamp(280., 400.))
}

pub(crate) fn muted_line(text: impl Into<gpui::SharedString>, colors: ShellColors) -> gpui::Div {
    div()
        .py_2()
        .text_sm()
        .text_color(colors.muted)
        .child(text.into())
}

/// What one parse of a file gave: always its fingerprint and metadata (the
/// catalogue's cards), the statistics only for the file the studio shows.
type Parsed = (Fingerprint, Info, Option<SchematicStats>);

fn analyze(bytes: &[u8], inspect: bool) -> Result<Parsed, String> {
    let schematic = lumilio_schematic::load_litematic(bytes).map_err(|error| error.to_string())?;
    let print = lumilio_schematic::fingerprint::fingerprint(&schematic);
    let info = lumilio_schematic::metadata::read(&schematic);
    let stats = inspect.then(|| lumilio_schematic::stats::stats(&schematic));
    Ok((print, info, stats))
}

impl Render for StudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.asked {
            self.asked = true;
            let (load, id) = (self.load.clone(), cx.entity_id().as_u64());
            window.defer(cx, move |window, cx| load(id, window, cx));
        }
        if self.search.is_none() {
            // InputState installs focus observers; make it outside drawing.
            let target = cx.weak_entity();
            window.defer(cx, move |window, cx| {
                let _ = target.update(cx, |this, cx| {
                    this.ensure_inputs(window, cx);
                    cx.notify();
                });
            });
        }
        self.relabel_inputs(window, cx);
        crate::toast::flush(&mut self.toasts, window, cx);
        if std::mem::take(&mut self.close_dialog) {
            self.dialog = None;
            window.close_dialog(cx);
        }
        self.open_review_dialog(window, cx);
        for icon in self.old_icons.drain(..) {
            window.drop_image(icon).ok();
        }
        let colors = ShellColors::current(cx);
        if self.placement == Placement::Pane {
            return self.pane(colors, cx).into_any_element();
        }
        let materials_width = materials_width(f32::from(window.viewport_size().width));
        let head = self.head(colors, cx);
        let viewport = self.viewport_column(colors, cx);
        let materials = self.inspector_column(materials_width, colors, cx);
        v_flex()
            .id("schematic-studio")
            .debug_selector(|| "schematic-studio".into())
            .w_full()
            .h_full()
            .min_h_0()
            .gap_3()
            .child(head)
            .child(
                h_flex()
                    .debug_selector(|| "studio-columns".into())
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .gap_3()
                    .child(viewport)
                    .child(materials),
            )
            .into_any_element()
    }
}
