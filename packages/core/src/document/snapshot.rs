use super::protocol::{
    DocumentAnalysisPayload, DocumentAnchor, DocumentDiagnostic, DocumentNodePreview,
    DocumentPathValue, DocumentTreeNode, GraphValueEditFallbackReason, GraphValueEditPlan,
    GraphValueEditRequest, QueryKind, QueryResult, QueryTargetKind, SemanticTokensPayload,
    SnapshotId, SnapshotQuery,
};
use crate::core::document_analysis::{
    DocumentAnalysisDemand, analyze_decoded_document_with_prepared_tree_and_demand,
    analyze_document_internal_with_demand, encode_document_value_json,
};
use crate::core::{
    LineIndex, NodeId, SemType, TokenSpan, encode_semantic_tokens,
    tree_node::{ParsedKey, TreeNodeKind},
};
use crate::formats::DecodedDocument;
use crate::wasm_types::{AnalysisSharedArtifacts, PathSegTag, PathSpan, WasmProtocol};
use std::collections::BTreeSet;

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
    pub graph_model_snapshot: Option<crate::core::GraphModelSnapshot>,
    pub graph_model_index: Option<crate::core::GraphModelIndex>,
    pub structural_safe: bool,
    pub fallback_reason: Option<String>,
    pub structural_span_index: Option<crate::core::StructuralSpanIndex>,
    pub tree_path_index: Option<crate::core::TreePathIndex>,
    pub(crate) graph_topology: Option<crate::core::graph_topology::GraphTopology>,
    pub(crate) layout_state: Option<crate::core::layout_engine::LayoutState>,
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
        graph_model_snapshot: crate::core::GraphModelSnapshot,
        graph_model_index: crate::core::GraphModelIndex,
    ) -> Self {
        self.graph_model_snapshot = Some(graph_model_snapshot);
        self.graph_model_index = Some(graph_model_index);
        self.structural_safe = true;
        self
    }

    pub fn with_tree_path_index(mut self, tree_path_index: crate::core::TreePathIndex) -> Self {
        self.tree_path_index = Some(tree_path_index);
        self
    }

    pub(crate) fn with_graph_runtime_state(
        mut self,
        topology: crate::core::graph_topology::GraphTopology,
        layout_state: crate::core::layout_engine::LayoutState,
    ) -> Self {
        self.graph_topology = Some(topology);
        self.layout_state = Some(layout_state);
        self
    }

    pub(crate) fn graph_topology(&self) -> Option<&crate::core::graph_topology::GraphTopology> {
        self.graph_topology.as_ref()
    }

    pub(crate) fn layout_state(&self) -> Option<&crate::core::layout_engine::LayoutState> {
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
    store: &crate::core::TreeStore,
    id: crate::core::NodeId,
) -> Option<DocumentTreeNode> {
    let node = store.get(id)?;
    Some(DocumentTreeNode {
        kind: tree_kind_code(node.kind),
        sem_type: sem_type_code(node.resolved_sem_type()),
        tag: node.tag.clone(),
        value: node.value.clone(),
        children: node
            .content
            .iter()
            .filter_map(|child| document_tree_node_from_store(store, *child))
            .collect(),
    })
}

fn tree_kind_code(kind: crate::core::TreeNodeKind) -> i32 {
    match kind {
        crate::core::TreeNodeKind::Sequence => 0,
        crate::core::TreeNodeKind::Mapping => 1,
        crate::core::TreeNodeKind::Scalar => 2,
        crate::core::TreeNodeKind::Alias => 3,
        crate::core::TreeNodeKind::Unknown => 4,
    }
}
fn sem_type_code(sem_type: Option<SemType>) -> i32 {
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
    /// Returns a result bound to this snapshot's identity.
    pub fn query(&self, query: &SnapshotQuery) -> QueryResult {
        let Some(analysis) = &self.analysis else {
            return QueryResult::default();
        };
        let Some(document) = &analysis.document else {
            return QueryResult::default();
        };

        match query.kind {
            QueryKind::ResolvePath | QueryKind::ResolveHover => {
                let Some((start, end)) = query.span else {
                    return QueryResult::default();
                };
                resolve_anchor_for_span(self.snapshot_id, analysis, document, start, end)
                    .map(|anchor| QueryResult {
                        anchors: vec![anchor],
                        ..Default::default()
                    })
                    .unwrap_or_default()
            }
            QueryKind::FindAnchors => query
                .path_pattern
                .as_deref()
                .and_then(|path| {
                    resolve_anchor_for_path(
                        self.snapshot_id,
                        analysis,
                        document,
                        path,
                        query.target.unwrap_or_default(),
                    )
                })
                .map(|anchor| QueryResult {
                    anchors: vec![anchor],
                    ..Default::default()
                })
                .unwrap_or_default(),
            QueryKind::RootValueKind => QueryResult {
                root_value_kind: document.store.get(document.root).map(node_value_kind),
                ..Default::default()
            },
            QueryKind::NodePreview => query
                .path_pattern
                .as_deref()
                .and_then(|path| node_preview_for_path(document, path))
                .map(|node_preview| QueryResult {
                    node_preview: Some(node_preview),
                    ..Default::default()
                })
                .unwrap_or_default(),
            QueryKind::PathValue => query
                .path_pattern
                .as_deref()
                .and_then(|path| path_value_for_path(analysis, document, path))
                .map(|path_value| QueryResult {
                    path_value: Some(path_value),
                    ..Default::default()
                })
                .unwrap_or_default(),
            QueryKind::FieldLabels => QueryResult {
                field_labels: collect_field_labels(document),
                ..Default::default()
            },
        }
    }
}

impl DocumentSnapshot {
    pub fn plan_graph_value_edit(&self, request: &GraphValueEditRequest) -> GraphValueEditPlan {
        if self.snapshot_id != request.snapshot_id || self.document_key != request.document_key {
            return super::graph_value_edit::graph_value_edit_fallback(
                GraphValueEditFallbackReason::SnapshotNotReady,
            );
        }
        let Some(analysis) = self.analysis.as_ref() else {
            return super::graph_value_edit::graph_value_edit_fallback(
                GraphValueEditFallbackReason::MissingAnalysis,
            );
        };
        let Some(document) = analysis.document.as_ref() else {
            return super::graph_value_edit::graph_value_edit_fallback(
                GraphValueEditFallbackReason::MissingDocument,
            );
        };
        let path_index = self
            .incremental
            .as_ref()
            .and_then(|state| state.tree_path_index.as_ref());
        super::graph_value_edit::plan_graph_value_edit(analysis, document, request, path_index)
    }
}

fn resolve_anchor_for_span(
    snapshot_id: SnapshotId,
    analysis: &AnalysisBundle,
    document: &DecodedDocument,
    start: u32,
    end: u32,
) -> Option<DocumentAnchor> {
    let line_column = analysis.line_index.offset_to_line_column(start as usize);
    let path = crate::core::compute_tree_path_segments_for_document(
        &document.store,
        document.root,
        analysis.ts_tree.as_ref(),
        &analysis.diagnostics,
        &analysis.language,
        &analysis.source,
        &analysis.line_index,
        line_column.line,
        line_column.column,
    );
    if path.is_empty() {
        return None;
    }
    let span = crate::core::compute_path_span_for_document(
        &document.store,
        document.root,
        analysis.ts_tree.as_ref(),
        &analysis.diagnostics,
        &analysis.language,
        &analysis.source,
        &path,
        false,
    );
    if span.start_byte < 0 || span.end_byte < span.start_byte {
        return None;
    }
    Some(DocumentAnchor {
        snapshot_id,
        path: crate::core::format_tree_path(&path),
        span_start: span.start_byte as u32,
        span_end: (span.end_byte as u32).max(end),
    })
}

fn resolve_anchor_for_path(
    snapshot_id: SnapshotId,
    analysis: &AnalysisBundle,
    document: &DecodedDocument,
    path_pattern: &str,
    target: QueryTargetKind,
) -> Option<DocumentAnchor> {
    let path = parse_snapshot_path(path_pattern)?;
    let span = crate::core::compute_path_span_for_document(
        &document.store,
        document.root,
        analysis.ts_tree.as_ref(),
        &analysis.diagnostics,
        &analysis.language,
        &analysis.source,
        &path,
        matches!(target, QueryTargetKind::Key),
    );
    if span.start_byte < 0 || span.end_byte < span.start_byte {
        return None;
    }
    Some(DocumentAnchor {
        snapshot_id,
        path: crate::core::format_tree_path(&path),
        span_start: span.start_byte as u32,
        span_end: span.end_byte as u32,
    })
}

fn parse_snapshot_path(path: &str) -> Option<Vec<crate::wasm_types::PathSeg<'static>>> {
    if path.is_empty() || path == "$" {
        return Some(Vec::new());
    }
    let bytes = path.as_bytes();
    let mut index = 0usize;
    if bytes.first() == Some(&b'$') {
        index = 1;
    }
    let mut segments = Vec::new();
    while index < bytes.len() {
        match bytes[index] {
            b'.' => {
                index += 1;
                let start = index;
                while index < bytes.len()
                    && matches!(bytes[index], b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'$')
                {
                    index += 1;
                }
                if start == index {
                    return None;
                }
                segments.push(crate::core::path_seg_key(Box::leak(
                    path[start..index].to_owned().into_boxed_str(),
                )));
            }
            b'[' => {
                let start = index + 1;
                let end = path[start..].find(']')? + start;
                let inner = path[start..end].trim();
                if inner.starts_with('"') {
                    let key = crate::core::unescape_json_string(inner)?;
                    segments.push(crate::core::path_seg_key(Box::leak(key.into_boxed_str())));
                } else {
                    let value = inner.parse::<i32>().ok()?;
                    segments.push(crate::core::path_seg_index(value));
                }
                index = end + 1;
            }
            _ => return None,
        }
    }
    Some(segments)
}

fn parsed_keys_from_snapshot_path(path: &str) -> Option<Vec<ParsedKey>> {
    parse_snapshot_path(path).map(|segments| {
        segments
            .iter()
            .map(|segment| match segment.tag {
                PathSegTag::Key => ParsedKey::Str(segment.key.to_owned()),
                PathSegTag::Index => ParsedKey::Int(i64::from(segment.index)),
            })
            .collect()
    })
}

fn node_id_for_path(
    document: &DecodedDocument,
    path_pattern: &str,
    prefer_key: bool,
) -> Option<NodeId> {
    let path = parsed_keys_from_snapshot_path(path_pattern)?;
    if path.is_empty() {
        return Some(document.root);
    }
    document
        .store
        .find_descendant_by_path(document.root, &path, prefer_key)
        .ok()
        .flatten()
}

fn node_value_kind(node: &crate::core::tree_node::TreeNode) -> String {
    match node.kind {
        TreeNodeKind::Mapping => "object",
        TreeNodeKind::Sequence => "array",
        TreeNodeKind::Scalar | TreeNodeKind::Alias | TreeNodeKind::Unknown => {
            match node.resolved_sem_type() {
                Some(SemType::Str) => "string",
                Some(SemType::Int) => "int",
                Some(SemType::Float) => "float",
                Some(SemType::Boolean) => "boolean",
                Some(SemType::Nil) => "null",
                Some(SemType::Map) => "object",
                Some(SemType::Seq) => "array",
                None => "unknown",
            }
        }
    }
    .to_owned()
}

fn node_value_type(node: &crate::core::tree_node::TreeNode) -> String {
    match node_value_kind(node).as_str() {
        "int" | "float" => "number".to_owned(),
        other => other.to_owned(),
    }
}

fn node_preview_from_node(node: &crate::core::tree_node::TreeNode) -> DocumentNodePreview {
    let value_type = node_value_type(node);
    DocumentNodePreview {
        kind: tree_kind_code(node.kind),
        sem_type: sem_type_code(node.resolved_sem_type()),
        tag: node.tag.clone(),
        value: node.value.clone(),
        value_type,
        is_scalar: matches!(node.kind, TreeNodeKind::Scalar),
    }
}

fn node_preview_for_path(
    document: &DecodedDocument,
    path_pattern: &str,
) -> Option<DocumentNodePreview> {
    let node_id = node_id_for_path(document, path_pattern, false)?;
    document.store.get(node_id).map(node_preview_from_node)
}

fn source_slice_by_bytes(source: &str, start: u32, end: u32) -> String {
    let start = usize::try_from(start).ok();
    let end = usize::try_from(end).ok();
    match (start, end) {
        (Some(start), Some(end)) if start <= end && end <= source.len() => {
            source.get(start..end).unwrap_or_default().to_owned()
        }
        _ => String::new(),
    }
}

fn path_value_for_path(
    analysis: &AnalysisBundle,
    document: &DecodedDocument,
    path_pattern: &str,
) -> Option<DocumentPathValue> {
    let node_id = node_id_for_path(document, path_pattern, false)?;
    let node = document.store.get(node_id)?;
    let source_text = source_slice_by_bytes(&analysis.source, node.start_byte, node.end_byte);
    let display_text = if source_text.is_empty() {
        node.value.clone()
    } else {
        source_text.clone()
    };
    Some(DocumentPathValue {
        value_type: node_value_type(node),
        value: node.value.clone(),
        source_text,
        display_text,
    })
}

fn collect_field_labels(document: &DecodedDocument) -> Vec<String> {
    fn visit(
        store: &crate::core::tree_store::TreeStore,
        node_id: NodeId,
        labels: &mut BTreeSet<String>,
    ) {
        let Some(node) = store.get(node_id) else {
            return;
        };
        if node.kind == TreeNodeKind::Mapping {
            let mut index = 0usize;
            while index + 1 < node.content.len() {
                if let Some(key_node) = store.get(node.content[index]) {
                    if key_node.is_map_key && !key_node.value.is_empty() {
                        labels.insert(key_node.value.clone());
                    }
                }
                visit(store, node.content[index + 1], labels);
                index += 2;
            }
            return;
        }
        for child in &node.content {
            visit(store, *child, labels);
        }
    }

    let mut labels = BTreeSet::new();
    visit(&document.store, document.root, &mut labels);
    labels.into_iter().collect()
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
        let id = NodeId(index);
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

    let mut resolver = crate::core::PathSpanResolver::new(
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
    let mut resolver = crate::core::PathSpanResolver::new(
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
        crate::core::parse_tree(language, source.as_bytes())
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
    let tree_sitter_ok = crate::core::parse_supported_language(language, source, None)
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
    let tree_sitter_ok = crate::core::parse_supported_language(language, source, None)
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
    use crate::core::codec_service::CodecService;

    use crate::document::protocol::{GraphPathSeg, GraphValueEditPlanMode};
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
            crate::core::TreePathIndex::build(&decoded.store, decoded.root),
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
            crate::core::TreePathIndex::build(&decoded.store, decoded.root),
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
        let path = [crate::core::path_seg_key("b")];
        let b_value = crate::core::find_node_by_path_with_index(
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
                ts_tree: crate::core::parse_tree("json", source.as_bytes()),
                token_spans: Vec::new(),
                diagnostics: Vec::new(),
                semantic_tokens: crate::core::encode_semantic_tokens("json", source),
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
