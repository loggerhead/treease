use crate::core::LineIndex;
use crate::formats::DecodedDocument;
use crate::wasm_types::AnalysisSharedArtifacts;

use super::{semantic_tokens_shared, value_json_shared};
fn build_lightweight_analysis_shared(
    language: &str,
    source: &str,
    decoded: &DecodedDocument,
) -> AnalysisSharedArtifacts {
    AnalysisSharedArtifacts {
        source_byte_length: source.len() as u32,
        semantic_tokens: semantic_tokens_shared::encode_document_semantic_tokens(language, source),
        value_json: value_json_shared::encode_document_value_json(decoded).unwrap_or_default(),
        ts_tree: None,
        token_spans: Vec::new(),
        line_index: LineIndex::build(source),
    }
}

fn diagnostics_for_decoded_source(language: &str, source: &str) -> Vec<u32> {
    // JSON has no vendored tree-sitter grammar; the JS grammar fallback used
    // by `parse_supported_language` is unreliable as a syntax authority for
    // valid JSON.  When this function is reached the source has already been
    // decoded successfully, so trust the decoder for JSON.
    if language == "json" {
        return Vec::new();
    }
    let tree_sitter_ok = crate::core::parse_supported_language(language, source, None)
        .map(|summary| !summary.has_error)
        .unwrap_or(true);
    if tree_sitter_ok {
        Vec::new()
    } else {
        crate::wasm::semantic_tokens_shared::encode_error_spans_raw(&[
            crate::wasm::semantic_tokens_shared::ErrorSpan {
                start_row: 0,
                start_col: 0,
                end_row: 0,
                end_col: source.len() as u32,
                kind: 1,
            },
        ])
    }
}
pub fn build_analysis_shared(
    language: &str,
    source: &str,
    decoded: Option<&DecodedDocument>,
) -> (AnalysisSharedArtifacts, Vec<u32>) {
    // Use the full analysis pipeline from core which does tree-sitter parsing,
    // error span collection, token span collection, and semantic token encoding.
    let mut analysis = crate::core::document_analysis::analyze_document_internal_with_demand(
        language,
        source.as_bytes(),
        false,
        crate::core::document_analysis::DocumentAnalysisDemand::full(),
    );

    if let Some(stored) = analysis.stored.take() {
        return (
            AnalysisSharedArtifacts {
                source_byte_length: source.len() as u32,
                semantic_tokens: stored.semantic_tokens_encoded,
                value_json: decoded
                    .and_then(value_json_shared::encode_document_value_json)
                    .or(stored.value_json)
                    .unwrap_or_default(),
                ts_tree: stored.ts_tree,
                token_spans: stored.token_spans,
                line_index: stored.line_index,
            },
            analysis.diagnostics_raw,
        );
    }

    if !analysis.diagnostics_raw.is_empty() {
        return (
            AnalysisSharedArtifacts {
                source_byte_length: source.len() as u32,
                semantic_tokens: Vec::new(),
                value_json: String::new(),
                ts_tree: None,
                token_spans: Vec::new(),
                line_index: LineIndex::default(),
            },
            analysis.diagnostics_raw,
        );
    }
    let fallback_shared = if let Some(decoded) = decoded {
        build_lightweight_analysis_shared(language, source, decoded)
    } else {
        AnalysisSharedArtifacts {
            source_byte_length: source.len() as u32,
            semantic_tokens: semantic_tokens_shared::encode_document_semantic_tokens(
                language, source,
            ),
            value_json: decoded
                .and_then(value_json_shared::encode_document_value_json)
                .unwrap_or_default(),
            ts_tree: None,
            token_spans: Vec::new(),
            line_index: LineIndex::default(),
        }
    };
    let fallback_diagnostics = if !analysis.diagnostics_raw.is_empty() {
        analysis.diagnostics_raw
    } else if decoded.is_some() {
        diagnostics_for_decoded_source(language, source)
    } else {
        diagnostics_for_decoded_source(language, source)
    };
    (fallback_shared, fallback_diagnostics)
}
