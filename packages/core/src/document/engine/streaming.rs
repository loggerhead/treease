use crate::core::streaming_graph_projector::{ProjectionUpdate, StreamingGraphProjector};
use crate::document::protocol::DocumentAnalysisPayload;
use crate::document::stream_state::StreamState;

use super::{DocumentEvent, EventBatch, JobEntry, open_batch, rejected_batch};

pub(super) fn advance_streaming_text_chunk(
    entry: &mut JobEntry,
    request_seq: u64,
    text: String,
) -> EventBatch {
    advance_streaming_chunk(entry, request_seq, text.as_bytes(), Some(&text))
}

pub(super) fn advance_streaming_binary_chunk(
    entry: &mut JobEntry,
    request_seq: u64,
    bytes: Vec<u8>,
) -> EventBatch {
    advance_streaming_chunk(entry, request_seq, &bytes, None)
}

fn advance_streaming_chunk(
    entry: &mut JobEntry,
    request_seq: u64,
    bytes: &[u8],
    text: Option<&str>,
) -> EventBatch {
    if entry.stream_state.is_none() {
        entry.stream_state = StreamState::for_language(
            &entry.spec.language,
            &entry.spec.document_key,
            entry.spec.settings,
        );
    }

    let state = entry
        .stream_state
        .as_mut()
        .expect("streaming state initialized above");
    let is_first = state.is_first_chunk();
    let mut events = Vec::new();
    let mut projection_update: Option<ProjectionUpdate> = None;

    match feed_stream_state(state, request_seq, bytes, text, &mut projection_update) {
        Ok(source_update) => {
            let source_len = state.source_len();
            if let Some(update) = projection_update {
                if entry.spec.output.analysis {
                    entry.line_count = entry
                        .line_count
                        .saturating_add(source_update.appended_line_count);
                    events.push(DocumentEvent::AnalysisDelta {
                        analysis: DocumentAnalysisPayload {
                            source_byte_length: source_len,
                            source_line_count: entry.line_count,
                            language: entry.spec.language.clone(),
                            ..Default::default()
                        },
                    });
                }

                events.push(DocumentEvent::ProjectionDelta {
                    clear: is_first,
                    graph_data: Some(update.delta),
                    patch_seq: update.patch_seq,
                    base_graph_version: update.base_graph_version,
                    graph_version: update.graph_version,
                });
            }

            events.push(DocumentEvent::Progress {
                processed_bytes: source_update.processed_bytes,
            });
            open_batch(request_seq, events)
        }
        Err(batch) => batch,
    }
}

fn feed_stream_state(
    state: &mut StreamState,
    request_seq: u64,
    bytes: &[u8],
    text: Option<&str>,
    projection_update: &mut Option<ProjectionUpdate>,
) -> Result<crate::document::stream_state::SourceDocUpdate, EventBatch> {
    match state {
        StreamState::Json {
            decoder,
            builder,
            first_chunk,
            projector,
            source_doc,
            token_spans,
            ..
        } => feed_json_chunk(
            decoder,
            builder,
            projector,
            first_chunk,
            source_doc,
            token_spans,
            request_seq,
            bytes,
            text,
            projection_update,
        ),
    }
}

fn feed_json_chunk(
    decoder: &mut crate::stream::streaming_json::StreamDecoder,
    builder: &mut crate::stream::tree_builder::Builder,
    projector: &mut StreamingGraphProjector,
    first_chunk: &mut bool,
    source_doc: &mut crate::document::stream_state::StreamingSourceDoc,
    token_spans: &mut Vec<crate::core::TokenSpan>,
    request_seq: u64,
    bytes: &[u8],
    text: Option<&str>,
    projection_update: &mut Option<ProjectionUpdate>,
) -> Result<crate::document::stream_state::SourceDocUpdate, EventBatch> {
    source_doc.push_input(bytes);
    let feed_result = match text {
        Some(text) => decoder.feed(text),
        None => decoder.feed_bytes(bytes),
    };
    if let Err(error) = feed_result {
        if text.is_none() && std::str::from_utf8(bytes).is_err() {
            return Ok(crate::document::stream_state::SourceDocUpdate {
                appended_line_count: 0,
                processed_bytes: source_doc.processed_bytes(),
            });
        }
        return Err(rejected_batch(
            request_seq,
            "stream_decode_error",
            format!("{error:?}"),
        ));
    }
    let events = decoder.take_events();
    let rewrites = decoder.take_source_rewrites();
    token_spans.extend(
        decoder
            .take_token_spans()
            .into_iter()
            .map(|span| crate::core::TokenSpan {
                start_row: span.start_row,
                start_col: span.start_col,
                end_row: span.end_row,
                end_col: span.end_col,
                token_type: span.token_type,
            }),
    );
    let (events, source_update) = match source_doc.commit_events(events, rewrites) {
        Ok(result) => result,
        Err(error) => {
            if text.is_none() && std::str::from_utf8(bytes).is_err() {
                return Ok(crate::document::stream_state::SourceDocUpdate {
                    appended_line_count: 0,
                    processed_bytes: source_doc.processed_bytes(),
                });
            }
            return Err(rejected_batch(
                request_seq,
                "stream_source_error",
                error.to_string(),
            ));
        }
    };
    for event in &events {
        builder
            .push(event)
            .map_err(|e| rejected_batch(request_seq, "builder_push_error", format!("{e:?}")))?;
    }
    update_from_projector(builder, projector, first_chunk, projection_update);
    Ok(source_update)
}

fn update_from_projector(
    builder: &mut crate::stream::tree_builder::Builder,
    projector: &mut StreamingGraphProjector,
    first_chunk: &mut bool,
    projection_update: &mut Option<ProjectionUpdate>,
) -> usize {
    let patches = builder.take_patches();
    let patch_count = patches.len();
    let Some((store, root)) = builder.tree_ref() else {
        return patch_count;
    };

    if *first_chunk {
        *first_chunk = false;
    }

    if let Some(update) = projector.update(store, root, &patches) {
        *projection_update = Some(update);
    }
    patch_count
}
