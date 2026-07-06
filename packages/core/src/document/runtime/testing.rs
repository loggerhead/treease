use super::super::protocol::{DocumentJobSpec, JobTerminal};
use super::DocumentRuntime;
use super::GLOBAL_DOCUMENT_RUNTIME;

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
    use super::super::super::job::entry::{DocumentJobHandle, JobEntry};
    use super::super::super::protocol::{
        CommitMode, DocumentEvent, DocumentInputPlan, DocumentJobKind, OutputPlan, SnapshotId,
    };
    use super::super::super::snapshot::{DocumentSnapshot, GraphProjection};
    use super::*;
    use crate::document::runtime::{close_job_terminal, commit_snapshot};

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
