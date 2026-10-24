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
mod history;
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
use studio_view::analyze;
#[cfg(test)]
use studio_view::materials_width;
pub use studio_view::{
    Act, Command, Connection, Event, Load, LoadModel, OpenWindow, Placement, StudioView,
    WriteFailure, tab_labels,
};
pub(crate) use studio_view::{Analysis, Inspection, InspectorTab, Loaded, muted_line};
pub use thumbnails::{THUMBNAIL_SIZE, render_thumbnail};
pub use window::StudioWindow;

mod loading;

mod studio_view;
