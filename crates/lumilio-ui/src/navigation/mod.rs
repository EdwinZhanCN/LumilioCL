//! The floating navigation bar (design language §6): history and the current
//! location on the leading side, the landmark capsule in the centre, the
//! current account and instance on the trailing side.

use std::rc::Rc;

use gpui::{App, Window};

use crate::route::Route;

pub use bar::{
    AccountChoice, CurrentAccount, CurrentInstance, InstanceChoice, Leading, ReadyUpdate, Trailing,
    render,
};

mod bar;
mod choices;
#[cfg(test)]
mod tests;

pub type RouteHandler = Rc<dyn Fn(&Route, &mut Window, &mut App)>;
pub type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;
pub type ChooseHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;
/// Chooses the choice at an index of a trailing chip's list.
type PickHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
