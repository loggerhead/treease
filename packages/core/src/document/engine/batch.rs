use crate::document::runtime::commit_snapshot;
use crate::document::snapshot;
use crate::document::stream_state::StreamState;

use super::super::materialize::{materialize, materialize_with_base_context};
use super::super::protocol::{CommitMode, DocumentInputPlan, DocumentJobKind};
use super::super::snapshot::DocumentSnapshot;
use super::{
    DocumentJobHandle, DocumentRuntime, EventBatch, rejected_batch, snapshot_events_for_terminal,
    terminal_batch,
};
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
) -> crate::core::LineIndex {
    let line_index = crate::core::LineIndex::build(&formatted.text);
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
    line_index: crate::core::LineIndex,
    token_spans: Vec<crate::core::TokenSpan>,
) -> snapshot::AnalysisBundle {
    let source_byte_length = source.len() as u32;
    let semantic_tokens = if token_spans.is_empty() {
        crate::core::encode_semantic_tokens(language, &source)
    } else {
        crate::core::encode_and_cache_semantic_tokens(None, "", &source, &token_spans)
    };
    let value_json =
        crate::core::document_analysis::encode_document_value_json(&document).unwrap_or_default();
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
    entry: &mut super::JobEntry,
    update: crate::document::stream_state::SourceDocUpdate,
) {
    entry.line_count = entry.line_count.saturating_add(update.appended_line_count);
}

pub(super) fn advance_close(
    runtime: &mut DocumentRuntime,
    handle: DocumentJobHandle,
    request_seq: u64,
) -> EventBatch {
    let Some(entry) = runtime.jobs.get_mut(&handle) else {
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
        let mut close_update: Option<crate::document::protocol::GraphDelta> = None;
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
                    match source_doc.commit_events(events, rewrites) {
                        Ok((events, update)) => {
                            record_streaming_source_update(entry, update);
                            for event in &events {
                                if let Err(error) = builder.push(event) {
                                    return rejected_batch(
                                        request_seq,
                                        "builder_push_error",
                                        format!("{error:?}"),
                                    );
                                }
                            }
                            Ok(())
                        }
                        Err(error) => Err(error.to_string()),
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
                if let Some((store, root)) = builder.tree_ref() {
                    if let Some(upd) = projector.update(store, root, &patches) {
                        close_update = Some(upd.delta);
                    }
                }
                if let Some(relayout) = projector.finalize_layout() {
                    match close_update.as_mut() {
                        Some(delta) => delta.nodes_updated.extend(relayout.nodes_updated),
                        None => close_update = Some(relayout),
                    }
                }
                streaming_incremental_state = projector.take_incremental_state();

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
                                    source = formatted.text.clone();
                                    encoded_source_text = Some(formatted.text);
                                    streamed_line_index = Some(line_index);
                                    // Clear streaming token spans: after smart format-on-close
                                    // the source text has changed, so positions collected during
                                    // streaming no longer correspond to the formatted text.
                                    // build_streaming_json_analysis will fall back to
                                    // encode_semantic_tokens for a fresh full-tree parse.
                                    token_spans.clear();
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
            let terminal = commit_snapshot(runtime, handle, snapshot, CommitMode::DiagnosticsOnly);
            let events =
                snapshot_events_for_terminal(runtime, handle, &terminal, &output_plan, None);
            return terminal_batch(request_seq, events, terminal);
        }

        return match decoded {
            Some(document) => {
                let line_index =
                    streamed_line_index.unwrap_or_else(|| crate::core::LineIndex::build(&source));
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
                if let Some(delta) = close_update {
                    graph = Some(GraphProjection {
                        ready: true,
                        clear: false,
                        graph_data: Some(delta),
                    });
                    incremental = streaming_incremental_state
                        .or_else(|| Some(crate::document::snapshot::IncrementalState::resumable()));
                }

                if let Err(detail) =
                    super::validate_snapshot_ready_outputs(graph.as_ref(), &output_plan)
                {
                    return rejected_batch(request_seq, "missing_requested_main_graph", detail);
                }

                let mut snapshot = DocumentSnapshot::with_analysis(document_key.clone(), analysis);
                snapshot.graph = graph;
                snapshot.incremental = incremental;
                let terminal =
                    commit_snapshot(runtime, handle, snapshot, CommitMode::Authoritative);
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

    let Some(entry) = runtime.jobs.get_mut(&handle) else {
        return rejected_batch(
            request_seq,
            "document_runtime_missing_job",
            format!("No document runtime job registered for handle {}", handle.0),
        );
    };

    let document_key = entry.spec.document_key.clone();
    let language = entry.spec.language.clone();
    let source = match entry.take_source_text() {
        Ok(source) => source,
        Err(error) => {
            return rejected_batch(
                request_seq,
                "invalid_utf8_source",
                format!("document source is not valid UTF-8: {error}"),
            );
        }
    };

    match entry.spec.kind {
        _ => {
            if entry.spec.kind == DocumentJobKind::ApplyEdits
                && entry.spec.base_snapshot_id.is_none()
            {
                return rejected_batch(
                    request_seq,
                    "missing_base_snapshot",
                    "ApplyEdits requires a base snapshot",
                );
            }
            let input_plan = entry.spec.input.clone();
            let output_plan = entry.spec.output;
            let edits = entry.spec.edits.clone();
            let mut source = source;

            let mut base_incremental_owned = None;
            let result = if entry.spec.kind == DocumentJobKind::ApplyEdits {
                let Some(base_snapshot_id) = entry.spec.base_snapshot_id else {
                    unreachable!("ApplyEdits missing base snapshot rejected above");
                };
                let Some(base) = runtime.snapshots.get(&base_snapshot_id.0) else {
                    return rejected_batch(
                        request_seq,
                        "base_snapshot_not_found",
                        "ApplyEdits base snapshot is not available",
                    );
                };
                let Some(base_analysis) = base.analysis.as_ref() else {
                    return rejected_batch(
                        request_seq,
                        "base_snapshot_missing_analysis",
                        "ApplyEdits base snapshot has no analysis",
                    );
                };
                source = base_analysis.source.clone();
                base_incremental_owned = base.incremental.clone();
                materialize_with_base_context(
                    &input_plan,
                    &document_key,
                    &language,
                    &source,
                    false,
                    &output_plan,
                    &edits,
                    base_analysis.ts_tree.clone(),
                    super::super::materialize::MaterializeBaseContext {
                        document: base_analysis.document.as_ref(),
                        incremental: base.incremental.as_ref(),
                        line_index: Some(&base_analysis.line_index),
                        semantic_tokens: Some(&base_analysis.semantic_tokens),
                    },
                )
            } else {
                materialize(
                    &input_plan,
                    &document_key,
                    &language,
                    &source,
                    false,
                    &output_plan,
                    &edits,
                    None,
                )
            };

            let parse_failed =
                result.analysis.document.is_none() && !result.analysis.diagnostics.is_empty();

            if !parse_failed {
                if let Err(detail) =
                    super::validate_snapshot_ready_outputs(result.graph.as_ref(), &output_plan)
                {
                    return rejected_batch(request_seq, "missing_requested_main_graph", detail);
                }
            }

            let mut snapshot =
                DocumentSnapshot::with_analysis(document_key.clone(), result.analysis);
            snapshot.graph = result.graph;
            snapshot.incremental = result.incremental.or(base_incremental_owned);
            let mode = if parse_failed {
                CommitMode::DiagnosticsOnly
            } else {
                CommitMode::Authoritative
            };
            let terminal = commit_snapshot(runtime, handle, snapshot, mode);
            let events =
                snapshot_events_for_terminal(runtime, handle, &terminal, &output_plan, None);
            terminal_batch(request_seq, events, terminal)
        }
    }
}
