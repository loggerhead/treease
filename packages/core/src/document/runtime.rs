use std::cell::RefCell;
use std::collections::BTreeMap;

mod commit;
mod testing;

use super::job::entry::{DocumentJobHandle, JobEntry};
use super::protocol::SnapshotId;
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
    pub next_job_handle: u64,
    pub next_snapshot_id: u64,
    /// Per-document request sequence counter for freshness/stale detection.
    pub request_seq_by_document: BTreeMap<String, u64>,
    pub jobs: BTreeMap<DocumentJobHandle, JobEntry>,
    pub snapshots: BTreeMap<u64, DocumentSnapshot>,
    pub latest_snapshot_by_document: BTreeMap<String, SnapshotId>,
}

thread_local! {
    static GLOBAL_DOCUMENT_RUNTIME: RefCell<DocumentRuntime> =
        RefCell::new(DocumentRuntime::default());
}

pub(crate) fn with_global_document_runtime<R>(
    f: impl FnOnce(&mut DocumentRuntime) -> R,
) -> Option<R> {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        let mut runtime = runtime.try_borrow_mut().ok()?;
        Some(f(&mut runtime))
    })
}

impl DocumentRuntime {
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

    fn store_snapshot_for_document(
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
