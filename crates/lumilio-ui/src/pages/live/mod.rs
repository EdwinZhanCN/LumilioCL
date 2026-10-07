//! Live Library, Discover and Activity (plan 0007). They render
//! [`LiveModel`] and report [`LiveIntent`]s; the application does the work.

mod accounts;
mod activity;
mod controls;
mod discover;
mod home;
mod library;

#[cfg(test)]
mod tests;

pub use self::accounts::accounts;
pub use self::activity::{ACTIVITY_TABS, activity};
pub use self::controls::{DiscoverChangeHandler, LiveControls, LiveCtx, all_loaders};
pub use self::discover::{discover, kind_label, project_icon};
pub use self::home::{HOME_RECENT, home_sections, record_readings};
pub use self::library::{
    COLLECTIONS_TAB, LIBRARY_LOADER, LIBRARY_SORT, arranged, library, library_tabs, loader_code,
    present_loaders, sort_labels,
};
