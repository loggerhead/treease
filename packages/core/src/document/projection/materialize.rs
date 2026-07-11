use crate::{
    core::{
        CoreError, SemType,
        incremental_edit::{
            DocumentTextEdit, apply_delta, apply_edit_to_source, apply_edits_to_tree_and_source,
            collect_subtree_node_ids, edit_byte_range, find_affected_node_id_for_edit,
            find_reparse_boundary_id,
        },
        parse_scalar_edit_replacement,
    },
    formats::DecodedDocument,
    stream::{self, DecodeOptions},
};

use crate::analysis::document_analysis::encode_document_value_json;
use crate::analysis::line_index::LineIndex;
use crate::analysis::span_index::StructuralSpanIndex;
use crate::graph::graph_projection_service;
use crate::tree::incremental_edit::{
    adjust_tree_store_offsets_from_collecting, find_exact_node_for_edit,
    recompute_tree_store_locations_for,
};
use crate::tree::tree_node::{NodeId, TreeNode, TreeNodeKind};
use crate::tree::tree_store::{TokenSpan, TreeStore};

use super::super::input::ByteStream;
use super::super::protocol::{DocumentInputPlan, JobTerminal, OutputPlan};
use super::super::runtime::store_snapshot_for_document;
use super::super::snapshot::{
    AnalysisBundle, DecodedAnalysisArtifacts, DecodedSpanAuthority, DocumentSnapshot,
    GraphProjection, IncrementalState, annotate_decoded_document_spans, build_analysis_from_shared,
    build_analysis_shared, build_decoded_analysis, build_decoded_analysis_from_owned_artifacts,
    build_decoded_analysis_with_artifacts, build_decoded_analysis_with_prepared_tree,
    build_lightweight_analysis_shared, diagnostics_for_decoded_source,
    generic_parse_failed_diagnostics,
};

/// Result of a single materialize operation.
#[derive(Debug, Clone, Default)]
pub struct MaterializeResult {
    pub analysis: AnalysisBundle,
    pub graph: Option<GraphProjection>,
    pub incremental: Option<IncrementalState>,
    pub terminal: Option<JobTerminal>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MaterializeBaseContext<'a> {
    pub document: Option<&'a DecodedDocument>,
    pub incremental: Option<&'a IncrementalState>,
    pub line_index: Option<&'a LineIndex>,
    pub semantic_tokens: Option<&'a [u32]>,
}

/// Unified materialize entry point for all input plans.
///
/// Decodes the source, builds the analysis bundle, optionally builds the
/// graph projection, and returns the result. On parse failure, returns a
/// diagnostics-only bundle with graph cleared.
///
/// This is the canonical path for non-streaming materialization. Streaming
/// This is the canonical path for non-streaming materialization. Streaming (JSON)
pub fn materialize(
    input_plan: &DocumentInputPlan,
    document_key: &str,
    language: &str,
    source: &str,
    nest: bool,
    output_plan: &OutputPlan,
    edits: &[DocumentTextEdit],
    prepared_ts_tree: Option<tree_sitter::Tree>,
) -> MaterializeResult {
    materialize_with_base_context(
        input_plan,
        document_key,
        language,
        source,
        nest,
        output_plan,
        edits,
        prepared_ts_tree,
        MaterializeBaseContext::default(),
    )
}

pub fn materialize_with_base(
    input_plan: &DocumentInputPlan,
    document_key: &str,
    language: &str,
    source: &str,
    nest: bool,
    output_plan: &OutputPlan,
    edits: &[DocumentTextEdit],
    prepared_ts_tree: Option<tree_sitter::Tree>,
    base_document: Option<&DecodedDocument>,
    base_incremental: Option<&IncrementalState>,
) -> MaterializeResult {
    materialize_with_base_context(
        input_plan,
        document_key,
        language,
        source,
        nest,
        output_plan,
        edits,
        prepared_ts_tree,
        MaterializeBaseContext {
            document: base_document,
            incremental: base_incremental,
            line_index: None,
            semantic_tokens: None,
        },
    )
}

pub fn materialize_with_base_context(
    input_plan: &DocumentInputPlan,
    document_key: &str,
    language: &str,
    source: &str,
    nest: bool,
    output_plan: &OutputPlan,
    edits: &[DocumentTextEdit],
    prepared_ts_tree: Option<tree_sitter::Tree>,
    base: MaterializeBaseContext<'_>,
) -> MaterializeResult {
    let mut source_text = source.to_owned();
    let mut incremental_tree = prepared_ts_tree;
    let has_text_edits =
        matches!(input_plan, DocumentInputPlan::BaseTextWithEdits) && !edits.is_empty();

    if has_text_edits {
        if let Some(mut tree) = incremental_tree.take() {
            match apply_edits_to_tree_and_source(source_text, edits, &mut tree) {
                Some(updated_source) => {
                    source_text = updated_source;
                    incremental_tree = Some(tree);
                }
                None => {
                    source_text = apply_edits_to_source(source.to_owned(), edits);
                }
            }
        } else {
            source_text = apply_edits_to_source(source_text, edits);
        }
    }

    if has_text_edits {
        if let Some(result) = try_structural_materialize(
            document_key,
            language,
            &source_text,
            output_plan,
            edits,
            incremental_tree.clone(),
            base,
            nest,
        ) {
            return result;
        }
    }

    if is_blank_source(&source_text) {
        return materialize_blank_document(document_key, language, &source_text, output_plan);
    }

    // Try incremental path: when we have a prepared tree-sitter tree,
    // apply tree edits and re-parse incrementally.
    if let Some(tree) = incremental_tree {
        if let Ok(decoded_doc) = decode_document_with_nesting(language, &source_text, nest) {
            let analysis = build_decoded_analysis_with_prepared_tree(
                document_key,
                language,
                &source_text,
                &decoded_doc,
                Some(tree),
            );
            let (graph, incremental) =
                build_graph_output(language, &decoded_doc, output_plan.graph);
            return MaterializeResult {
                analysis,
                graph,
                incremental,
                terminal: None,
            };
        }
    }
    // Fall through to full decode if incremental path did not produce a decoded document.

    let decoded = decode_document_with_nesting(language, &source_text, nest);

    match decoded {
        Ok(decoded) => {
            let analysis = build_decoded_analysis(document_key, language, &source_text, &decoded);

            let (graph, incremental) = build_graph_output(language, &decoded, output_plan.graph);

            MaterializeResult {
                analysis,
                graph,
                incremental,
                terminal: None,
            }
        }
        Err(error) => {
            // Parse-failed: build diagnostics-only analysis, clear graph.
            let (shared, mut diagnostics) = build_analysis_shared(language, &source_text, None);
            if diagnostics.is_empty()
                && matches!(error, CoreError::Parse(_) | CoreError::ParseMessage { .. })
            {
                diagnostics = generic_parse_failed_diagnostics(&source_text);
            }
            let analysis = AnalysisBundle {
                key: document_key.to_owned(),
                language: language.to_owned(),
                source: source_text.clone(),
                source_byte_length: shared.source_byte_length,
                document: None,
                ts_tree: None,
                token_spans: Vec::new(),
                diagnostics,
                semantic_tokens: Vec::new(),
                value_json: String::new(),
                line_index: shared.line_index,
            };
            MaterializeResult {
                analysis,
                graph: None,
                incremental: Some(IncrementalState::fallback("parse_failed")),
                terminal: None,
            }
        }
    }
}

pub(crate) fn is_blank_source(source: &str) -> bool {
    source.trim().is_empty()
}

fn materialize_blank_document(
    document_key: &str,
    language: &str,
    source: &str,
    output_plan: &OutputPlan,
) -> MaterializeResult {
    MaterializeResult {
        analysis: AnalysisBundle {
            key: document_key.to_owned(),
            language: language.to_owned(),
            source: source.to_owned(),
            source_byte_length: source.len() as u32,
            document: None,
            ts_tree: None,
            token_spans: Vec::new(),
            diagnostics: Vec::new(),
            semantic_tokens: Vec::new(),
            value_json: String::new(),
            line_index: LineIndex::build(source),
        },
        graph: output_plan.graph.then(|| GraphProjection {
            ready: true,
            clear: true,
            graph_data: None,
        }),
        incremental: None,
        terminal: None,
    }
}

/// Materialize for streaming decode — starts from an already-decoded document
/// instead of re-decoding from source.  Uses a lightweight analysis path that
/// skips the expensive `analyze_document_internal` call (which re-parses the
/// source from scratch) since we already have the decoded tree.
pub fn materialize_from_decoded(
    decoded: DecodedDocument,
    language: &str,
    source: &str,
    document_key: &str,
    output_plan: &OutputPlan,
    build_ts_tree: bool,
    skip_graph_output: bool,
) -> MaterializeResult {
    let shared = build_lightweight_analysis_shared(language, source, &decoded, build_ts_tree);
    let diagnostics = diagnostics_for_decoded_source(language, source);
    let (graph, incremental) = if skip_graph_output {
        (None, None)
    } else {
        build_graph_output(language, &decoded, output_plan.graph)
    };
    let document = if !diagnostics.is_empty() && shared.value_json.is_empty() {
        None
    } else {
        let mut doc = decoded;
        annotate_decoded_document_spans(
            language,
            source,
            &mut doc,
            shared.ts_tree.as_ref(),
            &diagnostics,
        );
        Some(doc)
    };
    let analysis = build_analysis_from_shared(
        document_key,
        language,
        source,
        document,
        shared,
        diagnostics,
    );
    MaterializeResult {
        analysis,
        graph,
        incremental,
        terminal: None,
    }
}

fn apply_edits_to_source(mut source_text: String, edits: &[DocumentTextEdit]) -> String {
    for edit in edits {
        if edit_byte_range(&source_text, edit).is_some() {
            source_text = apply_edit_to_source(&source_text, edit);
        }
    }
    source_text
}

#[derive(Debug, Clone)]
struct StructuralEditOutcome {
    changed_root: NodeId,
    old_changed_end: u32,
    affected_nodes: Vec<NodeId>,
    removed_node_ids: Vec<NodeId>,
    scalar_semantic_change: Option<ScalarSemanticChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScalarSemanticChange {
    old_sem_type: Option<SemType>,
    new_sem_type: SemType,
    old_byte_len: u32,
    new_byte_len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructuralSemanticTokenPlan {
    ReuseBase,
    Reencode,
}

impl StructuralEditOutcome {
    fn scalar(
        changed_root: NodeId,
        old_changed_end: u32,
        scalar_semantic_change: ScalarSemanticChange,
    ) -> Self {
        Self {
            changed_root,
            old_changed_end,
            affected_nodes: vec![changed_root],
            removed_node_ids: Vec::new(),
            scalar_semantic_change: Some(scalar_semantic_change),
        }
    }
}

fn try_structural_materialize(
    document_key: &str,
    language: &str,
    source_text: &str,
    output_plan: &OutputPlan,
    edits: &[DocumentTextEdit],
    incremental_tree: Option<tree_sitter::Tree>,
    base: MaterializeBaseContext<'_>,
    nest: bool,
) -> Option<MaterializeResult> {
    if edits.len() != 1 {
        return None;
    }
    if !base.incremental.is_some_and(|state| state.can_resume) {
        return None;
    }
    let mut decoded = base.document.cloned()?;
    let edit = &edits[0];
    let built_span_index = base
        .incremental
        .and_then(|state| state.structural_span_index.as_ref())
        .is_none()
        .then(|| StructuralSpanIndex::build(&decoded.store, decoded.root));
    let span_index = base
        .incremental
        .and_then(|state| state.structural_span_index.as_ref())
        .or(built_span_index.as_ref());
    let delta = crate::tree::edit_delta(edit);
    let outcome = if let Some(target) =
        find_exact_node_for_edit(&decoded.store, decoded.root, edit, span_index)
    {
        let scalar_semantic_change = update_scalar_node(language, &mut decoded, target, edit)?;
        StructuralEditOutcome::scalar(target, edit.old_end_byte, scalar_semantic_change)
    } else {
        apply_subtree_node_edit(language, &mut decoded, source_text, edit, nest)?
    };
    let mut affected_nodes = outcome.affected_nodes.clone();
    if delta != 0 {
        let update = adjust_tree_store_offsets_from_collecting(
            &mut decoded.store,
            decoded.root,
            outcome.old_changed_end,
            delta,
            Some(outcome.changed_root),
        );
        affected_nodes.extend(update.adjusted);
    }
    affected_nodes.sort_by_key(|id| id.0);
    affected_nodes.dedup();

    let line_index = base
        .line_index
        .and_then(|index| index.apply_single_edit(edit))
        .unwrap_or_else(|| LineIndex::build(source_text));
    let mut structural_span_index = output_plan.graph.then(StructuralSpanIndex::default);
    recompute_tree_store_locations_for(
        &mut decoded.store,
        &affected_nodes,
        &line_index,
        structural_span_index.as_mut(),
    );
    let semantic_tokens =
        semantic_tokens_for_structural_edit(language, source_text, &outcome, base.semantic_tokens);
    let value_json = encode_document_value_json(&decoded).unwrap_or_default();
    let analysis = build_decoded_analysis_from_owned_artifacts(
        document_key,
        language,
        source_text,
        decoded.clone(),
        DecodedAnalysisArtifacts {
            ts_tree: incremental_tree,
            token_spans: Vec::new(),
            diagnostics: Vec::new(),
            semantic_tokens,
            value_json,
            line_index: line_index.clone(),
            span_authority: DecodedSpanAuthority::Complete,
        },
    );
    let (graph, incremental) = if output_plan.graph {
        build_structural_graph_output(
            language,
            &decoded,
            edit,
            base.incremental,
            structural_span_index,
            &outcome,
        )
    } else {
        (None, None)
    };

    Some(MaterializeResult {
        analysis,
        graph,
        incremental,
        terminal: None,
    })
}
fn structural_semantic_token_plan(outcome: &StructuralEditOutcome) -> StructuralSemanticTokenPlan {
    let Some(change) = outcome.scalar_semantic_change else {
        return StructuralSemanticTokenPlan::Reencode;
    };
    if change.old_sem_type == Some(change.new_sem_type)
        && change.old_byte_len == change.new_byte_len
    {
        StructuralSemanticTokenPlan::ReuseBase
    } else {
        StructuralSemanticTokenPlan::Reencode
    }
}

fn semantic_tokens_for_structural_edit(
    language: &str,
    source_text: &str,
    outcome: &StructuralEditOutcome,
    base_semantic_tokens: Option<&[u32]>,
) -> Vec<u32> {
    match (
        structural_semantic_token_plan(outcome),
        base_semantic_tokens,
    ) {
        (StructuralSemanticTokenPlan::ReuseBase, Some(tokens)) => tokens.to_vec(),
        _ => crate::language::encode_semantic_tokens(language, source_text),
    }
}

fn build_graph_output(
    language: &str,
    decoded: &DecodedDocument,
    enabled: bool,
) -> (Option<GraphProjection>, Option<IncrementalState>) {
    if !enabled {
        return (None, None);
    }

    let Some(build) = graph_projection_service::build_graph_model_for_tree_with_runtime_state(
        &decoded.store,
        decoded.root,
        language,
    )
    .ok() else {
        return (None, None);
    };
    let graph_data = graph_projection_service::model_to_graph_delta(&build.model);
    let index = crate::graph::graph_model_index::GraphModelIndex::build(&build.model);
    let incremental = IncrementalState::resumable()
        .with_graph_state(
            crate::graph::graph_builder::GraphModelSnapshot::owned(build.model),
            index,
        )
        .with_graph_runtime_state(build.topology, build.layout_state)
        .with_tree_path_index(crate::tree::TreePathIndex::build(
            &decoded.store,
            decoded.root,
        ));
    (
        Some(GraphProjection {
            ready: true,
            clear: true,
            graph_data: Some(graph_data),
        }),
        Some(incremental),
    )
}
fn build_incremental_graph_output(
    language: &str,
    decoded: &DecodedDocument,
    edit: &DocumentTextEdit,
    base_incremental: Option<&IncrementalState>,
    old_model: &crate::graph::graph_builder::GraphModel,
    structural_outcome: &StructuralEditOutcome,
) -> Option<(GraphProjection, IncrementalState)> {
    if let Some(resumed) =
        build_resumed_graph_output(language, decoded, base_incremental, structural_outcome)
    {
        return Some(resumed);
    }

    let old_index_owned;
    let old_index =
        if let Some(index) = base_incremental.and_then(|state| state.graph_model_index.as_ref()) {
            index
        } else {
            old_index_owned = crate::graph::graph_model_index::GraphModelIndex::build(old_model);
            &old_index_owned
        };
    let builder_config =
        graph_projection_service::projection_builder_config().to_graph_builder_config();
    let graph_language = graph_projection_service::graph_language_from_name(language);
    let result = crate::graph::graph_delta_service::build_incremental_graph_delta_with_index(
        old_model,
        old_index,
        &decoded.store,
        decoded.root,
        edit,
        builder_config,
        graph_language,
    )?;
    Some((
        GraphProjection {
            ready: true,
            clear: false,
            graph_data: Some(graph_projection_service::to_document_graph_delta(
                &result.delta,
            )),
        },
        IncrementalState::resumable()
            .with_graph_state(result.model_snapshot, result.graph_index)
            .with_tree_path_index(structural_tree_path_index(
                base_incremental,
                decoded,
                structural_outcome,
            )),
    ))
}

fn build_resumed_graph_output(
    language: &str,
    decoded: &DecodedDocument,
    base_incremental: Option<&IncrementalState>,
    structural_outcome: &StructuralEditOutcome,
) -> Option<(GraphProjection, IncrementalState)> {
    if !structural_outcome.removed_node_ids.is_empty() {
        return None;
    }
    let base = base_incremental?;
    let mut projector =
        crate::graph::streaming_graph_projector::StreamingGraphProjector::from_incremental_state(
            language, base,
        )?;
    let mut anchors = structural_outcome.affected_nodes.clone();
    anchors.push(structural_outcome.changed_root);
    anchors.sort_by_key(|id| id.0);
    anchors.dedup();
    if anchors.is_empty() {
        return None;
    }
    let patches: Vec<_> = anchors
        .into_iter()
        .map(|node_id| crate::stream::tree_patch::TreePatch::NodeSealed { node_id })
        .collect();
    let update = projector.update(&decoded.store, decoded.root, &patches)?;
    if graph_delta_is_empty(&update.delta) {
        return None;
    }
    let incremental =
        projector
            .take_incremental_state()?
            .with_tree_path_index(structural_tree_path_index(
                base_incremental,
                decoded,
                structural_outcome,
            ));
    Some((
        GraphProjection {
            ready: true,
            clear: false,
            graph_data: Some(update.delta),
        },
        incremental,
    ))
}

fn graph_delta_is_empty(delta: &crate::document::protocol::GraphDelta) -> bool {
    delta.nodes_added.is_empty()
        && delta.nodes_updated.is_empty()
        && delta.nodes_removed.is_empty()
        && delta.edges_added.is_empty()
        && delta.edges_removed.is_empty()
        && delta.table_patches.is_empty()
        && delta.layout_patches.is_empty()
}
fn build_structural_graph_output(
    language: &str,
    decoded: &DecodedDocument,
    edit: &DocumentTextEdit,
    base_incremental: Option<&IncrementalState>,
    structural_span_index: Option<StructuralSpanIndex>,
    structural_outcome: &StructuralEditOutcome,
) -> (Option<GraphProjection>, Option<IncrementalState>) {
    let old_model_owned;
    let old_model = if let Some(snapshot) =
        base_incremental.and_then(|state| state.graph_model_snapshot.as_ref())
    {
        old_model_owned = snapshot.materialize();
        Some(&old_model_owned)
    } else {
        None
    };
    let (graph, mut incremental, fallback_reason) = if let Some(old_model) = old_model {
        if let Some((graph, incremental)) = build_incremental_graph_output(
            language,
            decoded,
            edit,
            base_incremental,
            old_model,
            structural_outcome,
        ) {
            (Some(graph), Some(incremental), None)
        } else {
            let (graph, incremental) = build_graph_output(language, decoded, true);
            (
                graph,
                incremental,
                Some("graph_incremental_unavailable".to_owned()),
            )
        }
    } else {
        let (graph, incremental) = build_graph_output(language, decoded, true);
        (graph, incremental, None)
    };
    if let Some(state) = incremental.as_mut() {
        state.structural_span_index = structural_span_index;
        if let Some(reason) = fallback_reason {
            state.structural_safe = false;
            state.fallback_reason = Some(reason);
        }
    }
    (graph, incremental)
}
fn update_scalar_node(
    language: &str,
    decoded: &mut DecodedDocument,
    target: crate::tree::NodeId,
    edit: &DocumentTextEdit,
) -> Option<ScalarSemanticChange> {
    let (old_sem_type, is_map_key) = {
        let node = decoded.store.get(target)?;
        if node.kind != TreeNodeKind::Scalar {
            return None;
        }
        (node.resolved_sem_type(), node.is_map_key)
    };
    let (sem_type, value) = parse_scalar_edit_replacement(language, &edit.replacement, is_map_key)?;
    {
        let node = decoded.store.get_mut(target)?;
        node.set_sem_type(sem_type);
        node.end_byte = edit.new_end_byte;
    }
    decoded.store.set_value(target, value).ok()?;
    Some(ScalarSemanticChange {
        old_sem_type,
        new_sem_type: sem_type,
        old_byte_len: edit.old_end_byte.saturating_sub(edit.start_byte),
        new_byte_len: edit.new_end_byte.saturating_sub(edit.start_byte),
    })
}
fn apply_subtree_node_edit(
    language: &str,
    decoded: &mut DecodedDocument,
    source_text: &str,
    edit: &DocumentTextEdit,
    nest: bool,
) -> Option<StructuralEditOutcome> {
    let affected = find_affected_node_id_for_edit(&decoded.store, decoded.root, edit)?;
    let boundary = find_reparse_boundary_id(&decoded.store, affected, decoded.root)?;
    if boundary == decoded.root {
        return None;
    }
    let old_boundary = decoded.store.get(boundary)?.clone();
    if !is_safe_subtree_boundary(&decoded.store, boundary, &old_boundary, edit) {
        return None;
    }
    let new_boundary_end = apply_delta(old_boundary.end_byte, crate::tree::edit_delta(edit))?;
    if new_boundary_end < old_boundary.start_byte || new_boundary_end as usize > source_text.len() {
        return None;
    }
    let start = old_boundary.start_byte as usize;
    let end = new_boundary_end as usize;
    let replacement_source = source_text.get(start..end)?;
    let old_subtree_ids = collect_subtree_node_ids(&decoded.store, boundary);
    let replacement = decode_document_with_nesting(language, replacement_source, nest).ok()?;
    let replacement_root_id = replacement_value_root(
        &replacement.store,
        replacement.root,
        &decoded.store,
        &old_boundary,
    )
    .unwrap_or(replacement.root);
    let replacement_root = replacement.store.get(replacement_root_id)?;
    if replacement_root.kind != old_boundary.kind {
        return None;
    }
    overwrite_subtree_from_decoded(
        &mut decoded.store,
        boundary,
        &old_boundary,
        &replacement.store,
        replacement_root_id,
        old_boundary.start_byte,
    )?;
    let affected_nodes = collect_subtree_node_ids(&decoded.store, boundary);
    let removed_node_ids = old_subtree_ids
        .into_iter()
        .filter(|id| *id != boundary)
        .collect();
    Some(StructuralEditOutcome {
        changed_root: boundary,
        old_changed_end: old_boundary.end_byte,
        affected_nodes,
        removed_node_ids,
        scalar_semantic_change: None,
    })
}

fn is_safe_subtree_boundary(
    store: &TreeStore,
    boundary: NodeId,
    node: &TreeNode,
    edit: &DocumentTextEdit,
) -> bool {
    matches!(node.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence)
        && !node.is_map_key
        && edit.start_byte >= node.start_byte
        && edit.old_end_byte <= node.end_byte
        && !subtree_has_alias_or_anchor(store, boundary)
}

fn subtree_has_alias_or_anchor(store: &TreeStore, root: NodeId) -> bool {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(node) = store.get(id) else {
            return true;
        };
        if node.kind == TreeNodeKind::Alias
            || node.alias().is_some()
            || store
                .anchor_for(id)
                .is_some_and(|anchor| !anchor.is_empty())
        {
            return true;
        }
        stack.extend(node.content.iter().copied());
    }
    false
}

fn replacement_value_root(
    replacement_store: &TreeStore,
    replacement_root: NodeId,
    old_store: &TreeStore,
    old_boundary: &TreeNode,
) -> Option<NodeId> {
    let old_key = old_boundary
        .key()
        .and_then(|id| old_store.value_for(id).ok())?;
    let root = replacement_store.get(replacement_root)?;
    if root.kind != TreeNodeKind::Mapping || root.content.len() != 2 {
        return None;
    }
    let key_id = root.content[0];
    let value_id = root.content[1];
    let key = replacement_store.get(key_id)?;
    let value = replacement_store.get(value_id)?;
    (key.is_map_key
        && replacement_store.value_for(key_id).ok() == Some(old_key)
        && value.kind == old_boundary.kind)
        .then_some(value_id)
}

fn overwrite_subtree_from_decoded(
    dst_store: &mut TreeStore,
    target: NodeId,
    old_target: &TreeNode,
    src_store: &TreeStore,
    src_root: NodeId,
    byte_base: u32,
) -> Option<()> {
    let src = src_store.get(src_root)?;
    let mut replacement = src.clone();
    let src_children = replacement.content.clone();
    replacement.content.clear();
    rebase_node_bytes(&mut replacement, byte_base)?;
    replacement.parent = old_target.parent;
    replacement.set_key(old_target.key());
    replacement.sequence_index = old_target.sequence_index;
    replacement.is_map_key = old_target.is_map_key;
    replacement.document = old_target.document;
    replacement.value = crate::tree::NodeValueRef::Missing;

    let document = old_target.document;
    let filename = dst_store.filename_for(target).ok()?.to_owned();
    let file_index = dst_store.file_index_for(target).ok()?;
    dst_store.set_document_meta(document, filename.clone(), file_index);
    {
        let target_node = dst_store.get_mut(target)?;
        *target_node = replacement;
    }
    sync_node_value_between_stores(src_store, src_root, dst_store, target)?;

    let cloned_children = clone_children_rebased(
        src_store,
        &src_children,
        dst_store,
        target,
        byte_base,
        document,
        &filename,
        file_index,
    )?;
    dst_store.get_mut(target)?.content = cloned_children;
    Some(())
}

fn clone_children_rebased(
    src_store: &TreeStore,
    src_children: &[NodeId],
    dst_store: &mut TreeStore,
    parent: NodeId,
    byte_base: u32,
    document: u32,
    filename: &str,
    file_index: i32,
) -> Option<Vec<NodeId>> {
    let parent_kind = dst_store.get(parent)?.kind;
    let mut cloned = Vec::with_capacity(src_children.len());
    if parent_kind == TreeNodeKind::Mapping {
        let mut index = 0;
        while index < src_children.len() {
            let key_id = clone_subtree_rebased(
                src_store,
                src_children[index],
                dst_store,
                Some(parent),
                byte_base,
                document,
                filename,
                file_index,
            )?;
            {
                let key = dst_store.get_mut(key_id)?;
                key.is_map_key = true;
                key.set_key(None);
                key.set_sequence_index(None);
            }
            cloned.push(key_id);

            if let Some(value_src) = src_children.get(index + 1) {
                let value_id = clone_subtree_rebased(
                    src_store,
                    *value_src,
                    dst_store,
                    Some(parent),
                    byte_base,
                    document,
                    filename,
                    file_index,
                )?;
                {
                    let value = dst_store.get_mut(value_id)?;
                    value.is_map_key = false;
                    value.set_key(Some(key_id));
                    value.set_sequence_index(None);
                }
                cloned.push(value_id);
            }
            index += 2;
        }
    } else {
        for (index, child_src) in src_children.iter().copied().enumerate() {
            let child_id = clone_subtree_rebased(
                src_store,
                child_src,
                dst_store,
                Some(parent),
                byte_base,
                document,
                filename,
                file_index,
            )?;
            if parent_kind == TreeNodeKind::Sequence {
                let child = dst_store.get_mut(child_id)?;
                child.is_map_key = false;
                child.set_key(None);
                child.set_sequence_index(Some(index as u32));
            }
            cloned.push(child_id);
        }
    }
    Some(cloned)
}

fn clone_subtree_rebased(
    src_store: &TreeStore,
    src_id: NodeId,
    dst_store: &mut TreeStore,
    parent: Option<NodeId>,
    byte_base: u32,
    document: u32,
    filename: &str,
    file_index: i32,
) -> Option<NodeId> {
    let src = src_store.get(src_id)?;
    let mut node = src.clone();
    let src_children = node.content.clone();
    node.content.clear();
    rebase_node_bytes(&mut node, byte_base)?;
    node.parent = parent;
    node.document = document;
    node.value = crate::tree::NodeValueRef::Missing;
    dst_store.set_document_meta(document, filename.to_owned(), file_index);
    let new_id = dst_store.add(node);
    sync_node_value_between_stores(src_store, src_id, dst_store, new_id)?;
    let cloned_children = clone_children_rebased(
        src_store,
        &src_children,
        dst_store,
        new_id,
        byte_base,
        document,
        filename,
        file_index,
    )?;
    dst_store.get_mut(new_id)?.content = cloned_children;
    Some(new_id)
}

fn rebase_node_bytes(node: &mut TreeNode, byte_base: u32) -> Option<()> {
    node.start_byte = node.start_byte.checked_add(byte_base)?;
    node.end_byte = node.end_byte.checked_add(byte_base)?;
    Some(())
}

fn sync_node_value_between_stores(
    src_store: &TreeStore,
    src_id: NodeId,
    dst_store: &mut TreeStore,
    dst_id: NodeId,
) -> Option<()> {
    match src_store.value_ref_for(src_id).ok()? {
        Some(_) => dst_store
            .set_value(dst_id, src_store.value_for(src_id).ok()?)
            .ok()?,
        None => dst_store.remove_value(dst_id).ok()?,
    }
    Some(())
}

/// Decode a document for materialization, with configurable JSON nesting.
/// Decode a document for materialization, with configurable JSON nesting.
///
/// Delegates to [`stream::decode_to_document_with_options`] which handles
/// both streaming (JSON) and non-streaming (YAML/TOML/etc.) languages.
fn decode_document_with_nesting(
    language: &str,
    source: &str,
    nest: bool,
) -> Result<DecodedDocument, CoreError> {
    stream::decode_to_document_with_options(
        language,
        source,
        DecodeOptions {
            nest_json: nest,
            emit_path: false,
        },
    )
}

fn structural_tree_path_index(
    base_incremental: Option<&IncrementalState>,
    decoded: &DecodedDocument,
    outcome: &StructuralEditOutcome,
) -> crate::tree::TreePathIndex {
    if let Some(base_index) = base_incremental.and_then(|state| state.tree_path_index.as_ref()) {
        return base_index.updated_for_structural_edit(
            &decoded.store,
            crate::tree::TreePathIndexStructuralUpdate {
                changed_root: outcome.changed_root,
                removed_node_ids: &outcome.removed_node_ids,
            },
        );
    }
    crate::tree::TreePathIndex::build(&decoded.store, decoded.root)
}
pub fn decode_for_stream_session(
    language: &str,
    input: ByteStream,
    nest: bool,
) -> Result<DecodedDocument, CoreError> {
    stream::decode_to_document_with_options(
        language,
        input.as_str(),
        DecodeOptions {
            nest_json: nest,
            emit_path: false,
        },
    )
}

pub fn store_incremental_analysis_snapshot(
    key: &str,
    language: &str,
    source: &str,
    decoded: &DecodedDocument,
    graph_payload: crate::document::protocol::GraphDelta,
    ts_tree: Option<tree_sitter::Tree>,
    token_spans: Vec<TokenSpan>,
    diagnostics: Vec<u32>,
    semantic_tokens: Vec<u32>,
    value_json: String,
) -> AnalysisBundle {
    let analysis = build_decoded_analysis_with_artifacts(
        key,
        language,
        source,
        decoded,
        ts_tree,
        token_spans,
        diagnostics,
        semantic_tokens,
        value_json,
    );
    let mut snapshot = DocumentSnapshot::with_incremental_analysis(
        key.to_owned(),
        analysis.clone(),
        IncrementalState::resumable(),
    );
    snapshot.graph = Some(GraphProjection {
        ready: true,
        clear: true,
        graph_data: Some(graph_payload),
    });
    let _ = store_snapshot_for_document(key, snapshot, true);
    analysis
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::lang_spec;
    use tree_sitter::Point;

    #[test]
    fn input_edit_positions_use_byte_columns() {
        let source = "å: 1\nb: 2\n";
        let start = source.find('1').expect("test source should contain value") as u32;
        let replacement = "10\n  - 20";
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start + 1,
            new_end_byte: start + replacement.len() as u32,
            replacement: replacement.to_owned(),
        };
        let input_edit = crate::tree::input_edit_for_source(source, &edit)
            .expect("edit should map to tree-sitter input edit");

        assert_eq!(input_edit.start_position, Point::new(0, 4));
        assert_eq!(input_edit.old_end_position, Point::new(0, 5));
        assert_eq!(input_edit.new_end_position, Point::new(1, 6));
    }

    #[test]
    fn input_edit_rejects_invalid_byte_boundaries() {
        let source = "å: 1\n";
        let edit = DocumentTextEdit {
            start_byte: 1,
            old_end_byte: 2,
            new_end_byte: 2,
            replacement: "a".to_owned(),
        };

        assert!(crate::tree::input_edit_for_source(source, &edit).is_none());
    }

    #[test]
    fn materialize_apply_edits_uses_prepared_tree_and_stores_next_tree() {
        let source = "items:\n  - one\n";
        let start = source.find("one").expect("test source should contain item") as u32;
        let replacement = "two\n  - three";
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start + "one".len() as u32,
            new_end_byte: start + replacement.len() as u32,
            replacement: replacement.to_owned(),
        };
        let prepared_tree = lang_spec::parse_tree("yaml", source.as_bytes())
            .expect("base yaml should parse with tree-sitter");

        let result = materialize(
            &DocumentInputPlan::BaseTextWithEdits,
            "doc",
            "yaml",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: false,
            },
            &[edit],
            Some(prepared_tree),
        );

        assert_eq!(result.analysis.source, "items:\n  - two\n  - three\n");
        assert!(result.analysis.document.is_some());
        assert!(result.analysis.ts_tree.is_some());
        assert!(result.analysis.diagnostics.is_empty());
    }

    #[test]
    fn materialize_apply_edits_syntax_fallback_does_not_decode_inside_analysis_builder() {
        let source = "items:\n  - one\n";
        let start = source.find("one").expect("test source should contain item") as u32;
        let replacement = "two\n  - three";
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start + "one".len() as u32,
            new_end_byte: start + replacement.len() as u32,
            replacement: replacement.to_owned(),
        };
        let prepared_tree = lang_spec::parse_tree("yaml", source.as_bytes())
            .expect("base yaml should parse with tree-sitter");

        let result = materialize(
            &DocumentInputPlan::BaseTextWithEdits,
            "doc-decode-ownership-edits",
            "yaml",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: false,
            },
            &[edit],
            Some(prepared_tree),
        );

        assert_eq!(result.analysis.source, "items:\n  - two\n  - three\n");
        assert!(result.analysis.document.is_some());
    }

    #[test]
    fn materialize_full_path_does_not_decode_inside_analysis_builder() {
        let source = "items:\n  - one\n";

        let result = materialize(
            &DocumentInputPlan::SourceText,
            "doc-decode-ownership-full",
            "yaml",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: false,
            },
            &[],
            None,
        );

        assert!(result.analysis.document.is_some());
    }
    #[test]
    fn materialize_full_projection_keeps_structural_span_index_lazy() {
        let result = materialize(
            &DocumentInputPlan::SourceText,
            "lazy-span",
            "json",
            r#"{"root":{"k":1}}"#,
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
            .expect("graph projection should keep incremental state");
        assert!(incremental.structural_span_index.is_none());
    }

    #[test]
    fn materialize_structural_apply_edits_builds_span_index_on_demand() {
        let source = r#"{"root":{"k":1}}"#;
        let start = source.find('1').expect("source should contain scalar") as u32;
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start + 1,
            new_end_byte: start + 1,
            replacement: "2".to_owned(),
        };

        let base_result = materialize(
            &DocumentInputPlan::SourceText,
            "lazy-span-base",
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
        let base_document = base_result
            .analysis
            .document
            .as_ref()
            .expect("base decode should succeed");
        let base_incremental = base_result
            .incremental
            .as_ref()
            .expect("base graph projection should keep incremental state");
        assert!(base_incremental.structural_span_index.is_none());

        let updated = materialize_with_base(
            &DocumentInputPlan::BaseTextWithEdits,
            "lazy-span-base",
            "json",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[edit],
            base_result.analysis.ts_tree.clone(),
            Some(base_document),
            Some(base_incremental),
        );

        let incremental = updated
            .incremental
            .as_ref()
            .expect("updated graph projection should keep incremental state");
        let span_index = incremental
            .structural_span_index
            .as_ref()
            .expect("structural apply should attach a fresh span index");
        let updated_source = updated.analysis.source.as_str();
        let updated_start = updated_source
            .find('2')
            .expect("updated source should contain replacement") as u32;
        let updated_document = updated
            .analysis
            .document
            .as_ref()
            .expect("updated decode should succeed");
        let node_id = span_index
            .find_exact_scalar(updated_start, updated_start + 1)
            .expect("new snapshot span index should locate edited scalar");
        updated_document
            .store
            .get(node_id)
            .expect("indexed node should exist");
        assert_eq!(updated_document.store.value_for(node_id).unwrap(), "2");
        assert_eq!(
            updated.graph.as_ref().map(|projection| projection.clear),
            Some(false)
        );
    }

    #[test]
    fn materialize_structural_apply_edits_reuses_attached_span_index() {
        let source = r#"{"root":{"k":1}}"#;
        let first_start = source.find('1').expect("source should contain scalar") as u32;
        let first_edit = DocumentTextEdit {
            start_byte: first_start,
            old_end_byte: first_start + 1,
            new_end_byte: first_start + 1,
            replacement: "2".to_owned(),
        };

        let base_result = materialize(
            &DocumentInputPlan::SourceText,
            "lazy-span-reuse",
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
        let base_document = base_result
            .analysis
            .document
            .as_ref()
            .expect("base decode should succeed");
        let base_incremental = base_result
            .incremental
            .as_ref()
            .expect("base graph projection should keep incremental state");

        let first = materialize_with_base(
            &DocumentInputPlan::BaseTextWithEdits,
            "lazy-span-reuse",
            "json",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[first_edit],
            base_result.analysis.ts_tree.clone(),
            Some(base_document),
            Some(base_incremental),
        );

        let first_source = first.analysis.source.clone();
        let first_document = first
            .analysis
            .document
            .as_ref()
            .expect("first structural edit should keep decoded document");
        let first_incremental = first
            .incremental
            .as_ref()
            .expect("first structural edit should keep incremental state");
        assert!(first_incremental.structural_span_index.is_some());

        let second_start = first_source
            .find('2')
            .expect("first updated source should contain replacement")
            as u32;
        let second_edit = DocumentTextEdit {
            start_byte: second_start,
            old_end_byte: second_start + 1,
            new_end_byte: second_start + 1,
            replacement: "3".to_owned(),
        };

        let second = materialize_with_base(
            &DocumentInputPlan::BaseTextWithEdits,
            "lazy-span-reuse",
            "json",
            &first_source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[second_edit],
            first.analysis.ts_tree.clone(),
            Some(first_document),
            Some(first_incremental),
        );

        assert_eq!(second.analysis.source, r#"{"root":{"k":3}}"#);
        let second_incremental = second
            .incremental
            .as_ref()
            .expect("second structural edit should keep incremental state");
        let second_span_index = second_incremental
            .structural_span_index
            .as_ref()
            .expect("second structural edit should preserve span index");
        let second_source = second.analysis.source.as_str();
        let second_value_start = second_source
            .find('3')
            .expect("second updated source should contain replacement")
            as u32;
        let second_document = second
            .analysis
            .document
            .as_ref()
            .expect("second structural edit should keep decoded document");
        let node_id = second_span_index
            .find_exact_scalar(second_value_start, second_value_start + 1)
            .expect("reused span index should locate edited scalar");
        second_document
            .store
            .get(node_id)
            .expect("indexed node should exist");
        assert_eq!(second_document.store.value_for(node_id).unwrap(), "3");
        assert_eq!(
            second.graph.as_ref().map(|projection| projection.clear),
            Some(false)
        );
    }

    #[test]
    fn materialize_structural_apply_edits_accepts_base_artifacts_context() {
        let source = r#"{"root":{"k":1},"tail":2}"#;
        let start = source.find('1').expect("source should contain scalar") as u32;
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start + 1,
            new_end_byte: start + 1,
            replacement: "3".to_owned(),
        };

        let base_result = materialize(
            &DocumentInputPlan::SourceText,
            "base-artifacts-context",
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
        let base_document = base_result
            .analysis
            .document
            .as_ref()
            .expect("base decode should succeed");
        let base_incremental = base_result
            .incremental
            .as_ref()
            .expect("base graph projection should keep incremental state");

        let updated_source = r#"{"root":{"k":3},"tail":2}"#;
        let updated = materialize_with_base_context(
            &DocumentInputPlan::BaseTextWithEdits,
            "base-artifacts-context",
            "json",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[edit],
            base_result.analysis.ts_tree.clone(),
            MaterializeBaseContext {
                document: Some(base_document),
                incremental: Some(base_incremental),
                line_index: Some(&base_result.analysis.line_index),
                semantic_tokens: Some(&base_result.analysis.semantic_tokens),
            },
        );

        assert_eq!(updated.analysis.source, updated_source);
        assert_eq!(
            updated.analysis.line_index,
            LineIndex::build(updated_source)
        );
        assert_eq!(
            updated.graph.as_ref().map(|projection| projection.clear),
            Some(false)
        );
        assert!(updated.analysis.document.is_some());
    }

    #[test]
    fn materialize_structural_reuses_semantic_tokens_for_same_width_same_type_scalar() {
        let source = r#"{"root":{"k":1},"tail":2}"#;
        let start = source.find('1').expect("source should contain scalar") as u32;
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start + 1,
            new_end_byte: start + 1,
            replacement: "3".to_owned(),
        };

        let base_result = materialize(
            &DocumentInputPlan::SourceText,
            "semantic-reuse-safe",
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
        let base_document = base_result.analysis.document.as_ref().unwrap();
        let base_incremental = base_result.incremental.as_ref().unwrap();

        let updated = materialize_with_base_context(
            &DocumentInputPlan::BaseTextWithEdits,
            "semantic-reuse-safe",
            "json",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[edit],
            base_result.analysis.ts_tree.clone(),
            MaterializeBaseContext {
                document: Some(base_document),
                incremental: Some(base_incremental),
                line_index: Some(&base_result.analysis.line_index),
                semantic_tokens: Some(&base_result.analysis.semantic_tokens),
            },
        );

        assert_eq!(
            updated.analysis.semantic_tokens,
            base_result.analysis.semantic_tokens
        );
    }

    #[test]
    fn materialize_structural_reencodes_semantic_tokens_for_type_change() {
        let source = r#"{"root":{"k":1},"tail":2}"#;
        let start = source.find('1').expect("source should contain scalar") as u32;
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start + 1,
            new_end_byte: start + "true".len() as u32,
            replacement: "true".to_owned(),
        };

        let base_result = materialize(
            &DocumentInputPlan::SourceText,
            "semantic-reuse-unsafe",
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
        let base_document = base_result.analysis.document.as_ref().unwrap();
        let base_incremental = base_result.incremental.as_ref().unwrap();

        let updated = materialize_with_base_context(
            &DocumentInputPlan::BaseTextWithEdits,
            "semantic-reuse-unsafe",
            "json",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[edit],
            base_result.analysis.ts_tree.clone(),
            MaterializeBaseContext {
                document: Some(base_document),
                incremental: Some(base_incremental),
                line_index: Some(&base_result.analysis.line_index),
                semantic_tokens: Some(&base_result.analysis.semantic_tokens),
            },
        );

        assert_eq!(updated.analysis.source, r#"{"root":{"k":true},"tail":2}"#);
        assert_eq!(
            updated.analysis.semantic_tokens,
            crate::language::encode_semantic_tokens("json", &updated.analysis.source)
        );
    }

    #[test]
    fn materialize_structural_subtree_edit_refreshes_tree_path_index_for_changed_subtree() {
        let source = r#"{"root":{"a":1,"b":2},"tail":3}"#;
        let replacement = r#"{"a":10,"c":30}"#;
        let old = r#"{"a":1,"b":2}"#;
        let start = source.find(old).expect("source should contain root object") as u32;
        let edit = DocumentTextEdit {
            start_byte: start,
            old_end_byte: start + old.len() as u32,
            new_end_byte: start + replacement.len() as u32,
            replacement: replacement.to_owned(),
        };

        let base_result = materialize(
            &DocumentInputPlan::SourceText,
            "subtree-path-index",
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
        let base_document = base_result.analysis.document.as_ref().unwrap();
        let base_incremental = base_result.incremental.as_ref().unwrap();

        let updated = materialize_with_base_context(
            &DocumentInputPlan::BaseTextWithEdits,
            "subtree-path-index",
            "json",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[edit],
            base_result.analysis.ts_tree.clone(),
            MaterializeBaseContext {
                document: Some(base_document),
                incremental: Some(base_incremental),
                line_index: Some(&base_result.analysis.line_index),
                semantic_tokens: Some(&base_result.analysis.semantic_tokens),
            },
        );

        let updated_document = updated.analysis.document.as_ref().unwrap();
        let updated_incremental = updated.incremental.as_ref().unwrap();
        let tree_path_index = updated_incremental
            .tree_path_index
            .as_ref()
            .expect("structural graph state should keep tree path index");
        let c_path = [
            crate::tree::path_seg_key("root"),
            crate::tree::path_seg_key("c"),
        ];
        let b_path = [
            crate::tree::path_seg_key("root"),
            crate::tree::path_seg_key("b"),
        ];

        let c_id = crate::tree::find_node_by_path_with_index(
            updated_document.root,
            &c_path,
            false,
            &updated_document.store,
            Some(tree_path_index),
        )
        .expect("updated tree path index should locate newly inserted $.root.c");
        assert_eq!(updated_document.store.value_for(c_id).unwrap(), "30");
        assert!(
            crate::tree::find_node_by_path_with_index(
                updated_document.root,
                &b_path,
                false,
                &updated_document.store,
                Some(tree_path_index),
            )
            .is_none(),
            "updated tree path index must not retain removed $.root.b"
        );
        assert_eq!(updated.graph.as_ref().map(|graph| graph.clear), Some(false));
    }

    #[test]
    fn materialize_whole_document_object_to_scalar_graph_delta_clears_or_removes_old_graph() {
        let source = r#"{"object":{"int":42,"float":0.125,"bool":true,"nil":null},"table_without_header":["a","b","c"]}"#;
        let replacement = "123";
        let edit = DocumentTextEdit {
            start_byte: 0,
            old_end_byte: source.len() as u32,
            new_end_byte: replacement.len() as u32,
            replacement: replacement.to_owned(),
        };

        let base_result = materialize(
            &DocumentInputPlan::SourceText,
            "whole-root-object-to-scalar",
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
        let base_graph = base_result
            .graph
            .as_ref()
            .and_then(|projection| projection.graph_data.as_ref())
            .expect("base graph should be present");
        let base_node_count = base_graph.nodes_added.len();
        assert!(
            base_node_count > 1,
            "base sample should render a multi-node graph"
        );

        let updated = materialize_with_base_context(
            &DocumentInputPlan::BaseTextWithEdits,
            "whole-root-object-to-scalar",
            "json",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[edit],
            base_result.analysis.ts_tree.clone(),
            MaterializeBaseContext {
                document: base_result.analysis.document.as_ref(),
                incremental: base_result.incremental.as_ref(),
                line_index: Some(&base_result.analysis.line_index),
                semantic_tokens: Some(&base_result.analysis.semantic_tokens),
            },
        );

        let projection = updated
            .graph
            .as_ref()
            .expect("root scalar replacement should produce final graph projection");
        let graph_data = projection
            .graph_data
            .as_ref()
            .expect("root scalar replacement should carry graph delta");
        assert!(
            projection.clear || graph_data.nodes_removed.len() == base_node_count,
            "root scalar replacement must either clear the graph or remove every old node"
        );
    }

    #[test]
    fn materialize_csv_header_edits_fall_back_to_reparse() {
        let source = "\"Region\",\"Code\"\n\"Afghanistan\",\"AF\"\n";
        let base_result = materialize(
            &DocumentInputPlan::SourceText,
            "csv-header-fallback",
            "csv",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[],
            None,
        );
        let base_document = base_result
            .analysis
            .document
            .as_ref()
            .expect("base decode should succeed");
        let header_node = base_document
            .store
            .nodes()
            .iter()
            .enumerate()
            .find(|(index, node)| {
                node.kind == TreeNodeKind::Scalar
                    && node.is_map_key
                    && base_document
                        .store
                        .value_for(crate::tree::NodeId::from_index(*index))
                        .is_ok_and(|value| value == "Region")
            })
            .map(|(_, node)| node)
            .expect("base csv should expose header key node");
        let start = header_node.start_byte as usize;
        let end = header_node.end_byte as usize;
        let replacement = source[start..end].replacen("Region", "Area", 1);
        let expected_source = format!("{}{}{}", &source[..start], replacement, &source[end..]);
        let edit = DocumentTextEdit {
            start_byte: header_node.start_byte,
            old_end_byte: header_node.end_byte,
            new_end_byte: header_node.start_byte + replacement.len() as u32,
            replacement,
        };

        let updated = materialize_with_base(
            &DocumentInputPlan::BaseTextWithEdits,
            "csv-header-fallback",
            "csv",
            source,
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[edit],
            base_result.analysis.ts_tree.clone(),
            Some(base_document),
            base_result.incremental.as_ref(),
        );

        assert_eq!(updated.analysis.source, expected_source);
        assert_eq!(
            updated.graph.as_ref().map(|projection| projection.clear),
            Some(true)
        );
        let updated_document = updated
            .analysis
            .document
            .as_ref()
            .expect("csv header edit should still decode after fallback");
        assert!(
            updated_document
                .store
                .nodes()
                .iter()
                .enumerate()
                .any(|(index, node)| {
                    node.kind == TreeNodeKind::Scalar
                        && node.is_map_key
                        && updated_document
                            .store
                            .value_for(crate::tree::NodeId::from_index(index))
                            .is_ok_and(|value| value == "Area")
                })
        );
    }

    #[test]
    fn materialize_parse_errors_without_tree_sitter_diagnostics_keep_generic_error_span() {
        let result = materialize(
            &DocumentInputPlan::SourceText,
            "csv-invalid",
            "csv",
            "\"Region\",\"Code\"\n\"Afghanistan\",\"AF",
            false,
            &OutputPlan {
                analysis: true,
                graph: true,
            },
            &[],
            None,
        );

        assert!(result.analysis.document.is_none());
        assert!(!result.analysis.diagnostics.is_empty());
        assert!(result.graph.is_none());
        assert_eq!(
            result
                .incremental
                .as_ref()
                .map(|state| state.fallback_reason.as_deref()),
            Some(Some("parse_failed"))
        );
    }

    /// Bug 1 regression: a valid JSON whose content trips tree-sitter's
    /// JavaScript grammar (used as the JSON validation fallback because the
    /// repo does not vendor tree-sitter-json) must NOT produce diagnostics
    /// after a successful streaming decode.  Previously
    /// `materialize_from_decoded` re-validated the source through the JS
    /// grammar and emitted a bogus full-source-span "Syntax error" for many
    /// large or specifically-shaped JSON payloads.
    #[test]
    fn materialize_from_decoded_does_not_invent_syntax_errors_on_valid_json() {
        // Minimal payload that the JavaScript grammar refuses but JSON allows.
        // The 2mb.json fixture exhibits the same pattern at scale; this is a
        // condensed reproducer that runs cheaply in unit tests.
        let source = include_str!("../../../../../test/fixtures/json/2mb.1.json");
        let decoded = stream::decode_to_document_with_options(
            "json",
            source,
            DecodeOptions {
                nest_json: false,
                emit_path: false,
            },
        )
        .expect("2mb.json fixture must decode as valid JSON");

        let result = materialize_from_decoded(
            decoded,
            "json",
            source,
            "doc-2mb",
            &OutputPlan {
                analysis: true,
                graph: false,
            },
            true,
            false,
        );

        assert!(
            result.analysis.diagnostics.is_empty(),
            "valid JSON must not yield diagnostics; got {} entries",
            result.analysis.diagnostics.len() / 5
        );
    }
}
