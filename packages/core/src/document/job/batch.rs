use crate::document::snapshot;
use crate::document::stream_state::{CommitEventsError, StreamState};

use super::super::materialization::{
    materialize_job, validate_snapshot_ready_outputs, MaterializationOutcome,
};
use super::super::projection::{is_blank_source, materialize};
use super::super::protocol::{CommitMode, DocumentInputPlan};
use super::super::snapshot::DocumentSnapshot;
use super::super::{protocol::EventBatch, runtime::DocumentRuntime};
use super::engine::{rejected_batch, snapshot_events_for_terminal, terminal_batch};
use super::entry::{DocumentJobHandle, JobEntry};
use crate::document::snapshot::GraphProjection;
fn format_preferences_from_job_settings(
    settings: crate::document::protocol::DocumentFormattingSettings,
) -> crate::formats::preferences::FormatPreferences {
    let mut prefs = crate::formats::preferences::FormatPreferences::base();
    prefs.indent = settings.indent;
    prefs.smart = settings.smart;
    prefs.max_line_length = settings.max_line_length;
    prefs.max_inline_complexity = settings.max_inline_complexity;
    prefs.max_array_inline_items = settings.max_array_inline_items;
    prefs.align_object_arrays = settings.align_object_arrays;
    prefs
}

fn apply_formatted_spans(
    decoded: &mut crate::formats::DecodedDocument,
    formatted: &crate::formats::encoder_json::JsonFormattedDocument,
) -> crate::analysis::line_index::LineIndex {
    let line_index = crate::analysis::line_index::LineIndex::build(&formatted.text);
    for span in &formatted.spans {
        if let Some(node) = decoded.store.get_mut(span.node_id) {
            node.start_byte = span.start_byte;
            node.end_byte = span.end_byte;
            let line_column = line_index.offset_to_line_column(span.start_byte as usize);
            node.line = line_column.line as i32 + 1;
            node.column = line_column.column as i32 + 1;
        }
    }
    line_index
}

fn build_streaming_json_analysis(
    document_key: &str,
    language: &str,
    source: String,
    document: crate::formats::DecodedDocument,
    line_index: crate::analysis::line_index::LineIndex,
    token_spans: Vec<crate::tree::tree_store::TokenSpan>,
) -> snapshot::AnalysisBundle {
    let source_byte_length = source.len() as u32;
    let semantic_tokens = if token_spans.is_empty() {
        crate::language::semantic_tokens::encode_semantic_tokens(language, &source)
    } else {
        crate::language::semantic_tokens::encode_and_cache_semantic_tokens(
            None,
            "",
            &source,
            &token_spans,
        )
    };
    let value_json = crate::analysis::document_analysis::encode_document_value_json(&document)
        .unwrap_or_default();
    snapshot::AnalysisBundle {
        key: document_key.to_owned(),
        language: language.to_owned(),
        source,
        source_byte_length,
        document: Some(document),
        ts_tree: None,
        token_spans,
        diagnostics: Vec::new(),
        semantic_tokens,
        value_json,
        line_index,
    }
}

fn record_streaming_source_update(
    entry: &mut JobEntry,
    update: crate::document::stream_state::SourceDocUpdate,
) {
    entry.line_count = entry.line_count.saturating_add(update.appended_line_count);
}

pub(super) fn advance_close(
    runtime: &mut DocumentRuntime,
    handle: DocumentJobHandle,
    request_seq: u64,
) -> EventBatch {
    let Some(entry) = runtime.job_mut(handle) else {
        return rejected_batch(
            request_seq,
            "document_runtime_missing_job",
            format!("No document runtime job registered for handle {}", handle.0),
        );
    };

    if let Some(mut stream_state) = entry.stream_state.take() {
        let document_key = entry.spec.document_key.clone();
        let language = entry.spec.language.clone();
        let mut source: String;
        let output_plan = entry.spec.output;
        let job_settings = entry.spec.settings;

        let mut diagnostics_only = false;
        let mut close_update: Option<crate::graph::streaming_graph_projector::ProjectionUpdate> =
            None;
        let mut streaming_incremental_state = None;
        let mut encoded_source_text: Option<String> = None;
        let mut format_error_detail: Option<String> = None;
        let mut streamed_line_index = None;

        let decoded = {
            let StreamState::Json {
                projector,
                decoder,
                builder,
                source_doc,
                token_spans,
                ..
            } = &mut stream_state;
            let finish_result = match decoder.finish_events() {
                Ok(events) => {
                    let rewrites = decoder.take_source_rewrites();
                    match source_doc
                        .commit_events_with(events, rewrites, |event| builder.push(&event))
                    {
                        Ok(update) => {
                            record_streaming_source_update(entry, update);
                            Ok(())
                        }
                        Err(CommitEventsError::Source(error)) => Err(error.to_string()),
                        Err(CommitEventsError::Callback(error)) => {
                            return rejected_batch(
                                request_seq,
                                "builder_push_error",
                                format!("{error:?}"),
                            );
                        }
                    }
                }
                Err(error) => Err(format!("{error:?}")),
            };

            if let Err(_error) = finish_result {
                diagnostics_only = true;
                match source_doc.finish() {
                    Ok(update) => {
                        record_streaming_source_update(entry, update);
                        source = source_doc.source_text().to_owned();
                    }
                    Err(error) => {
                        return rejected_batch(
                            request_seq,
                            "invalid_utf8_source",
                            format!("document source is not valid UTF-8: {error}"),
                        );
                    }
                }
                None
            } else {
                match source_doc.finish() {
                    Ok(update) => record_streaming_source_update(entry, update),
                    Err(error) => {
                        return rejected_batch(
                            request_seq,
                            "stream_source_error",
                            error.to_string(),
                        );
                    }
                }
                streamed_line_index = Some(source_doc.line_index());
                source = source_doc.source_text().to_owned();

                let patches = builder.take_patches();
                if let (Some(projector), Some((store, root))) =
                    (projector.as_mut(), builder.tree_ref())
                {
                    if let Some(upd) = projector.update(store, root, &patches) {
                        close_update = Some(upd);
                    }
                }
                if let Some(projector) = projector.as_mut() {
                    if let Some(relayout) = projector.finalize_layout() {
                        match close_update.as_mut() {
                            Some(update) => {
                                update.delta.nodes_updated.extend(relayout.nodes_updated)
                            }
                            None => {
                                close_update = Some(
                                    crate::graph::streaming_graph_projector::ProjectionUpdate {
                                        delta: relayout,
                                        patch_seq: 0,
                                        base_graph_version: 0,
                                        graph_version: 0,
                                        new_nodes: 0,
                                        updated_nodes: 0,
                                        new_edges: 0,
                                        removed_edges: 0,
                                        rows_appended: 0,
                                        cells_updated: 0,
                                        layout_summaries: 0,
                                    },
                                )
                            }
                        }
                    }
                }
                streaming_incremental_state = projector
                    .as_mut()
                    .and_then(|projector| projector.take_incremental_state());

                match builder.take_document() {
                    Ok(mut document) => {
                        if job_settings.formatting.smart
                            && job_settings.formatting.format_source_on_close
                        {
                            let prefs =
                                format_preferences_from_job_settings(job_settings.formatting);
                            match crate::formats::encoder_json::format_json_document_with_spans(
                                &document, &prefs,
                            ) {
                                Ok(formatted) => {
                                    let line_index =
                                        apply_formatted_spans(&mut document, &formatted);
                                    *token_spans = formatted.semantic_token_spans.clone();
                                    source = formatted.text.clone();
                                    encoded_source_text = Some(formatted.text);
                                    streamed_line_index = Some(line_index);
                                    Some(document)
                                }
                                Err(error) => {
                                    format_error_detail = Some(error.to_string());
                                    None
                                }
                            }
                        } else {
                            Some(document)
                        }
                    }
                    Err(_) => {
                        diagnostics_only = true;
                        None
                    }
                }
            }
        };

        // Nest expansion (or future content-expansion) rewrites the source
        // text. Propagate the canonical rewritten source when smart
        // formatting didn't already produce the final source.
        let has_expanded_source = stream_state.has_expanded_source();
        if encoded_source_text.is_none() && has_expanded_source {
            encoded_source_text = Some(source.clone());
        }

        let mut streaming_token_spans = stream_state.take_token_spans();
        if has_expanded_source {
            streaming_token_spans.clear();
        }

        if let Some(detail) = format_error_detail {
            return rejected_batch(request_seq, "json_close_format_failed", detail);
        }

        if is_blank_source(&source) {
            let result = materialize(
                &DocumentInputPlan::SourceText,
                &document_key,
                &language,
                &source,
                false,
                &output_plan,
                &[],
                None,
            );
            if let Err(detail) =
                validate_snapshot_ready_outputs(result.graph.as_ref(), &output_plan)
            {
                return rejected_batch(request_seq, "missing_requested_main_graph", detail);
            }
            let mut snapshot =
                DocumentSnapshot::with_analysis(document_key.clone(), result.analysis);
            snapshot.graph = result.graph;
            snapshot.incremental = result.incremental;
            let terminal = runtime.commit_snapshot(handle, snapshot, CommitMode::Authoritative);
            let events =
                snapshot_events_for_terminal(runtime, handle, &terminal, &output_plan, None);
            return terminal_batch(request_seq, events, terminal);
        }

        if diagnostics_only {
            let result = materialize(
                &DocumentInputPlan::SourceText,
                &document_key,
                &language,
                &source,
                false,
                &output_plan,
                &[],
                None,
            );
            let snapshot = DocumentSnapshot::with_analysis(document_key.clone(), result.analysis);
            let terminal = runtime.commit_snapshot(handle, snapshot, CommitMode::DiagnosticsOnly);
            let events =
                snapshot_events_for_terminal(runtime, handle, &terminal, &output_plan, None);
            return terminal_batch(request_seq, events, terminal);
        }

        return match decoded {
            Some(document) => {
                let line_index = streamed_line_index
                    .unwrap_or_else(|| crate::analysis::LineIndex::build(&source));
                let topology_bytes = crate::graph::graph_topology::document_topology_bytes(
                    &document.store,
                    document.root,
                );
                let analysis = build_streaming_json_analysis(
                    &document_key,
                    &language,
                    source,
                    document,
                    line_index,
                    streaming_token_spans,
                );

                let mut graph = None;
                let mut incremental = None;
                if let Some(update) = close_update {
                    graph = Some(GraphProjection {
                        ready: true,
                        clear: update.base_graph_version == 0,
                        graph_data: Some(update.delta),
                        topology_bytes,
                    });
                    incremental = streaming_incremental_state.or(Some(
                        crate::document::snapshot::IncrementalState::resumable(),
                    ));
                }

                if let Err(detail) = validate_snapshot_ready_outputs(graph.as_ref(), &output_plan) {
                    return rejected_batch(request_seq, "missing_requested_main_graph", detail);
                }

                let mut snapshot = DocumentSnapshot::with_analysis(document_key.clone(), analysis);
                snapshot.graph = graph;
                snapshot.incremental = incremental;
                let terminal = runtime.commit_snapshot(handle, snapshot, CommitMode::Authoritative);
                let events = snapshot_events_for_terminal(
                    runtime,
                    handle,
                    &terminal,
                    &output_plan,
                    encoded_source_text.as_deref(),
                );
                terminal_batch(request_seq, events, terminal)
            }
            None => rejected_batch(
                request_seq,
                "stream_document_error",
                "stream decode produced no document",
            ),
        };
    }

    match materialize_job(runtime, handle) {
        MaterializationOutcome::Rejected { code, detail } => {
            rejected_batch(request_seq, code, detail)
        }
        MaterializationOutcome::Ready { snapshot, output } => {
            let terminal = runtime.commit_snapshot(handle, snapshot, CommitMode::Authoritative);
            let events = snapshot_events_for_terminal(runtime, handle, &terminal, &output, None);
            terminal_batch(request_seq, events, terminal)
        }
        MaterializationOutcome::DiagnosticsOnly { snapshot, output } => {
            let terminal = runtime.commit_snapshot(handle, snapshot, CommitMode::DiagnosticsOnly);
            let events = snapshot_events_for_terminal(runtime, handle, &terminal, &output, None);
            terminal_batch(request_seq, events, terminal)
        }
    }
}
