use super::job_entry::{DocumentJobHandle, JobEntry};
use super::metrics::{DocumentEngineMetrics, with_global_document_engine_metrics};
use super::protocol::{
    AdvanceInput, DocumentEvent, DocumentJobKind, DocumentJobSpec, EventBatch, JobTerminal,
    OutputPlan, ProjectionDelta, SnapshotId,
};
use super::runtime::{DocumentRuntime, close_job_terminal, with_global_document_runtime};
use super::stream_state::supports_streaming_language;
pub(crate) mod batch;
mod streaming;

fn rejected_batch(
    request_seq: u64,
    code: impl Into<String>,
    detail: impl Into<String>,
) -> EventBatch {
    EventBatch {
        request_seq,
        events: Vec::new(),
        terminal: Some(JobTerminal::Rejected {
            code: code.into(),
            detail: detail.into(),
        }),
    }
}

fn open_batch(request_seq: u64, events: Vec<DocumentEvent>) -> EventBatch {
    EventBatch {
        request_seq,
        events,
        terminal: None,
    }
}

fn terminal_batch(
    request_seq: u64,
    events: Vec<DocumentEvent>,
    terminal: JobTerminal,
) -> EventBatch {
    EventBatch {
        request_seq,
        events,
        terminal: Some(terminal),
    }
}

fn projection_from_graph(
    graph: Option<&super::snapshot::GraphProjection>,
) -> Option<ProjectionDelta> {
    graph.map(|projection| ProjectionDelta {
        clear: projection.clear,
        graph_data: projection.graph_data.clone(),
        ..Default::default()
    })
}

fn validate_snapshot_ready_outputs(
    graph: Option<&super::snapshot::GraphProjection>,
    output: &OutputPlan,
) -> Result<(), String> {
    if !output.graph {
        return Ok(());
    }
    let projection = graph.ok_or_else(|| "requested main graph was not produced".to_owned())?;
    if projection.graph_data.is_none() && !projection.clear {
        return Err("requested main graph was empty".to_owned());
    }
    Ok(())
}

fn snapshot_ready_event(
    snapshot_id: SnapshotId,
    analysis: &super::snapshot::AnalysisBundle,
    graph: Option<&super::snapshot::GraphProjection>,
    output: &OutputPlan,
) -> Result<DocumentEvent, String> {
    let analysis_payload = output
        .analysis
        .then(|| super::snapshot::analysis_payload_from_bundle(analysis, true));
    validate_snapshot_ready_outputs(graph, output)?;
    let main_graph = if output.graph {
        projection_from_graph(graph)
    } else {
        None
    };

    Ok(DocumentEvent::SnapshotReady {
        snapshot_id,
        analysis: analysis_payload,
        main_graph,
        source_text: None,
    })
}

fn parse_failed_events(
    snapshot_id: SnapshotId,
    analysis: &super::snapshot::AnalysisBundle,
) -> Vec<DocumentEvent> {
    vec![
        DocumentEvent::ParseFailed {
            snapshot_id,
            analysis: super::snapshot::analysis_payload_from_bundle(analysis, false),
        },
        DocumentEvent::ProjectionDelta {
            clear: true,
            graph_data: None,
            patch_seq: 0,
            base_graph_version: 0,
            graph_version: 0,
        },
    ]
}

fn with_snapshot_source_text(event: DocumentEvent, source_text: Option<&str>) -> DocumentEvent {
    match event {
        DocumentEvent::SnapshotReady {
            snapshot_id,
            analysis,
            main_graph,
            ..
        } => DocumentEvent::SnapshotReady {
            snapshot_id,
            analysis,
            main_graph,
            source_text: source_text.map(str::to_owned),
        },
        other => other,
    }
}

fn latest_job_snapshot_id(
    runtime: &DocumentRuntime,
    handle: DocumentJobHandle,
) -> Option<SnapshotId> {
    runtime
        .jobs
        .get(&handle)
        .and_then(|entry| entry.latest_snapshot_id)
}

pub(super) fn snapshot_events_for_terminal(
    runtime: &DocumentRuntime,
    handle: DocumentJobHandle,
    terminal: &JobTerminal,
    output: &OutputPlan,
    source_text: Option<&str>,
) -> Vec<DocumentEvent> {
    let Some(snapshot_id) = latest_job_snapshot_id(runtime, handle) else {
        return Vec::new();
    };
    let Some(snapshot) = runtime.snapshots.get(&snapshot_id.0) else {
        return Vec::new();
    };
    let Some(analysis) = snapshot.analysis.as_ref() else {
        return Vec::new();
    };

    match terminal {
        JobTerminal::Completed => {
            snapshot_ready_event(snapshot_id, analysis, snapshot.graph.as_ref(), output)
                .ok()
                .map(|event| with_snapshot_source_text(event, source_text))
                .into_iter()
                .collect()
        }
        JobTerminal::ParseFailed => parse_failed_events(snapshot_id, analysis),
        _ => Vec::new(),
    }
}

pub fn start_job(
    runtime: &mut DocumentRuntime,
    metrics: &mut DocumentEngineMetrics,
    spec: DocumentJobSpec,
) -> DocumentJobHandle {
    let handle = runtime.allocate_job_handle();
    let request_seq = runtime.allocate_request_seq(&spec.document_key);
    runtime.jobs.insert(
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
    metrics.record_job_started();
    handle
}

pub fn advance_job(
    runtime: &mut DocumentRuntime,
    metrics: &mut DocumentEngineMetrics,
    handle: DocumentJobHandle,
    input: AdvanceInput,
) -> EventBatch {
    let Some(existing) = runtime.jobs.get(&handle) else {
        let batch = rejected_batch(
            0,
            "document_runtime_missing_job",
            format!("No document runtime job registered for handle {}", handle.0),
        );
        metrics.record_job_advanced(&batch);
        return batch;
    };
    let request_seq = existing.request_seq;

    if existing.terminal.is_some() {
        let batch = EventBatch {
            request_seq,
            events: Vec::new(),
            terminal: existing.terminal.clone(),
        };
        metrics.record_job_advanced(&batch);
        return batch;
    }

    let batch = match input {
        AdvanceInput::TextChunk(text) => {
            let entry = runtime
                .jobs
                .get_mut(&handle)
                .expect("job existence checked above");
            let language = &entry.spec.language;
            let is_streaming = matches!(entry.spec.kind, DocumentJobKind::AnalyzeSource)
                && supports_streaming_language(language);

            if is_streaming {
                streaming::advance_streaming_text_chunk(entry, request_seq, text)
            } else {
                entry.append_source_text(&text);
                open_batch(request_seq, Vec::new())
            }
        }
        AdvanceInput::BinaryChunk(bytes) => {
            let entry = runtime
                .jobs
                .get_mut(&handle)
                .expect("job existence checked above");
            let language = &entry.spec.language;
            let is_streaming = matches!(entry.spec.kind, DocumentJobKind::AnalyzeSource)
                && supports_streaming_language(language);

            if is_streaming {
                streaming::advance_streaming_binary_chunk(entry, request_seq, bytes)
            } else {
                entry.append_source_bytes(&bytes);
                open_batch(request_seq, Vec::new())
            }
        }
        AdvanceInput::Close => batch::advance_close(runtime, handle, request_seq),
        AdvanceInput::Poll => EventBatch {
            request_seq,
            events: Vec::new(),
            terminal: None,
        },
    };
    metrics.record_job_advanced(&batch);
    batch
}

pub fn close_job(
    runtime: &mut DocumentRuntime,
    metrics: &mut DocumentEngineMetrics,
    handle: DocumentJobHandle,
    terminal: JobTerminal,
) -> EventBatch {
    let batch = close_job_terminal(runtime, handle, terminal);
    metrics.record_job_closed(&batch);
    batch
}

pub fn cancel_job(
    runtime: &mut DocumentRuntime,
    metrics: &mut DocumentEngineMetrics,
    handle: DocumentJobHandle,
) -> EventBatch {
    let (batch, should_record_cancel) = if let Some(entry) = runtime.jobs.get(&handle) {
        if let Some(terminal) = entry.terminal.clone() {
            (
                EventBatch {
                    request_seq: entry.request_seq,
                    events: Vec::new(),
                    terminal: Some(terminal),
                },
                false,
            )
        } else {
            (
                close_job_terminal(runtime, handle, JobTerminal::Cancelled),
                true,
            )
        }
    } else {
        (
            rejected_batch(
                0,
                "document_runtime_missing_job",
                format!("No document runtime job registered for handle {}", handle.0),
            ),
            true,
        )
    };
    if should_record_cancel {
        metrics.record_job_cancelled(&batch);
    } else {
        metrics.record_job_closed(&batch);
    }
    batch
}

pub fn start_global_job(spec: DocumentJobSpec) -> Option<DocumentJobHandle> {
    with_global_document_runtime(|runtime| {
        with_global_document_engine_metrics(|metrics| start_job(runtime, metrics, spec))
    })?
}

pub fn cancel_global_job(handle: DocumentJobHandle) -> Option<EventBatch> {
    with_global_document_runtime(|runtime| {
        with_global_document_engine_metrics(|metrics| cancel_job(runtime, metrics, handle))
    })?
}

pub fn advance_global_job(handle: DocumentJobHandle, input: AdvanceInput) -> Option<EventBatch> {
    with_global_document_runtime(|runtime| {
        with_global_document_engine_metrics(|metrics| advance_job(runtime, metrics, handle, input))
    })?
}

pub fn close_global_job(handle: DocumentJobHandle, terminal: JobTerminal) -> Option<EventBatch> {
    with_global_document_runtime(|runtime| {
        with_global_document_engine_metrics(|metrics| close_job(runtime, metrics, handle, terminal))
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DocumentTextEdit;
    use crate::document::protocol::DocumentEvent;
    use crate::document::protocol::{
        DocumentInputPlan, DocumentJobKind, GraphPathSeg, GraphValueEditPlanMode,
        GraphValueEditRequest, OutputPlan,
    };
    use crate::document::snapshot::DocumentSnapshot;

    fn make_spec(key: &str) -> DocumentJobSpec {
        DocumentJobSpec {
            kind: DocumentJobKind::AnalyzeSource,
            document_key: key.to_owned(),
            language: "json".to_owned(),
            input: DocumentInputPlan::SourceText,
            settings: crate::document::protocol::DocumentJobSettings::default(),
            output: OutputPlan {
                analysis: true,
                graph: false,
            },
            base_snapshot_id: None,
            edits: vec![],
        }
    }

    fn add_job(rt: &mut DocumentRuntime, key: &str) -> DocumentJobHandle {
        let h = rt.allocate_job_handle();
        let seq = rt.allocate_request_seq(key);
        rt.jobs.insert(
            h,
            JobEntry {
                spec: make_spec(key),
                request_seq: seq,
                latest_snapshot_id: None,
                terminal: None,
                source_buffer: None,
                source_bytes: None,
                stream_state: None,
                line_count: 0,
            },
        );
        h
    }

    #[test]
    fn validate_snapshot_ready_outputs_accepts_requested_graph_data() {
        let graph = super::super::snapshot::GraphProjection {
            ready: true,
            clear: true,
            graph_data: Some(Default::default()),
        };
        let output = OutputPlan {
            analysis: true,
            graph: true,
        };

        assert!(validate_snapshot_ready_outputs(Some(&graph), &output).is_ok());
    }

    #[test]
    fn validate_snapshot_ready_outputs_accepts_requested_clear_only_graph() {
        let graph = super::super::snapshot::GraphProjection {
            ready: true,
            clear: true,
            graph_data: None,
        };
        let output = OutputPlan {
            analysis: false,
            graph: true,
        };

        assert!(validate_snapshot_ready_outputs(Some(&graph), &output).is_ok());
    }

    #[test]
    fn validate_snapshot_ready_outputs_rejects_requested_non_clear_empty_graph() {
        let graph = super::super::snapshot::GraphProjection {
            ready: true,
            clear: false,
            graph_data: None,
        };
        let output = OutputPlan {
            analysis: false,
            graph: true,
        };

        assert_eq!(
            validate_snapshot_ready_outputs(Some(&graph), &output),
            Err("requested main graph was empty".to_owned()),
        );
    }

    #[test]
    fn advance_job_text_chunk_accumulates_source() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_job(&mut rt, "t1");
        let b = advance_job(&mut rt, &mut m, h, AdvanceInput::TextChunk("hello ".into()));
        assert!(b.terminal.is_none());
        // Streaming JSON jobs no longer store source in entry.source_buffer;
        // the source is managed by StreamingSourceDoc inside the stream state.
        assert_eq!(rt.jobs.get(&h).unwrap().source_buffer.as_deref(), None);
    }

    #[test]
    fn advance_job_poll_returns_no_terminal() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_job(&mut rt, "t2");
        let b = advance_job(&mut rt, &mut m, h, AdvanceInput::Poll);
        assert!(b.events.is_empty() && b.terminal.is_none());
    }

    #[test]
    fn advance_job_close_materializes_analyze_source() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_job(&mut rt, "t3");
        advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::TextChunk(r#"{"k":1}"#.into()),
        );
        let b = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        assert!(matches!(b.terminal, Some(JobTerminal::Completed)));
        let id = b
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady { snapshot_id, .. } => Some(*snapshot_id),
                _ => None,
            })
            .expect("Close should emit SnapshotReady");
        assert!(rt.snapshots.contains_key(&id.0));
        assert_eq!(rt.snapshots.get(&id.0).unwrap().document_key, "t3");
    }

    #[test]
    fn advance_job_missing_job_returns_rejected() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let b = advance_job(&mut rt, &mut m, DocumentJobHandle(999), AdvanceInput::Close);
        assert!(matches!(b.terminal, Some(JobTerminal::Rejected { .. })));
    }

    #[test]
    fn advance_job_idempotent_close() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_job(&mut rt, "t4");
        advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::TextChunk(r#"{"k":2}"#.into()),
        );
        let first = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        assert!(first.terminal.is_some());
        let second = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        assert_eq!(first.terminal, second.terminal);
    }

    #[test]
    fn advance_job_binary_chunk_accumulates_source_bytes() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_job(&mut rt, "t5");
        let b = advance_job(&mut rt, &mut m, h, AdvanceInput::BinaryChunk(vec![1]));
        assert!(b.terminal.is_none());
        // Streaming JSON jobs no longer store source in entry.source_bytes;
        // the source is managed by StreamingSourceDoc inside the stream state.
        assert_eq!(rt.jobs.get(&h).unwrap().source_bytes.as_deref(), None);
    }
    #[test]
    fn streaming_json_binary_chunks_accept_split_utf8_and_commit_snapshot() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_streaming_job(&mut rt, "binary-json-split-utf8");
        let source = r#"{"k":"你","n":1}"#;
        let bytes = source.as_bytes();
        let split = source.find('你').expect("fixture contains utf8 scalar") + 1;

        let first = advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::BinaryChunk(bytes[..split].to_vec()),
        );
        assert!(first.terminal.is_none());

        let second = advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::BinaryChunk(bytes[split..].to_vec()),
        );
        assert!(second.terminal.is_none());

        let close = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        assert!(matches!(close.terminal, Some(JobTerminal::Completed)));
        let snapshot_id = close
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady { snapshot_id, .. } => Some(*snapshot_id),
                _ => None,
            })
            .expect("binary streaming close should emit SnapshotReady");
        let snapshot = rt
            .snapshots
            .get(&snapshot_id.0)
            .expect("snapshot should be committed");
        assert_eq!(snapshot.analysis.as_ref().unwrap().source, source);
        assert!(
            snapshot
                .incremental
                .as_ref()
                .is_some_and(|state| state.can_resume)
        );
    }

    #[test]
    fn non_streaming_binary_chunks_accumulate_until_close() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = rt.allocate_job_handle();
        let seq = rt.allocate_request_seq("binary-csv");
        rt.jobs.insert(
            h,
            JobEntry {
                spec: DocumentJobSpec {
                    kind: DocumentJobKind::AnalyzeSource,
                    document_key: "binary-csv".into(),
                    language: "csv".into(),
                    input: DocumentInputPlan::SourceText,
                    settings: crate::document::protocol::DocumentJobSettings::default(),
                    output: OutputPlan {
                        analysis: true,
                        graph: true,
                    },
                    base_snapshot_id: None,
                    edits: vec![],
                },
                request_seq: seq,
                latest_snapshot_id: None,
                terminal: None,
                source_buffer: None,
                source_bytes: None,
                stream_state: None,
                line_count: 0,
            },
        );

        let first = advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::BinaryChunk(b"a,b\n".to_vec()),
        );
        assert!(first.terminal.is_none());
        assert!(first.events.is_empty());

        let second = advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::BinaryChunk(b"1,2".to_vec()),
        );
        assert!(second.terminal.is_none());
        assert!(second.events.is_empty());

        let close = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        assert!(matches!(close.terminal, Some(JobTerminal::Completed)));
        let snapshot_id = close
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady { snapshot_id, .. } => Some(*snapshot_id),
                _ => None,
            })
            .expect("csv binary close should emit SnapshotReady");
        let snapshot = rt.snapshots.get(&snapshot_id.0).unwrap();
        assert_eq!(snapshot.analysis.as_ref().unwrap().source, "a,b\n1,2");
    }

    #[test]
    fn binary_chunks_reject_invalid_utf8_on_close_without_snapshot() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_job(&mut rt, "binary-invalid-utf8");

        let chunk = advance_job(&mut rt, &mut m, h, AdvanceInput::BinaryChunk(vec![0xff]));
        assert!(chunk.terminal.is_none());

        let close = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        assert!(matches!(
            close.terminal,
            Some(JobTerminal::Rejected { ref code, .. }) if code == "invalid_utf8_source"
        ));
        assert!(rt.snapshots.is_empty());
    }

    #[test]
    fn advance_job_apply_edits_without_base_snapshot_is_rejected() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = rt.allocate_job_handle();
        let seq = rt.allocate_request_seq("t6");
        rt.jobs.insert(
            h,
            JobEntry {
                spec: DocumentJobSpec {
                    kind: DocumentJobKind::ApplyEdits,
                    document_key: "t6".into(),
                    language: "json".into(),
                    input: DocumentInputPlan::BaseTextWithEdits,
                    settings: crate::document::protocol::DocumentJobSettings::default(),
                    output: OutputPlan {
                        analysis: true,
                        graph: false,
                    },
                    base_snapshot_id: None,
                    edits: vec![DocumentTextEdit {
                        start_byte: 0,
                        old_end_byte: 0,
                        new_end_byte: 0,
                        replacement: "x".into(),
                    }],
                },
                request_seq: seq,
                latest_snapshot_id: None,
                terminal: None,
                source_buffer: Some(r#"{"k":1}"#.into()),
                source_bytes: None,
                stream_state: None,
                line_count: 0,
            },
        );
        let b = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        assert!(matches!(b.terminal, Some(JobTerminal::Rejected { .. })));
    }
    #[test]
    fn advance_job_apply_edits_with_base_snapshot_emits_snapshot_ready() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();

        let source = r#"{"root":{"k":1}}"#;
        let edit_start = source.find('1').expect("fixture should contain value") as u32;
        let base_result = crate::document::materialize::materialize(
            &DocumentInputPlan::SourceText,
            "t6-base",
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
        let base_snapshot_id = SnapshotId(1);
        let mut base_snapshot = DocumentSnapshot::with_analysis("t6-base", base_result.analysis);
        base_snapshot.snapshot_id = base_snapshot_id;
        base_snapshot.graph = base_result.graph;
        base_snapshot.incremental = base_result.incremental;
        rt.next_snapshot_id = base_snapshot_id.0;
        rt.snapshots.insert(base_snapshot_id.0, base_snapshot);
        rt.latest_snapshot_by_document
            .insert("t6-base".into(), base_snapshot_id);

        let h = start_job(
            &mut rt,
            &mut m,
            DocumentJobSpec {
                kind: DocumentJobKind::ApplyEdits,
                document_key: "t6-base".into(),
                language: "json".into(),
                input: DocumentInputPlan::BaseTextWithEdits,
                settings: crate::document::protocol::DocumentJobSettings::default(),
                output: OutputPlan {
                    analysis: true,
                    graph: true,
                },
                base_snapshot_id: Some(base_snapshot_id),
                edits: vec![DocumentTextEdit {
                    start_byte: edit_start,
                    old_end_byte: edit_start + 1,
                    new_end_byte: edit_start + 1,
                    replacement: "2".into(),
                }],
            },
        );

        let batch = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);

        assert!(matches!(batch.terminal, Some(JobTerminal::Completed)));
        let (snapshot_id, analysis, main_graph) = batch
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady {
                    snapshot_id,
                    analysis,
                    main_graph,
                    ..
                } => Some((*snapshot_id, analysis.as_ref(), main_graph.as_ref())),
                _ => None,
            })
            .expect("ApplyEdits close batch should emit SnapshotReady");
        assert!(rt.snapshots.contains_key(&snapshot_id.0));
        assert_eq!(
            analysis.and_then(|payload| payload.value_json.as_deref()),
            Some(r#"{"root":{"k":2}}"#),
        );
        assert!(main_graph.is_some());
        assert_eq!(main_graph.map(|projection| projection.clear), Some(false));
    }

    #[test]
    fn advance_job_apply_edits_reuses_incremental_state_for_json_scalar() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();

        let source = r#"{"root":{"k":1}}"#;
        let edit_start = source.find('1').expect("fixture should contain value") as u32;
        let base_result = crate::document::materialize::materialize(
            &DocumentInputPlan::SourceText,
            "t6-reuse",
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
        let base_snapshot_id = SnapshotId(1);
        let mut base_snapshot = DocumentSnapshot::with_analysis("t6-reuse", base_result.analysis);
        base_snapshot.snapshot_id = base_snapshot_id;
        base_snapshot.graph = base_result.graph;
        base_snapshot.incremental = base_result.incremental;
        rt.next_snapshot_id = base_snapshot_id.0;
        rt.snapshots.insert(base_snapshot_id.0, base_snapshot);
        rt.latest_snapshot_by_document
            .insert("t6-reuse".into(), base_snapshot_id);

        let h = start_job(
            &mut rt,
            &mut m,
            DocumentJobSpec {
                kind: DocumentJobKind::ApplyEdits,
                document_key: "t6-reuse".into(),
                language: "json".into(),
                input: DocumentInputPlan::BaseTextWithEdits,
                settings: crate::document::protocol::DocumentJobSettings::default(),
                output: OutputPlan {
                    analysis: true,
                    graph: true,
                },
                base_snapshot_id: Some(base_snapshot_id),
                edits: vec![DocumentTextEdit {
                    start_byte: edit_start,
                    old_end_byte: edit_start + 1,
                    new_end_byte: edit_start + 1,
                    replacement: "2".into(),
                }],
            },
        );

        let batch = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);

        assert!(matches!(batch.terminal, Some(JobTerminal::Completed)));
        let snapshot_id = batch
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady { snapshot_id, .. } => Some(*snapshot_id),
                _ => None,
            })
            .expect("ApplyEdits close batch should emit SnapshotReady");
        let snapshot = rt
            .snapshots
            .get(&snapshot_id.0)
            .expect("snapshot should be stored");
        let incremental = snapshot
            .incremental
            .as_ref()
            .expect("incremental state should be preserved");
        assert!(incremental.can_resume);
        assert!(incremental.graph_model_snapshot.is_some());
        assert!(incremental.graph_model_index.is_some());
        let analysis = snapshot.analysis.as_ref().expect("analysis should exist");
        assert!(analysis.ts_tree.is_some());
        assert_eq!(analysis.value_json, r#"{"root":{"k":2}}"#);
    }

    struct IncrementalCase {
        language: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
    }

    #[test]
    fn advance_job_apply_edits_structural_incremental_all_languages() {
        let cases = [
            IncrementalCase {
                language: "json",
                source: r#"{"root":{"k":1}}"#,
                old: "1",
                replacement: "2",
            },
            IncrementalCase {
                language: "yaml",
                source: "root:\n  k: old\n",
                old: "old",
                replacement: "new",
            },
            IncrementalCase {
                language: "toml",
                source: "root = { k = \"old\" }\n",
                old: "\"old\"",
                replacement: "\"new\"",
            },
            IncrementalCase {
                language: "csv",
                source: "name,age\nAda,37\n",
                old: "Ada",
                replacement: "Bob",
            },
            IncrementalCase {
                language: "python",
                source: "{\"root\": {\"name\": \"old\"}, \"n\": 1}",
                old: "\"old\"",
                replacement: "\"new\"",
            },
            IncrementalCase {
                language: "javascript",
                source: "({root: {name: \"old\"}, n: 1})",
                old: "\"old\"",
                replacement: "\"new\"",
            },
        ];

        for case in cases {
            let mut rt = DocumentRuntime::default();
            let mut m = DocumentEngineMetrics::default();
            let edit_start =
                case.source.find(case.old).unwrap_or_else(|| {
                    panic!("fixture for {} should contain old value", case.language)
                }) as u32;
            let base_result = crate::document::materialize::materialize(
                &DocumentInputPlan::SourceText,
                case.language,
                case.language,
                case.source,
                false,
                &OutputPlan {
                    analysis: true,
                    graph: true,
                },
                &[],
                None,
            );
            let base_snapshot_id = SnapshotId(1);
            let mut base_snapshot =
                DocumentSnapshot::with_analysis(case.language, base_result.analysis);
            base_snapshot.snapshot_id = base_snapshot_id;
            base_snapshot.graph = base_result.graph;
            base_snapshot.incremental = base_result.incremental;
            rt.next_snapshot_id = base_snapshot_id.0;
            rt.snapshots.insert(base_snapshot_id.0, base_snapshot);
            rt.latest_snapshot_by_document
                .insert(case.language.to_owned(), base_snapshot_id);

            let h = start_job(
                &mut rt,
                &mut m,
                DocumentJobSpec {
                    kind: DocumentJobKind::ApplyEdits,
                    document_key: case.language.to_owned(),
                    language: case.language.to_owned(),
                    input: DocumentInputPlan::BaseTextWithEdits,
                    settings: crate::document::protocol::DocumentJobSettings::default(),
                    output: OutputPlan {
                        analysis: true,
                        graph: true,
                    },
                    base_snapshot_id: Some(base_snapshot_id),
                    edits: vec![DocumentTextEdit {
                        start_byte: edit_start,
                        old_end_byte: edit_start + case.old.len() as u32,
                        new_end_byte: edit_start + case.replacement.len() as u32,
                        replacement: case.replacement.to_owned(),
                    }],
                },
            );

            let batch = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);

            assert!(
                matches!(batch.terminal, Some(JobTerminal::Completed)),
                "{} ApplyEdits should complete",
                case.language,
            );
            let snapshot_id = batch
                .events
                .iter()
                .find_map(|event| match event {
                    DocumentEvent::SnapshotReady {
                        snapshot_id,
                        main_graph,
                        ..
                    } => {
                        assert_eq!(
                            main_graph.as_ref().map(|projection| projection.clear),
                            Some(false),
                            "{} should emit incremental graph delta",
                            case.language,
                        );
                        Some(*snapshot_id)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{} should emit SnapshotReady", case.language));
            let snapshot = rt
                .snapshots
                .get(&snapshot_id.0)
                .expect("snapshot should be stored");
            let incremental = snapshot
                .incremental
                .as_ref()
                .unwrap_or_else(|| panic!("{} should keep incremental state", case.language));
            assert!(
                incremental.can_resume,
                "{} should remain resumable",
                case.language
            );
            assert!(
                incremental.fallback_reason.is_none(),
                "{} should not record structural fallback",
                case.language,
            );
            let analysis = snapshot.analysis.as_ref().expect("analysis should exist");
            assert!(
                analysis.source.contains(case.replacement),
                "{} source should include replacement",
                case.language,
            );
            assert!(
                analysis.document.is_some(),
                "{} should keep decoded document",
                case.language
            );
        }
    }

    #[test]
    fn advance_job_apply_edits_structural_incremental_subtree_boundaries() {
        let cases = [
            IncrementalCase {
                language: "json",
                source: r#"{"root":{"profile":{"name":"Alice","role":"admin"},"count":1},"tail":0}"#,
                old: r#"{"name":"Alice","role":"admin"}"#,
                replacement: r#"{"name":"Bob","role":"owner","team":"ops"}"#,
            },
            IncrementalCase {
                language: "json",
                source: r#"{"root":{"items":[1,2],"count":1},"tail":0}"#,
                old: r#"[1,2]"#,
                replacement: r#"[1,2,3]"#,
            },
        ];

        for (case_index, case) in cases.into_iter().enumerate() {
            let document_key = format!("subtree-{}-{case_index}", case.language);
            let mut rt = DocumentRuntime::default();
            let mut m = DocumentEngineMetrics::default();
            let edit_start =
                case.source.find(case.old).unwrap_or_else(|| {
                    panic!("fixture for {} should contain subtree", case.language)
                }) as u32;
            let base_result = crate::document::materialize::materialize(
                &DocumentInputPlan::SourceText,
                &document_key,
                case.language,
                case.source,
                false,
                &OutputPlan {
                    analysis: true,
                    graph: true,
                },
                &[],
                None,
            );
            let base_snapshot_id = SnapshotId(1);
            let mut base_snapshot =
                DocumentSnapshot::with_analysis(case.language, base_result.analysis);
            base_snapshot.snapshot_id = base_snapshot_id;
            base_snapshot.graph = base_result.graph;
            base_snapshot.incremental = base_result.incremental;
            rt.next_snapshot_id = base_snapshot_id.0;
            rt.snapshots.insert(base_snapshot_id.0, base_snapshot);
            rt.latest_snapshot_by_document
                .insert(document_key.clone(), base_snapshot_id);

            let h = start_job(
                &mut rt,
                &mut m,
                DocumentJobSpec {
                    kind: DocumentJobKind::ApplyEdits,
                    document_key: document_key.clone(),
                    language: case.language.to_owned(),
                    input: DocumentInputPlan::BaseTextWithEdits,
                    settings: crate::document::protocol::DocumentJobSettings::default(),
                    output: OutputPlan {
                        analysis: true,
                        graph: true,
                    },
                    base_snapshot_id: Some(base_snapshot_id),
                    edits: vec![DocumentTextEdit {
                        start_byte: edit_start,
                        old_end_byte: edit_start + case.old.len() as u32,
                        new_end_byte: edit_start + case.replacement.len() as u32,
                        replacement: case.replacement.to_owned(),
                    }],
                },
            );

            let batch = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);

            assert!(
                matches!(batch.terminal, Some(JobTerminal::Completed)),
                "{} subtree ApplyEdits should complete",
                case.language,
            );
            let snapshot_id = batch
                .events
                .iter()
                .find_map(|event| match event {
                    DocumentEvent::SnapshotReady {
                        snapshot_id,
                        main_graph,
                        ..
                    } => {
                        assert_eq!(
                            main_graph.as_ref().map(|projection| projection.clear),
                            Some(false),
                            "{} subtree edit should emit incremental graph delta",
                            case.language,
                        );
                        Some(*snapshot_id)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{} should emit SnapshotReady", case.language));
            let snapshot = rt
                .snapshots
                .get(&snapshot_id.0)
                .expect("snapshot should be stored");
            let incremental = snapshot
                .incremental
                .as_ref()
                .unwrap_or_else(|| panic!("{} should keep incremental state", case.language));
            assert!(
                incremental.fallback_reason.is_none(),
                "{} subtree edit should not record structural fallback",
                case.language,
            );
            let analysis = snapshot.analysis.as_ref().expect("analysis should exist");
            assert!(
                analysis.source.contains(case.replacement),
                "{} source should include subtree replacement",
                case.language,
            );
        }
    }

    #[test]
    fn advance_job_multiple_text_chunks_then_close_produces_snapshot() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = start_job(
            &mut rt,
            &mut m,
            DocumentJobSpec {
                kind: DocumentJobKind::AnalyzeSource,
                document_key: "t-chunks".into(),
                language: "csv".into(),
                input: DocumentInputPlan::SourceText,
                settings: crate::document::protocol::DocumentJobSettings::default(),
                output: OutputPlan {
                    analysis: true,
                    graph: false,
                },
                base_snapshot_id: None,
                edits: vec![],
            },
        );
        // Feed first chunk
        let b1 = advance_job(&mut rt, &mut m, h, AdvanceInput::TextChunk("a,b\n".into()));
        assert!(b1.terminal.is_none());
        assert!(b1.events.is_empty());
        // Feed second chunk
        let b2 = advance_job(&mut rt, &mut m, h, AdvanceInput::TextChunk("1,2".into()));
        assert!(b2.terminal.is_none());
        // Close
        let b3 = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        assert!(matches!(b3.terminal, Some(JobTerminal::Completed)));
    }

    // ── streaming JSON tests ───────────────────────────────────────────────

    fn make_streaming_spec(key: &str) -> DocumentJobSpec {
        DocumentJobSpec {
            kind: DocumentJobKind::AnalyzeSource,
            document_key: key.to_owned(),
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

    fn add_streaming_job(rt: &mut DocumentRuntime, key: &str) -> DocumentJobHandle {
        let h = rt.allocate_job_handle();
        let seq = rt.allocate_request_seq(key);
        rt.jobs.insert(
            h,
            JobEntry {
                spec: make_streaming_spec(key),
                request_seq: seq,
                latest_snapshot_id: None,
                terminal: None,
                source_buffer: None,
                source_bytes: None,
                stream_state: None,
                line_count: 0,
            },
        );
        h
    }

    #[test]
    fn streaming_json_single_chunk_emits_projection_delta() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_streaming_job(&mut rt, "stream-1");
        let b = advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::TextChunk(r#"{"k":1}"#.into()),
        );
        assert!(b.terminal.is_none());
        // Should emit at least one ProjectionDelta with clear=true for first chunk
        let has_projection = b
            .events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ProjectionDelta { clear: true, .. }));
        assert!(
            has_projection,
            "first chunk should emit ProjectionDelta with clear=true"
        );
        // Should emit a Progress event
        let has_progress = b
            .events
            .iter()
            .any(|e| matches!(e, DocumentEvent::Progress { .. }));
        assert!(has_progress, "should emit Progress event");
    }

    #[test]
    fn streaming_json_progress_reports_cumulative_source_bytes() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_streaming_job(&mut rt, "stream-progress-cumulative");

        let first = advance_job(&mut rt, &mut m, h, AdvanceInput::TextChunk("[1".into()));
        let second = advance_job(&mut rt, &mut m, h, AdvanceInput::TextChunk(",2]".into()));

        fn progress_bytes(batch: &EventBatch) -> Option<u32> {
            batch.events.iter().find_map(|event| match event {
                DocumentEvent::Progress { processed_bytes } => Some(*processed_bytes),
                _ => None,
            })
        }

        assert_eq!(progress_bytes(&first), Some(2));
        assert_eq!(progress_bytes(&second), Some(5));
    }

    #[test]
    fn streaming_csv_chunk_still_accumulates_no_events() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = rt.allocate_job_handle();
        let seq = rt.allocate_request_seq("csv-1");
        rt.jobs.insert(
            h,
            JobEntry {
                spec: DocumentJobSpec {
                    kind: DocumentJobKind::AnalyzeSource,
                    document_key: "csv-1".into(),
                    language: "csv".into(),
                    input: DocumentInputPlan::SourceText,
                    settings: crate::document::protocol::DocumentJobSettings::default(),
                    output: OutputPlan {
                        analysis: true,
                        graph: true,
                    },
                    base_snapshot_id: None,
                    edits: vec![],
                },
                request_seq: seq,
                latest_snapshot_id: None,
                terminal: None,
                source_buffer: None,
                source_bytes: None,
                stream_state: None,
                line_count: 0,
            },
        );
        let b = advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::TextChunk("a,b\n1,2".into()),
        );
        assert!(b.terminal.is_none());
        assert!(
            b.events.is_empty(),
            "CSV chunk should not emit streaming events"
        );
        assert_eq!(
            rt.jobs.get(&h).unwrap().source_buffer.as_deref(),
            Some("a,b\n1,2")
        );
    }

    #[test]
    fn streaming_non_streaming_language_chunks_accumulate_until_close() {
        let cases = [
            ("yaml", "root:\n", "  k: v\n"),
            ("toml", "name = ", "\"Ada\"\n"),
            ("csv", "name,age\n", "Ada,37\n"),
            ("python", "{\"name\": ", "\"Ada\"}"),
            ("javascript", "({name: ", "\"Ada\"})"),
        ];

        for (language, first, second) in cases {
            let mut rt = DocumentRuntime::default();
            let mut m = DocumentEngineMetrics::default();
            let h = start_job(
                &mut rt,
                &mut m,
                DocumentJobSpec {
                    kind: DocumentJobKind::AnalyzeSource,
                    document_key: format!("non-stream-{language}"),
                    language: language.to_owned(),
                    input: DocumentInputPlan::SourceText,
                    settings: crate::document::protocol::DocumentJobSettings::default(),
                    output: OutputPlan {
                        analysis: true,
                        graph: true,
                    },
                    base_snapshot_id: None,
                    edits: vec![],
                },
            );

            let b1 = advance_job(&mut rt, &mut m, h, AdvanceInput::TextChunk(first.into()));
            assert!(
                b1.terminal.is_none(),
                "{language} first chunk should stay open"
            );
            assert!(
                b1.events.is_empty(),
                "{language} first chunk should not stream events"
            );
            let b2 = advance_job(&mut rt, &mut m, h, AdvanceInput::TextChunk(second.into()));
            assert!(
                b2.terminal.is_none(),
                "{language} second chunk should stay open"
            );
            assert!(
                b2.events.is_empty(),
                "{language} second chunk should not stream events"
            );

            let batch = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
            assert!(
                matches!(batch.terminal, Some(JobTerminal::Completed)),
                "{language} close should materialize once",
            );
            assert!(
                batch
                    .events
                    .iter()
                    .any(|event| matches!(event, DocumentEvent::SnapshotReady { .. })),
                "{language} close should emit SnapshotReady",
            );
        }
    }

    #[test]
    fn streaming_json_multi_chunk_incremental_delta() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_streaming_job(&mut rt, "stream-multi");

        // First chunk
        let b1 = advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::TextChunk("{\"a\":\"".into()),
        );
        assert!(b1.terminal.is_none());
        let first_clear = b1
            .events
            .iter()
            .any(|e| matches!(e, DocumentEvent::ProjectionDelta { clear: true, .. }));
        assert!(
            first_clear,
            "first chunk ProjectionDelta should have clear=true"
        );

        // Second chunk
        let b2 = advance_job(&mut rt, &mut m, h, AdvanceInput::TextChunk("1\"}".into()));
        assert!(b2.terminal.is_none());
        let second_clear = b2
            .events
            .iter()
            .filter(|e| matches!(e, DocumentEvent::ProjectionDelta { .. }))
            .all(|e| matches!(e, DocumentEvent::ProjectionDelta { clear: false, .. }));
        assert!(second_clear, "subsequent chunks should have clear=false");

        // Close
        let b3 = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        assert!(matches!(b3.terminal, Some(JobTerminal::Completed)));
        let has_snapshot_ready = b3
            .events
            .iter()
            .any(|e| matches!(e, DocumentEvent::SnapshotReady { .. }));
        assert!(has_snapshot_ready, "Close should emit SnapshotReady");
    }

    #[test]
    fn streaming_json_close_stores_reusable_incremental_state() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_streaming_job(&mut rt, "stream-state");

        advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::TextChunk(r#"{"a":1}"#.into()),
        );
        let batch = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);

        assert!(matches!(batch.terminal, Some(JobTerminal::Completed)));
        let snapshot_id = batch
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady { snapshot_id, .. } => Some(*snapshot_id),
                _ => None,
            })
            .expect("Close should emit SnapshotReady");
        let snapshot = rt
            .snapshots
            .get(&snapshot_id.0)
            .expect("snapshot should be stored");
        let analysis = snapshot
            .analysis
            .as_ref()
            .expect("snapshot should keep analysis");
        assert!(analysis.document.is_some());
        assert!(
            analysis.ts_tree.is_none(),
            "streaming JSON snapshot should not rebuild tree-sitter state on close"
        );
        let incremental = snapshot
            .incremental
            .as_ref()
            .expect("snapshot should keep incremental state");
        assert!(incremental.can_resume);
        assert!(
            incremental.graph_model_snapshot.is_some(),
            "snapshot should keep graph model for later edits"
        );
        assert!(
            incremental.graph_model_index.is_some(),
            "snapshot should keep graph model index for later edits"
        );
    }

    #[test]
    fn streaming_json_snapshot_persists_graph_runtime_incremental_state() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_streaming_job(&mut rt, "runtime-graph-state");

        let chunk = advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::TextChunk(r#"{"a":1,"b":{"c":2}}"#.to_owned()),
        );
        assert!(chunk.terminal.is_none());

        let close = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        let snapshot_id = close
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady { snapshot_id, .. } => Some(*snapshot_id),
                _ => None,
            })
            .expect("Close should emit SnapshotReady");
        let snapshot = rt
            .snapshots
            .get(&snapshot_id.0)
            .expect("snapshot should be stored");
        let incremental = snapshot
            .incremental
            .as_ref()
            .expect("snapshot should keep incremental state");

        assert!(incremental.graph_model_snapshot.is_some());
        assert!(incremental.graph_model_index.is_some());
        assert!(incremental.graph_topology().is_some());
        assert!(incremental.layout_state().is_some());
    }

    #[test]
    fn streaming_json_close_freezes_existing_analysis_without_materialize_pass() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_streaming_job(&mut rt, "stream-freeze");

        advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::TextChunk(r#"{"a":1,"b":[2,3]}"#.into()),
        );

        let batch = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);

        assert!(matches!(batch.terminal, Some(JobTerminal::Completed)));
    }

    #[test]
    fn streaming_json_incomplete_close_commits_diagnostics_snapshot() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_streaming_job(&mut rt, "stream-invalid");

        let b1 = advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::TextChunk(r#"{"a":"#.into()),
        );
        assert!(b1.terminal.is_none());

        let b2 = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);
        assert!(matches!(b2.terminal, Some(JobTerminal::ParseFailed)));
        let snapshot_id = b2
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::ParseFailed { snapshot_id, .. } => Some(*snapshot_id),
                _ => None,
            })
            .expect("Close should emit ParseFailed");
        let snapshot = rt
            .snapshots
            .get(&snapshot_id.0)
            .expect("snapshot should be stored");
        let analysis = snapshot
            .analysis
            .as_ref()
            .expect("diagnostics snapshot should keep analysis");
        assert!(
            !analysis.diagnostics.is_empty(),
            "invalid json should produce diagnostics"
        );
        assert!(
            snapshot.graph.is_none(),
            "diagnostics snapshot should clear graph"
        );
    }

    fn blank_source_spec(key: &str, language: &str) -> DocumentJobSpec {
        DocumentJobSpec {
            kind: DocumentJobKind::AnalyzeSource,
            document_key: key.to_owned(),
            language: language.to_owned(),
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

    fn assert_blank_source_clear_snapshot(
        rt: &DocumentRuntime,
        batch: &EventBatch,
        document_key: &str,
    ) -> SnapshotId {
        assert!(matches!(batch.terminal, Some(JobTerminal::Completed)));
        let (snapshot_id, analysis, main_graph) = batch
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady {
                    snapshot_id,
                    analysis,
                    main_graph,
                    ..
                } => Some((*snapshot_id, analysis.as_ref(), main_graph.as_ref())),
                _ => None,
            })
            .expect("blank source close should emit SnapshotReady");
        let analysis = analysis.expect("blank source SnapshotReady should carry analysis");
        assert!(analysis.tree.is_none());
        assert!(analysis.value_json.is_none());
        assert!(analysis.diagnostics.is_empty());
        let main_graph = main_graph.expect("blank source SnapshotReady should carry mainGraph");
        assert!(main_graph.clear);
        assert!(main_graph.graph_data.is_none());
        let snapshot = rt
            .snapshots
            .get(&snapshot_id.0)
            .expect("empty snapshot should be stored");
        let stored = snapshot
            .analysis
            .as_ref()
            .expect("empty snapshot should keep analysis");
        assert!(stored.document.is_none());
        assert!(stored.diagnostics.is_empty());
        assert_eq!(stored.source, "");
        assert_eq!(
            rt.latest_snapshot_by_document.get(document_key),
            Some(&snapshot_id),
            "blank source should become the authoritative latest snapshot",
        );
        snapshot_id
    }

    #[test]
    fn blank_non_streaming_source_close_commits_authoritative_clear_snapshot() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = start_job(
            &mut rt,
            &mut m,
            blank_source_spec("blank-yaml-close", "yaml"),
        );

        let batch = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);

        assert_blank_source_clear_snapshot(&rt, &batch, "blank-yaml-close");
    }

    #[test]
    fn blank_streaming_source_close_commits_authoritative_clear_snapshot() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_streaming_job(&mut rt, "stream-blank-json");

        let first = advance_job(&mut rt, &mut m, h, AdvanceInput::TextChunk(String::new()));
        assert!(first.terminal.is_none());
        let close = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);

        assert_blank_source_clear_snapshot(&rt, &close, "stream-blank-json");
    }

    #[test]
    fn streaming_root_scalar_snapshot_main_graph_is_authoritative_replacement() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = add_streaming_job(&mut rt, "stream-root-scalar");

        let chunk = advance_job(&mut rt, &mut m, h, AdvanceInput::TextChunk("123".into()));
        assert!(chunk.terminal.is_none());
        let close = advance_job(&mut rt, &mut m, h, AdvanceInput::Close);

        let main_graph = close
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady { main_graph, .. } => main_graph.as_ref(),
                _ => None,
            })
            .expect("root scalar close should emit SnapshotReady.mainGraph");
        assert!(
            main_graph.clear
                || main_graph
                    .graph_data
                    .as_ref()
                    .is_some_and(|delta| !delta.nodes_removed.is_empty()),
            "SnapshotReady.mainGraph must be safe to apply over a previously rendered complex graph"
        );
    }

    #[test]
    fn streaming_apply_edits_json_still_accumulates() {
        let mut rt = DocumentRuntime::default();
        let mut m = DocumentEngineMetrics::default();
        let h = rt.allocate_job_handle();
        let seq = rt.allocate_request_seq("apply-edits");
        rt.jobs.insert(
            h,
            JobEntry {
                spec: DocumentJobSpec {
                    kind: DocumentJobKind::ApplyEdits,
                    document_key: "apply-edits".into(),
                    language: "json".into(),
                    input: DocumentInputPlan::BaseTextWithEdits,
                    settings: crate::document::protocol::DocumentJobSettings::default(),
                    output: OutputPlan {
                        analysis: true,
                        graph: true,
                    },
                    base_snapshot_id: None,
                    edits: vec![],
                },
                request_seq: seq,
                latest_snapshot_id: None,
                terminal: None,
                source_buffer: None,
                source_bytes: None,
                stream_state: None,
                line_count: 0,
            },
        );
        // ApplyEdits even with JSON should accumulate, not stream
        let b = advance_job(
            &mut rt,
            &mut m,
            h,
            AdvanceInput::TextChunk(r#"{"k":1}"#.into()),
        );
        assert!(b.terminal.is_none());
        assert!(
            b.events.is_empty(),
            "ApplyEdits should not stream, even for JSON"
        );
        assert_eq!(
            rt.jobs.get(&h).unwrap().source_buffer.as_deref(),
            Some(r#"{"k":1}"#)
        );
    }

    #[test]
    fn analyze_source_snapshot_carries_tree_path_index_for_graph_output() {
        let result = crate::document::materialize::materialize(
            &DocumentInputPlan::SourceText,
            "doc-json",
            "json",
            r#"{"wide":{"target":"old"}}"#,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[],
            None,
        );

        let incremental = result
            .incremental
            .as_ref()
            .expect("graph output should create incremental state");
        let path = [
            crate::core::path_seg_key("wide"),
            crate::core::path_seg_key("target"),
        ];
        let node_id = incremental
            .tree_path_index
            .as_ref()
            .and_then(|index| index.value_node_for_segments(&path));

        assert!(node_id.is_some(), "snapshot should carry value path index");
    }

    #[test]
    fn structural_incremental_analysis_uses_reused_line_index_and_keeps_snapshot_ready() {
        let source = r#"{"root":{"k":1},"tail":2}"#;
        let mut rt = DocumentRuntime::default();
        let mut metrics = DocumentEngineMetrics::default();

        let base = crate::document::materialize::materialize(
            &DocumentInputPlan::SourceText,
            "doc-json",
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
        let base_snapshot_id = SnapshotId(1);
        let mut base_snapshot = DocumentSnapshot::with_analysis("doc-json", base.analysis);
        base_snapshot.snapshot_id = base_snapshot_id;
        base_snapshot.graph = base.graph;
        base_snapshot.incremental = base.incremental;
        rt.next_snapshot_id = base_snapshot_id.0;
        rt.snapshots.insert(base_snapshot_id.0, base_snapshot);
        rt.latest_snapshot_by_document
            .insert("doc-json".to_owned(), base_snapshot_id);

        let edit_start = source.find('1').expect("fixture contains scalar") as u32;
        let handle = start_job(
            &mut rt,
            &mut metrics,
            DocumentJobSpec {
                kind: DocumentJobKind::ApplyEdits,
                document_key: "doc-json".to_owned(),
                language: "json".to_owned(),
                input: DocumentInputPlan::BaseTextWithEdits,
                settings: crate::document::protocol::DocumentJobSettings::default(),
                output: OutputPlan {
                    analysis: true,
                    graph: true,
                },
                base_snapshot_id: Some(base_snapshot_id),
                edits: vec![DocumentTextEdit {
                    start_byte: edit_start,
                    old_end_byte: edit_start + 1,
                    new_end_byte: edit_start + 1,
                    replacement: "3".to_owned(),
                }],
            },
        );

        let batch = advance_job(&mut rt, &mut metrics, handle, AdvanceInput::Close);
        assert!(matches!(batch.terminal, Some(JobTerminal::Completed)));
        let next_id = batch
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady {
                    snapshot_id,
                    main_graph,
                    ..
                } => {
                    assert_eq!(main_graph.as_ref().map(|graph| graph.clear), Some(false));
                    Some(*snapshot_id)
                }
                _ => None,
            })
            .expect("structural edit should emit SnapshotReady");
        let snapshot = rt.snapshots.get(&next_id.0).expect("snapshot stored");
        let analysis = snapshot.analysis.as_ref().expect("analysis stored");
        assert_eq!(analysis.value_json, r#"{"root":{"k":3},"tail":2}"#);
        assert_eq!(analysis.line_index.source_len(), analysis.source.len());
    }

    #[test]
    fn structural_incremental_preserves_spans_after_length_changing_edit() {
        let source = r#"{"root":{"k":"old"},"tail":"after"}"#;
        let edit_start = source.find("\"old\"").expect("fixture contains old") as u32;
        let next_source = r#"{"root":{"k":"replacement"},"tail":"after"}"#;

        let base = crate::document::materialize::materialize(
            &DocumentInputPlan::SourceText,
            "doc-json-span",
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
        let base_incremental = base.incremental.as_ref();
        let result = crate::document::materialize::materialize_with_base(
            &DocumentInputPlan::BaseTextWithEdits,
            "doc-json-span",
            "json",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[DocumentTextEdit {
                start_byte: edit_start,
                old_end_byte: edit_start + "\"old\"".len() as u32,
                new_end_byte: edit_start + "\"replacement\"".len() as u32,
                replacement: "\"replacement\"".to_owned(),
            }],
            None,
            base.analysis.document.as_ref(),
            base_incremental,
        );

        let document = result
            .analysis
            .document
            .as_ref()
            .expect("structural result should keep decoded document");
        let tail_start = next_source
            .find("\"after\"")
            .expect("next fixture contains tail") as u32;
        let tail_id = crate::core::find_node_by_path(
            document.root,
            &[crate::core::path_seg_key("tail")],
            false,
            &document.store,
        )
        .expect("tail value should be resolved");
        let tail = document.store.get(tail_id).expect("tail node should exist");
        assert_eq!(tail.start_byte, tail_start);
        assert_eq!(tail.end_byte, tail_start + "\"after\"".len() as u32);
    }

    #[test]
    fn structural_incremental_remains_resumable_across_two_scalar_edits() {
        let first_source = r#"{"root":{"a":1,"b":2}}"#;
        let first = crate::document::materialize::materialize(
            &DocumentInputPlan::SourceText,
            "doc-repeat",
            "json",
            first_source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[],
            None,
        );

        let a_start = first_source
            .find('1')
            .expect("fixture contains first scalar") as u32;
        let second = crate::document::materialize::materialize_with_base(
            &DocumentInputPlan::BaseTextWithEdits,
            "doc-repeat",
            "json",
            first_source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[DocumentTextEdit {
                start_byte: a_start,
                old_end_byte: a_start + 1,
                new_end_byte: a_start + 1,
                replacement: "3".to_owned(),
            }],
            None,
            first.analysis.document.as_ref(),
            first.incremental.as_ref(),
        );
        assert!(
            second
                .incremental
                .as_ref()
                .is_some_and(|state| state.can_resume)
        );

        let second_source = r#"{"root":{"a":3,"b":2}}"#;
        let b_start = second_source
            .find('2')
            .expect("fixture contains second scalar") as u32;
        let third = crate::document::materialize::materialize_with_base(
            &DocumentInputPlan::BaseTextWithEdits,
            "doc-repeat",
            "json",
            second_source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[DocumentTextEdit {
                start_byte: b_start,
                old_end_byte: b_start + 1,
                new_end_byte: b_start + 1,
                replacement: "4".to_owned(),
            }],
            None,
            second.analysis.document.as_ref(),
            second.incremental.as_ref(),
        );

        assert!(
            third
                .incremental
                .as_ref()
                .is_some_and(|state| state.can_resume)
        );
        assert_eq!(third.analysis.value_json, r#"{"root":{"a":3,"b":4}}"#);
        assert_eq!(third.graph.as_ref().map(|g| g.clear), Some(false));
    }

    #[test]
    fn structural_subtree_edit_recomputes_descendant_locations() {
        let source = "{\n  \"root\": {\n    \"a\": 1\n  },\n  \"tail\": 2\n}";
        let mut rt = DocumentRuntime::default();
        let mut metrics = DocumentEngineMetrics::default();

        let base = crate::document::materialize::materialize(
            &DocumentInputPlan::SourceText,
            "doc-json-subtree",
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
        let base_snapshot_id = SnapshotId(1);
        let mut base_snapshot = DocumentSnapshot::with_analysis("doc-json-subtree", base.analysis);
        base_snapshot.snapshot_id = base_snapshot_id;
        base_snapshot.graph = base.graph;
        base_snapshot.incremental = base.incremental;
        rt.next_snapshot_id = base_snapshot_id.0;
        rt.snapshots.insert(base_snapshot_id.0, base_snapshot);
        rt.latest_snapshot_by_document
            .insert("doc-json-subtree".to_owned(), base_snapshot_id);

        let old = "{\n    \"a\": 1\n  }";
        let replacement = "{\n    \"a\": 11,\n    \"b\": 22\n  }";
        let start = source.find(old).expect("fixture contains subtree") as u32;
        let handle = start_job(
            &mut rt,
            &mut metrics,
            DocumentJobSpec {
                kind: DocumentJobKind::ApplyEdits,
                document_key: "doc-json-subtree".to_owned(),
                language: "json".to_owned(),
                input: DocumentInputPlan::BaseTextWithEdits,
                settings: crate::document::protocol::DocumentJobSettings::default(),
                output: OutputPlan {
                    analysis: true,
                    graph: true,
                },
                base_snapshot_id: Some(base_snapshot_id),
                edits: vec![DocumentTextEdit {
                    start_byte: start,
                    old_end_byte: start + old.len() as u32,
                    new_end_byte: start + replacement.len() as u32,
                    replacement: replacement.to_owned(),
                }],
            },
        );

        let batch = advance_job(&mut rt, &mut metrics, handle, AdvanceInput::Close);
        assert!(matches!(batch.terminal, Some(JobTerminal::Completed)));
        let snapshot_id = batch
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady {
                    snapshot_id,
                    main_graph,
                    ..
                } => {
                    assert_eq!(main_graph.as_ref().map(|graph| graph.clear), Some(false));
                    Some(*snapshot_id)
                }
                _ => None,
            })
            .expect("structural subtree edit should emit SnapshotReady");
        let snapshot = rt.snapshots.get(&snapshot_id.0).expect("snapshot stored");
        let analysis = snapshot.analysis.as_ref().expect("analysis stored");
        let document = analysis.document.as_ref().expect("document stored");
        let path = [
            crate::core::path_seg_key("root"),
            crate::core::path_seg_key("b"),
        ];
        let node_id = crate::core::find_node_by_path_with_index(
            document.root,
            &path,
            false,
            &document.store,
            snapshot
                .incremental
                .as_ref()
                .and_then(|state| state.tree_path_index.as_ref()),
        )
        .expect("replacement subtree should contain $.root.b");
        let node = document.store.get(node_id).expect("b node stored");
        assert_eq!(node.value, "22");
        assert!(node.start_byte > start);
        assert!(
            node.line >= 3,
            "replacement descendant should have full-document line"
        );
    }

    #[test]
    fn structural_edit_snapshot_tree_path_index_plans_next_graph_value_edit() {
        let source = r#"{"root":{"a":1},"tail":2}"#;
        let mut rt = DocumentRuntime::default();
        let mut metrics = DocumentEngineMetrics::default();

        let base = crate::document::materialize::materialize(
            &DocumentInputPlan::SourceText,
            "doc-json-index",
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
        let base_snapshot_id = SnapshotId(1);
        let mut base_snapshot = DocumentSnapshot::with_analysis("doc-json-index", base.analysis);
        base_snapshot.snapshot_id = base_snapshot_id;
        base_snapshot.graph = base.graph;
        base_snapshot.incremental = base.incremental;
        rt.next_snapshot_id = base_snapshot_id.0;
        rt.snapshots.insert(base_snapshot_id.0, base_snapshot);
        rt.latest_snapshot_by_document
            .insert("doc-json-index".to_owned(), base_snapshot_id);

        let old = r#"{"a":1}"#;
        let replacement = r#"{"a":1,"b":2}"#;
        let start = source.find(old).expect("fixture contains subtree") as u32;
        let handle = start_job(
            &mut rt,
            &mut metrics,
            DocumentJobSpec {
                kind: DocumentJobKind::ApplyEdits,
                document_key: "doc-json-index".to_owned(),
                language: "json".to_owned(),
                input: DocumentInputPlan::BaseTextWithEdits,
                settings: crate::document::protocol::DocumentJobSettings::default(),
                output: OutputPlan {
                    analysis: true,
                    graph: true,
                },
                base_snapshot_id: Some(base_snapshot_id),
                edits: vec![DocumentTextEdit {
                    start_byte: start,
                    old_end_byte: start + old.len() as u32,
                    new_end_byte: start + replacement.len() as u32,
                    replacement: replacement.to_owned(),
                }],
            },
        );

        let batch = advance_job(&mut rt, &mut metrics, handle, AdvanceInput::Close);
        let snapshot_id = batch
            .events
            .iter()
            .find_map(|event| match event {
                DocumentEvent::SnapshotReady { snapshot_id, .. } => Some(*snapshot_id),
                _ => None,
            })
            .expect("structural edit should emit SnapshotReady");
        let snapshot = rt.snapshots.get(&snapshot_id.0).expect("snapshot stored");
        let plan = snapshot.plan_graph_value_edit(&GraphValueEditRequest {
            document_key: "doc-json-index".to_owned(),
            snapshot_id,
            language: "json".to_owned(),
            path: vec![
                GraphPathSeg {
                    tag: 0,
                    key: "root".to_owned(),
                    index: 0,
                },
                GraphPathSeg {
                    tag: 0,
                    key: "b".to_owned(),
                    index: 0,
                },
            ],
            prefer_key: false,
            value: serde_json::json!(4),
        });

        assert_eq!(plan.mode, GraphValueEditPlanMode::Edits);
        assert_eq!(plan.edits.len(), 1);
        assert_eq!(plan.edits[0].replacement, "4");
    }

    #[test]
    fn incremental_state_carries_graph_model_snapshot() {
        let source = r#"{"rows":[{"name":"Ada"}]}"#;
        let result = crate::document::materialize::materialize(
            &DocumentInputPlan::SourceText,
            "doc-graph-snapshot",
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
        let incremental = result
            .incremental
            .expect("graph output should create incremental state");
        assert!(incremental.graph_model_snapshot.is_some());
        assert!(incremental.graph_model_index.is_some());
    }
}
