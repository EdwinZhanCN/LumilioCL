//! Live Home, Activity and Accounts (plan 0007). They render
//! [`LiveModel`](crate::live::LiveModel) and report
//! [`LiveIntent`](crate::live::LiveIntent)s; the application does the work.

mod accounts;
mod activity;
mod controls;
mod home;

#[cfg(test)]
mod tests;

pub use self::accounts::{ACCOUNT_GROUP, accounts, shown_account};
pub use self::activity::{ACTIVITY_TABS, activity};
pub use self::controls::{LiveControls, LiveCtx};
pub use self::home::{home_sections, record_readings};
