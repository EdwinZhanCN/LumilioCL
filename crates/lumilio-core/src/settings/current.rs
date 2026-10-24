use tokio::sync::Mutex;

use super::{SettingsError, SettingsStore};
use crate::context::ContextHub;
use crate::instances::{InstanceRecord, Instances};

/// Keeps the saved current game one that exists: when it is missing or gone,
/// the fallback becomes current and is saved. A save that fails leaves the old
/// choice, which reads as `None` until the next change.
pub(crate) async fn settle_current(
    instances: &Instances,
    context: &ContextHub,
    settings: &Mutex<SettingsStore>,
) {
    let records = instances.lock().await.instances().to_vec();
    // The published choice still names a game: the settings stay untouched.
    if context
        .get()
        .current_instance
        .is_some_and(|id| records.iter().any(|record| record.id == id))
    {
        return;
    }
    let _ = settle_current_in(&records, &mut *settings.lock().await);
}

/// See [`settle_current`]; also run once on opening.
pub(crate) fn settle_current_in(
    records: &[InstanceRecord],
    settings: &mut SettingsStore,
) -> Result<(), SettingsError> {
    let saved = settings.get().current_instance.clone();
    if saved
        .as_deref()
        .is_some_and(|id| records.iter().any(|record| record.id == id))
    {
        return Ok(());
    }
    let next = crate::context::fallback_instance(records).map(str::to_owned);
    if next == saved {
        return Ok(());
    }
    settings.set_current_instance(next)
}
