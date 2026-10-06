//! The Instance page's Content tab (IA `instance/content.md`): each file with
//! where it came from (P-CONTENT-ITEM), filters, bulk actions (P-BULK), local
//! files (P-ADD-FILES) and the switch-version dialog (P-VERSION-SWITCH).
//!
//! A child module of `instance_detail`, so it shares the view's private state.

mod actions;
mod bulk;
mod model;
mod panel;
mod row;
mod switch;

#[cfg(test)]
mod tests;

pub use self::model::ContentFilter;
pub use self::switch::VersionSwitch;

/// The Content tab's filter dropdown (design language §10: Select / dropdown).
pub use super::Dropdown as FilterSelect;

use std::collections::BTreeSet;

/// The selection, kept per kind: file names.
pub type Selection = BTreeSet<String>;
