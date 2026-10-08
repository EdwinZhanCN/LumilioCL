//! GPUI views and interaction state for LumilioCL.

pub mod accounts;
pub mod assets;
pub mod auth_servers;
pub mod collections;
pub mod controls;
pub mod cover;
pub mod dependency_prompt;
pub mod export_form;
// Preserved as a rejected visual experiment; excluded from production builds.
#[cfg(test)]
pub mod backdrop;
pub mod game_picker;
pub mod hero;
pub mod history;
pub mod home;
pub mod i18n;
pub mod instance_detail;
pub mod key;
pub mod kit;
pub mod live;
pub mod microsoft_login;
pub mod model_view;
pub mod navigation;
pub mod new_game;
pub mod pages;
pub mod photosensitivity;
pub mod placeholders;
pub mod platform;
pub mod plugin_setting_dialog;
pub mod project_detail;
pub mod route;
pub mod settings_dialog;
pub mod settings_forms;
pub mod shell;
pub mod skin_dialog;
pub mod skin_view;
pub mod theme;
pub mod third_party_login;
pub mod toast;
pub mod version_picker;

pub use shell::{
    ActivitySummary, LauncherShell, ShellIntent, build_root, follow_system_appearance,
    window_titlebar,
};
