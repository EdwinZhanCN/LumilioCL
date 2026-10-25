//! The Accounts page: the accounts as a list, the chosen one's detail with
//! its look (a preview and the wardrobe), and adding, signing in, refreshing
//! and removing accounts. [`Accounts`] draws the page and does the work; the
//! shell owns it, gives it the accounts and passes its events on. Which
//! account later launches use stays the shell's choice.

pub use accounts_actions::{Accounts, AccountsEvent};
pub use page::{ACCOUNT_GROUP, AccountsInputs, AccountsView};

mod accounts_actions;
mod look;
mod page;
mod servers;
mod sign_in;
mod view;
mod wardrobe_work;

#[cfg(test)]
mod tests;
