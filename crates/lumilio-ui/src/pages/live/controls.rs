use super::super::ViewState;
use super::library::SORT_LABELS;
use crate::kit::Emit;
use crate::live::{
    DiscoverChange, LiveHandler, LiveIntent, LiveModel, PAGE_SIZES, SORTS, sort_label,
};
use crate::theme::ShellColors;
use gpui::prelude::*;
use gpui::{App, Entity, Window};
use gpui_component::IndexPath;
use gpui_component::input::InputState;
use gpui_component::select::{SearchableVec, SelectState};

/// Inputs the live pages own. Created lazily, because they need a window.
pub struct LiveControls {
    pub library_filter: Entity<InputState>,
    pub discover_search: Entity<InputState>,
    pub sort: Entity<SelectState<SearchableVec<String>>>,
    pub page_size: Entity<SelectState<SearchableVec<String>>>,
    /// The box that narrows the sidebar's version list.
    pub version_search: Entity<InputState>,
    /// The version list's scroll, so it keeps the wheel while it can scroll.
    pub version_scroll: gpui::ScrollHandle,
    /// The Library's ordering and loader filter.
    pub library_sort: Entity<SelectState<SearchableVec<String>>>,
    pub library_loader: Entity<SelectState<SearchableVec<String>>>,
    /// The loader codes behind the loader list's entries after "全部".
    pub library_loaders: Vec<usize>,
    /// The Settings plugin list's scroll, so it keeps the wheel while it can
    /// scroll.
    pub plugin_scroll: gpui::ScrollHandle,
}

/// The text of the "no loader filter" entry.
pub const ALL_LOADERS: &str = "全部加载器";

impl LiveControls {
    pub fn new(window: &mut Window, cx: &mut gpui::Context<Self>) -> Self {
        Self {
            library_filter: cx.new(|cx| InputState::new(window, cx).placeholder("搜索游戏")),
            discover_search: cx
                .new(|cx| InputState::new(window, cx).placeholder("搜索 Modrinth，回车确认")),
            sort: cx.new(|cx| {
                let labels: Vec<String> = SORTS.iter().map(|s| sort_label(*s).to_owned()).collect();
                SelectState::new(
                    SearchableVec::new(labels),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            page_size: cx.new(|cx| {
                let labels: Vec<String> = PAGE_SIZES.iter().map(u32::to_string).collect();
                let at = PAGE_SIZES.iter().position(|size| *size == 20).unwrap_or(0);
                SelectState::new(
                    SearchableVec::new(labels),
                    Some(IndexPath::new(at)),
                    window,
                    cx,
                )
            }),
            version_search: cx.new(|cx| InputState::new(window, cx).placeholder("搜索版本")),
            version_scroll: gpui::ScrollHandle::new(),
            library_sort: cx.new(|cx| {
                let labels: Vec<String> = SORT_LABELS.iter().map(|s| (*s).to_owned()).collect();
                SelectState::new(
                    SearchableVec::new(labels),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            library_loader: cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(vec![ALL_LOADERS.to_owned()]),
                    Some(IndexPath::default()),
                    window,
                    cx,
                )
            }),
            library_loaders: Vec::new(),
            plugin_scroll: gpui::ScrollHandle::new(),
        }
    }
}

pub type DiscoverChangeHandler = std::rc::Rc<dyn Fn(DiscoverChange, &mut Window, &mut App)>;

pub struct LiveCtx<'a> {
    pub colors: ShellColors,
    pub model: &'a LiveModel,
    pub state: &'a ViewState,
    pub emit: Emit,
    pub handler: LiveHandler,
    /// Applies a change to the Discover query and searches again.
    pub change: DiscoverChangeHandler,
    pub controls: &'a LiveControls,
    pub filter: String,
    /// What is typed in the sidebar's version search.
    pub version_filter: String,
}

pub(super) fn send(
    handler: &LiveHandler,
    intent: LiveIntent,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| handler(intent.clone(), window, cx)
}
