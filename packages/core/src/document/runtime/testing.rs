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
            .map(|runtime| runtime.active_job_count())
            .unwrap_or_default()
    })
}

pub fn document_runtime_contains_document_for_tests(document_key: &str) -> bool {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        runtime
            .try_borrow()
            .map(|runtime| runtime.contains_active_document(document_key))
            .unwrap_or(false)
    })
}

pub fn document_runtime_terminal_for_document_for_tests(document_key: &str) -> Option<JobTerminal> {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        runtime
            .try_borrow()
            .ok()
            .and_then(|runtime| runtime.latest_terminal_for_document(document_key))
    })
}

pub fn document_runtime_latest_job_spec_for_document_for_tests(
    document_key: &str,
) -> Option<DocumentJobSpec> {
    GLOBAL_DOCUMENT_RUNTIME.with(|runtime| {
        runtime
            .try_borrow()
            .ok()
            .and_then(|runtime| runtime.latest_job_spec_for_document(document_key))
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::job::entry::{DocumentJobHandle, JobEntry};
    use super::super::super::protocol::{
        CommitMode, DocumentEvent, DocumentInputPlan, DocumentJobKind, GraphPathSeg,
        GraphValueEditFallbackReason, GraphValueEditRequest, OutputPlan, ProjectionRequest,
        QueryKind, SnapshotId, SnapshotQuery, SnapshotReadResult,
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

    fn semantic_snapshot(document_key: &str, source: &str) -> DocumentSnapshot {
        let materialized = crate::document::materialize(
            &DocumentInputPlan::SourceText,
            document_key,
            "json",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[],
            None,
        );
        DocumentSnapshot::with_analysis(document_key, materialized.analysis)
    }

    fn edit_request(
        document_key: &str,
        snapshot_id: SnapshotId,
        path: Vec<GraphPathSeg>,
    ) -> GraphValueEditRequest {
        GraphValueEditRequest {
            document_key: document_key.to_owned(),
            snapshot_id,
            language: "json".to_owned(),
            path,
            prefer_key: false,
            value: serde_json::json!("updated"),
        }
    }

    fn insert_job(
        runtime: &mut DocumentRuntime,
        document_key: &str,
        request_seq: u64,
    ) -> DocumentJobHandle {
        let handle = runtime.allocate_job_handle();
        runtime.insert_job_entry_for_test(
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
        let mut stored = snapshot("doc-close");
        stored.snapshot_id = SnapshotId(7);
        runtime.store_snapshot_for_document("doc-close", stored, true);

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
            .job_snapshot_id(handle)
            .expect("diagnostics snapshot id should be recorded");

        assert_eq!(
            runtime.latest_authoritative_snapshot_id("doc-diagnostics"),
            Some(initial)
        );
        assert_eq!(
            runtime
                .snapshot(diagnostics_snapshot_id)
                .and_then(|stored| stored.graph.as_ref()),
            None
        );
        assert_eq!(
            runtime.job(handle).and_then(|entry| entry.terminal.clone()),
            Some(JobTerminal::ParseFailed)
        );
    }

    #[test]
    fn stale_authoritative_commit_does_not_replace_latest_snapshot() {
        let mut runtime = DocumentRuntime::default();
        let latest = runtime.store_snapshot_for_document("doc-stale", snapshot("doc-stale"), true);
        runtime.allocate_request_seq("doc-stale");
        runtime.allocate_request_seq("doc-stale");
        let handle = insert_job(&mut runtime, "doc-stale", 1);

        let terminal = commit_snapshot(
            &mut runtime,
            handle,
            snapshot("doc-stale"),
            CommitMode::Authoritative,
        );
        assert!(matches!(terminal, JobTerminal::Completed));
        let stale_snapshot_id = runtime
            .job_snapshot_id(handle)
            .expect("stale snapshot id should be recorded");

        assert_ne!(stale_snapshot_id, latest);
        assert_eq!(
            runtime.latest_authoritative_snapshot_id("doc-stale"),
            Some(latest)
        );
        assert_eq!(
            runtime.job(handle).and_then(|entry| entry.terminal.clone()),
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
    fn latest_active_job_does_not_inherit_older_terminal() {
        let mut runtime = DocumentRuntime::default();
        let mut metrics = crate::document::metrics::DocumentEngineMetrics::default();
        let older = runtime.start_job(&mut metrics, job_spec("doc-terminal"));
        runtime.close_job(&mut metrics, older, JobTerminal::Cancelled);
        runtime.start_job(&mut metrics, job_spec("doc-terminal"));

        let terminal = runtime.latest_terminal_for_document("doc-terminal");

        assert_eq!(terminal, None);
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

        let entry = runtime.job(handle).expect("job should exist");
        assert_eq!(entry.request_seq, 7);
        assert!(entry.latest_snapshot_id.is_some());
        assert_eq!(entry.terminal, Some(JobTerminal::ParseFailed));
    }

    #[test]
    fn runtime_state_machine_keeps_older_authoritative_commit_out_of_latest() {
        let mut runtime = DocumentRuntime::default();
        let mut metrics = crate::document::metrics::DocumentEngineMetrics::default();
        let older = runtime.start_job(&mut metrics, job_spec("doc-freshness"));
        let newer = runtime.start_job(&mut metrics, job_spec("doc-freshness"));

        runtime.commit_snapshot(newer, snapshot("doc-freshness"), CommitMode::Authoritative);
        let latest_after_newer = runtime
            .latest_authoritative_snapshot_id("doc-freshness")
            .expect("newer snapshot should become authoritative");

        runtime.commit_snapshot(older, snapshot("doc-freshness"), CommitMode::Authoritative);

        assert_eq!(
            runtime.latest_authoritative_snapshot_id("doc-freshness"),
            Some(latest_after_newer)
        );
    }

    #[test]
    fn runtime_snapshot_read_requires_matching_document_identity() {
        let mut runtime = DocumentRuntime::default();
        let snapshot_id =
            runtime.store_snapshot_for_document("doc-read", snapshot("doc-read"), true);
        let query = SnapshotQuery {
            snapshot_id,
            kind: QueryKind::RootValueKind,
            ..SnapshotQuery::default()
        };

        let result = runtime.query_snapshot("other-document", &query);

        assert_eq!(result, SnapshotReadResult::SnapshotNotReady);
    }

    #[test]
    fn runtime_snapshot_read_never_falls_back_to_latest_snapshot() {
        let mut runtime = DocumentRuntime::default();
        runtime.store_snapshot_for_document("doc-read", snapshot("doc-read"), true);
        let query = SnapshotQuery {
            snapshot_id: SnapshotId(u64::MAX),
            kind: QueryKind::RootValueKind,
            ..SnapshotQuery::default()
        };

        let result = runtime.query_snapshot("doc-read", &query);

        assert_eq!(result, SnapshotReadResult::SnapshotNotReady);
    }

    #[test]
    fn runtime_graph_value_edit_read_requires_explicit_snapshot() {
        let runtime = DocumentRuntime::default();
        let request = GraphValueEditRequest {
            document_key: "doc-read".to_owned(),
            snapshot_id: SnapshotId(1),
            language: "json".to_owned(),
            path: Vec::new(),
            prefer_key: false,
            value: serde_json::Value::Null,
        };

        let result = runtime.plan_graph_value_edit(&request);

        assert_eq!(result, SnapshotReadResult::SnapshotNotReady);
    }

    #[test]
    fn runtime_graph_value_edit_read_requires_matching_document_identity() {
        let mut runtime = DocumentRuntime::default();
        let snapshot_id = runtime.store_snapshot_for_document(
            "doc-read",
            semantic_snapshot("doc-read", r#"{"value":"old"}"#),
            true,
        );

        let result = runtime.plan_graph_value_edit(&edit_request(
            "other-document",
            snapshot_id,
            vec![GraphPathSeg {
                tag: 0,
                key: "value".to_owned(),
                index: 0,
            }],
        ));

        assert_eq!(result, SnapshotReadResult::SnapshotNotReady);
    }

    #[test]
    fn diagnostics_only_snapshot_is_not_semantically_readable_across_all_read_kinds() {
        let mut runtime = DocumentRuntime::default();
        let mut diagnostics = semantic_snapshot("doc-diagnostics-read", r#"{"value":1}"#);
        diagnostics
            .analysis
            .as_mut()
            .expect("analysis should exist")
            .document = None;
        let snapshot_id =
            runtime.store_snapshot_for_document("doc-diagnostics-read", diagnostics, false);

        let query = runtime.query_snapshot(
            "doc-diagnostics-read",
            &SnapshotQuery {
                snapshot_id,
                kind: QueryKind::RootValueKind,
                ..SnapshotQuery::default()
            },
        );
        let hover = runtime
            .build_hover_subgraph_projection(&ProjectionRequest {
                snapshot_id,
                path: "$".to_owned(),
            })
            .expect("unavailable semantic data is a read status");
        let plan = runtime.plan_graph_value_edit(&edit_request(
            "doc-diagnostics-read",
            snapshot_id,
            Vec::new(),
        ));

        assert_eq!(query, SnapshotReadResult::SnapshotNotReady);
        assert_eq!(hover, SnapshotReadResult::SnapshotNotReady);
        assert_eq!(plan, SnapshotReadResult::SnapshotNotReady);
    }

    #[test]
    fn runtime_reads_share_root_quoted_key_array_index_and_invalid_path_semantics() {
        let mut runtime = DocumentRuntime::default();
        let document_key = "doc-path-contract";
        let snapshot_id = runtime.store_snapshot_for_document(
            document_key,
            semantic_snapshot(document_key, r#"{"rows":[{"quoted.key":"old"}]}"#),
            true,
        );
        let quoted_path = r#"$.rows[0]["quoted.key"]"#;

        for path in ["$", quoted_path] {
            let query = runtime.query_snapshot(
                document_key,
                &SnapshotQuery {
                    snapshot_id,
                    kind: QueryKind::NodePreview,
                    path_pattern: Some(path.to_owned()),
                    ..SnapshotQuery::default()
                },
            );
            let hover = runtime
                .build_hover_subgraph_projection(&ProjectionRequest {
                    snapshot_id,
                    path: path.to_owned(),
                })
                .expect("valid path should project");

            assert!(matches!(query, SnapshotReadResult::Ready { .. }), "{path}");
            assert!(matches!(hover, SnapshotReadResult::Ready { .. }), "{path}");
        }

        let plan = runtime.plan_graph_value_edit(&edit_request(
            document_key,
            snapshot_id,
            vec![
                GraphPathSeg {
                    tag: 0,
                    key: "rows".to_owned(),
                    index: 0,
                },
                GraphPathSeg {
                    tag: 1,
                    key: String::new(),
                    index: 0,
                },
                GraphPathSeg {
                    tag: 0,
                    key: "quoted.key".to_owned(),
                    index: 0,
                },
            ],
        ));
        assert!(matches!(plan, SnapshotReadResult::Ready { .. }));

        let invalid_query = runtime.query_snapshot(
            document_key,
            &SnapshotQuery {
                snapshot_id,
                kind: QueryKind::NodePreview,
                path_pattern: Some("$.missing".to_owned()),
                ..SnapshotQuery::default()
            },
        );
        assert!(matches!(invalid_query, SnapshotReadResult::Ready { .. }));
        assert!(
            runtime
                .build_hover_subgraph_projection(&ProjectionRequest {
                    snapshot_id,
                    path: "$.missing".to_owned(),
                })
                .is_err()
        );
        let invalid_plan = runtime.plan_graph_value_edit(&edit_request(
            document_key,
            snapshot_id,
            vec![GraphPathSeg {
                tag: 0,
                key: "missing".to_owned(),
                index: 0,
            }],
        ));
        assert!(matches!(
            invalid_plan,
            SnapshotReadResult::Ready {
                data: super::super::super::protocol::GraphValueEditPlan {
                    reason: Some(GraphValueEditFallbackReason::InvalidPath),
                    ..
                }
            }
        ));
    }

    #[test]
    fn runtime_hover_projection_read_requires_explicit_snapshot() {
        let runtime = DocumentRuntime::default();
        let request = ProjectionRequest {
            snapshot_id: SnapshotId(1),
            path: "$".to_owned(),
        };

        let result = runtime
            .build_hover_subgraph_projection(&request)
            .expect("missing snapshot is a read status, not a projection error");

        assert_eq!(result, SnapshotReadResult::SnapshotNotReady);
    }

    #[test]
    fn global_runtime_reports_reentrant_access_as_explicit_error() {
        reset_runtime_for_tests();
        let query = SnapshotQuery {
            snapshot_id: SnapshotId(1),
            kind: QueryKind::RootValueKind,
            ..SnapshotQuery::default()
        };

        let nested = super::super::with_global_document_runtime_mut(|_| {
            super::super::query_global_snapshot("doc-read", &query)
        })
        .expect("outer runtime borrow should succeed");

        assert_eq!(nested, Err(super::super::RuntimeAccessError::Busy));
    }
}
