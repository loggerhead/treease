use super::protocol::{
    DocumentAnalysisPayload, DocumentDiagnostic, DocumentTreeNode, GraphValueEditFallbackReason,
    GraphValueEditPlan, GraphValueEditRequest, QueryResult, SemanticTokensPayload, SnapshotId,
    SnapshotQuery,
};
use crate::analysis::document_analysis::{
    analyze_decoded_document_with_prepared_tree_and_demand, analyze_document_internal_with_demand,
    encode_document_value_json, DocumentAnalysisDemand,
};
use crate::analysis::line_index::LineIndex;
use crate::analysis::span_index::StructuralSpanIndex;
use crate::formats::DecodedDocument;
use crate::language::semantic_tokens::encode_semantic_tokens;
use crate::tree::tree_node::NodeId;
use crate::tree::tree_store::TokenSpan;
use crate::wasm_types::{AnalysisSharedArtifacts, PathSpan, WasmProtocol};
use crate::{core::SemType, graph, layout, tree};

#[derive(Debug, Clone, Default)]
pub struct DocumentSnapshot {
    pub snapshot_id: SnapshotId,
    pub document_key: String,
    pub analysis: Option<AnalysisBundle>,
    pub graph: Option<GraphProjection>,
    pub incremental: Option<IncrementalState>,
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisBundle {
    pub key: String,
    pub language: String,
    pub source: String,
    pub source_byte_length: u32,
    pub document: Option<DecodedDocument>,
    pub ts_tree: Option<tree_sitter::Tree>,
    pub token_spans: Vec<TokenSpan>,
    pub diagnostics: Vec<u32>,
    pub semantic_tokens: Vec<u32>,
    pub value_json: String,
    pub line_index: LineIndex,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GraphProjection {
    pub ready: bool,
    pub clear: bool,
    pub graph_data: Option<crate::document::protocol::GraphDelta>,
    pub topology_bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedSpanAuthority {
    Unknown,
    Complete,
    UnresolvedNodes(Vec<NodeId>),
}

impl Default for DecodedSpanAuthority {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Default)]
pub struct DecodedAnalysisArtifacts {
    pub ts_tree: Option<tree_sitter::Tree>,
    pub token_spans: Vec<TokenSpan>,
    pub diagnostics: Vec<u32>,
    pub semantic_tokens: Vec<u32>,
    pub value_json: String,
    pub line_index: LineIndex,
    pub span_authority: DecodedSpanAuthority,
}
#[derive(Debug, Clone, Default)]
pub struct IncrementalState {
    pub can_resume: bool,
    pub graph_model_snapshot: Option<graph::graph_builder::GraphModelSnapshot>,
    pub graph_model_index: Option<graph::graph_model_index::GraphModelIndex>,
    pub structural_safe: bool,
    pub fallback_reason: Option<String>,
    pub structural_span_index: Option<StructuralSpanIndex>,
    pub tree_path_index: Option<tree::tree_path_index::TreePathIndex>,
    pub(crate) graph_topology: Option<graph::graph_topology::GraphTopology>,
    pub(crate) layout_state: Option<layout::layout_engine::LayoutState>,
}

impl IncrementalState {
    pub fn resumable() -> Self {
        Self {
            can_resume: true,
            ..Self::default()
        }
    }

    pub fn with_graph_state(
        mut self,
        graph_model_snapshot: graph::graph_builder::GraphModelSnapshot,
        graph_model_index: graph::graph_model_index::GraphModelIndex,
    ) -> Self {
        self.graph_model_snapshot = Some(graph_model_snapshot);
        self.graph_model_index = Some(graph_model_index);
        self.structural_safe = true;
        self
    }

    pub fn with_tree_path_index(
        mut self,
        tree_path_index: tree::tree_path_index::TreePathIndex,
    ) -> Self {
        self.tree_path_index = Some(tree_path_index);
        self
    }

    pub(crate) fn with_graph_runtime_state(
        mut self,
        topology: graph::graph_topology::GraphTopology,
        layout_state: layout::layout_engine::LayoutState,
    ) -> Self {
        self.graph_topology = Some(topology);
        self.layout_state = Some(layout_state);
        self
    }

    pub(crate) fn graph_topology(&self) -> Option<&graph::graph_topology::GraphTopology> {
        self.graph_topology.as_ref()
    }

    pub(crate) fn layout_state(&self) -> Option<&layout::layout_engine::LayoutState> {
        self.layout_state.as_ref()
    }

    pub fn fallback(reason: impl Into<String>) -> Self {
        Self {
            fallback_reason: Some(reason.into()),
            ..Self::default()
        }
    }
}

impl DocumentSnapshot {
    fn with_analysis_parts(
        document_key: impl Into<String>,
        analysis: AnalysisBundle,
        incremental: Option<IncrementalState>,
    ) -> Self {
        Self {
            snapshot_id: SnapshotId::default(),
            document_key: document_key.into(),
            analysis: Some(analysis),
            graph: None,
            incremental,
        }
    }

    pub fn with_analysis(document_key: impl Into<String>, analysis: AnalysisBundle) -> Self {
        Self::with_analysis_parts(document_key, analysis, None)
    }

    pub fn with_incremental_analysis(
        document_key: impl Into<String>,
        analysis: AnalysisBundle,
        incremental: IncrementalState,
    ) -> Self {
        Self::with_analysis_parts(document_key, analysis, Some(incremental))
    }
}

impl DocumentDiagnostic {
    pub fn from_flat(raw: &[u32]) -> Vec<Self> {
        raw.chunks_exact(5)
            .map(|chunk| Self {
                start_line_number: chunk[0].saturating_add(1),
                start_column: chunk[1].saturating_add(1),
                end_line_number: chunk[2].saturating_add(1),
                end_column: chunk[3].saturating_add(1),
                kind: chunk[4],
            })
            .collect()
    }
}

impl DocumentSnapshot {
    pub fn analysis_payload(&self, include_tree_value: bool) -> Option<DocumentAnalysisPayload> {
        self.analysis
            .as_ref()
            .map(|analysis| analysis_payload_from_bundle(analysis, include_tree_value))
    }
}

pub fn analysis_payload_from_bundle(
    analysis: &AnalysisBundle,
    include_tree_value: bool,
) -> DocumentAnalysisPayload {
    let tree = include_tree_value
        .then(|| {
            analysis
                .document
                .as_ref()
                .and_then(|doc| document_tree_node_from_store(&doc.store, doc.root))
        })
        .flatten();
    let value_json = if include_tree_value && !analysis.value_json.is_empty() {
        Some(analysis.value_json.clone())
    } else {
        None
    };

    DocumentAnalysisPayload {
        tree,
        value_json,
        diagnostics: DocumentDiagnostic::from_flat(&analysis.diagnostics),
        semantic_tokens: SemanticTokensPayload {
            data: analysis.semantic_tokens.clone(),
            version: 1,
        },
        source_byte_length: analysis.source_byte_length,
        language: analysis.language.clone(),
        ..Default::default()
    }
}

fn document_tree_node_from_store(
    store: &crate::tree::TreeStore,
    id: crate::tree::NodeId,
) -> Option<DocumentTreeNode> {
    let node = store.get(id)?;
    Some(DocumentTreeNode {
        kind: tree_kind_code(node.kind),
        sem_type: sem_type_code(node.resolved_sem_type()),
        tag: node.tag.to_string_value(),
        value: store.value_string_for(id).ok()?,
        children: node
            .content
            .iter()
            .filter_map(|child| document_tree_node_from_store(store, *child))
            .collect(),
    })
}

pub(crate) fn tree_kind_code(kind: crate::tree::TreeNodeKind) -> i32 {
    match kind {
        crate::tree::TreeNodeKind::Sequence => 0,
        crate::tree::TreeNodeKind::Mapping => 1,
        crate::tree::TreeNodeKind::Scalar => 2,
        crate::tree::TreeNodeKind::Alias => 3,
        crate::tree::TreeNodeKind::Unknown => 4,
    }
}
pub(crate) fn sem_type_code(sem_type: Option<SemType>) -> i32 {
    match sem_type {
        Some(SemType::Map) => 0,
        Some(SemType::Seq) => 1,
        Some(SemType::Str) => 2,
        Some(SemType::Int) => 3,
        Some(SemType::Float) => 4,
        Some(SemType::Boolean) => 5,
        Some(SemType::Nil) => 6,
        None => -1,
    }
}

fn encode_document_semantic_tokens(language: &str, source: &str) -> Vec<u32> {
    let Some(protocol) = WasmProtocol::from_name(language) else {
        return Vec::new();
    };
    encode_semantic_tokens(protocol.canonical_name(), source)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawErrorSpan {
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
    kind: u32,
}

fn encode_error_spans_raw(spans: &[RawErrorSpan]) -> Vec<u32> {
    if spans.is_empty() {
        return Vec::new();
    }
    let mut raw = Vec::with_capacity(spans.len() * 5);
    for span in spans {
        raw.extend([
            span.start_row,
            span.start_col,
            span.end_row,
            span.end_col,
            span.kind,
        ]);
    }
    raw
}
impl DocumentSnapshot {
    /// Execute a query against this snapshot.
    /// Queries are read-only: they never produce new authoritative state.
    pub(crate) fn query(&self, query: &SnapshotQuery) -> QueryResult {
        super::reads::query_snapshot(self, query)
    }
}

impl DocumentSnapshot {
    pub(crate) fn plan_graph_value_edit(
        &self,
        request: &GraphValueEditRequest,
    ) -> GraphValueEditPlan {
        let Some(analysis) = self.analysis.as_ref() else {
            return super::value_edit::graph_value_edit_fallback(
                GraphValueEditFallbackReason::MissingAnalysis,
            );
        };
        let Some(document) = analysis.document.as_ref() else {
            return super::value_edit::graph_value_edit_fallback(
                GraphValueEditFallbackReason::MissingDocument,
            );
        };
        let path_index = self
            .incremental
            .as_ref()
            .and_then(|state| state.tree_path_index.as_ref());
        super::value_edit::plan_graph_value_edit(analysis, document, request, path_index)
    }
}

// ── Analysis builders (existing) ───────────────────────────────────────

pub fn build_stored_analysis<E>(
    key: &str,
    language: &str,
    source: &str,
    decoded: &Result<DecodedDocument, E>,
) -> AnalysisBundle {
    let (shared, diagnostics) = build_analysis_shared(language, source, decoded.as_ref().ok());

    AnalysisBundle {
        key: key.to_owned(),
        language: language.to_owned(),
        source: source.to_owned(),
        source_byte_length: shared.source_byte_length,
        document: decoded.as_ref().ok().cloned(),
        ts_tree: shared.ts_tree,
        token_spans: shared.token_spans,
        diagnostics,
        semantic_tokens: shared.semantic_tokens,
        value_json: shared.value_json,
        line_index: shared.line_index,
    }
}

pub fn build_decoded_analysis(
    key: &str,
    language: &str,
    source: &str,
    decoded: &DecodedDocument,
) -> AnalysisBundle {
    build_decoded_analysis_with_prepared_tree(key, language, source, decoded, None)
}

pub(crate) fn build_decoded_analysis_with_prepared_tree(
    key: &str,
    language: &str,
    source: &str,
    decoded: &DecodedDocument,
    prepared_ts_tree: Option<tree_sitter::Tree>,
) -> AnalysisBundle {
    let mut analysis = analyze_decoded_document_with_prepared_tree_and_demand(
        language,
        source.as_bytes(),
        false,
        prepared_ts_tree,
        DocumentAnalysisDemand::full(),
        decoded,
    );

    let Some(stored) = analysis.stored.take() else {
        let diagnostics = if analysis.diagnostics_raw.is_empty() {
            diagnostics_for_decoded_source(language, source)
        } else {
            analysis.diagnostics_raw
        };
        let shared = build_lightweight_analysis_shared(language, source, decoded, false);
        let mut document = decoded.clone();
        annotate_decoded_document_spans(
            language,
            source,
            &mut document,
            shared.ts_tree.as_ref(),
            &diagnostics,
        );
        return build_analysis_from_shared(
            key,
            language,
            source,
            Some(document),
            shared,
            diagnostics,
        );
    };

    let diagnostics = analysis.diagnostics_raw;
    let shared = AnalysisSharedArtifacts {
        source_byte_length: source.len() as u32,
        semantic_tokens: stored.semantic_tokens_encoded,
        value_json: stored
            .value_json
            .or_else(|| encode_document_value_json(decoded))
            .unwrap_or_default(),
        ts_tree: stored.ts_tree,
        token_spans: stored.token_spans,
        line_index: stored.line_index,
    };
    let document = if !diagnostics.is_empty() && shared.value_json.is_empty() {
        None
    } else {
        let mut document = decoded.clone();
        annotate_decoded_document_spans(
            language,
            source,
            &mut document,
            shared.ts_tree.as_ref(),
            &diagnostics,
        );
        Some(document)
    };
    build_analysis_from_shared(key, language, source, document, shared, diagnostics)
}

pub fn build_decoded_analysis_with_artifacts(
    key: &str,
    language: &str,
    source: &str,
    decoded: &DecodedDocument,
    ts_tree: Option<tree_sitter::Tree>,
    token_spans: Vec<TokenSpan>,
    diagnostics: Vec<u32>,
    semantic_tokens: Vec<u32>,
    value_json: String,
) -> AnalysisBundle {
    let shared = AnalysisSharedArtifacts {
        source_byte_length: source.len() as u32,
        semantic_tokens,
        value_json,
        ts_tree,
        token_spans,
        line_index: LineIndex::build(source),
    };
    let document = if !diagnostics.is_empty() && shared.value_json.is_empty() {
        None
    } else {
        let mut document = decoded.clone();
        annotate_decoded_document_spans(
            language,
            source,
            &mut document,
            shared.ts_tree.as_ref(),
            &diagnostics,
        );
        Some(document)
    };
    build_analysis_from_shared(key, language, source, document, shared, diagnostics)
}

pub fn build_decoded_analysis_from_owned_artifacts(
    key: &str,
    language: &str,
    source: &str,
    mut decoded: DecodedDocument,
    artifacts: DecodedAnalysisArtifacts,
) -> AnalysisBundle {
    let DecodedAnalysisArtifacts {
        ts_tree,
        token_spans,
        diagnostics,
        semantic_tokens,
        value_json,
        line_index,
        span_authority,
    } = artifacts;
    let shared = AnalysisSharedArtifacts {
        source_byte_length: source.len() as u32,
        semantic_tokens,
        value_json,
        ts_tree,
        token_spans,
        line_index,
    };
    let document = if !diagnostics.is_empty() && shared.value_json.is_empty() {
        None
    } else {
        match span_authority {
            DecodedSpanAuthority::Complete => {}
            DecodedSpanAuthority::UnresolvedNodes(node_ids) => {
                annotate_decoded_document_spans_for_nodes(
                    language,
                    source,
                    &mut decoded,
                    shared.ts_tree.as_ref(),
                    &diagnostics,
                    &shared.line_index,
                    &node_ids,
                );
            }
            DecodedSpanAuthority::Unknown => {
                annotate_decoded_document_spans_with_line_index(
                    language,
                    source,
                    &mut decoded,
                    shared.ts_tree.as_ref(),
                    &diagnostics,
                    &shared.line_index,
                );
            }
        }
        Some(decoded)
    };
    build_analysis_from_shared(key, language, source, document, shared, diagnostics)
}

fn apply_resolved_spans(
    document: &mut DecodedDocument,
    line_index: &LineIndex,
    resolved: Vec<(NodeId, PathSpan)>,
) {
    for (id, span) in resolved {
        if span.start_byte < 0 || span.end_byte < span.start_byte {
            continue;
        }
        let start_byte = u32::try_from(span.start_byte).unwrap_or(0);
        let end_byte = u32::try_from(span.end_byte).unwrap_or(start_byte);
        let line_column = line_index.offset_to_line_column(start_byte as usize);
        if let Some(node) = document.store.get_mut(id) {
            node.start_byte = start_byte;
            node.end_byte = end_byte;
            node.line = line_column.line as i32 + 1;
            node.column = line_column.column as i32 + 1;
        }
    }
}

fn annotate_decoded_document_spans_with_line_index(
    language: &str,
    source: &str,
    document: &mut DecodedDocument,
    ts_tree: Option<&tree_sitter::Tree>,
    diagnostics: &[u32],
    line_index: &LineIndex,
) {
    if !diagnostics.is_empty() || document.store.is_empty() {
        return;
    }

    let root_end = source.len() as u32;
    let mut direct_updates = Vec::new();
    let mut unresolved = Vec::new();
    for index in 0..document.store.len() {
        let id = NodeId::from_index(index);
        let Some(node) = document.store.get(id) else {
            continue;
        };
        if node.end_byte > node.start_byte {
            continue;
        }

        if id == document.root && ts_tree.is_none() {
            direct_updates.push((id, 0, root_end, 1, 1));
            continue;
        }
        unresolved.push((id, node.is_map_key));
    }

    for (id, start_byte, end_byte, line, column) in direct_updates {
        if let Some(node) = document.store.get_mut(id) {
            node.start_byte = start_byte;
            node.end_byte = end_byte;
            node.line = line;
            node.column = column;
        }
    }

    let mut resolver = crate::tree::PathSpanResolver::new(
        &document.store,
        document.root,
        ts_tree,
        diagnostics,
        language,
        source,
    );
    apply_resolved_spans(document, line_index, resolver.resolve_nodes(unresolved));
}

fn annotate_decoded_document_spans_for_nodes(
    language: &str,
    source: &str,
    document: &mut DecodedDocument,
    ts_tree: Option<&tree_sitter::Tree>,
    diagnostics: &[u32],
    line_index: &LineIndex,
    unresolved_node_ids: &[NodeId],
) {
    if !diagnostics.is_empty() || document.store.is_empty() || unresolved_node_ids.is_empty() {
        return;
    }

    let unresolved = unresolved_node_ids
        .iter()
        .filter_map(|id| document.store.get(*id).map(|node| (*id, node.is_map_key)))
        .collect::<Vec<_>>();
    let mut resolver = crate::tree::PathSpanResolver::new(
        &document.store,
        document.root,
        ts_tree,
        diagnostics,
        language,
        source,
    );
    apply_resolved_spans(document, line_index, resolver.resolve_nodes(unresolved));
}
pub fn build_graph_cache_analysis(
    key: &str,
    language: &str,
    source: &str,
    decoded: &DecodedDocument,
) -> AnalysisBundle {
    let mut document = decoded.clone();
    let shared = AnalysisSharedArtifacts {
        source_byte_length: source.len() as u32,
        semantic_tokens: Vec::new(),
        value_json: String::new(),
        ts_tree: None,
        token_spans: Vec::new(),
        line_index: LineIndex::build(source),
    };
    annotate_decoded_document_spans(language, source, &mut document, None, &[]);
    build_analysis_from_shared(key, language, source, Some(document), shared, Vec::new())
}

pub fn build_source_only_analysis(key: &str, language: &str, source: &str) -> AnalysisBundle {
    build_analysis_from_shared(
        key,
        language,
        source,
        None,
        AnalysisSharedArtifacts {
            source_byte_length: source.len() as u32,
            semantic_tokens: Vec::new(),
            value_json: String::new(),
            ts_tree: None,
            token_spans: Vec::new(),
            line_index: LineIndex::build(source),
        },
        Vec::new(),
    )
}

pub fn build_analysis_shared(
    language: &str,
    source: &str,
    decoded: Option<&DecodedDocument>,
) -> (AnalysisSharedArtifacts, Vec<u32>) {
    let mut analysis = analyze_document_internal_with_demand(
        language,
        source.as_bytes(),
        false,
        DocumentAnalysisDemand::full(),
    );

    if let Some(stored) = analysis.stored.take() {
        return (
            AnalysisSharedArtifacts {
                source_byte_length: source.len() as u32,
                semantic_tokens: stored.semantic_tokens_encoded,
                value_json: decoded
                    .and_then(encode_document_value_json)
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
        build_lightweight_analysis_shared(language, source, decoded, true)
    } else {
        AnalysisSharedArtifacts {
            source_byte_length: source.len() as u32,
            semantic_tokens: encode_document_semantic_tokens(language, source),
            value_json: decoded
                .and_then(encode_document_value_json)
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
        diagnostics_for_source(language, source)
    };
    (fallback_shared, fallback_diagnostics)
}

pub(super) fn build_analysis_from_shared(
    key: &str,
    language: &str,
    source: &str,
    document: Option<DecodedDocument>,
    shared: AnalysisSharedArtifacts,
    diagnostics: Vec<u32>,
) -> AnalysisBundle {
    AnalysisBundle {
        key: key.to_owned(),
        language: language.to_owned(),
        source: source.to_owned(),
        source_byte_length: shared.source_byte_length,
        document,
        ts_tree: shared.ts_tree,
        token_spans: shared.token_spans,
        diagnostics,
        semantic_tokens: shared.semantic_tokens,
        value_json: shared.value_json,
        line_index: shared.line_index,
    }
}

pub(super) fn build_lightweight_analysis_shared(
    language: &str,
    source: &str,
    decoded: &DecodedDocument,
    build_ts_tree: bool,
) -> AnalysisSharedArtifacts {
    let ts_tree = if build_ts_tree {
        crate::language::parse_tree(language, source.as_bytes())
    } else {
        None
    };
    AnalysisSharedArtifacts {
        source_byte_length: source.len() as u32,
        semantic_tokens: encode_document_semantic_tokens(language, source),
        value_json: encode_document_value_json(decoded).unwrap_or_default(),
        ts_tree,
        token_spans: Vec::new(),
        line_index: LineIndex::build(source),
    }
}

/// Bulk-fill missing byte/line/column spans on a decoded document.
///
/// Complexity contract: this pass should trend toward
/// `O(document.store.len() + source.len())`, plus bounded span recovery for
/// nodes that still lack spans. It MUST NOT rebuild line indexes or parse trees
/// inside the per-node loop.
///
/// This pass uses a snapshot-bound bulk path/span resolver so one annotation
/// sweep reuses the same path index, structured fallback tree, and line index
/// instead of rebuilding them per node.
pub(super) fn annotate_decoded_document_spans(
    language: &str,
    source: &str,
    document: &mut DecodedDocument,
    ts_tree: Option<&tree_sitter::Tree>,
    diagnostics: &[u32],
) {
    let line_index = LineIndex::build(source);
    annotate_decoded_document_spans_with_line_index(
        language,
        source,
        document,
        ts_tree,
        diagnostics,
        &line_index,
    );
}

pub(crate) fn generic_parse_failed_diagnostics(source: &str) -> Vec<u32> {
    encode_error_spans_raw(&[RawErrorSpan {
        start_row: 0,
        start_col: 0,
        end_row: 0,
        end_col: source.len() as u32,
        kind: 1,
    }])
}

pub(super) fn diagnostics_for_decoded_source(language: &str, source: &str) -> Vec<u32> {
    // JSON has no vendored tree-sitter grammar in this repo; the JavaScript
    // grammar is used only as a query helper, not as a syntax authority, and
    // it incorrectly reports errors for many valid large JSON payloads.
    // Validity is already established by the streaming JSON decoder before
    // this function is reached (callers only invoke it on a successfully
    // decoded source), so trust the decoder for JSON.
    if language == "json" {
        return Vec::new();
    }
    let tree_sitter_ok = crate::language::parse_supported_language(language, source, None)
        .map(|summary| !summary.has_error)
        .unwrap_or(true);
    if tree_sitter_ok {
        Vec::new()
    } else {
        generic_parse_failed_diagnostics(source)
    }
}

fn diagnostics_for_source(language: &str, source: &str) -> Vec<u32> {
    if language == "json" {
        return Vec::new();
    }
    let tree_sitter_ok = crate::language::parse_supported_language(language, source, None)
        .map(|summary| !summary.has_error)
        .unwrap_or(true);
    if tree_sitter_ok {
        Vec::new()
    } else {
        generic_parse_failed_diagnostics(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::codec_service::CodecService;

    use crate::document::protocol::{
        GraphPathSeg, GraphValueEditPlanMode, QueryKind, QueryTargetKind,
    };
    #[test]
    fn snapshot_query_resolves_path_from_analysis_bundle_without_store_tree_entry() {
        let source = "user:\n  name: Ada\n  age: 37\n";
        let decoded = CodecService::new()
            .decode("yaml", source)
            .expect("yaml fixture should decode");
        assert!(decoded.store.get_tree_entry("doc").is_none());

        let analysis = build_decoded_analysis("doc", "yaml", source, &decoded);
        let mut snapshot = DocumentSnapshot::with_analysis("doc", analysis);
        snapshot.snapshot_id = SnapshotId(7);

        let offset = source.find("Ada").expect("fixture should contain value") as u32;
        let result = snapshot.query(&SnapshotQuery {
            snapshot_id: SnapshotId(7),
            kind: QueryKind::ResolvePath,
            path_pattern: None,
            span: Some((offset, offset)),
            target: Some(QueryTargetKind::Value),
        });

        assert_eq!(result.anchors.len(), 1);
        assert_eq!(result.anchors[0].path, "$.user.name");
    }

    #[test]
    fn snapshot_query_resolves_span_from_analysis_bundle_without_store_tree_entry() {
        let source = "user:\n  name: Ada\n  age: 37\n";
        let decoded = CodecService::new()
            .decode("yaml", source)
            .expect("yaml fixture should decode");
        let analysis = build_decoded_analysis("doc", "yaml", source, &decoded);
        let mut snapshot = DocumentSnapshot::with_analysis("doc", analysis);
        snapshot.snapshot_id = SnapshotId(9);

        let result = snapshot.query(&SnapshotQuery {
            snapshot_id: SnapshotId(9),
            kind: QueryKind::FindAnchors,
            path_pattern: Some("$.user.name".to_owned()),
            span: None,
            target: Some(QueryTargetKind::Value),
        });

        assert_eq!(result.anchors.len(), 1);
        assert!(result.anchors[0].span_start < result.anchors[0].span_end);
    }

    #[test]
    fn snapshot_query_builds_search_index_from_snapshot_document() {
        let source = r#"{"user":{"name":"Ada","roles":["admin"]}}"#;
        let decoded = CodecService::new()
            .decode("json", source)
            .expect("json fixture should decode");
        let analysis = build_decoded_analysis("doc-json", "json", source, &decoded);
        let mut snapshot = DocumentSnapshot::with_analysis("doc-json", analysis);
        snapshot.snapshot_id = SnapshotId(10);

        let result = snapshot.query(&SnapshotQuery {
            snapshot_id: SnapshotId(10),
            kind: QueryKind::SearchIndex,
            path_pattern: None,
            span: None,
            target: None,
        });

        assert!(result.search_items.iter().any(|item| {
            item.path == "$.user.name"
                && item.label == "name"
                && item.value_text == "Ada"
                && item.target == QueryTargetKind::Key
        }));
        assert!(result.search_items.iter().any(|item| {
            item.path == "$.user.roles[0]"
                && item.label == "admin"
                && item.target == QueryTargetKind::Value
        }));
    }

    #[test]
    fn graph_value_edit_plans_json_scalar_from_snapshot_span() {
        let source = r#"{"name":"old","n":1}"#;
        let decoded = CodecService::new()
            .decode("json", source)
            .expect("json fixture should decode");
        let analysis = build_decoded_analysis("doc-json", "json", source, &decoded);
        let mut snapshot = DocumentSnapshot::with_analysis("doc-json", analysis);
        snapshot.snapshot_id = SnapshotId(11);

        let plan = snapshot.plan_graph_value_edit(&GraphValueEditRequest {
            document_key: "doc-json".to_owned(),
            snapshot_id: SnapshotId(11),
            language: "json".to_owned(),
            path: vec![GraphPathSeg {
                tag: 0,
                key: "name".to_owned(),
                index: 0,
            }],
            prefer_key: false,
            raw_replacement: None,
            value: serde_json::json!({
                "kind": 2,
                "semType": 2,
                "tag": "",
                "value": "new",
                "children": [],
            }),
        });

        assert_eq!(plan.mode, GraphValueEditPlanMode::Edits);
        assert_eq!(plan.edits.len(), 1);
        let edit = &plan.edits[0];
        let start = source.find("\"old\"").expect("fixture contains old value") as u32;
        assert_eq!(edit.start_byte, start);
        assert_eq!(edit.old_end_byte, start + "\"old\"".len() as u32);
        assert_eq!(edit.replacement, "\"new\"");
    }

    struct PlannerCase {
        language: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        expected_old: &'static str,
        expected_replacement: &'static str,
    }

    #[test]
    fn graph_value_edit_plans_scalar_edits_for_non_streaming_languages() {
        let cases = vec![
            PlannerCase {
                language: "yaml",
                source: "name: old\n",
                path: vec![GraphPathSeg {
                    tag: 0,
                    key: "name".to_owned(),
                    index: 0,
                }],
                expected_old: "old",
                expected_replacement: "'new'",
            },
            PlannerCase {
                language: "toml",
                source: "name = \"old\"\n",
                path: vec![GraphPathSeg {
                    tag: 0,
                    key: "name".to_owned(),
                    index: 0,
                }],
                expected_old: "\"old\"",
                expected_replacement: "\"new\"",
            },
            PlannerCase {
                language: "csv",
                source: "name,age\nold,1\n",
                path: vec![
                    GraphPathSeg {
                        tag: 1,
                        key: String::new(),
                        index: 0,
                    },
                    GraphPathSeg {
                        tag: 0,
                        key: "name".to_owned(),
                        index: 0,
                    },
                ],
                expected_old: "old",
                expected_replacement: "new",
            },
            PlannerCase {
                language: "python",
                source: "{\"name\": \"old\"}",
                path: vec![GraphPathSeg {
                    tag: 0,
                    key: "name".to_owned(),
                    index: 0,
                }],
                expected_old: "\"old\"",
                expected_replacement: "'new'",
            },
            PlannerCase {
                language: "javascript",
                source: "({name: \"old\"})",
                path: vec![GraphPathSeg {
                    tag: 0,
                    key: "name".to_owned(),
                    index: 0,
                }],
                expected_old: "\"old\"",
                expected_replacement: "\"new\"",
            },
        ];

        for case in cases {
            let decoded = CodecService::new()
                .decode(case.language, case.source)
                .unwrap_or_else(|_| panic!("{} fixture should decode", case.language));
            let analysis =
                build_decoded_analysis(case.language, case.language, case.source, &decoded);
            let mut snapshot = DocumentSnapshot::with_analysis(case.language, analysis);
            snapshot.snapshot_id = SnapshotId(21);

            let plan = snapshot.plan_graph_value_edit(&GraphValueEditRequest {
                document_key: case.language.to_owned(),
                snapshot_id: SnapshotId(21),
                language: case.language.to_owned(),
                path: case.path,
                prefer_key: false,
                raw_replacement: None,
                value: serde_json::json!("new"),
            });

            assert_eq!(
                plan.mode,
                GraphValueEditPlanMode::Edits,
                "{}",
                case.language
            );
            assert_eq!(plan.edits.len(), 1, "{}", case.language);
            let edit = &plan.edits[0];
            assert_eq!(
                &case.source[edit.start_byte as usize..edit.old_end_byte as usize],
                case.expected_old,
                "{} should target the scalar span",
                case.language,
            );
            assert_eq!(
                edit.replacement, case.expected_replacement,
                "{}",
                case.language
            );
        }
    }

    #[test]
    fn build_decoded_analysis_from_owned_artifacts_preserves_document_and_line_index() {
        let source = "root:\n  k: value\n";
        let decoded = CodecService::new()
            .decode("yaml", source)
            .expect("yaml fixture should decode");
        let line_index = LineIndex::build(source);
        let artifacts = DecodedAnalysisArtifacts {
            ts_tree: None,
            token_spans: Vec::new(),
            diagnostics: Vec::new(),
            semantic_tokens: Vec::new(),
            value_json: r#"{"root":{"k":"value"}}"#.to_owned(),
            line_index: line_index.clone(),
            span_authority: DecodedSpanAuthority::Unknown,
        };

        let analysis = build_decoded_analysis_from_owned_artifacts(
            "doc-yaml", "yaml", source, decoded, artifacts,
        );

        assert_eq!(analysis.key, "doc-yaml");
        assert_eq!(analysis.language, "yaml");
        assert_eq!(analysis.source_byte_length, source.len() as u32);
        assert_eq!(analysis.line_index, line_index);
        assert_eq!(analysis.value_json, r#"{"root":{"k":"value"}}"#);
        assert!(analysis.document.is_some());
    }

    #[test]
    fn span_authority_complete_skips_full_annotate() {
        let source = "root:\n  k: value\n";
        let decoded = CodecService::new()
            .decode("yaml", source)
            .expect("yaml fixture should decode");
        let node_count_before = decoded.store.len();
        let line_index = LineIndex::build(source);

        let artifacts = DecodedAnalysisArtifacts {
            ts_tree: None,
            token_spans: Vec::new(),
            diagnostics: Vec::new(),
            semantic_tokens: Vec::new(),
            value_json: r#"{"root":{"k":"value"}}"#.to_owned(),
            line_index: line_index.clone(),
            span_authority: DecodedSpanAuthority::Complete,
        };

        let analysis = build_decoded_analysis_from_owned_artifacts(
            "test-complete",
            "yaml",
            source,
            decoded,
            artifacts,
        );

        assert_eq!(analysis.key, "test-complete");
        assert_eq!(analysis.line_index, line_index);
        assert_eq!(
            analysis.document.as_ref().map(|d| d.store.len()),
            Some(node_count_before),
            "Complete authority should not add or remove nodes",
        );
    }
    #[test]
    fn graph_value_edit_uses_snapshot_tree_path_index_for_wide_mapping() {
        let mut entries = Vec::new();
        for index in 0..128 {
            entries.push(format!("\"k{index}\":{index}"));
        }
        entries.push("\"target\":\"old\"".to_owned());
        let source = format!("{{{}}}", entries.join(","));
        let decoded = CodecService::new()
            .decode("json", &source)
            .expect("json fixture should decode");
        let analysis = build_decoded_analysis("wide-doc", "json", &source, &decoded);
        let mut snapshot = DocumentSnapshot::with_analysis("wide-doc", analysis);
        snapshot.snapshot_id = SnapshotId(41);
        snapshot.incremental = Some(IncrementalState::resumable().with_tree_path_index(
            crate::tree::TreePathIndex::build(&decoded.store, decoded.root),
        ));
        let plan = snapshot.plan_graph_value_edit(&GraphValueEditRequest {
            document_key: "wide-doc".to_owned(),
            snapshot_id: SnapshotId(41),
            language: "json".to_owned(),
            path: vec![GraphPathSeg {
                tag: 0,
                key: "target".to_owned(),
                index: 0,
            }],
            prefer_key: false,
            raw_replacement: None,
            value: serde_json::json!("new"),
        });
        assert_eq!(plan.mode, GraphValueEditPlanMode::Edits);
        assert_eq!(plan.edits.len(), 1);
        let edit = &plan.edits[0];
        assert_eq!(
            &source[edit.start_byte as usize..edit.old_end_byte as usize],
            "\"old\""
        );
        assert_eq!(edit.replacement, "\"new\"");
    }
    #[test]
    fn graph_value_edit_plans_csv_header_key_edit() {
        let source = "name,age\nAda,37\n";
        let decoded = CodecService::new()
            .decode("csv", source)
            .expect("csv fixture should decode");
        let analysis = build_decoded_analysis("csv-doc", "csv", source, &decoded);
        let mut snapshot = DocumentSnapshot::with_analysis("csv-doc", analysis);
        snapshot.snapshot_id = SnapshotId(51);
        snapshot.incremental = Some(IncrementalState::resumable().with_tree_path_index(
            crate::tree::TreePathIndex::build(&decoded.store, decoded.root),
        ));
        let plan = snapshot.plan_graph_value_edit(&GraphValueEditRequest {
            document_key: "csv-doc".to_owned(),
            snapshot_id: SnapshotId(51),
            language: "csv".to_owned(),
            path: vec![
                GraphPathSeg {
                    tag: 1,
                    key: String::new(),
                    index: 0,
                },
                GraphPathSeg {
                    tag: 0,
                    key: "name".to_owned(),
                    index: 0,
                },
            ],
            prefer_key: true,
            raw_replacement: None,
            value: serde_json::json!("full name"),
        });
        assert_eq!(plan.mode, GraphValueEditPlanMode::Edits);
        assert_eq!(plan.edits.len(), 1);
        let edit = &plan.edits[0];
        assert_eq!(
            &source[edit.start_byte as usize..edit.old_end_byte as usize],
            "name"
        );
        assert_eq!(edit.replacement, "full name");
    }

    #[test]
    fn owned_artifacts_resolve_only_declared_unresolved_spans() {
        let source = r#"{"a":1,"b":2}"#;
        let mut decoded = CodecService::new()
            .decode("json", source)
            .expect("json fixture should decode");
        let path = [crate::tree::path_seg_key("b")];
        let b_value = crate::tree::find_node_by_path_with_index(
            decoded.root,
            &path,
            false,
            &decoded.store,
            None,
        )
        .expect("fixture should contain $.b");

        {
            let node = decoded
                .store
                .get_mut(b_value)
                .expect("b value node should exist");
            node.start_byte = 0;
            node.end_byte = 0;
            node.line = 0;
            node.column = 0;
        }

        let line_index = LineIndex::build(source);
        let analysis = build_decoded_analysis_from_owned_artifacts(
            "doc-json",
            "json",
            source,
            decoded,
            DecodedAnalysisArtifacts {
                ts_tree: crate::language::parse_tree("json", source.as_bytes()),
                token_spans: Vec::new(),
                diagnostics: Vec::new(),
                semantic_tokens: crate::language::encode_semantic_tokens("json", source),
                value_json: r#"{"a":1,"b":2}"#.to_owned(),
                line_index,
                span_authority: DecodedSpanAuthority::UnresolvedNodes(vec![b_value]),
            },
        );

        let document = analysis.document.expect("analysis should keep document");
        let node = document
            .store
            .get(b_value)
            .expect("b value node should exist");
        assert_eq!(
            node.start_byte,
            source.find('2').expect("fixture contains 2") as u32
        );
        assert_eq!(node.end_byte, node.start_byte + 1);
        assert_eq!(node.line, 1);
        assert!(node.column > 0);
    }
}
