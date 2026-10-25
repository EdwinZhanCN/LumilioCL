use gpui::{App, Window};

use super::super::ViewState;
use crate::kit::Emit;
use crate::live::{LiveHandler, LiveIntent, LiveModel};
use crate::theme::ShellColors;

pub struct LiveCtx<'a> {
    pub colors: ShellColors,
    pub model: &'a LiveModel,
    pub state: &'a ViewState,
    pub emit: Emit,
    pub handler: LiveHandler,
    /// The window's size, for layouts that change with the room they have.
    pub window_width: f32,
    pub window_height: f32,
}

pub(crate) fn send(
    handler: &LiveHandler,
    intent: LiveIntent,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| handler(intent.clone(), window, cx)
}
