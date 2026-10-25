use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, ExternalPaths, IntoElement, Render, Window, div};
use lumilio_core::{GameSettingsView, SettingsPreset};

use super::library_actions::{Library, LibraryEvent};
use super::presets_view::presets;
use super::view::{LibraryCtx, library};
use crate::kit::{self, Emit, ViewIntent};
use crate::live::{CollectionRow, LibraryCard, LiveHandler, LiveIntent};
use crate::theme::ShellColors;

/// The tab that shows the person's collections.
pub const COLLECTIONS_TAB: usize = 2;
/// The tab that shows the game option presets; it frames itself.
pub const PRESETS_TAB: usize = 3;
/// View-state groups of the Library's ordering and loader filter.
pub const LIBRARY_SORT: u8 = 210;
pub const LIBRARY_LOADER: u8 = 211;
/// The view-state group that remembers which preset the detail shows, by id.
pub const PRESET_GROUP: u8 = 230;

/// What the page shows of the rest of the launcher, as the shell knows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LibraryInputs {
    pub library: Vec<LibraryCard>,
    /// The library has been read at least once.
    pub library_loaded: bool,
    pub collections: Vec<CollectionRow>,
    pub settings_presets: Vec<SettingsPreset>,
    /// Why the presets could not be read, if they could not.
    pub presets_error: Option<String>,
    /// Every game's options, for the games that follow each preset.
    pub game_settings: Vec<GameSettingsView>,
}

impl LibraryInputs {
    /// The games of one collection that are still in the library.
    pub fn collection_cards(&self, collection: &CollectionRow) -> Vec<&LibraryCard> {
        collection
            .members
            .iter()
            .filter_map(|id| self.library.iter().find(|card| &card.id == id))
            .collect()
    }
}

/// The page's tab and the choices made on it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LibraryView {
    pub tab: usize,
    /// The order, by its place in [`super::sort_labels`].
    pub sort: usize,
    /// The loader shown, by [`super::loader_code`] (0 is every loader).
    pub loader: usize,
    /// The preset the 预设 tab shows, by id.
    pub preset: i64,
}

impl LibraryView {
    fn apply(&mut self, intent: ViewIntent) {
        match intent {
            ViewIntent::LibraryTab(tab) => self.tab = tab,
            ViewIntent::Choose(LIBRARY_SORT, sort) => self.sort = sort,
            ViewIntent::Choose(LIBRARY_LOADER, loader) => self.loader = loader,
            ViewIntent::Choose(PRESET_GROUP, id) => {
                self.preset = i64::try_from(id).unwrap_or(0);
            }
            _ => {}
        }
    }
}

impl Library {
    /// Where the page sends what other features answer: the shell's handler.
    pub fn set_handler(&mut self, handler: LiveHandler) {
        self.handler = Some(handler);
    }

    /// What the page shows, as the shell knows it.
    pub fn set_inputs(&mut self, inputs: LibraryInputs, cx: &mut Context<Self>) {
        if self.inputs != inputs {
            self.inputs = inputs;
            cx.notify();
        }
    }

    #[must_use]
    pub fn view(&self) -> LibraryView {
        self.view
    }

    /// Takes the order and loader filter the preferences remember; they are
    /// not a new choice to save.
    pub fn apply_preferences(&mut self, sort: u8, loader: u8, cx: &mut Context<Self>) {
        self.view.sort = usize::from(sort);
        self.view.loader = usize::from(loader);
        cx.notify();
    }

    /// Opens a tab directly, for design review and tests.
    pub fn show_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.view.tab = tab;
        cx.notify();
    }

    /// Applies a choice made on the page. The order and loader filter
    /// outlive this run: the application is told to remember them.
    pub fn apply_view_intent(&mut self, intent: ViewIntent, cx: &mut Context<Self>) {
        self.view.apply(intent);
        if matches!(intent, ViewIntent::Choose(LIBRARY_SORT | LIBRARY_LOADER, _)) {
            cx.emit(LibraryEvent::Remember {
                sort: u8::try_from(self.view.sort).unwrap_or(0),
                loader: u8::try_from(self.view.loader).unwrap_or(0),
            });
        }
        cx.notify();
    }

    /// Shows this preset in the 预设 tab's detail.
    pub(super) fn show_preset(&mut self, id: i64, cx: &mut Context<Self>) {
        self.view.preset = id;
        cx.notify();
    }
}

impl Render for Library {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_controls(window, cx);
        let this = cx.entity().downgrade();
        let emit: Emit = Rc::new(move |intent, _, cx| {
            let _ = this.update(cx, |library, cx| library.apply_view_intent(intent, cx));
        });
        let handler = self
            .handler
            .clone()
            .unwrap_or_else(|| Rc::new(|_, _, _| {}));
        let controls = self
            .controls
            .as_ref()
            .expect("controls are made before the page draws");
        let ctx = LibraryCtx {
            colors: ShellColors::current(cx),
            model: &self.inputs,
            view: self.view,
            emit,
            handler: handler.clone(),
            controls,
            filter: controls.filter.read(cx).value().to_string(),
        };
        // The presets tab is master–detail: its panes scroll, the page does not.
        let page = if self.view.tab == PRESETS_TAB {
            presets(&ctx)
        } else {
            kit::page("live-library", library(&ctx)).into_any_element()
        };
        // ia[library]: 拖入整合包 | 把文件拖到页面上 | 同“导入整合包”（.mrpack、CurseForge 整合包 .zip，或 MultiMC/Prism/本启动器备份的 .zip）；不是的提示一句
        div()
            .id("live-library-drop")
            .size_full()
            .on_drop::<ExternalPaths>(move |paths, window, cx| {
                handler(LiveIntent::DropFiles(paths.paths().to_vec()), window, cx)
            })
            .child(page)
    }
}
