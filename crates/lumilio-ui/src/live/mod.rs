//! The presentation model for live data (plan 0007, T2).
//!
//! Everything here is owned, window-free and built from `lumilio-core`
//! values by pure functions, so it is unit-tested without a GPUI context. The
//! shell renders it and reports [`LiveIntent`]s; the application decides what
//! they mean.

mod accounts;
mod activity;
mod home;
mod intent;
mod library;
mod settings;
mod update;

#[cfg(test)]
mod tests;

pub use live_model::LiveModel;

pub use self::accounts::{
    AccountRow, account_failure, account_rows, auth_message, server_name, skin_message,
    yggdrasil_message,
};
pub use self::activity::{
    ACTIVITY_CATEGORIES, ActivityRow, ActivityState, TrafficReading, activity_in_tab,
    activity_rows, bytes_text, eta_text, file_amount, recovery_message, source_host,
};
pub use self::home::{
    HomePlaces, PLACE_SERVERS, PLACE_WORLDS, Place, PlaceKind, PlacePicture, PlaceTarget,
    home_places, home_presentation, world_mode,
};
pub use self::intent::{LiveHandler, LiveIntent};
pub use self::library::{
    CollectionRow, LibraryCard, ago, attention_rows, cover_loader, instance_meta, library_card,
    library_cards, loader_label, relative_time, seed_of, world_of,
};
pub use self::settings::{JavaRow, SettingsView, java_label, settings_view};
pub use self::update::{UpdateStatus, UpdateUnavailability};

mod live_model;
