//! What a running task downloads, for the download center (plan
//! download-engine T5): the files on their way with their source, attempt
//! and connections, how often a download was tried again, and the sources
//! given up for others. It is built from the task's transfer events and kept
//! only while the task runs.

use crate::transfer::TransferEvent;

/// Sources given up that a task remembers, newest last.
const SWITCHES_KEPT: usize = 8;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TaskDownloads {
    /// Files on their way, in the order they started.
    pub files: Vec<FileDownload>,
    /// Sources given up and what took their place, newest last.
    pub switches: Vec<SourceSwitch>,
    /// Attempts that failed and were made again.
    pub retries: usize,
}

/// One file on its way.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileDownload {
    /// The transfer's id within its task.
    pub id: String,
    /// The destination's file name.
    pub name: String,
    /// Where it downloads from; `None` while it waits for a connection.
    pub source: Option<String>,
    /// The attempt at `source`, from 1.
    pub attempt: usize,
    /// Connections writing it at once; more than one once it is split.
    pub connections: usize,
    pub done: u64,
    pub total: Option<u64>,
}

/// A source given up for a file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSwitch {
    pub file: String,
    pub from: String,
    pub reason: String,
    /// The source tried next; `None` when it was the last.
    pub to: Option<String>,
}

impl TaskDownloads {
    pub(crate) fn apply(&mut self, event: &TransferEvent) {
        match event {
            TransferEvent::Started { id, destination } => {
                self.files.retain(|file| &file.id != id);
                self.files.push(FileDownload {
                    id: id.clone(),
                    name: destination.file_name().map_or_else(
                        || destination.display().to_string(),
                        |name| name.to_string_lossy().into_owned(),
                    ),
                    source: None,
                    attempt: 0,
                    connections: 0,
                    done: 0,
                    total: None,
                });
            }
            TransferEvent::Downloading {
                id,
                source,
                attempt,
            } => {
                if let Some(file) = self.file(id) {
                    file.source = Some(source.clone());
                    file.attempt = *attempt;
                    file.connections = 1;
                }
            }
            TransferEvent::Progress {
                id,
                completed,
                total,
            } => {
                if let Some(file) = self.file(id) {
                    file.done = *completed;
                    file.total = *total;
                }
            }
            TransferEvent::Connections { id, count } => {
                if let Some(file) = self.file(id) {
                    file.connections = *count;
                }
            }
            TransferEvent::Retrying { .. } => self.retries += 1,
            TransferEvent::SourceFailed {
                id,
                source,
                reason,
                next,
            } => {
                let file = self
                    .files
                    .iter()
                    .find(|file| &file.id == id)
                    .map_or_else(|| id.clone(), |file| file.name.clone());
                self.switches.push(SourceSwitch {
                    file,
                    from: source.clone(),
                    reason: reason.clone(),
                    to: next.clone(),
                });
                let extra = self.switches.len().saturating_sub(SWITCHES_KEPT);
                self.switches.drain(..extra);
            }
            TransferEvent::Completed { id, .. } | TransferEvent::Failed { id, .. } => {
                self.files.retain(|file| &file.id != id);
            }
        }
    }

    fn file(&mut self, id: &str) -> Option<&mut FileDownload> {
        self.files.iter_mut().find(|file| file.id == id)
    }
}
