use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, watch};
use tokio::task::JoinSet;

type ActivityFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
type ActivityAction = Arc<dyn Fn(ActivityContext) -> ActivityFuture + Send + Sync>;

#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<watch::Sender<bool>>,
}

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        let (sender, _receiver) = watch::channel(false);
        Self {
            cancelled: Arc::new(sender),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.send_replace(true);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        let mut receiver = self.cancelled.subscribe();
        while !*receiver.borrow() {
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Blocked,
}

impl ActivityState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Blocked
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressSnapshot {
    completed: u64,
    total: Option<u64>,
}

impl ProgressSnapshot {
    pub fn new(completed: u64, total: Option<u64>) -> Result<Self, String> {
        if total.is_some_and(|total| completed > total) {
            return Err(format!(
                "progress {completed} exceeds total {}",
                total.unwrap_or_default()
            ));
        }
        Ok(Self { completed, total })
    }

    #[must_use]
    pub const fn completed(self) -> u64 {
        self.completed
    }

    #[must_use]
    pub const fn total(self) -> Option<u64> {
        self.total
    }

    #[must_use]
    pub fn fraction(self) -> Option<f64> {
        self.total.map(|total| {
            if total == 0 {
                1.0
            } else {
                self.completed as f64 / total as f64
            }
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivityEvent {
    Registered {
        id: String,
        label: String,
    },
    Started {
        id: String,
    },
    Progress {
        id: String,
        progress: ProgressSnapshot,
    },
    Retrying {
        id: String,
        source: String,
        attempt: usize,
    },
    Finished {
        id: String,
        state: ActivityState,
        detail: Option<String>,
    },
}

#[derive(Clone)]
pub struct ActivityContext {
    id: String,
    cancellation: CancellationToken,
    events: broadcast::Sender<ActivityEvent>,
    progress: Arc<Mutex<BTreeMap<String, ProgressSnapshot>>>,
}

impl ActivityContext {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub async fn cancelled(&self) {
        self.cancellation.cancelled().await;
    }

    pub fn report_progress(&self, completed: u64, total: Option<u64>) -> Result<(), String> {
        let mut snapshots = self.progress.lock().expect("activity progress lock");
        let previous = snapshots.get(&self.id).copied();
        if previous.is_some_and(|previous| completed < previous.completed) {
            return Err(format!(
                "activity {} progress moved backwards from {} to {completed}",
                self.id,
                previous
                    .map(ProgressSnapshot::completed)
                    .unwrap_or_default()
            ));
        }
        if let Some(previous_total) = previous.and_then(ProgressSnapshot::total)
            && total.is_some_and(|total| total != previous_total)
        {
            return Err(format!(
                "activity {} progress total changed from {previous_total}",
                self.id
            ));
        }
        let progress = ProgressSnapshot::new(
            completed,
            total.or_else(|| previous.and_then(ProgressSnapshot::total)),
        )?;
        snapshots.insert(self.id.clone(), progress);
        drop(snapshots);
        let _ = self.events.send(ActivityEvent::Progress {
            id: self.id.clone(),
            progress,
        });
        Ok(())
    }

    pub fn report_retry(&self, source: impl Into<String>, attempt: usize) {
        let _ = self.events.send(ActivityEvent::Retrying {
            id: self.id.clone(),
            source: source.into(),
            attempt,
        });
    }
}

pub struct ActivityNode {
    id: String,
    label: String,
    stage: Option<String>,
    prerequisites: BTreeSet<String>,
    action: ActivityAction,
}

impl ActivityNode {
    #[must_use]
    pub fn new<F, Fut>(id: impl Into<String>, label: impl Into<String>, action: F) -> Self
    where
        F: Fn(ActivityContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        Self {
            id: id.into(),
            label: label.into(),
            stage: None,
            prerequisites: BTreeSet::new(),
            action: Arc::new(move |context| Box::pin(action(context))),
        }
    }

    #[must_use]
    pub fn after(mut self, prerequisite: impl Into<String>) -> Self {
        self.prerequisites.insert(prerequisite.into());
        self
    }

    #[must_use]
    pub fn in_stage(mut self, stage: impl Into<String>) -> Self {
        self.stage = Some(stage.into());
        self
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn stage(&self) -> Option<&str> {
        self.stage.as_deref()
    }

    #[must_use]
    pub fn prerequisites(&self) -> &BTreeSet<String> {
        &self.prerequisites
    }
}

#[derive(Default)]
pub struct ActivityGraph {
    nodes: BTreeMap<String, ActivityNode>,
}

impl ActivityGraph {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, node: ActivityNode) -> Result<(), ActivityGraphError> {
        if node.id.trim().is_empty() {
            return Err(ActivityGraphError::EmptyId);
        }
        if self.nodes.contains_key(&node.id) {
            return Err(ActivityGraphError::DuplicateId(node.id));
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    fn validate(&self) -> Result<(), ActivityGraphError> {
        for node in self.nodes.values() {
            for prerequisite in &node.prerequisites {
                if !self.nodes.contains_key(prerequisite) {
                    return Err(ActivityGraphError::MissingPrerequisite {
                        activity: node.id.clone(),
                        prerequisite: prerequisite.clone(),
                    });
                }
            }
        }

        let mut permanent = BTreeSet::new();
        let mut temporary = BTreeSet::new();
        for id in self.nodes.keys() {
            visit_node(id, &self.nodes, &mut temporary, &mut permanent)?;
        }
        Ok(())
    }
}

fn visit_node(
    id: &str,
    nodes: &BTreeMap<String, ActivityNode>,
    temporary: &mut BTreeSet<String>,
    permanent: &mut BTreeSet<String>,
) -> Result<(), ActivityGraphError> {
    if permanent.contains(id) {
        return Ok(());
    }
    if !temporary.insert(id.to_owned()) {
        return Err(ActivityGraphError::Cycle(id.to_owned()));
    }
    for prerequisite in &nodes[id].prerequisites {
        visit_node(prerequisite, nodes, temporary, permanent)?;
    }
    temporary.remove(id);
    permanent.insert(id.to_owned());
    Ok(())
}

#[derive(Clone, Debug)]
pub struct ActivityRecord {
    id: String,
    label: String,
    stage: Option<String>,
    state: ActivityState,
    progress: Option<ProgressSnapshot>,
    detail: Option<String>,
}

impl ActivityRecord {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn stage(&self) -> Option<&str> {
        self.stage.as_deref()
    }

    #[must_use]
    pub const fn state(&self) -> ActivityState {
        self.state
    }

    #[must_use]
    pub const fn progress(&self) -> Option<ProgressSnapshot> {
        self.progress
    }

    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ActivityReport {
    records: BTreeMap<String, ActivityRecord>,
}

impl ActivityReport {
    #[must_use]
    pub fn records(&self) -> &BTreeMap<String, ActivityRecord> {
        &self.records
    }

    #[must_use]
    pub fn state(&self, id: &str) -> Option<ActivityState> {
        self.records.get(id).map(ActivityRecord::state)
    }

    #[must_use]
    pub fn failure(&self, id: &str) -> Option<&str> {
        self.records.get(id).and_then(ActivityRecord::detail)
    }
}

#[derive(Clone)]
pub struct ActivityScheduler {
    concurrency: usize,
    events: broadcast::Sender<ActivityEvent>,
}

impl ActivityScheduler {
    pub fn new(concurrency: usize) -> Result<Self, ActivityGraphError> {
        if concurrency == 0 {
            return Err(ActivityGraphError::ZeroConcurrency);
        }
        let (events, _) = broadcast::channel(256);
        Ok(Self {
            concurrency,
            events,
        })
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ActivityEvent> {
        self.events.subscribe()
    }

    pub async fn run(
        &self,
        graph: ActivityGraph,
        cancellation: CancellationToken,
    ) -> Result<ActivityReport, ActivityGraphError> {
        graph.validate()?;
        let mut pending = graph.nodes;
        let mut records = BTreeMap::new();
        let progress = Arc::new(Mutex::new(BTreeMap::new()));
        for node in pending.values() {
            records.insert(
                node.id.clone(),
                ActivityRecord {
                    id: node.id.clone(),
                    label: node.label.clone(),
                    stage: node.stage.clone(),
                    state: ActivityState::Queued,
                    progress: None,
                    detail: None,
                },
            );
            let _ = self.events.send(ActivityEvent::Registered {
                id: node.id.clone(),
                label: node.label.clone(),
            });
        }

        if cancellation.is_cancelled() {
            for record in records.values_mut() {
                finish_record(record, ActivityState::Cancelled, None, &self.events);
            }
            apply_progress(&mut records, &progress);
            return Ok(ActivityReport { records });
        }

        let mut running = BTreeSet::new();
        let mut completed = BTreeSet::new();
        let mut join_set = JoinSet::new();

        while !pending.is_empty() || !running.is_empty() {
            let blocked = pending
                .iter()
                .filter(|(_, node)| {
                    node.prerequisites.iter().any(|prerequisite| {
                        records.get(prerequisite).is_some_and(|record| {
                            record.state.is_terminal() && record.state != ActivityState::Succeeded
                        })
                    })
                })
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            for id in blocked {
                pending.remove(&id);
                if let Some(record) = records.get_mut(&id) {
                    finish_record(
                        record,
                        ActivityState::Blocked,
                        Some("a prerequisite did not succeed".to_owned()),
                        &self.events,
                    );
                }
            }

            while running.len() < self.concurrency {
                let ready = pending
                    .iter()
                    .find(|(_, node)| {
                        node.prerequisites
                            .iter()
                            .all(|prerequisite| completed.contains(prerequisite))
                    })
                    .map(|(id, _)| id.clone());
                let Some(id) = ready else {
                    break;
                };
                let node = pending.remove(&id).expect("ready node exists");
                records.get_mut(&id).expect("registered node").state = ActivityState::Running;
                let _ = self.events.send(ActivityEvent::Started { id: id.clone() });
                running.insert(id.clone());
                let context = ActivityContext {
                    id: id.clone(),
                    cancellation: cancellation.clone(),
                    events: self.events.clone(),
                    progress: progress.clone(),
                };
                join_set.spawn(async move { (id, (node.action)(context).await) });
            }

            if running.is_empty() {
                break;
            }

            let joined = tokio::select! {
                () = cancellation.cancelled() => {
                    join_set.abort_all();
                    while join_set.join_next().await.is_some() {}
                    for id in &running {
                        if let Some(record) = records.get_mut(id) {
                            finish_record(record, ActivityState::Cancelled, None, &self.events);
                        }
                    }
                    for (_, node) in pending {
                        if let Some(record) = records.get_mut(&node.id) {
                            finish_record(record, ActivityState::Cancelled, None, &self.events);
                        }
                    }
                    apply_progress(&mut records, &progress);
                    return Ok(ActivityReport { records });
                }
                result = join_set.join_next() => result,
            };

            if let Some(joined) = joined {
                match joined {
                    Ok((id, Ok(()))) => {
                        running.remove(&id);
                        completed.insert(id.clone());
                        finish_record(
                            records.get_mut(&id).expect("running record"),
                            ActivityState::Succeeded,
                            None,
                            &self.events,
                        );
                    }
                    Ok((id, Err(message))) => {
                        running.remove(&id);
                        finish_record(
                            records.get_mut(&id).expect("running record"),
                            ActivityState::Failed,
                            Some(message),
                            &self.events,
                        );
                    }
                    Err(join_error) => {
                        return Err(ActivityGraphError::Worker(join_error.to_string()));
                    }
                }
            }
        }

        apply_progress(&mut records, &progress);
        Ok(ActivityReport { records })
    }
}

fn apply_progress(
    records: &mut BTreeMap<String, ActivityRecord>,
    progress: &Arc<Mutex<BTreeMap<String, ProgressSnapshot>>>,
) {
    for (id, snapshot) in progress.lock().expect("activity progress lock").iter() {
        if let Some(record) = records.get_mut(id) {
            record.progress = Some(*snapshot);
        }
    }
}

fn finish_record(
    record: &mut ActivityRecord,
    state: ActivityState,
    detail: Option<String>,
    events: &broadcast::Sender<ActivityEvent>,
) {
    record.state = state;
    record.detail.clone_from(&detail);
    let _ = events.send(ActivityEvent::Finished {
        id: record.id.clone(),
        state,
        detail,
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivityGraphError {
    EmptyId,
    DuplicateId(String),
    MissingPrerequisite {
        activity: String,
        prerequisite: String,
    },
    Cycle(String),
    ZeroConcurrency,
    Worker(String),
}

impl Display for ActivityGraphError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => formatter.write_str("activity id must not be empty"),
            Self::DuplicateId(id) => write!(formatter, "duplicate activity id: {id}"),
            Self::MissingPrerequisite {
                activity,
                prerequisite,
            } => write!(
                formatter,
                "activity {activity} references missing prerequisite {prerequisite}"
            ),
            Self::Cycle(id) => write!(formatter, "activity graph contains a cycle at {id}"),
            Self::ZeroConcurrency => formatter.write_str("activity concurrency must be positive"),
            Self::Worker(message) => write!(formatter, "activity worker failed: {message}"),
        }
    }
}

impl Error for ActivityGraphError {}

#[cfg(test)]
mod tests {
    use super::{ActivityGraph, ActivityGraphError, ActivityNode};

    #[test]
    fn graph_rejects_missing_prerequisites_before_execution() {
        let mut graph = ActivityGraph::new();
        graph
            .add(ActivityNode::new("child", "Child", |_| async { Ok(()) }).after("missing-parent"))
            .unwrap();

        assert!(matches!(
            graph.validate(),
            Err(ActivityGraphError::MissingPrerequisite { .. })
        ));
    }
}
