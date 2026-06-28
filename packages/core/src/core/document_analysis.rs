use crate::core::codec_service::CodecService;
use crate::core::lang_spec::{self, has_structured_path, parse_tree, query_from_language};
use crate::core::semantic_tokens::{
    collect_token_spans_with_tree, encode_and_cache_semantic_tokens,
};
use crate::core::tree_sitter_support::tree_sitter_language_for_spec;
use crate::core::{SemType, TreeNode, TreeNodeKind};
use crate::formats::DecodedDocument;
use crate::stream::streaming_json;

use super::line_index::LineIndex;
use super::tree_store::{TokenSpan, TreeStore};

fn token_spans_from_json(spans: Vec<streaming_json::TokenSpan>) -> Vec<TokenSpan> {
    spans
        .into_iter()
        .map(|span| TokenSpan {
            start_row: span.start_row,
            start_col: span.start_col,
            end_row: span.end_row,
            end_col: span.end_col,
            token_type: span.token_type,
        })
        .collect()
}

fn rebase_token_spans_from_trimmed_source(
    original_source: &str,
    trimmed_source: &str,
    token_spans: &mut [TokenSpan],
) {
    if token_spans.is_empty() || trimmed_source.is_empty() {
        return;
    }

    let Some(prefix_len) = original_source.find(trimmed_source) else {
        return;
    };
    if prefix_len == 0 {
        return;
    }

    let original_index = LineIndex::build(original_source);
    let trimmed_index = LineIndex::build(trimmed_source);
    for span in token_spans {
        let start_offset =
            prefix_len + trimmed_index.line_column_to_offset(span.start_row, span.start_col);
        let end_offset =
            prefix_len + trimmed_index.line_column_to_offset(span.end_row, span.end_col);
        let start = original_index.offset_to_line_column(start_offset);
        let end = original_index.offset_to_line_column(end_offset);
        span.start_row = start.line;
        span.start_col = start.column;
        span.end_row = end.line;
        span.end_col = end.column;
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorSpan {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
    pub kind: u32,
}

#[derive(Debug, Clone)]
pub struct StoredDocumentAnalysisOwned {
    pub language: String,
    pub root: crate::core::NodeId,
    pub source: Vec<u8>,
    pub ts_tree: Option<tree_sitter::Tree>,
    pub token_spans: Vec<TokenSpan>,
    pub diagnostics_raw: Vec<u32>,
    pub semantic_tokens_encoded: Vec<u32>,
    pub value_json: Option<String>,
    pub line_index: LineIndex,
}

impl Default for StoredDocumentAnalysisOwned {
    fn default() -> Self {
        Self {
            language: String::new(),
            root: crate::core::NodeId(0),
            source: Vec::new(),
            ts_tree: None,
            token_spans: Vec::new(),
            diagnostics_raw: Vec::new(),
            semantic_tokens_encoded: Vec::new(),
            value_json: None,
            line_index: LineIndex::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransientDocumentAnalysis {
    pub diagnostics_raw: Vec<u32>,
    pub stored: Option<StoredDocumentAnalysisOwned>,
}

impl TransientDocumentAnalysis {
    pub fn deinit(&mut self) {
        self.diagnostics_raw.clear();
        self.stored = None;
    }

    pub fn take_diagnostics_raw(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.diagnostics_raw)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StoredDocumentAnalysisDemand {
    pub ts_tree: bool,
    pub token_spans: bool,
    pub semantic_tokens: bool,
    pub value_json: bool,
}

impl StoredDocumentAnalysisDemand {
    pub const fn full() -> Self {
        Self {
            ts_tree: true,
            token_spans: true,
            semantic_tokens: true,
            value_json: true,
        }
    }

    fn requests_any(self) -> bool {
        self.ts_tree || self.token_spans || self.semantic_tokens || self.value_json
    }

    fn needs_non_streaming_ts_tree(self) -> bool {
        self.ts_tree || self.token_spans || self.semantic_tokens
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentAnalysisDemand {
    pub diagnostics: bool,
    pub stored: Option<StoredDocumentAnalysisDemand>,
}

impl DocumentAnalysisDemand {
    pub const fn diagnostics_only() -> Self {
        Self {
            diagnostics: true,
            stored: None,
        }
    }

    pub const fn full() -> Self {
        Self {
            diagnostics: true,
            stored: Some(StoredDocumentAnalysisDemand::full()),
        }
    }

    fn stored_demand(self) -> Option<StoredDocumentAnalysisDemand> {
        self.stored.filter(|stored| stored.requests_any())
    }

    fn needs_non_streaming_ts_tree(self) -> bool {
        self.diagnostics
            || self
                .stored_demand()
                .is_some_and(StoredDocumentAnalysisDemand::needs_non_streaming_ts_tree)
    }
}

/// Collect error spans from a tree-sitter [`Tree`] with fine-grained distinction
/// between error nodes, missing nodes, and nodes that contain errors in
/// descendants but are not themselves erroneous.
///
///
/// * `is_error` -- the node itself is an ERROR node (or has kind "ERROR").
/// * `is_missing` -- the node is MISSING (or has kind "MISSING").
/// * `has_error_child` -- at least one direct child is an error, missing, or
///   contains an error.
///
/// A span is emitted when:
///   `is_error || is_missing || (has_error && !has_error_child)`
///
/// The `kind` field is `2` for missing nodes and `1` for error nodes.
pub fn collect_error_spans(tree: &tree_sitter::Tree) -> Vec<ErrorSpan> {
    let mut spans = Vec::new();
    let root = tree.root_node();

    // Stack for depth-first traversal.
    let mut stack: Vec<tree_sitter::Node<'_>> = Vec::new();
    stack.push(root);

    while let Some(node) = stack.pop() {
        let node_type = node.kind();
        let is_error = node.is_error() || node_type == "ERROR";
        let is_missing = node.is_missing() || node_type == "MISSING";
        let has_error = node.has_error();

        // Check children for error/missing and push them onto the stack.
        let mut cursor = node.walk();
        let mut has_error_child = false;
        for child in node.children(&mut cursor) {
            let child_type = child.kind();
            if child.has_error()
                || child.is_error()
                || child.is_missing()
                || child_type == "ERROR"
                || child_type == "MISSING"
            {
                has_error_child = true;
            }
            stack.push(child);
        }

        if is_error || is_missing || (has_error && !has_error_child) {
            let start = node.start_position();
            let end = node.end_position();
            let end_row = end.row;
            let mut end_col = end.column;

            // Zero-width spans get a minimum width of 1 column.
            if start.row == end_row && start.column == end_col {
                end_col += 1;
            }

            spans.push(ErrorSpan {
                start_row: start.row as u32,
                start_col: start.column as u32,
                end_row: end_row as u32,
                end_col: end_col as u32,
                kind: if is_missing { 2 } else { 1 },
            });
        }
    }

    spans
}

fn source_without_utf8_bom(source: &str) -> (&str, usize) {
    if let Some(stripped) = source.strip_prefix('\u{feff}') {
        return (stripped, source.len() - stripped.len());
    }
    (source, 0)
}

fn collect_streaming_error_spans(language_name: &str, source: &str) -> Vec<ErrorSpan> {
    let (normalized_source, prefix_len) = source_without_utf8_bom(source);
    let mut spans: Vec<ErrorSpan> = match language_name {
        "json" => streaming_json::error_spans(normalized_source)
            .into_iter()
            .map(|span| ErrorSpan {
                start_row: span.start_row,
                start_col: span.start_col,
                end_row: span.end_row,
                end_col: span.end_col,
                kind: span.kind,
            })
            .collect(),
        // Non-streaming languages: error spans are collected via tree-sitter
        // in analyze_document_internal / analyze_document_internal_with_prepared_tree.
        _ => Vec::new(),
    };

    if prefix_len > 0 {
        let prefix_col_delta = source[..prefix_len].chars().count() as u32;
        for span in &mut spans {
            if span.start_row == 0 {
                span.start_col = span.start_col.saturating_add(prefix_col_delta);
            }
            if span.end_row == 0 {
                span.end_col = span.end_col.saturating_add(prefix_col_delta);
            }
        }
    }

    if spans.is_empty() {
        match language_name {
            "json" if normalized_source.trim().is_empty() => {
                spans.push(error_span_at_offset(source, source.len()));
            }
            _ => {}
        }
    }

    spans
}

pub fn error_spans_to_diagnostics_raw(spans: &[ErrorSpan]) -> Vec<u32> {
    let shared: Vec<crate::wasm::semantic_tokens_shared::ErrorSpan> = spans
        .iter()
        .map(|s| crate::wasm::semantic_tokens_shared::ErrorSpan {
            start_row: s.start_row,
            start_col: s.start_col,
            end_row: s.end_row,
            end_col: s.end_col,
            kind: s.kind,
        })
        .collect();
    crate::wasm::semantic_tokens_shared::encode_error_spans_raw(&shared)
}

/// Store a [`TransientDocumentAnalysis`] into the [`TreeStore`], writing back
/// diagnostics, semantic tokens, value JSON, token spans, and the optional
/// tree-sitter tree under `cache_key`.
///
pub fn store_transient_document_analysis(
    store: &mut TreeStore,
    cache_key: &str,
    analysis: &mut TransientDocumentAnalysis,
) {
    let Some(stored) = analysis.stored.take() else {
        return;
    };

    let source_str = String::from_utf8_lossy(&stored.source).into_owned();

    store.set_document_analysis(
        cache_key,
        &stored.language,
        stored.root,
        stored.ts_tree,
        &source_str,
        stored.token_spans,
        analysis.diagnostics_raw.clone(),
        stored.semantic_tokens_encoded,
        stored.value_json.unwrap_or_default(),
    );
}

pub fn analyze_document_internal_via_streaming_codec(
    language_name: &str,
    source: &[u8],
    nest: bool,
    analysis: &mut TransientDocumentAnalysis,
) -> bool {
    analyze_document_internal_via_streaming_codec_with_demand(
        language_name,
        source,
        nest,
        DocumentAnalysisDemand::full(),
        analysis,
    )
}

fn analyze_document_internal_via_streaming_codec_with_demand(
    language_name: &str,
    source: &[u8],
    _nest: bool,
    demand: DocumentAnalysisDemand,
    analysis: &mut TransientDocumentAnalysis,
) -> bool {
    if language_name != "json" {
        return false;
    }

    let text = String::from_utf8_lossy(source);
    let spans = collect_streaming_error_spans(language_name, &text);
    if demand.diagnostics {
        analysis.diagnostics_raw = error_spans_to_diagnostics_raw(&spans);
    } else {
        analysis.diagnostics_raw.clear();
    }
    if !spans.is_empty() {
        analysis.stored = None;
        return true;
    }

    let Some(stored_demand) = demand.stored_demand() else {
        analysis.stored = None;
        return true;
    };

    let line_index = LineIndex::build(&text);
    let stored = match language_name {
        "json" => {
            let decoded = match streaming_json::decode_slice_to_tree_with_token_spans(&text) {
                Ok(decoded) => decoded,
                Err(_) => return false,
            };
            let value_json = if stored_demand.value_json {
                Some(
                    encode_document_value_json(&decoded.document)
                        .unwrap_or_else(|| text.clone().into_owned()),
                )
            } else {
                None
            };
            let root = decoded.document.root;
            let mut token_spans = if stored_demand.token_spans || stored_demand.semantic_tokens {
                let mut token_spans = token_spans_from_json(decoded.token_spans);
                rebase_token_spans_from_trimmed_source(
                    &text,
                    source_without_utf8_bom(&text).0.trim(),
                    &mut token_spans,
                );
                token_spans
            } else {
                Vec::new()
            };
            let semantic_tokens_encoded =
                if stored_demand.semantic_tokens && !token_spans.is_empty() {
                    encode_and_cache_semantic_tokens(None, "", &text, &token_spans)
                } else {
                    Vec::new()
                };
            if !stored_demand.token_spans {
                token_spans.clear();
            }
            StoredDocumentAnalysisOwned {
                language: language_name.to_string(),
                root,
                source: source.to_vec(),
                ts_tree: if stored_demand.ts_tree {
                    crate::core::lang_spec::parse_tree(language_name, source)
                } else {
                    None
                },
                token_spans,
                diagnostics_raw: analysis.diagnostics_raw.clone(),
                semantic_tokens_encoded,
                value_json,
                line_index,
            }
        }
        _ => return false,
    };
    analysis.stored = Some(stored);
    true
}

/// Analyze a document with an explicit output demand.
pub fn analyze_document_internal_with_demand(
    language_name: &str,
    source: &[u8],
    nest: bool,
    demand: DocumentAnalysisDemand,
) -> TransientDocumentAnalysis {
    analyze_document_internal_with_prepared_tree_and_demand(
        language_name,
        source,
        nest,
        None,
        demand,
    )
}

/// Legacy full-analysis wrapper.
pub fn analyze_document_internal(
    language_name: &str,
    source: &[u8],
    nest: bool,
) -> TransientDocumentAnalysis {
    analyze_document_internal_with_demand(
        language_name,
        source,
        nest,
        DocumentAnalysisDemand::full(),
    )
}

/// Legacy full-analysis wrapper with optional prepared tree-sitter [`Tree`].
pub fn analyze_document_internal_with_prepared_tree(
    language_name: &str,
    source: &[u8],
    nest: bool,
    prepared_ts_tree: Option<tree_sitter::Tree>,
) -> TransientDocumentAnalysis {
    analyze_document_internal_with_prepared_tree_and_demand(
        language_name,
        source,
        nest,
        prepared_ts_tree,
        DocumentAnalysisDemand::full(),
    )
}

/// Analyze a document with an optional pre-parsed tree-sitter [`Tree`] and an
/// explicit output demand.
pub fn analyze_document_internal_with_prepared_tree_and_demand(
    language_name: &str,
    source: &[u8],
    _nest: bool,
    prepared_ts_tree: Option<tree_sitter::Tree>,
    demand: DocumentAnalysisDemand,
) -> TransientDocumentAnalysis {
    let mut analysis = TransientDocumentAnalysis::default();

    if prepared_ts_tree.is_none()
        && analyze_document_internal_via_streaming_codec_with_demand(
            language_name,
            source,
            _nest,
            demand,
            &mut analysis,
        )
    {
        return analysis;
    }

    let text = String::from_utf8_lossy(source);
    let stored_demand = demand.stored_demand();
    let mut ts_tree: Option<tree_sitter::Tree> = None;

    if demand.needs_non_streaming_ts_tree() {
        ts_tree = if let Some(old_tree) = prepared_ts_tree {
            lang_spec::parse_tree_incremental(language_name, source, Some(&old_tree))
        } else {
            lang_spec::parse_tree(language_name, source)
        };

        if demand.diagnostics {
            if let Some(ref tree) = ts_tree {
                let spans = collect_error_spans(tree);
                analysis.diagnostics_raw = error_spans_to_diagnostics_raw(&spans);
            } else {
                analysis.diagnostics_raw.clear();
            }
        }
    }

    if source.is_empty() {
        return analysis;
    }

    let Some(stored_demand) = stored_demand else {
        return analysis;
    };

    let codec = CodecService::new();
    let decoded = match codec.decode(language_name, &text) {
        Ok(doc) => doc,
        Err(_) => return analysis,
    };

    let mut store_tree: Option<tree_sitter::Tree> = None;
    if stored_demand.ts_tree && has_structured_path(language_name) {
        store_tree = ts_tree.take();
        if store_tree.is_none() {
            store_tree = parse_tree(language_name, source);
        }
    }

    let mut token_spans: Vec<TokenSpan> = Vec::new();
    let mut semantic_tokens_encoded: Vec<u32> = Vec::new();

    if stored_demand.token_spans || stored_demand.semantic_tokens {
        let token_tree = store_tree.clone().or(ts_tree.take());

        if let Some(ref tree) = token_tree {
            if let Some(query_src) = query_from_language(language_name) {
                if let Some(spec) = lang_spec::find_spec(language_name) {
                    if let Some(lang) = tree_sitter_language_for_spec(&spec) {
                        token_spans = collect_token_spans_with_tree(tree, &lang, query_src, &text);
                    }
                }
            }
        }

        if stored_demand.semantic_tokens && !token_spans.is_empty() {
            semantic_tokens_encoded =
                encode_and_cache_semantic_tokens(None, "", &text, &token_spans);
        }
        if !stored_demand.token_spans {
            token_spans.clear();
        }
    }

    let value_json = if stored_demand.value_json {
        encode_document_value_json(&decoded)
    } else {
        None
    };

    let line_index = LineIndex::build(&text);

    analysis.stored = Some(StoredDocumentAnalysisOwned {
        language: language_name.to_string(),
        root: decoded.root,
        source: source.to_vec(),
        ts_tree: store_tree,
        token_spans,
        diagnostics_raw: analysis.diagnostics_raw.clone(),
        semantic_tokens_encoded,
        value_json,
        line_index,
    });

    analysis
}

/// Build analysis artifacts from a caller-owned decoded document.
///
/// This path MUST NOT decode `source`. The caller owns `DecodedDocument`
/// creation so materialization can enforce a single decode per job.
pub(crate) fn analyze_decoded_document_with_prepared_tree_and_demand(
    language_name: &str,
    source: &[u8],
    _nest: bool,
    prepared_ts_tree: Option<tree_sitter::Tree>,
    demand: DocumentAnalysisDemand,
    decoded: &DecodedDocument,
) -> TransientDocumentAnalysis {
    let mut analysis = TransientDocumentAnalysis::default();

    if prepared_ts_tree.is_none()
        && analyze_decoded_document_via_streaming_codec_with_demand(
            language_name,
            source,
            demand,
            &mut analysis,
            decoded,
        )
    {
        return analysis;
    }

    let text = String::from_utf8_lossy(source);
    let stored_demand = demand.stored_demand();
    let mut ts_tree: Option<tree_sitter::Tree> = None;

    if demand.needs_non_streaming_ts_tree() {
        ts_tree = if let Some(old_tree) = prepared_ts_tree {
            lang_spec::parse_tree_incremental(language_name, source, Some(&old_tree))
        } else {
            lang_spec::parse_tree(language_name, source)
        };

        if demand.diagnostics {
            if let Some(ref tree) = ts_tree {
                let spans = collect_error_spans(tree);
                analysis.diagnostics_raw = error_spans_to_diagnostics_raw(&spans);
            } else {
                analysis.diagnostics_raw.clear();
            }
        }
    }

    if source.is_empty() {
        return analysis;
    }

    let Some(stored_demand) = stored_demand else {
        return analysis;
    };

    let mut store_tree: Option<tree_sitter::Tree> = None;
    if stored_demand.ts_tree && has_structured_path(language_name) {
        store_tree = ts_tree.take();
        if store_tree.is_none() {
            store_tree = parse_tree(language_name, source);
        }
    }

    let mut token_spans: Vec<TokenSpan> = Vec::new();
    let mut semantic_tokens_encoded: Vec<u32> = Vec::new();

    if stored_demand.token_spans || stored_demand.semantic_tokens {
        let token_tree = store_tree.clone().or(ts_tree.take());

        if let Some(ref tree) = token_tree {
            if let Some(query_src) = query_from_language(language_name) {
                if let Some(spec) = lang_spec::find_spec(language_name) {
                    if let Some(lang) = tree_sitter_language_for_spec(&spec) {
                        token_spans = collect_token_spans_with_tree(tree, &lang, query_src, &text);
                    }
                }
            }
        }

        if stored_demand.semantic_tokens && !token_spans.is_empty() {
            semantic_tokens_encoded =
                encode_and_cache_semantic_tokens(None, "", &text, &token_spans);
        }
        if !stored_demand.token_spans {
            token_spans.clear();
        }
    }

    let value_json = if stored_demand.value_json {
        encode_document_value_json(decoded)
    } else {
        None
    };

    let line_index = LineIndex::build(&text);

    analysis.stored = Some(StoredDocumentAnalysisOwned {
        language: language_name.to_string(),
        root: decoded.root,
        source: source.to_vec(),
        ts_tree: store_tree,
        token_spans,
        diagnostics_raw: analysis.diagnostics_raw.clone(),
        semantic_tokens_encoded,
        value_json,
        line_index,
    });

    analysis
}

fn analyze_decoded_document_via_streaming_codec_with_demand(
    language_name: &str,
    source: &[u8],
    demand: DocumentAnalysisDemand,
    analysis: &mut TransientDocumentAnalysis,
    decoded: &DecodedDocument,
) -> bool {
    if language_name != "json" {
        return false;
    }

    let text = String::from_utf8_lossy(source);
    let spans = collect_streaming_error_spans(language_name, &text);
    if demand.diagnostics {
        analysis.diagnostics_raw = error_spans_to_diagnostics_raw(&spans);
    } else {
        analysis.diagnostics_raw.clear();
    }
    if !spans.is_empty() {
        analysis.stored = None;
        return true;
    }

    let Some(stored_demand) = demand.stored_demand() else {
        analysis.stored = None;
        return true;
    };

    let line_index = LineIndex::build(&text);
    let value_json = if stored_demand.value_json {
        Some(encode_document_value_json(decoded).unwrap_or_else(|| text.clone().into_owned()))
    } else {
        None
    };
    let mut token_spans = if stored_demand.token_spans || stored_demand.semantic_tokens {
        let (token_source, prefix_len) = source_without_utf8_bom(&text);
        let mut token_spans = match streaming_json::token_spans(token_source) {
            Ok(spans) => token_spans_from_json(spans),
            Err(_) => return false,
        };
        if prefix_len > 0 {
            let prefix_col_delta = prefix_len as u32;
            for span in &mut token_spans {
                if span.start_row == 0 {
                    span.start_col = span.start_col.saturating_add(prefix_col_delta);
                }
                if span.end_row == 0 {
                    span.end_col = span.end_col.saturating_add(prefix_col_delta);
                }
            }
        }
        token_spans
    } else {
        Vec::new()
    };
    let semantic_tokens_encoded = if stored_demand.semantic_tokens && !token_spans.is_empty() {
        encode_and_cache_semantic_tokens(None, "", &text, &token_spans)
    } else {
        Vec::new()
    };
    if !stored_demand.token_spans {
        token_spans.clear();
    }

    analysis.stored = Some(StoredDocumentAnalysisOwned {
        language: language_name.to_string(),
        root: decoded.root,
        source: source.to_vec(),
        ts_tree: if stored_demand.ts_tree {
            crate::core::lang_spec::parse_tree(language_name, source)
        } else {
            None
        },
        token_spans,
        diagnostics_raw: analysis.diagnostics_raw.clone(),
        semantic_tokens_encoded,
        value_json,
        line_index,
    });
    true
}

pub(crate) fn encode_document_value_json(document: &DecodedDocument) -> Option<String> {
    let mut out = String::new();
    encode_analysis_node(document, document.root, &mut out)?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{DocumentAnalysisDemand, analyze_document_internal_with_demand};

    #[test]
    fn json_root_string_keeps_semantic_tokens_in_document_analysis() {
        let analysis = analyze_document_internal_with_demand(
            "json",
            br#""left-string""#,
            false,
            DocumentAnalysisDemand::full(),
        );
        let stored = analysis
            .stored
            .as_ref()
            .expect("document analysis should keep stored artifacts");
        assert!(
            !stored.semantic_tokens_encoded.is_empty(),
            "root string should keep semantic tokens in document analysis",
        );
    }
}

fn encode_analysis_node(
    document: &DecodedDocument,
    node_id: crate::core::NodeId,
    out: &mut String,
) -> Option<()> {
    let node = document.store.get(node_id)?;
    match node.kind {
        TreeNodeKind::Sequence => {
            out.push('[');
            for (index, child) in node.content.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                encode_analysis_node(document, *child, out)?;
            }
            out.push(']');
        }
        TreeNodeKind::Mapping => {
            out.push('{');
            let mut first = true;
            for pair in node.content.chunks_exact(2) {
                let key_node = document.store.get(pair[0])?;
                if !first {
                    out.push(',');
                }
                first = false;
                out.push_str(&crate::formats::escape_json_string(&key_node.value));
                out.push(':');
                encode_analysis_node(document, pair[1], out)?;
            }
            out.push('}');
        }
        TreeNodeKind::Alias | TreeNodeKind::Scalar | TreeNodeKind::Unknown => {
            encode_scalar(node, out)?;
        }
    }
    Some(())
}

fn encode_scalar(node: &TreeNode, out: &mut String) -> Option<()> {
    match node.resolved_sem_type().unwrap_or(SemType::Str) {
        SemType::Nil => out.push_str("null"),
        SemType::Boolean => match crate::core::core_helpers::parse_bool(&node.value) {
            Some(true) => out.push_str("true"),
            Some(false) => out.push_str("false"),
            None => out.push_str(&crate::formats::escape_json_string(&node.value)),
        },
        SemType::Int => match node.value.parse::<i64>() {
            Ok(value) => out.push_str(&value.to_string()),
            Err(_) => out.push_str(&crate::formats::escape_json_string(&node.value)),
        },
        SemType::Float => match node.value.parse::<f64>() {
            Ok(value) => out.push_str(&value.to_string()),
            Err(_) => out.push_str(&crate::formats::escape_json_string(&node.value)),
        },
        SemType::Map | SemType::Seq | SemType::Str => {
            out.push_str(&crate::formats::escape_json_string(&node.value));
        }
    }
    Some(())
}

fn error_span_at_offset(source: &str, offset: usize) -> ErrorSpan {
    let index = LineIndex::build(source);
    let clamped = offset.min(source.len());
    let start = index.offset_to_line_column(clamped);
    let end_offset = if clamped < source.len() {
        clamped.saturating_add(1)
    } else {
        clamped
    };
    let mut end = index.offset_to_line_column(end_offset);
    if start.line == end.line && start.column == end.column {
        end.column = end.column.saturating_add(1);
    }
    ErrorSpan {
        start_row: start.line,
        start_col: start.column,
        end_row: end.line,
        end_col: end.column,
        kind: 1,
    }
}
