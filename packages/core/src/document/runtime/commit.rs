use super::super::job::entry::{DocumentJobHandle, JobEntry};
use super::super::protocol::{CommitMode, DocumentEvent, EventBatch, JobTerminal, SnapshotId};
use super::super::snapshot::DocumentSnapshot;
use super::{DocumentRuntime, with_global_document_runtime, with_global_document_runtime_mut};

impl DocumentRuntime {
    pub fn close_job_terminal(
        &mut self,
        handle: DocumentJobHandle,
        terminal: JobTerminal,
    ) -> EventBatch {
        let Some(entry) = self.job_mut(handle) else {
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

        let (terminal, events) = Self::terminal_with_latest_snapshot(entry, terminal);
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
        &mut self,
        handle: DocumentJobHandle,
        snapshot: DocumentSnapshot,
        mode: CommitMode,
    ) -> JobTerminal {
        let Some(entry) = self.job(handle) else {
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
                if self.is_stale(&document_key, request_seq) {
                    let snapshot_id =
                        self.store_snapshot_for_document(&document_key, snapshot, false);
                    if let Some(entry) = self.job_mut(handle) {
                        entry.latest_snapshot_id = Some(snapshot_id);
                        entry.terminal = Some(JobTerminal::Completed);
                    }
                    JobTerminal::Completed
                } else {
                    let snapshot_id =
                        self.store_snapshot_for_document(&document_key, snapshot, true);
                    if let Some(entry) = self.job_mut(handle) {
                        entry.latest_snapshot_id = Some(snapshot_id);
                        entry.terminal = Some(JobTerminal::Completed);
                    }
                    JobTerminal::Completed
                }
            }
            CommitMode::DiagnosticsOnly => {
                let mut diag_snapshot = snapshot;
                diag_snapshot.graph = None;
                let snapshot_id =
                    self.store_snapshot_for_document(&document_key, diag_snapshot, false);
                if let Some(entry) = self.job_mut(handle) {
                    entry.latest_snapshot_id = Some(snapshot_id);
                    entry.terminal = Some(JobTerminal::ParseFailed);
                }
                JobTerminal::ParseFailed
            }
        }
    }
}

pub fn close_job_terminal(
    runtime: &mut DocumentRuntime,
    handle: DocumentJobHandle,
    terminal: JobTerminal,
) -> EventBatch {
    runtime.close_job_terminal(handle, terminal)
}

pub fn commit_snapshot(
    runtime: &mut DocumentRuntime,
    handle: DocumentJobHandle,
    snapshot: DocumentSnapshot,
    mode: CommitMode,
) -> JobTerminal {
    runtime.commit_snapshot(handle, snapshot, mode)
}

pub fn store_snapshot_for_document(
    document_key: &str,
    snapshot: DocumentSnapshot,
    authoritative: bool,
) -> Option<SnapshotId> {
    with_global_document_runtime_mut(|runtime| {
        runtime.store_snapshot_for_document(document_key, snapshot, authoritative)
    })
    .ok()
}

pub fn stored_snapshot_for_document(document_key: &str) -> Option<DocumentSnapshot> {
    with_global_document_runtime(|runtime| {
        runtime.newest_snapshot_for_document(document_key).cloned()
    })
    .ok()
    .flatten()
}
