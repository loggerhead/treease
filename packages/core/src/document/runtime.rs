use std::cell::RefCell;
use std::collections::BTreeMap;

use super::job_entry::{DocumentJobHandle, JobEntry};
use super::protocol::{
    CommitMode, DocumentEvent, DocumentJobSpec, EventBatch, JobTerminal, SnapshotId,
};
use super::snapshot::DocumentSnapshot;

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

pub fn close_job_terminal(
    runtime: &mut DocumentRuntime,
    handle: DocumentJobHandle,
    terminal: JobTerminal,
) -> EventBatch {
    let Some(entry) = runtime.jobs.get_mut(&handle) else {
        return EventBatch {
            request_seq: 0,
            events: Vec::new(),
            terminal: Some(JobTerminal::Rejected {
                code: "document_runtime_missing_job".into(),
                detail: format!("No document runtime job registered for handle {}", handle.0),
            }),
        };
    };
    let request_seq = entry.request_seq;

    let (terminal, events) = terminal_with_latest_snapshot(entry, terminal);
    let terminal = entry.terminal.get_or_insert(terminal).clone();
    EventBatch {
        request_seq,
        events,
        terminal: Some(terminal),
    }
}

fn terminal_with_latest_snapshot(
    entry: &JobEntry,
    terminal: JobTerminal,
) -> (JobTerminal, Vec<DocumentEvent>) {
    match (terminal, entry.latest_snapshot_id) {
        (JobTerminal::Completed, Some(snapshot_id)) => (
            JobTerminal::Completed,
            vec![DocumentEvent::SnapshotReady {
                snapshot_id,
                analysis: None,
                main_graph: None,
                source_text: None,
            }],
        ),
        (terminal, _) => (terminal, Vec::new()),
    }
}

pub fn commit_snapshot(
    runtime: &mut DocumentRuntime,
    handle: DocumentJobHandle,
    snapshot: DocumentSnapshot,
    mode: CommitMode,
) -> JobTerminal {
    let Some(entry) = runtime.jobs.get(&handle) else {
        return JobTerminal::Rejected {
            code: "document_runtime_missing_job".into(),
            detail: format!("No document runtime job registered for handle {}", handle.0),
        };
    };

    if let Some(terminal) = entry.terminal.clone() {
        return terminal;
    }

    let document_key = entry.spec.document_key.clone();
    let request_seq = entry.request_seq;

    match mode {
        CommitMode::Authoritative => {
            // Check freshness: if a newer request_seq exists, mark as stale.
            if runtime.is_stale(&document_key, request_seq) {
                // Store the snapshot but don't update latest (stale).
                let snapshot_id =
                    runtime.store_snapshot_for_document(&document_key, snapshot, false);
                if let Some(entry) = runtime.jobs.get_mut(&handle) {
                    entry.latest_snapshot_id = Some(snapshot_id);
                    entry.terminal = Some(JobTerminal::Completed);
                }
                JobTerminal::Completed
            } else {
                // Authoritative: store and mark as latest.
                let snapshot_id =
                    runtime.store_snapshot_for_document(&document_key, snapshot, true);
                if let Some(entry) = runtime.jobs.get_mut(&handle) {
                    entry.latest_snapshot_id = Some(snapshot_id);
                    entry.terminal = Some(JobTerminal::Completed);
                }
                JobTerminal::Completed
            }
        }

        CommitMode::DiagnosticsOnly => {
            // Diagnostics-only: store snapshot with diagnostics, clear graph.
            let mut diag_snapshot = snapshot;
            diag_snapshot.graph = None; // Clear any graph projection.
            let snapshot_id =
                runtime.store_snapshot_for_document(&document_key, diag_snapshot, false);
            if let Some(entry) = runtime.jobs.get_mut(&handle) {
                entry.latest_snapshot_id = Some(snapshot_id);
                entry.terminal = Some(JobTerminal::ParseFailed);
            }
            JobTerminal::ParseFailed
        }
    }
}

pub fn store_snapshot_for_document(
    document_key: &str,
    snapshot: DocumentSnapshot,
    authoritative: bool,
) -> Option<SnapshotId> {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        let mut runtime = runtime.try_borrow_mut().ok()?;
        Some(runtime.store_snapshot_for_document(document_key, snapshot, authoritative))
    })
}

pub fn stored_snapshot_for_document(document_key: &str) -> Option<DocumentSnapshot> {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        let runtime = runtime.try_borrow().ok()?;
        runtime
            .snapshots
            .values()
            .filter(|snapshot| snapshot.document_key == document_key)
            .max_by_key(|snapshot| snapshot.snapshot_id.0)
            .cloned()
    })
}

pub fn reset_runtime_for_tests() {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        if let Ok(mut runtime) = runtime.try_borrow_mut() {
            *runtime = DocumentRuntime::default();
        }
    });
}

pub fn document_runtime_job_count_for_tests() -> usize {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        runtime
            .try_borrow()
            .map(|runtime| {
                runtime
                    .jobs
                    .values()
                    .filter(|entry| entry.terminal.is_none())
                    .count()
            })
            .unwrap_or_default()
    })
}

pub fn document_runtime_contains_document_for_tests(document_key: &str) -> bool {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        runtime
            .try_borrow()
            .map(|runtime| {
                runtime.jobs.values().any(|entry| {
                    entry.terminal.is_none() && entry.spec.document_key == document_key
                })
            })
            .unwrap_or(false)
    })
}

pub fn document_runtime_terminal_for_document_for_tests(document_key: &str) -> Option<JobTerminal> {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        runtime.try_borrow().ok().and_then(|runtime| {
            runtime
                .jobs
                .iter()
                .rev()
                .find_map(|(_, entry)| {
                    (entry.spec.document_key == document_key).then(|| entry.terminal.clone())
                })
                .flatten()
        })
    })
}

pub fn document_runtime_latest_job_spec_for_document_for_tests(
    document_key: &str,
) -> Option<DocumentJobSpec> {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        runtime.try_borrow().ok().and_then(|runtime| {
            runtime.jobs.iter().rev().find_map(|(_, entry)| {
                (entry.spec.document_key == document_key).then(|| entry.spec.clone())
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::protocol::{DocumentInputPlan, DocumentJobKind, OutputPlan};
    use crate::document::snapshot::GraphProjection;

    fn job_spec(document_key: &str) -> DocumentJobSpec {
        DocumentJobSpec {
            kind: DocumentJobKind::AnalyzeSource,
            document_key: document_key.to_owned(),
            language: "json".to_owned(),
            input: DocumentInputPlan::SourceText,
            settings: crate::document::protocol::DocumentJobSettings::default(),
            output: OutputPlan {
                analysis: true,
                graph: true,
            },
            base_snapshot_id: None,
            edits: vec![],
        }
    }

    fn snapshot(document_key: &str) -> DocumentSnapshot {
        DocumentSnapshot {
            document_key: document_key.to_owned(),
            graph: Some(GraphProjection {
                ready: true,
                clear: true,
                graph_data: None,
            }),
            ..DocumentSnapshot::default()
        }
    }

    fn insert_job(
        runtime: &mut DocumentRuntime,
        document_key: &str,
        request_seq: u64,
    ) -> DocumentJobHandle {
        let handle = runtime.allocate_job_handle();
        runtime.jobs.insert(
            handle,
            JobEntry {
                spec: job_spec(document_key),
                request_seq,
                latest_snapshot_id: None,
                terminal: None,
                source_buffer: None,
                source_bytes: None,
                stream_state: None,
                line_count: 0,
            },
        );
        handle
    }

    #[test]
    fn close_job_terminal_is_idempotent_and_emits_snapshot_ready_once() {
        let mut runtime = DocumentRuntime::default();
        let handle = insert_job(&mut runtime, "doc-close", 1);
        runtime
            .jobs
            .get_mut(&handle)
            .expect("job should exist")
            .latest_snapshot_id = Some(SnapshotId(7));

        let first = close_job_terminal(&mut runtime, handle, JobTerminal::Completed);
        assert_eq!(
            first.events,
            vec![DocumentEvent::SnapshotReady {
                snapshot_id: SnapshotId(7),
                analysis: None,
                main_graph: None,
                source_text: None,
            }]
        );
        assert_eq!(first.terminal, Some(JobTerminal::Completed));

        let second = close_job_terminal(&mut runtime, handle, JobTerminal::Cancelled);
        assert!(second.events.is_empty());
        assert_eq!(second.terminal, first.terminal);
    }

    #[test]
    fn diagnostics_only_commit_clears_graph_and_does_not_replace_latest_snapshot() {
        let mut runtime = DocumentRuntime::default();
        let initial = runtime.store_snapshot_for_document(
            "doc-diagnostics",
            snapshot("doc-diagnostics"),
            true,
        );
        let handle = insert_job(&mut runtime, "doc-diagnostics", 1);

        let terminal = commit_snapshot(
            &mut runtime,
            handle,
            snapshot("doc-diagnostics"),
            CommitMode::DiagnosticsOnly,
        );
        assert!(matches!(terminal, JobTerminal::ParseFailed));
        let diagnostics_snapshot_id = runtime
            .jobs
            .get(&handle)
            .and_then(|entry| entry.latest_snapshot_id)
            .expect("diagnostics snapshot id should be recorded");

        assert_eq!(
            runtime.latest_snapshot_by_document.get("doc-diagnostics"),
            Some(&initial)
        );
        assert_eq!(
            runtime
                .snapshots
                .get(&diagnostics_snapshot_id.0)
                .and_then(|stored| stored.graph.as_ref()),
            None
        );
        assert_eq!(
            runtime
                .jobs
                .get(&handle)
                .and_then(|entry| entry.terminal.clone()),
            Some(JobTerminal::ParseFailed)
        );
    }

    #[test]
    fn stale_authoritative_commit_does_not_replace_latest_snapshot() {
        let mut runtime = DocumentRuntime::default();
        let latest = runtime.store_snapshot_for_document("doc-stale", snapshot("doc-stale"), true);
        runtime
            .request_seq_by_document
            .insert("doc-stale".to_owned(), 2);
        let handle = insert_job(&mut runtime, "doc-stale", 1);

        let terminal = commit_snapshot(
            &mut runtime,
            handle,
            snapshot("doc-stale"),
            CommitMode::Authoritative,
        );
        assert!(matches!(terminal, JobTerminal::Completed));
        let stale_snapshot_id = runtime
            .jobs
            .get(&handle)
            .and_then(|entry| entry.latest_snapshot_id)
            .expect("stale snapshot id should be recorded");

        assert_ne!(stale_snapshot_id, latest);
        assert_eq!(
            runtime.latest_snapshot_by_document.get("doc-stale"),
            Some(&latest)
        );
        assert_eq!(
            runtime
                .jobs
                .get(&handle)
                .and_then(|entry| entry.terminal.clone()),
            Some(JobTerminal::Completed)
        );
    }

    #[test]
    fn terminal_identity_lives_on_event_batch_request_seq() {
        let mut runtime = DocumentRuntime::default();
        let handle = insert_job(&mut runtime, "doc-req-seq", 42);

        let terminal = commit_snapshot(
            &mut runtime,
            handle,
            snapshot("doc-req-seq"),
            CommitMode::Authoritative,
        );
        assert!(matches!(terminal, JobTerminal::Completed));

        let batch = close_job_terminal(&mut runtime, handle, JobTerminal::Completed);
        assert_eq!(batch.request_seq, 42);
        assert_eq!(batch.terminal, Some(JobTerminal::Completed));
    }

    #[test]
    fn parse_failed_terminal_is_lifecycle_only() {
        let mut runtime = DocumentRuntime::default();
        let handle = insert_job(&mut runtime, "doc-parse-fail-seq", 7);

        let terminal = commit_snapshot(
            &mut runtime,
            handle,
            snapshot("doc-parse-fail-seq"),
            CommitMode::DiagnosticsOnly,
        );
        assert!(matches!(terminal, JobTerminal::ParseFailed));

        let entry = runtime.jobs.get(&handle).expect("job should exist");
        assert_eq!(entry.request_seq, 7);
        assert!(entry.latest_snapshot_id.is_some());
        assert_eq!(entry.terminal, Some(JobTerminal::ParseFailed));
    }
}
