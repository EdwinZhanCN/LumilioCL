//! Live Home (plan 0007). It renders
//! [`LiveModel`](crate::live::LiveModel) and reports
//! [`LiveIntent`](crate::live::LiveIntent)s; the application does the work.

mod controls;
mod home;

#[cfg(test)]
mod tests;

pub use self::controls::LiveCtx;
pub use self::home::{home_sections, record_readings};
