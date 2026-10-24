use std::rc::Rc;

use gpui::{App, Window};
use lumilio_core::ProjectKind;

use super::super::controls::DiscoverControls;
use super::super::model::DiscoverChange;
use super::super::page::Choices;
use super::super::state::DiscoverState;
use crate::kit::Emit;
use crate::theme::ShellColors;

/// What the page's rows and menus ask the page to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoverIntent {
    /// Search again with the query as it stands.
    SearchAgain,
    OpenProject {
        kind: ProjectKind,
        /// The content source (plugin ID).
        source: String,
        slug: String,
    },
    /// Install a project: into the current game, or as a new one for a
    /// modpack.
    Install {
        kind: ProjectKind,
        source: String,
        slug: String,
        title: String,
        /// One specific version, for mods, packs and shaders.
        version: Option<String>,
    },
    /// Replace an installed file with its newer version.
    UpdateInstalled {
        kind: ProjectKind,
        source: String,
        project: String,
        title: String,
        file_name: String,
        version_id: String,
    },
    /// Put text on the clipboard and say so in a toast.
    CopyText { text: String, notice: String },
}

pub type DiscoverHandler = Rc<dyn Fn(DiscoverIntent, &mut Window, &mut App)>;

/// Applies a change to the query and searches again.
pub type ChangeHandler = Rc<dyn Fn(DiscoverChange, &mut Window, &mut App)>;

/// What the page's views draw from.
pub struct DiscoverCtx<'a> {
    pub colors: ShellColors,
    pub model: &'a DiscoverState,
    pub state: &'a Choices,
    pub emit: Emit,
    pub handler: DiscoverHandler,
    pub change: ChangeHandler,
    pub controls: &'a DiscoverControls,
    /// What is typed in the sidebar's version search.
    pub version_filter: String,
    /// The window's size, for the layout that folds the filters.
    pub window_width: f32,
    pub window_height: f32,
}

pub(super) fn send(
    handler: &DiscoverHandler,
    intent: DiscoverIntent,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| handler(intent.clone(), window, cx)
}
