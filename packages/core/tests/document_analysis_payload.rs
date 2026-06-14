use treease_core::core::CodecService;
use treease_core::document::materialize::materialize;
use treease_core::document::protocol::{DocumentInputPlan, OutputPlan};
use treease_core::document::snapshot::{DocumentSnapshot, build_decoded_analysis};
use treease_core::wasm_types::SemType;

fn mapping_value_sem_type(
    node: &treease_core::document::DocumentTreeNode,
    key: &str,
) -> Option<i32> {
    let children = &node.children;
    let mut index = 0usize;
    while index + 1 < children.len() {
        let key_node = &children[index];
        let value_node = &children[index + 1];
        if key_node.value == key {
            return Some(value_node.sem_type);
        }
        index += 2;
    }
    None
}

fn mapping_value<'a>(
    node: &'a treease_core::document::DocumentTreeNode,
    key: &str,
) -> Option<&'a treease_core::document::DocumentTreeNode> {
    let children = &node.children;
    let mut index = 0usize;
    while index + 1 < children.len() {
        let key_node = &children[index];
        let value_node = &children[index + 1];
        if key_node.value == key {
            return Some(value_node);
        }
        index += 2;
    }
    None
}

#[test]
fn snapshot_analysis_payload_preserves_tree_sem_types_and_value_json() {
    let result = materialize(
        &DocumentInputPlan::SourceText,
        "analysis-payload",
        "json",
        r#"{"profile":{"name":"Ada"},"count":1,"enabled":true}"#,
        false,
        &OutputPlan {
            analysis: true,
            graph: false,
        },
        &[],
        None,
    );

    let payload = DocumentSnapshot::with_analysis("analysis-payload", result.analysis)
        .analysis_payload(true)
        .expect("analysis payload should exist");

    assert_eq!(
        payload.value_json.as_deref(),
        Some(r#"{"profile":{"name":"Ada"},"count":1,"enabled":true}"#),
    );
    assert!(payload.diagnostics.is_empty());
    assert!(!payload.semantic_tokens.data.is_empty());

    let root = payload
        .tree
        .expect("tree payload should exist when include_tree_value=true");
    assert_eq!(root.sem_type, SemType::Map as i32);
    assert_eq!(
        mapping_value_sem_type(&root, "count"),
        Some(SemType::Int as i32)
    );
    assert_eq!(
        mapping_value_sem_type(&root, "enabled"),
        Some(SemType::Boolean as i32)
    );
    let profile = mapping_value(&root, "profile").expect("profile node should exist");
    assert_eq!(profile.sem_type, SemType::Map as i32);
    assert_eq!(
        mapping_value_sem_type(profile, "name"),
        Some(SemType::Str as i32)
    );
}
#[test]
fn parse_failed_event_preserves_analysis_payload_shape() {
    let mut runtime = treease_core::document::DocumentRuntime::default();
    let mut metrics = treease_core::document::metrics::DocumentEngineMetrics::default();
    let handle = treease_core::document::engine::start_job(
        &mut runtime,
        &mut metrics,
        treease_core::document::DocumentJobSpec {
            kind: treease_core::document::DocumentJobKind::AnalyzeSource,
            document_key: "invalid-json".into(),
            language: "json".into(),
            input: treease_core::document::DocumentInputPlan::SourceText,
            settings: treease_core::document::DocumentJobSettings::default(),
            output: treease_core::document::OutputPlan {
                analysis: true,
                graph: true,
            },
            base_snapshot_id: None,
            edits: vec![],
        },
    );

    let first_batch = treease_core::document::engine::advance_job(
        &mut runtime,
        &mut metrics,
        handle,
        treease_core::document::AdvanceInput::TextChunk("{\"broken\":".into()),
    );
    assert!(first_batch.terminal.is_none());

    let close_batch = treease_core::document::engine::advance_job(
        &mut runtime,
        &mut metrics,
        handle,
        treease_core::document::AdvanceInput::Close,
    );

    let analysis = close_batch
        .events
        .iter()
        .find_map(|event| match event {
            treease_core::document::DocumentEvent::ParseFailed { analysis, .. } => Some(analysis),
            _ => None,
        })
        .expect("invalid json close should emit ParseFailed");

    assert!(analysis.tree.is_none());
    assert!(analysis.value_json.is_none());
    assert!(!analysis.diagnostics.is_empty());
    assert_eq!(analysis.language, "json");
}
#[test]
fn decoded_json_analysis_preserves_full_artifacts_and_source_positions() {
    let source = "  {\"profile\":{\"name\":\"Ada\"}}\n";
    let decoded = CodecService::new()
        .decode("json", source)
        .expect("json fixture should decode");

    let analysis = build_decoded_analysis("doc-json", "json", source, &decoded);

    assert!(!analysis.semantic_tokens.is_empty());
    assert!(!analysis.token_spans.is_empty());
    assert_eq!(
        analysis
            .token_spans
            .first()
            .map(|span| (span.start_row, span.start_col)),
        Some((0, 2))
    );
}

#[test]
fn wasm_shared_decoded_json_analysis_preserves_full_artifacts() {
    let source = "  {\"profile\":{\"name\":\"Ada\"}}\n";
    let decoded = CodecService::new()
        .decode("json", source)
        .expect("json fixture should decode");

    let (shared, diagnostics) =
        treease_core::internal::wasm::document_analysis_shared::build_analysis_shared(
            "json",
            source,
            Some(&decoded),
        );

    assert!(diagnostics.is_empty());
    assert!(!shared.semantic_tokens.is_empty());
    assert!(!shared.token_spans.is_empty());
    assert_eq!(
        shared
            .token_spans
            .first()
            .map(|span| (span.start_row, span.start_col)),
        Some((0, 2))
    );
}
#[test]
fn bom_prefixed_decoded_json_analysis_still_returns_full_artifacts() {
    let source = "\u{feff}  {\"profile\":{\"name\":\"Ada\"}}\n";
    let decoded = CodecService::new()
        .decode("json", source)
        .expect("bom-prefixed json fixture should decode");

    let analysis = build_decoded_analysis("doc-json-bom", "json", source, &decoded);

    assert!(analysis.diagnostics.is_empty());
    assert!(!analysis.semantic_tokens.is_empty());
    assert!(!analysis.token_spans.is_empty());
    assert!(
        analysis
            .token_spans
            .first()
            .is_some_and(|span| span.start_col > 0)
    );
}

#[test]
fn decoded_json_analysis_matches_legacy_utf8_token_columns() {
    let source = "{\n  \"键\": \"值\"\n}";
    let decoded = CodecService::new()
        .decode("json", source)
        .expect("utf8 json fixture should decode");

    let analysis = build_decoded_analysis("doc-json-utf8", "json", source, &decoded);
    let expected: Vec<(u32, u32, u32, u32, u32)> =
        treease_core::internal::stream::streaming_json::token_spans(source)
            .expect("legacy streaming token scan should succeed")
            .into_iter()
            .map(|span| {
                (
                    span.start_row,
                    span.start_col,
                    span.end_row,
                    span.end_col,
                    span.token_type,
                )
            })
            .collect();
    let actual: Vec<(u32, u32, u32, u32, u32)> = analysis
        .token_spans
        .iter()
        .map(|span| {
            (
                span.start_row,
                span.start_col,
                span.end_row,
                span.end_col,
                span.token_type,
            )
        })
        .collect();

    assert_eq!(actual, expected);
}
#[test]
fn nested_json_materialize_keeps_analysis_value_consistent_with_nest_mode() {
    let result = materialize(
        &DocumentInputPlan::SourceText,
        "analysis-nested-json",
        "json",
        r#"{"nested":"{\"inner\":42}"}"#,
        true,
        &OutputPlan {
            analysis: true,
            graph: false,
        },
        &[],
        None,
    );

    let payload = DocumentSnapshot::with_analysis("analysis-nested-json", result.analysis)
        .analysis_payload(true)
        .expect("analysis payload should exist");
    let root = payload
        .tree
        .expect("tree payload should exist when include_tree_value=true");
    let value: serde_json::Value = serde_json::from_str(
        payload
            .value_json
            .as_deref()
            .expect("analysis value_json should exist"),
    )
    .expect("analysis value_json should parse");
    let nested = value.get("nested").expect("nested value should exist");

    assert!(nested.is_object());
    assert_eq!(
        mapping_value_sem_type(&root, "nested"),
        Some(SemType::Map as i32)
    );
}
