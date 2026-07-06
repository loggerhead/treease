use super::*;
use crate::document::metrics::reset_global_document_engine_metrics_for_tests;
use crate::document::protocol::{
    DocumentEvent, DocumentInputPlan, DocumentJobKind, DocumentJobSettings, GraphPathSeg,
    GraphValueEditFallbackReason, GraphValueEditPlanMode, JobTerminal, SnapshotReadResult,
};
use crate::document::runtime::{
    document_runtime_job_count_for_tests, document_runtime_latest_job_spec_for_document_for_tests,
    reset_runtime_for_tests, stored_snapshot_for_document,
};
use crate::graph::graph_projection_service::reset_builder_config;
use crate::tree::incremental_edit::DocumentTextEdit;
use crate::wasm::reset_test_runtime;
use serde_json::json;
use std::cell::RefCell;

const NESTED_JSON_INPUT: &str = r#"{"nested":"{\"inner\":42}"}"#;

thread_local! {
    static TEST_RUNNING: RefCell<bool> = const { RefCell::new(false) };
}

struct TestGuard;

impl Drop for TestGuard {
    fn drop(&mut self) {
        TEST_RUNNING.with(|running| *running.borrow_mut() = false);
    }
}

fn lock_test_mutex() -> TestGuard {
    TEST_RUNNING.with(|running| {
        let mut running = running.borrow_mut();
        assert!(!*running, "test mutex re-entered");
        *running = true;
    });
    TestGuard
}

fn reset_test_state() {
    reset_test_runtime();
    reset_runtime_for_tests();
    reset_global_document_engine_metrics_for_tests();
    reset_builder_config();
}

fn key_seg(key: &str) -> GraphPathSeg {
    GraphPathSeg {
        tag: 0,
        key: key.to_owned(),
        index: 0,
    }
}

fn index_seg(index: i32) -> GraphPathSeg {
    GraphPathSeg {
        tag: 1,
        key: String::new(),
        index,
    }
}

fn scalar_edit_value(value: &str) -> serde_json::Value {
    json!({
        "kind": 2,
        "semType": 2,
        "tag": "",
        "value": value,
        "children": [],
    })
}

#[test]
fn document_job_settings_serialization_uses_smart_without_format_on_close() {
    let value =
        serde_json::to_value(DocumentJobSettings::default()).expect("settings should serialize");
    let formatting = value
        .get("formatting")
        .and_then(|value| value.as_object())
        .expect("formatting settings should be an object");

    assert!(
        formatting.get("formatOnClose").is_none(),
        "smart formatting should use the existing smart field without a separate formatOnClose flag"
    );
    assert_eq!(formatting.get("smart"), Some(&serde_json::json!(false)));
}

#[test]
fn start_document_job_accepts_settings_snapshot() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "settings-json".to_owned(),
        language: "json".to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: crate::document::protocol::DocumentFormattingSettings {
                indent: 2,
                smart: true,
                format_source_on_close: true,
                max_line_length: 100,
                max_inline_complexity: 1,
                max_array_inline_items: 6,
                align_object_arrays: true,
            },
        },
    })
    .expect("job should start");

    assert!(started.job_handle > 0);
}

#[test]
fn streaming_job_settings_materialize_nested_json_when_nest_enabled() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "stream-nested-json".to_owned(),
        language: "json".to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: crate::document::protocol::DocumentFormattingSettings::default(),
        },
    })
    .expect("job should start");

    let _ = text_chunk(started.job_handle, NESTED_JSON_INPUT);
    let batch = close(started.job_handle);
    let analysis = batch
        .events
        .iter()
        .find_map(|event| match event {
            DocumentEvent::SnapshotReady { analysis, .. } => analysis.as_ref(),
            _ => None,
        })
        .expect("snapshot analysis");

    assert!(analysis.value_json.is_none());
    assert_eq!(analysis.language, "json");
}

#[test]
fn streaming_close_formats_nested_json_source_when_nest_enabled() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "close-format-json".to_owned(),
        language: "json".to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: crate::document::protocol::DocumentFormattingSettings {
                indent: 2,
                smart: true,
                format_source_on_close: true,
                max_line_length: 100,
                max_inline_complexity: 1,
                max_array_inline_items: 6,
                align_object_arrays: true,
            },
        },
    })
    .expect("job should start");

    let _ = text_chunk(started.job_handle, NESTED_JSON_INPUT);
    let batch = close(started.job_handle);
    let source_text = batch
        .events
        .iter()
        .find_map(|event| match event {
            DocumentEvent::SnapshotReady { source_text, .. } => source_text.as_ref(),
            _ => None,
        })
        .expect("snapshot ready");

    assert!(
        source_text.contains("\n  \"nested\": "),
        "source should be smart-formatted after nested expansion. got: {source_text}"
    );
    assert_eq!(source_text, "{\n  \"nested\": {\"inner\": 42}\n}\n");
}

#[test]
fn streaming_close_emits_expanded_snapshot_for_top_level_nested_string_when_nest_enabled() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "close-format-top-level-nested-json".to_owned(),
        language: "json".to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: crate::document::protocol::DocumentFormattingSettings {
                indent: 2,
                smart: true,
                format_source_on_close: true,
                max_line_length: 100,
                max_inline_complexity: 1,
                max_array_inline_items: 6,
                align_object_arrays: true,
            },
        },
    })
    .expect("job should start");

    let _ = text_chunk(started.job_handle, r#""{\"a\":1}""#);
    let batch = close(started.job_handle);
    let source_text = batch
        .events
        .iter()
        .find_map(|event| match event {
            DocumentEvent::SnapshotReady { source_text, .. } => source_text.as_ref(),
            _ => None,
        })
        .expect("nested top-level string should emit expanded source text");
    assert_eq!(source_text, "{\"a\": 1}\n");
}

#[test]
fn streaming_close_recursively_rewrites_nested_json_source_when_nest_enabled() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let nested_input =
        serde_json::to_string(r#"{"a":1,"b":"{\"c\":\"d\"}"}"#).expect("input should serialize");

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "close-format-recursive-nested-json".to_owned(),
        language: "json".to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: crate::document::protocol::DocumentFormattingSettings {
                indent: 2,
                smart: true,
                format_source_on_close: false,
                max_line_length: 100,
                max_inline_complexity: 1,
                max_array_inline_items: 6,
                align_object_arrays: true,
            },
        },
    })
    .expect("job should start");

    let _ = text_chunk(started.job_handle, &nested_input);
    let batch = close(started.job_handle);
    let source_text = batch
        .events
        .iter()
        .find_map(|event| match event {
            DocumentEvent::SnapshotReady { source_text, .. } => source_text.as_ref(),
            _ => None,
        })
        .expect("snapshot ready should emit rewritten source text");

    assert_eq!(source_text, r#"{"a":1,"b":{"c":"d"}}"#);
}

#[test]
fn streaming_close_uses_smart_setting_for_final_source_format() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "smart-setting-format-json".to_owned(),
        language: "json".to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings::default(),
            formatting: crate::document::protocol::DocumentFormattingSettings {
                indent: 2,
                smart: true,
                format_source_on_close: true,
                max_line_length: 100,
                max_inline_complexity: 1,
                max_array_inline_items: 6,
                align_object_arrays: true,
            },
        },
    })
    .expect("job should start");

    let raw = r#"{"outer":{"inner":42},"items":[1,2]}"#;
    let _ = text_chunk(started.job_handle, raw);
    let batch = close(started.job_handle);
    let source_text = batch
        .events
        .iter()
        .find_map(|event| match event {
            DocumentEvent::SnapshotReady { source_text, .. } => source_text.as_ref(),
            _ => None,
        })
        .expect("smart formatting should return final sourceText");

    assert_ne!(source_text, raw);
    assert!(source_text.contains('\n'));
    assert!(source_text.contains("  \"outer\""));
}

#[test]
fn streaming_smart_close_semantic_tokens_match_formatted_source() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let settings = DocumentJobSettings {
        parser: crate::document::protocol::DocumentParserSettings::default(),
        formatting: crate::document::protocol::DocumentFormattingSettings {
            indent: 2,
            smart: true,
            format_source_on_close: true,
            max_line_length: 100,
            max_inline_complexity: 1,
            max_array_inline_items: 6,
            align_object_arrays: true,
        },
    };

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "smart-tokens-json".to_owned(),
        language: "json".to_owned(),
        output_graph: false,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings,
    })
    .expect("job should start");

    // Multi-level nesting so smart formatting changes line structure significantly.
    let raw = r#"{"a":{"b":1,"c":[2,3]},"d":"hello","e":true,"f":null}"#;
    let _ = text_chunk(started.job_handle, raw);
    let _ = close(started.job_handle);

    let snapshot =
        stored_snapshot_for_document("smart-tokens-json").expect("snapshot should be stored");
    let analysis = snapshot.analysis.as_ref().expect("analysis should exist");

    // Formatted source should differ from raw.
    assert_ne!(
        analysis.source, raw,
        "smart formatting should change layout"
    );

    // Semantic tokens produced by streaming close MUST match what a fresh,
    // full re-encode of the same (formatted) source would produce.  Any
    // mismatch means the streaming token spans were not correctly remapped
    // to the formatted text.
    let fresh_tokens = crate::language::encode_semantic_tokens("json", &analysis.source);
    assert!(
        !fresh_tokens.is_empty(),
        "fresh encode should produce tokens for the formatted source"
    );
    assert_eq!(
        analysis.semantic_tokens, fresh_tokens,
        "streaming close semantic tokens must match fresh encode of formatted source.\n\
         Source:\n{}\n",
        analysis.source
    );
}

#[test]
fn streaming_nested_close_semantic_tokens_match_expanded_source_when_nest_enabled() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "nested-tokens-json".to_owned(),
        language: "json".to_owned(),
        output_graph: false,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: crate::document::protocol::DocumentFormattingSettings {
                indent: 2,
                smart: true,
                format_source_on_close: false,
                max_line_length: 100,
                max_inline_complexity: 1,
                max_array_inline_items: 6,
                align_object_arrays: true,
            },
        },
    })
    .expect("job should start");

    let raw = r#"{"a":"{\"b\":1}"}"#;
    let _ = text_chunk(started.job_handle, raw);
    let _ = close(started.job_handle);

    let snapshot =
        stored_snapshot_for_document("nested-tokens-json").expect("snapshot should be stored");
    let analysis = snapshot.analysis.as_ref().expect("analysis should exist");

    assert_eq!(
        analysis.source, r#"{"a":{"b":1}}"#,
        "source should be rewritten to the expanded nested JSON"
    );

    let fresh_tokens = crate::language::encode_semantic_tokens("json", &analysis.source);
    assert!(
        !fresh_tokens.is_empty(),
        "fresh encode should produce tokens for the expanded source"
    );
    assert_eq!(
        analysis.semantic_tokens, fresh_tokens,
        "streaming close semantic tokens must match fresh encode of expanded source.\n\
         Source:\n{}\n",
        analysis.source
    );
}

#[test]
fn streaming_close_materializes_nested_json_for_followup_graph_edits() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "edit-formatted-nested".to_owned(),
        language: "json".to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: crate::document::protocol::DocumentFormattingSettings {
                indent: 2,
                smart: true,
                format_source_on_close: true,
                max_line_length: 100,
                max_inline_complexity: 1,
                max_array_inline_items: 6,
                align_object_arrays: true,
            },
        },
    })
    .expect("job should start");

    let _ = text_chunk(started.job_handle, NESTED_JSON_INPUT);
    let batch = close(started.job_handle);
    let snapshot = batch
        .events
        .iter()
        .find_map(|event| match event {
            DocumentEvent::SnapshotReady {
                source_text,
                analysis,
                ..
            } => Some((source_text.clone(), analysis.clone())),
            _ => None,
        })
        .expect("snapshot ready");
    let source_text = snapshot.0.expect("sourceText present");
    let analysis = snapshot.1.expect("analysis present");

    assert!(analysis.value_json.is_none());
    assert_eq!(analysis.language, "json");
    assert_eq!(source_text, "{\n  \"nested\": {\"inner\": 42}\n}\n");
}

fn start_source_job(document_key: &str, language: &str) -> StartDocumentJobOutput {
    start_document_job_impl(StartDocumentJobRequest {
        document_key: document_key.to_owned(),
        language: language.to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings::default(),
    })
    .expect("source job should start")
}

fn start_apply_job(
    document_key: &str,
    language: &str,
    base_snapshot_id: SnapshotId,
    edits: Vec<DocumentTextEdit>,
) -> StartDocumentJobOutput {
    start_document_job_impl(StartDocumentJobRequest {
        document_key: document_key.to_owned(),
        language: language.to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: Some(base_snapshot_id),
        edits,
        settings: DocumentJobSettings::default(),
    })
    .expect("apply-edits job should start")
}

fn job_handle_u32(job_handle: u64) -> u32 {
    u32::try_from(job_handle).expect("test job handle should fit into u32")
}

fn text_chunk(job_handle: u64, text: &str) -> EventBatch {
    advance_document_job_impl(job_handle_u32(job_handle), "textChunk", Some(text), None)
        .expect("text chunk should advance")
}

fn close(job_handle: u64) -> EventBatch {
    advance_document_job_impl(job_handle_u32(job_handle), "close", None, None)
        .expect("close should advance")
}

fn snapshot_id_from_batch(batch: &EventBatch) -> SnapshotId {
    batch
        .events
        .iter()
        .find_map(|event| match event {
            DocumentEvent::SnapshotReady { snapshot_id, .. } => Some(*snapshot_id),
            DocumentEvent::ParseFailed { snapshot_id, .. } => Some(*snapshot_id),
            _ => None,
        })
        .expect("batch should contain terminal snapshot event")
}

fn analyze_document_via_job(
    document_key: &str,
    language: &str,
    chunks: &[&str],
) -> (SnapshotId, Vec<EventBatch>) {
    let started = start_source_job(document_key, language);
    assert!(started.batch.events.is_empty());
    let mut batches = vec![started.batch];
    for chunk in chunks {
        batches.push(text_chunk(started.job_handle, chunk));
    }
    let close_batch = close(started.job_handle);
    let snapshot_id = snapshot_id_from_batch(&close_batch);
    batches.push(close_batch);
    (snapshot_id, batches)
}

fn replace_edit(source: &str, old: &str, replacement: &str) -> DocumentTextEdit {
    let start = source
        .find(old)
        .unwrap_or_else(|| panic!("source should contain {old:?}")) as u32;
    DocumentTextEdit {
        start_byte: start,
        old_end_byte: start + old.len() as u32,
        new_end_byte: start + replacement.len() as u32,
        replacement: replacement.to_owned(),
    }
}
fn full_replace_edit(source: &str, replacement: &str) -> DocumentTextEdit {
    DocumentTextEdit {
        start_byte: 0,
        old_end_byte: source.len() as u32,
        new_end_byte: replacement.len() as u32,
        replacement: replacement.to_owned(),
    }
}

fn build_json_table_document(rows: usize) -> String {
    let items = (0..rows)
        .map(|index| format!(r#"{{"id":{index},"name":"row-{index}","status":"ready"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"{{"table_with_header":[{items}]}}"#)
}

fn build_yaml_table_document(rows: usize) -> String {
    let mut out = String::from("table_with_header:\n");
    for index in 0..rows {
        out.push_str(&format!(
            "  - id: {index}\n    name: row-{index}\n    status: ready\n"
        ));
    }
    out
}

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("packages/core parent")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn read_repo_fixture(relative_path: &str) -> String {
    std::fs::read_to_string(repo_root().join(relative_path))
        .unwrap_or_else(|error| panic!("fixture {relative_path} should be readable: {error}"))
}

fn midpoint_char_boundary(source: &str) -> usize {
    assert!(
        source.len() > 1,
        "fixture source must be at least two bytes to split"
    );
    let mut split = source.len() / 2;
    while split < source.len() && !source.is_char_boundary(split) {
        split += 1;
    }
    if split >= source.len() {
        split = source.len() / 2;
        while split > 0 && !source.is_char_boundary(split) {
            split -= 1;
        }
    }
    assert!(
        split > 0 && split < source.len(),
        "computed split must stay inside the source"
    );
    split
}

fn split_before_needle(source: &str, needle: &str) -> usize {
    source
        .find(needle)
        .unwrap_or_else(|| panic!("source should contain split needle {needle:?}"))
}

fn split_after_needle_fragment(source: &str, needle: &str, fragment_len: usize) -> usize {
    let split = split_before_needle(source, needle) + fragment_len;
    assert!(
        source.is_char_boundary(split),
        "fragment split must land on a char boundary"
    );
    split
}
fn split_inside_first_json_string(source: &str) -> usize {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }

        let content_start = index + 1;
        let mut cursor = content_start;
        let mut escaped = false;
        while cursor < bytes.len() {
            let byte = bytes[cursor];
            if escaped {
                escaped = false;
                cursor += 1;
                continue;
            }
            if byte == b'\\' {
                escaped = true;
                cursor += 1;
                continue;
            }
            if byte == b'"' {
                let content = &source[content_start..cursor];
                if content.chars().count() > 1 {
                    return content_start + midpoint_char_boundary(content);
                }
                break;
            }
            cursor += 1;
        }

        index = cursor.saturating_add(1);
    }

    panic!("source should contain a splittable json string");
}

fn split_inside_featureful_json_boundary(source: &str) -> usize {
    if let Some(pos) = source.find('\\') {
        let split = pos + 1;
        if split > 0 && split < source.len() && source.is_char_boundary(split) {
            return split;
        }
    }

    if let Some((byte_index, _)) = source.char_indices().find(|(_, ch)| !ch.is_ascii()) {
        if byte_index > 0 && byte_index < source.len() {
            return byte_index;
        }
    }

    split_inside_first_json_string(source)
}

fn split_inside_escape_or_json_string(source: &str) -> usize {
    if let Some(pos) = source.find('\\') {
        let split = pos + 1;
        if split > 0 && split < source.len() && source.is_char_boundary(split) {
            return split;
        }
    }

    split_inside_first_json_string(source)
}

fn split_inside_unicode_or_json_string(source: &str) -> usize {
    if let Some((byte_index, _)) = source.char_indices().find(|(_, ch)| !ch.is_ascii()) {
        if byte_index > 0 && byte_index < source.len() {
            return byte_index;
        }
    }

    if let Some(pos) = source.find("\\u") {
        let split = pos + 2;
        if split > 0 && split < source.len() && source.is_char_boundary(split) {
            return split;
        }
    }

    split_inside_first_json_string(source)
}
fn assert_projection_delta(batch: &EventBatch, clear: bool, context: &str) {
    assert!(
        batch.events.iter().any(|event| matches!(
            event,
            DocumentEvent::ProjectionDelta {
                clear: event_clear,
                ..
            } if *event_clear == clear
        )),
        "{context} should emit ProjectionDelta clear={clear}",
    );
}

fn assert_snapshot_source(document_key: &str, expected_source: &str) {
    let snapshot = stored_snapshot_for_document(document_key).expect("snapshot should be stored");
    let analysis = snapshot.analysis.as_ref().expect("analysis should exist");
    assert_eq!(analysis.source, expected_source, "{document_key}");
}

fn assert_streaming_fixture_round_trip(
    language: &str,
    document_key: &str,
    source: &str,
    split_at: usize,
) {
    assert!(
        split_at > 0 && split_at < source.len(),
        "split must land strictly inside the source"
    );

    let single_key = format!("{document_key}-single");
    let started = start_source_job(&single_key, language);
    let single_batch = text_chunk(started.job_handle, source);
    assert_projection_delta(&single_batch, true, &single_key);
    let close_batch = close(started.job_handle);
    assert!(
        matches!(close_batch.terminal, Some(JobTerminal::Completed)),
        "{single_key} should complete"
    );
    assert_snapshot_source(&single_key, source);

    let multi_key = format!("{document_key}-multi");
    let started = start_source_job(&multi_key, language);
    let first = text_chunk(started.job_handle, &source[..split_at]);
    assert_projection_delta(&first, true, &multi_key);
    let second = text_chunk(started.job_handle, &source[split_at..]);
    assert_projection_delta(&second, false, &multi_key);
    let close_batch = close(started.job_handle);
    assert!(
        matches!(close_batch.terminal, Some(JobTerminal::Completed)),
        "{multi_key} should complete"
    );
    assert_snapshot_source(&multi_key, source);
    assert_eq!(document_runtime_job_count_for_tests(), 0);
}

fn assert_non_streaming_materialization(
    language: &str,
    document_key: &str,
    source: &str,
    split_at: usize,
) {
    assert!(
        split_at > 0 && split_at < source.len(),
        "split must land strictly inside the source"
    );
    let started = start_source_job(document_key, language);
    let first = text_chunk(started.job_handle, &source[..split_at]);
    assert!(
        first.events.is_empty(),
        "{document_key} should buffer first chunk"
    );
    assert!(
        first.terminal.is_none(),
        "{document_key} should stay open after first chunk"
    );
    let second = text_chunk(started.job_handle, &source[split_at..]);
    assert!(
        second.events.is_empty(),
        "{document_key} should buffer second chunk"
    );
    assert!(
        second.terminal.is_none(),
        "{document_key} should stay open after second chunk"
    );
    let close_batch = close(started.job_handle);
    assert!(
        matches!(close_batch.terminal, Some(JobTerminal::Completed)),
        "{document_key} close should complete"
    );
    assert!(
        close_batch
            .events
            .iter()
            .any(|event| matches!(event, DocumentEvent::SnapshotReady { .. })),
        "{document_key} close should emit SnapshotReady"
    );
    assert_snapshot_source(document_key, source);
    assert_eq!(document_runtime_job_count_for_tests(), 0);
}

fn assert_parse_failed_snapshot(document_key: &str, close_batch: &EventBatch) {
    assert!(
        matches!(close_batch.terminal, Some(JobTerminal::ParseFailed)),
        "{document_key} should terminate with ParseFailed"
    );
    assert!(
        close_batch
            .events
            .iter()
            .any(|event| matches!(event, DocumentEvent::ParseFailed { .. })),
        "{document_key} should emit ParseFailed",
    );
    let snapshot =
        stored_snapshot_for_document(document_key).expect("diagnostics snapshot should be stored");
    let analysis = snapshot.analysis.as_ref().expect("analysis should exist");
    assert!(
        !analysis.diagnostics.is_empty(),
        "{document_key} should retain diagnostics"
    );
    assert!(
        analysis.document.is_none(),
        "{document_key} diagnostics snapshot should not keep decoded document"
    );
    assert!(
        snapshot.graph.is_none(),
        "{document_key} should clear graph"
    );
}

fn edit_tree_from_plain(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Null => json!({
            "kind": 2,
            "semType": 6,
            "tag": "",
            "value": "",
            "children": [],
        }),
        serde_json::Value::Bool(value) => json!({
            "kind": 2,
            "semType": 5,
            "tag": "",
            "value": if value { "true" } else { "false" },
            "children": [],
        }),
        serde_json::Value::Number(value) => json!({
            "kind": 2,
            "semType": if value.is_i64() || value.is_u64() { 3 } else { 4 },
            "tag": "",
            "value": value.to_string(),
            "children": [],
        }),
        serde_json::Value::String(value) => scalar_edit_value(&value),
        serde_json::Value::Array(items) => json!({
            "kind": 0,
            "semType": 1,
            "tag": "",
            "value": "",
            "children": items
                .into_iter()
                .map(edit_tree_from_plain)
                .collect::<Vec<_>>(),
        }),
        serde_json::Value::Object(entries) => {
            let mut children = Vec::with_capacity(entries.len() * 2);
            for (key, child) in entries {
                children.push(scalar_edit_value(&key));
                children.push(edit_tree_from_plain(child));
            }
            json!({
                "kind": 1,
                "semType": 0,
                "tag": "",
                "value": "",
                "children": children,
            })
        }
    }
}

mod us01_import_and_us11_progress;
mod us04_path_targeting;
mod us07_bidirectional_edit_sync;
mod us12_streaming_snapshots;
