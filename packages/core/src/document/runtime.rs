use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;

mod commit;
mod testing;

use super::job::entry::{DocumentJobHandle, JobEntry};
use super::metrics::with_global_document_engine_metrics;
use super::protocol::{
    AdvanceInput, DocumentJobSpec, EventBatch, GraphValueEditPlan, GraphValueEditRequest,
    JobTerminal, ProjectionDelta, ProjectionRequest, QueryResult, SnapshotId, SnapshotQuery,
    SnapshotReadResult,
};
use super::snapshot::DocumentSnapshot;

pub use commit::{
    close_job_terminal, commit_snapshot, store_snapshot_for_document, stored_snapshot_for_document,
};
pub use testing::{
    document_runtime_contains_document_for_tests, document_runtime_job_count_for_tests,
    document_runtime_latest_job_spec_for_document_for_tests,
    document_runtime_terminal_for_document_for_tests, reset_runtime_for_tests,
};

#[derive(Debug, Default)]
pub struct DocumentRuntime {
    next_job_handle: u64,
    next_snapshot_id: u64,
    /// Per-document request sequence counter for freshness/stale detection.
    request_seq_by_document: BTreeMap<String, u64>,
    jobs: BTreeMap<DocumentJobHandle, JobEntry>,
    snapshots: BTreeMap<u64, DocumentSnapshot>,
    latest_snapshot_by_document: BTreeMap<String, SnapshotId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAccessError {
    Busy,
}

impl fmt::Display for RuntimeAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("document runtime is already borrowed"),
        }
    }
}

impl std::error::Error for RuntimeAccessError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartedDocumentJob {
    pub handle: DocumentJobHandle,
    pub request_seq: u64,
}

#[derive(Debug, Clone, Copy)]
enum SnapshotReadState<'a> {
    Ready(&'a DocumentSnapshot),
    Missing,
    DocumentIdentityMismatch,
    SemanticDataUnavailable,
}

thread_local! {
    static GLOBAL_DOCUMENT_RUNTIME: RefCell<DocumentRuntime> =
        RefCell::new(DocumentRuntime::default());
}

fn with_global_document_runtime_mut<R>(
    f: impl FnOnce(&mut DocumentRuntime) -> R,
) -> Result<R, RuntimeAccessError> {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        let mut runtime = runtime
            .try_borrow_mut()
            .map_err(|_| RuntimeAccessError::Busy)?;
        Ok(f(&mut runtime))
    })
}

fn with_global_document_runtime<R>(
    f: impl FnOnce(&DocumentRuntime) -> R,
) -> Result<R, RuntimeAccessError> {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        let runtime = runtime.try_borrow().map_err(|_| RuntimeAccessError::Busy)?;
        Ok(f(&runtime))
    })
}

impl DocumentRuntime {
    pub(crate) fn insert_job(&mut self, spec: DocumentJobSpec) -> StartedDocumentJob {
        let handle = self.allocate_job_handle();
        let request_seq = self.allocate_request_seq(&spec.document_key);
        self.jobs.insert(
            handle,
            JobEntry {
                spec,
                request_seq,
                latest_snapshot_id: None,
                terminal: None,
                source_buffer: None,
                stream_state: None,
                ..Default::default()
            },
        );
        StartedDocumentJob {
            handle,
            request_seq,
        }
    }

    pub(crate) fn job(&self, handle: DocumentJobHandle) -> Option<&JobEntry> {
        self.jobs.get(&handle)
    }

    pub(crate) fn job_mut(&mut self, handle: DocumentJobHandle) -> Option<&mut JobEntry> {
        self.jobs.get_mut(&handle)
    }

    pub(crate) fn job_lifecycle(
        &self,
        handle: DocumentJobHandle,
    ) -> Option<(u64, Option<JobTerminal>)> {
        self.job(handle)
            .map(|entry| (entry.request_seq, entry.terminal.clone()))
    }

    pub(crate) fn job_snapshot_id(&self, handle: DocumentJobHandle) -> Option<SnapshotId> {
        self.job(handle).and_then(|entry| entry.latest_snapshot_id)
    }

    pub(crate) fn active_job_count(&self) -> usize {
        self.jobs
            .values()
            .filter(|entry| entry.terminal.is_none())
            .count()
    }

    pub(crate) fn contains_active_document(&self, document_key: &str) -> bool {
        self.jobs
            .values()
            .any(|entry| entry.terminal.is_none() && entry.spec.document_key == document_key)
    }

    pub(crate) fn latest_terminal_for_document(&self, document_key: &str) -> Option<JobTerminal> {
        self.jobs
            .iter()
            .rev()
            .find_map(|(_, entry)| {
                (entry.spec.document_key == document_key).then(|| entry.terminal.clone())
            })
            .flatten()
    }

    pub(crate) fn latest_job_spec_for_document(
        &self,
        document_key: &str,
    ) -> Option<DocumentJobSpec> {
        self.jobs.iter().rev().find_map(|(_, entry)| {
            (entry.spec.document_key == document_key).then(|| entry.spec.clone())
        })
    }

    pub(crate) fn snapshot(&self, snapshot_id: SnapshotId) -> Option<&DocumentSnapshot> {
        self.snapshots.get(&snapshot_id.0)
    }

    pub(crate) fn newest_snapshot_for_document(
        &self,
        document_key: &str,
    ) -> Option<&DocumentSnapshot> {
        self.snapshots
            .values()
            .filter(|snapshot| snapshot.document_key == document_key)
            .max_by_key(|snapshot| snapshot.snapshot_id.0)
    }

    #[cfg(test)]
    pub(crate) fn insert_job_entry_for_test(&mut self, handle: DocumentJobHandle, entry: JobEntry) {
        self.jobs.insert(handle, entry);
    }

    #[cfg(test)]
    pub(crate) fn snapshot_count_for_test(&self) -> usize {
        self.snapshots.len()
    }

    fn resolve_snapshot_read(
        &self,
        document_key: Option<&str>,
        snapshot_id: SnapshotId,
    ) -> SnapshotReadState<'_> {
        let Some(snapshot) = self.snapshot(snapshot_id) else {
            return SnapshotReadState::Missing;
        };
        if document_key.is_some_and(|key| snapshot.document_key != key) {
            return SnapshotReadState::DocumentIdentityMismatch;
        }
        if snapshot
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.document.as_ref())
            .is_none()
        {
            return SnapshotReadState::SemanticDataUnavailable;
        }
        SnapshotReadState::Ready(snapshot)
    }

    pub fn latest_authoritative_snapshot_id(&self, document_key: &str) -> Option<SnapshotId> {
        self.latest_snapshot_by_document.get(document_key).copied()
    }

    pub fn query_snapshot(
        &self,
        document_key: &str,
        query: &SnapshotQuery,
    ) -> SnapshotReadResult<QueryResult> {
        let SnapshotReadState::Ready(snapshot) =
            self.resolve_snapshot_read(Some(document_key), query.snapshot_id)
        else {
            return SnapshotReadResult::SnapshotNotReady;
        };
        SnapshotReadResult::Ready {
            data: snapshot.query(query),
        }
    }

    pub fn plan_graph_value_edit(
        &self,
        request: &GraphValueEditRequest,
    ) -> SnapshotReadResult<GraphValueEditPlan> {
        let SnapshotReadState::Ready(snapshot) =
            self.resolve_snapshot_read(Some(&request.document_key), request.snapshot_id)
        else {
            return SnapshotReadResult::SnapshotNotReady;
        };
        SnapshotReadResult::Ready {
            data: snapshot.plan_graph_value_edit(request),
        }
    }

    pub fn build_hover_subgraph_projection(
        &self,
        request: &ProjectionRequest,
    ) -> Result<SnapshotReadResult<ProjectionDelta>, &'static str> {
        let SnapshotReadState::Ready(snapshot) =
            self.resolve_snapshot_read(None, request.snapshot_id)
        else {
            return Ok(SnapshotReadResult::SnapshotNotReady);
        };
        super::reads::build_hover_subgraph_projection_for_snapshot(snapshot, &request.path)
            .map(|data| SnapshotReadResult::Ready { data })
    }

    pub(crate) fn allocate_job_handle(&mut self) -> DocumentJobHandle {
        self.next_job_handle = self.next_job_handle.saturating_add(1);
        DocumentJobHandle(self.next_job_handle)
    }

    /// Allocate a monotonic request_seq for the given document_key.
    /// Returns the new seq; the runtime uses this for freshness/stale detection.
    pub(crate) fn allocate_request_seq(&mut self, document_key: &str) -> u64 {
        let entry = self
            .request_seq_by_document
            .entry(document_key.to_owned())
            .or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }

    /// Check whether a given request_seq for a document is stale
    /// (i.e. a newer request_seq has been allocated since).
    pub(crate) fn is_stale(&self, document_key: &str, request_seq: u64) -> bool {
        self.request_seq_by_document
            .get(document_key)
            .is_some_and(|latest| *latest > request_seq)
    }

    fn allocate_snapshot_id(&mut self) -> SnapshotId {
        self.next_snapshot_id = self.next_snapshot_id.saturating_add(1);
        SnapshotId(self.next_snapshot_id)
    }

    pub(crate) fn store_snapshot_for_document(
        &mut self,
        document_key: &str,
        mut snapshot: DocumentSnapshot,
        authoritative: bool,
    ) -> SnapshotId {
        let snapshot_id = if snapshot.snapshot_id == SnapshotId::default() {
            self.allocate_snapshot_id()
        } else {
            self.next_snapshot_id = self.next_snapshot_id.max(snapshot.snapshot_id.0);
            snapshot.snapshot_id
        };
        snapshot.snapshot_id = snapshot_id;
        snapshot.document_key = document_key.to_owned();
        // Only update latest_snapshot_by_document for authoritative commits.
        // Transient/diagnostics-only commits store the snapshot but don't
        // replace the document's canonical latest.
        if authoritative {
            self.latest_snapshot_by_document
                .insert(document_key.to_owned(), snapshot_id);
        }
        self.snapshots.insert(snapshot_id.0, snapshot);
        for entry in self.jobs.values_mut() {
            if entry.spec.document_key == document_key && entry.terminal.is_none() {
                entry.latest_snapshot_id = Some(snapshot_id);
            }
        }
        snapshot_id
    }
}

pub fn start_global_job(spec: DocumentJobSpec) -> Result<StartedDocumentJob, RuntimeAccessError> {
    with_global_document_runtime_mut(|runtime| {
        with_global_document_engine_metrics(|metrics| {
            runtime.start_job_with_identity(metrics, spec)
        })
        .ok_or(RuntimeAccessError::Busy)
    })?
}

pub fn advance_global_job(
    handle: DocumentJobHandle,
    input: AdvanceInput,
) -> Result<EventBatch, RuntimeAccessError> {
    with_global_document_runtime_mut(|runtime| {
        with_global_document_engine_metrics(|metrics| runtime.advance_job(metrics, handle, input))
            .ok_or(RuntimeAccessError::Busy)
    })?
}

pub fn cancel_global_job(handle: DocumentJobHandle) -> Result<EventBatch, RuntimeAccessError> {
    with_global_document_runtime_mut(|runtime| {
        with_global_document_engine_metrics(|metrics| runtime.cancel_job(metrics, handle))
            .ok_or(RuntimeAccessError::Busy)
    })?
}

pub fn close_global_job(
    handle: DocumentJobHandle,
    terminal: JobTerminal,
) -> Result<EventBatch, RuntimeAccessError> {
    with_global_document_runtime_mut(|runtime| {
        with_global_document_engine_metrics(|metrics| runtime.close_job(metrics, handle, terminal))
            .ok_or(RuntimeAccessError::Busy)
    })?
}

pub fn query_global_snapshot(
    document_key: &str,
    query: &SnapshotQuery,
) -> Result<SnapshotReadResult<QueryResult>, RuntimeAccessError> {
    with_global_document_runtime(|runtime| runtime.query_snapshot(document_key, query))
}

pub fn plan_global_graph_value_edit(
    request: &GraphValueEditRequest,
) -> Result<SnapshotReadResult<GraphValueEditPlan>, RuntimeAccessError> {
    with_global_document_runtime(|runtime| runtime.plan_graph_value_edit(request))
}

pub fn build_global_hover_subgraph_projection(
    request: &ProjectionRequest,
) -> Result<SnapshotReadResult<ProjectionDelta>, &'static str> {
    with_global_document_runtime(|runtime| runtime.build_hover_subgraph_projection(request))
        .map_err(|_| "projection runtime error")?
}
