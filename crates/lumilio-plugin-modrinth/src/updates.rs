//! Which version an installed file is offered as its update.
//!
//! Adapted from Modrinth App (`packages/app-lib/src/state/cache.rs` and
//! `packages/app-lib/src/state/instances/commands/check_content_updates.rs`,
//! GPL-3.0-only): a file is offered versions at least as stable as its own,
//! and less stable ones only for files that nothing that stable fits. Without
//! this, a release is offered the newest beta as its update.

use std::collections::BTreeMap;

use lumilio_plugin_api::content::{Compatibility, FileIdentity, ReleaseChannel, Version};
use lumilio_plugin_api::{HostContext, PluginError};

use super::modrinth::{Update, identify};

/// The newest version of each file for `compatibility`, on the channels that
/// the installed version allows. Files Modrinth does not know are absent.
pub(super) fn latest_for(
    ctx: &dyn HostContext,
    files: &[FileIdentity],
    compatibility: &Compatibility,
) -> Result<BTreeMap<String, Version>, PluginError> {
    let installed = identify(ctx, files, None)?;
    let mut by_channel: BTreeMap<ReleaseChannel, Vec<FileIdentity>> = BTreeMap::new();
    for file in files {
        if let Some(version) = installed.get(&file.key) {
            by_channel
                .entry(version.channel)
                .or_default()
                .push(file.clone());
        }
    }
    let mut latest = BTreeMap::new();
    for (channel, mut waiting) in by_channel {
        for version_types in fallbacks(channel) {
            if waiting.is_empty() {
                break;
            }
            let update = Update {
                compatibility,
                version_types,
            };
            let found = identify(ctx, &waiting, Some(update))?;
            waiting.retain(|file| !found.contains_key(&file.key));
            latest.extend(found);
        }
    }
    Ok(latest)
}

/// The version types asked for in turn: those at least as stable as the
/// installed version, then each less stable one for the files still unanswered.
fn fallbacks(installed: ReleaseChannel) -> &'static [&'static [&'static str]] {
    match installed {
        ReleaseChannel::Release => &[&["release"], &["beta"], &["alpha"]],
        ReleaseChannel::Beta => &[&["release", "beta"], &["alpha"]],
        ReleaseChannel::Alpha => &[&["release", "beta", "alpha"]],
    }
}
