use super::render::ArtStyle;
use super::{HomeIntent, HomeIntentHandler};
use crate::key::Key;
use gpui::{ClickEvent, SharedString};
use gpui_component::Icon;

/// A key on the page: orange when primary, white otherwise.
pub(super) fn page_button(
    id: &'static str,
    label: &'static str,
    icon: LocalActionIcon,
    intent: HomeIntent,
    primary: bool,
    intent_handler: Option<HomeIntentHandler>,
) -> Key {
    let button = base_button(id, label, icon, intent, intent_handler);
    if primary {
        button.primary()
    } else {
        button.white()
    }
}

/// A key on world art.
pub(super) fn art_button(
    id: &'static str,
    label: &'static str,
    icon: LocalActionIcon,
    intent: HomeIntent,
    style: ArtStyle,
    intent_handler: Option<HomeIntentHandler>,
) -> Key {
    base_button(id, label, icon, intent, intent_handler).kind(style.0)
}

/// Labelled keys carry no tooltip: the label already says it
/// (design language §9 reserves tooltips for icon-only controls).
pub(super) fn base_button(
    id: &'static str,
    label: &'static str,
    icon: LocalActionIcon,
    intent: HomeIntent,
    intent_handler: Option<HomeIntentHandler>,
) -> Key {
    let enabled = intent_handler.is_some();
    let mut button = Key::new(id)
        .label(label)
        .icon(Icon::new(icon))
        .disabled(!enabled);
    if let Some(handler) = intent_handler {
        button = button.on_click(move |_: &ClickEvent, window, cx| handler(intent, window, cx));
    }
    button
}

#[derive(Clone, Copy)]
pub(super) enum LocalActionIcon {
    Import,
    Create,
    Continue,
    Recover,
    TechnicalDetails,
    Cancel,
    Stop,
}

impl gpui_component::IconNamed for LocalActionIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Import => "icons/lucide/download.svg",
            Self::Create => "icons/lucide/plus.svg",
            Self::Continue => "icons/lucide/play.svg",
            Self::Recover => "icons/lucide/refresh-cw.svg",
            Self::TechnicalDetails => "icons/lucide/info.svg",
            Self::Cancel => "icons/lucide/x.svg",
            Self::Stop => "icons/lucide/square.svg",
        }
        .into()
    }
}
