use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::error::ServiceError;
use crate::settings::SettingsStore;
use crate::transfer::{ConnectionBudget, SourceChain, TransferEngine, Transport};

/// Connections all downloads share when the setting is automatic (plan
/// download-engine D2).
const TRANSFER_CONCURRENCY: usize = 16;

#[derive(Clone)]
pub(crate) struct Network<T> {
    transport: T,
    connections: ConnectionBudget,
    settings: Arc<Mutex<SettingsStore>>,
    /// Where stopped downloads keep their bytes.
    partials: PathBuf,
}

impl<T: Transport + Clone> Network<T> {
    pub(crate) fn new(
        transport: T,
        settings: Arc<Mutex<SettingsStore>>,
        partials: PathBuf,
    ) -> Result<Self, ServiceError> {
        Ok(Self {
            transport,
            connections: ConnectionBudget::new(TRANSFER_CONCURRENCY)
                .map_err(|error| ServiceError::Install(error.to_string()))?,
            settings,
            partials,
        })
    }

    pub(crate) const fn transport(&self) -> &T {
        &self.transport
    }

    /// The sources the settings name, in order.
    pub(crate) async fn chain(&self) -> Result<SourceChain, ServiceError> {
        Ok(self.settings.lock().await.source_chain()?)
    }

    /// The connections every download shares, sized by the current setting.
    pub(crate) async fn connections(&self) -> ConnectionBudget {
        let size = self
            .settings
            .lock()
            .await
            .get()
            .download_concurrency
            .map_or(TRANSFER_CONCURRENCY, |count| count as usize);
        // The setting is validated as positive; a zero keeps the old size.
        let _ = self.connections.resize(size);
        self.connections.clone()
    }

    /// The shared connections as they are sized now, without reading the
    /// settings.
    pub(crate) fn budget(&self) -> ConnectionBudget {
        self.connections.clone()
    }

    /// Where stopped downloads keep their bytes.
    pub(crate) fn partials(&self) -> PathBuf {
        self.partials.clone()
    }

    /// An engine for one task's downloads, on the shared connections.
    pub(crate) async fn transfer_engine(&self) -> TransferEngine<T> {
        TransferEngine::with_budget(self.transport.clone(), self.connections().await)
            .keeping_partials(self.partials.clone())
    }

    /// An engine for documents read into memory, on the shared connections
    /// as they are sized now. It takes no lock, so callers holding the
    /// settings may use it.
    pub(crate) fn fetcher(&self) -> TransferEngine<T> {
        TransferEngine::with_budget(self.transport.clone(), self.connections.clone())
    }
}
