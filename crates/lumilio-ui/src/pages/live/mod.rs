//! Live Library, Discover and Activity (plan 0007). They render
//! [`LiveModel`] and report [`LiveIntent`]s; the application does the work.

mod accounts;
mod activity;
mod controls;
mod discover;
mod library;

#[cfg(test)]
mod tests;

pub use self::accounts::accounts;
pub use self::activity::{ACTIVITY_TABS, activity};
pub use self::controls::{ALL_LOADERS, ALL_VERSIONS, DiscoverChangeHandler, LiveControls, LiveCtx};
pub use self::discover::{DISCOVER_KINDS, DISCOVER_TABS, discover, project_icon};
pub use self::library::{
    COLLECTIONS_TAB, LIBRARY_LOADER, LIBRARY_SORT, LIBRARY_TABS, SORT_LABELS, arranged, library,
    loader_code, present_loaders,
};
