use std::collections::{HashMap, HashSet};

use super::graph_builder::{
    BuilderConfig, GraphBuilder, GraphCell, GraphEdge, GraphKind, GraphLanguage, GraphModel,
    GraphNode, GraphTable, PathSeg,
};
use super::graph_builder_preorder::{GraphDelta, GraphTableCellPatch};
use super::graph_fragment_index::FragmentInfo;
use super::graph_model_index::{GraphModelIndex, node_stable_id};
use super::graph_relayout::compute_ancestor_relayout_chain;
use crate::tree::incremental_edit::{
    DocumentTextEdit, find_affected_node_id_for_edit, find_reparse_boundary_id,
};
use crate::tree::{NodeId, TreeNode, TreeNodeKind, TreeStore};

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

pub type TableCellPatch = GraphTableCellPatch;

// ---------------------------------------------------------------------------
// IncrementalGraphDeltaResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct IncrementalGraphDeltaResult {
    pub model_snapshot: crate::graph::GraphModelSnapshot,
    pub graph_index: GraphModelIndex,
    pub delta: GraphDelta,
    pub relayout_chain: Vec<u64>,
    /// Non-empty when the table-cell in-place patch path was taken.
    pub table_cell_patches: Vec<TableCellPatch>,
}

// ===========================================================================
// Main entry point
// ===========================================================================

pub fn build_incremental_graph_delta(
    old_model: &GraphModel,
    new_store: &TreeStore,
    new_root: NodeId,
    edit: &DocumentTextEdit,
    config: BuilderConfig,
    language: GraphLanguage,
) -> Option<IncrementalGraphDeltaResult> {
    let old_index = GraphModelIndex::build(old_model);
    build_incremental_graph_delta_with_index(
        old_model, &old_index, new_store, new_root, edit, config, language,
    )
}

pub(crate) fn build_incremental_graph_delta_with_index(
    old_model: &GraphModel,
    old_index: &GraphModelIndex,
    new_store: &TreeStore,
    new_root: NodeId,
    edit: &DocumentTextEdit,
    config: BuilderConfig,
    language: GraphLanguage,
) -> Option<IncrementalGraphDeltaResult> {
    if let Some(result) = build_table_scalar_cell_delta(
        old_model,
        old_index,
        new_store,
        new_root,
        edit,
        config.clone(),
        language,
    ) {
        return Some(result);
    }

    let boundary = impacted_boundary(new_store, new_root, edit)?;
    let impact_path = graph_path_for_node(new_store, boundary)?;
    if impact_path.is_empty() {
        return None;
    }

    let impact_stable_id = old_index.stable_id_for_path(&impact_path)?;
    let old_root_fragment = old_index.fragment_for_stable_id(impact_stable_id)?;

    let impacted_stable_ids = old_index
        .fragment_index()
        .collect_subtree_stable_ids(impact_stable_id);
    let impacted_old_render_handles: HashSet<u32> = impacted_stable_ids
        .iter()
        .filter_map(|sid| old_index.render_handle_for_stable_id(*sid))
        .collect();
    if impacted_old_render_handles.is_empty() {
        return None;
    }

    let new_boundary = find_node_id_by_graph_path(new_store, new_root, &impact_path)?;

    let rebuilt_subtree = build_subtree_model(
        config.clone(),
        language,
        new_store,
        new_boundary,
        &impact_path,
    )?;

    let (mut merged_nodes, mut merged_edges, _old_render_to_temp, temp_to_final) =
        build_merged_skeleton(
            old_model,
            &impacted_old_render_handles,
            &rebuilt_subtree,
            old_root_fragment,
            old_index,
        )?;

    let root_temp_idx = root_temp_index(&merged_nodes)?;
    {
        let mut layout_builder = GraphBuilder::new(config.clone(), language);
        layout_builder.nodes = std::mem::take(&mut merged_nodes);
        layout_builder.edges = std::mem::take(&mut merged_edges);
        layout_builder.layout_graph(root_temp_idx);
        layout_builder.apply_node_bounds();
        layout_builder.apply_edge_bezier_args();
        merged_nodes = layout_builder.nodes;
        merged_edges = layout_builder.edges;
    }
    remap_node_and_edge_render_handles(&mut merged_nodes, &mut merged_edges, &temp_to_final);

    let mut model = GraphModel {
        nodes: merged_nodes,
        edges: merged_edges,
        ..Default::default()
    };
    model.rebuild_edge_index();

    let delta = build_impacted_direct_delta_candidate(
        old_index,
        &model,
        &impacted_stable_ids,
        &impacted_old_render_handles,
    );

    let changed_stable_ids = delta
        .nodes_added
        .iter()
        .map(|node| node.stable_id)
        .chain(delta.nodes_updated.iter().map(|node| node.stable_id))
        .collect::<Vec<_>>();

    // Build index via incremental update on a clone of old_index.
    // Falls back to full rebuild if the incremental path cannot be applied.
    let mut graph_index = old_index.clone();
    graph_index.incremental_update(&model, impact_stable_id, &impacted_stable_ids);

    let relayout_chain =
        compute_ancestor_relayout_chain(graph_index.fragment_index(), &changed_stable_ids);

    Some(IncrementalGraphDeltaResult {
        model_snapshot: crate::graph::GraphModelSnapshot::owned(model),
        graph_index,
        delta,
        relayout_chain,
        table_cell_patches: Vec::new(),
    })
}
/// Build the merged skeleton for incremental graph delta.
///
/// Copies un-impacted old nodes (with temp render handles), appends rebuilt subtree nodes
/// (with final render handles), merges un-impacted old edges, adds rebuilt subtree edges,
/// and captures the bridging edge from the old parent to the rebuilt root.
///
/// Returns `(merged_nodes, merged_edges, old_render_to_temp, temp_to_final, subtree_old_root_edge)`.
fn build_merged_skeleton(
    old_model: &GraphModel,
    impacted_old_render_handles: &HashSet<u32>,
    rebuilt_subtree: &GraphModel,
    old_root_fragment: &FragmentInfo,
    old_index: &GraphModelIndex,
) -> Option<(Vec<GraphNode>, Vec<GraphEdge>, HashMap<u32, u32>, Vec<u32>)> {
    let mut merged_nodes: Vec<GraphNode> = Vec::new();
    let mut merged_edges: Vec<GraphEdge> = Vec::new();
    let mut old_render_to_temp: HashMap<u32, u32> = HashMap::new();
    let mut temp_to_final: Vec<u32> = Vec::new();

    let old_max_render_handle = old_model
        .nodes
        .iter()
        .map(|n| n.render_handle)
        .max()
        .unwrap_or(0);

    // Phase 1: copy un-impacted old nodes with temp render handles
    for old_node in &old_model.nodes {
        if impacted_old_render_handles.contains(&old_node.render_handle) {
            continue;
        }
        let mut cloned = old_node.clone();
        let temp_index: u32 = merged_nodes.len() as u32;
        cloned.render_handle = temp_index;
        cloned.preorder_first = temp_index;
        cloned.preorder_last = temp_index;
        merged_nodes.push(cloned);
        old_render_to_temp.insert(old_node.render_handle, temp_index);
        temp_to_final.push(old_node.render_handle);
    }

    // Phase 2: append rebuilt subtree nodes with depth adjustment and final handles
    let mut next_render_handle = old_max_render_handle.saturating_add(1);
    for rebuilt_node in &rebuilt_subtree.nodes {
        let mut appended = rebuilt_node.clone();
        appended.depth += old_root_fragment.depth;
        let temp_index: u32 = merged_nodes.len() as u32;
        appended.render_handle = temp_index;
        appended.preorder_first = temp_index;
        appended.preorder_last = temp_index;
        merged_nodes.push(appended);

        let stable_id = node_stable_id(rebuilt_node);
        let final_render_handle = old_index
            .render_handle_for_stable_id(stable_id)
            .unwrap_or_else(|| {
                let assigned = next_render_handle;
                next_render_handle = next_render_handle.saturating_add(1);
                assigned
            });
        temp_to_final.push(final_render_handle);
    }

    let subtree_temp_base = merged_nodes.len() as u32 - rebuilt_subtree.nodes.len() as u32;

    // Phase 3: merge un-impacted old edges, capturing bridge edge
    let bridge = {
        let mut found: Option<GraphEdge> = None;
        for edge in &old_model.edges {
            let from_impacted = impacted_old_render_handles.contains(&edge.from_render_handle);
            let to_impacted = impacted_old_render_handles.contains(&edge.to_render_handle);
            if to_impacted
                && edge.to_render_handle == old_root_fragment.render_handle
                && !from_impacted
            {
                found = Some(edge.clone());
            }
            if from_impacted || to_impacted {
                continue;
            }
            let mut kept = edge.clone();
            kept.from_render_handle = *old_render_to_temp.get(&edge.from_render_handle)?;
            kept.to_render_handle = *old_render_to_temp.get(&edge.to_render_handle)?;
            merged_edges.push(kept);
        }
        found
    };

    // Phase 4: add rebuilt subtree edges (in temp handles)
    for edge in &rebuilt_subtree.edges {
        let mut appended = edge.clone();
        appended.from_render_handle = subtree_temp_base + edge.from_render_handle;
        appended.to_render_handle = subtree_temp_base + edge.to_render_handle;
        merged_edges.push(appended);
    }

    // Phase 5: create bridge edge from old parent to rebuilt subtree root
    if let Some(mut bridged) = bridge {
        bridged.from_render_handle = *old_render_to_temp.get(&bridged.from_render_handle)?;
        bridged.to_render_handle = subtree_temp_base;
        merged_edges.push(bridged);
    } else {
        return None;
    }

    Some((
        merged_nodes,
        merged_edges,
        old_render_to_temp,
        temp_to_final,
    ))
}

fn build_impacted_direct_delta_candidate(
    old_index: &GraphModelIndex,
    model: &GraphModel,
    impacted_stable_ids: &[u64],
    _impacted_old_render_handles: &HashSet<u32>,
) -> GraphDelta {
    let impacted_set: HashSet<u64> = impacted_stable_ids.iter().copied().collect();
    let mut nodes_added = Vec::new();
    let mut nodes_updated = Vec::new();

    for node in &model.nodes {
        let stable_id = node_stable_id(node);
        if !impacted_set.contains(&stable_id) && old_index.node_hash(stable_id).is_some() {
            continue;
        }
        let new_hash = crate::graph::graph_model_index::hash_graph_node(node);
        match old_index.node_hash(stable_id) {
            Some(old_hash) if old_hash == new_hash => {}
            Some(_) => nodes_updated.push(node.clone()),
            None => nodes_added.push(node.clone()),
        }
    }

    let mut edges_added = Vec::new();
    for edge in &model.edges {
        let key = old_index.edge_key_for(edge);
        let new_hash = crate::graph::graph_model_index::hash_edge(edge);
        if old_index.edge_hash(&key) != Some(new_hash) {
            edges_added.push(edge.clone());
        }
    }

    GraphDelta {
        clear: false,
        nodes_added,
        nodes_updated,
        nodes_removed: Vec::new(),
        edges_added,
        edges_removed: Vec::new(),
        table_cell_patches: Vec::new(),
    }
}
fn build_table_scalar_cell_delta(
    old_model: &GraphModel,
    old_index: &GraphModelIndex,
    new_store: &TreeStore,
    new_root: NodeId,
    edit: &DocumentTextEdit,
    config: BuilderConfig,
    language: GraphLanguage,
) -> Option<IncrementalGraphDeltaResult> {
    let affected_id = find_affected_node_id_for_edit(new_store, new_root, edit)?;
    let affected = new_store.get(affected_id)?;
    if affected.kind != TreeNodeKind::Scalar {
        return None;
    }

    let impact_path = graph_path_for_node(new_store, affected_id)?;
    if impact_path.is_empty() {
        return None;
    }

    let impact = old_index.find_table_cell_by_path(&impact_path)?;
    if !impact.is_header_table || !impact.is_editable_scalar_value {
        return None;
    }

    let old_table_node = find_node_by_render_handle(&old_model.nodes, impact.table_render_handle)?;
    if old_table_node.kind != GraphKind::Table {
        return None;
    }
    let old_table = old_table_node.table.as_ref()?;

    let new_table_source = find_node_id_by_graph_path(new_store, new_root, &old_table_node.path)?;
    let new_table_source_node = new_store.get(new_table_source)?;
    if new_table_source_node.kind != TreeNodeKind::Sequence {
        return None;
    }
    if !super::graph_topology::is_header_table_sequence(new_store, new_table_source) {
        return None;
    }

    let new_columns = build_header_table_columns(new_store, new_table_source);
    if !same_table_column_cells(&old_table.columns, &new_columns) {
        return None;
    }

    let row_idx = impact.row_index as usize;
    let col_idx = impact.column_index as usize;
    if row_idx >= old_table.rows.len() {
        return None;
    }
    if col_idx >= old_table.columns.len() || col_idx >= old_table.column_widths.len() {
        return None;
    }

    let old_cell = &old_table.rows[row_idx][col_idx];
    if old_cell.path != impact.cell_path {
        return None;
    }

    let patch_cell_path = impact.cell_path.clone();
    let patch_cell = cell_from_value_node(new_store, &patch_cell_path, affected_id, affected);
    if patch_cell.path != impact.cell_path {
        return None;
    }

    if table_cell_patch_preserves_geometry(
        &config,
        language,
        old_table,
        impact.row_index,
        impact.column_index,
        &patch_cell,
    ) {
        let mut final_patch_cell = patch_cell;
        copy_cell_geometry(&mut final_patch_cell, old_cell);

        let table_cell_patches = vec![TableCellPatch {
            table_render_handle: impact.table_render_handle,
            row_index: impact.row_index,
            column_index: impact.column_index,
            cell: final_patch_cell,
        }];
        let model_snapshot = crate::graph::GraphModelSnapshot::owned(old_model.clone())
            .with_table_cell_patch(table_cell_patches[0].clone());
        let patched_table_node = model_snapshot
            .patched_node(impact.table_render_handle)
            .expect("patched table node should exist in model snapshot");
        let mut merged_graph_index = old_index.clone();
        merged_graph_index.patch_node_without_edge_changes(&patched_table_node);
        let delta = GraphDelta {
            clear: false,
            nodes_added: Vec::new(),
            nodes_updated: Vec::new(),
            nodes_removed: Vec::new(),
            edges_added: Vec::new(),
            edges_removed: Vec::new(),
            table_cell_patches: table_cell_patches.clone(),
        };
        return Some(IncrementalGraphDeltaResult {
            model_snapshot,
            graph_index: merged_graph_index,
            delta,
            relayout_chain: Vec::new(),
            table_cell_patches,
        });
    }

    let table_path = old_table_node.path.clone();
    let rebuilt_table_model = build_subtree_model(
        config.clone(),
        language,
        new_store,
        new_table_source,
        &table_path,
    )?;
    let mut rebuilt_table_node = rebuilt_table_model.nodes.first()?.clone();
    rebuilt_table_node.depth = old_table_node.depth;

    let rebuilt_table = rebuilt_table_node.table.as_ref()?;
    if !same_table_column_cells(&old_table.columns, &rebuilt_table.columns) {
        return None;
    }
    if impact.row_index as usize >= rebuilt_table.rows.len() {
        return None;
    }
    if impact.column_index as usize >= rebuilt_table.columns.len() {
        return None;
    }
    let rebuilt_cell = &rebuilt_table.rows[impact.row_index as usize][impact.column_index as usize];
    if rebuilt_cell.path != impact.cell_path {
        return None;
    }

    let mut merged_nodes: Vec<GraphNode> = Vec::with_capacity(old_model.nodes.len());
    let mut merged_edges: Vec<GraphEdge> = Vec::with_capacity(old_model.edges.len());
    let mut temp_to_final: Vec<u32> = Vec::with_capacity(old_model.nodes.len());
    let mut old_render_to_temp: HashMap<u32, u32> = HashMap::new();

    for old_node in &old_model.nodes {
        let temp_index: u32 = merged_nodes.len() as u32;
        let mut cloned = if old_node.render_handle == impact.table_render_handle {
            rebuilt_table_node.clone()
        } else {
            old_node.clone()
        };
        cloned.render_handle = temp_index;
        cloned.preorder_first = temp_index;
        cloned.preorder_last = temp_index;
        merged_nodes.push(cloned);
        old_render_to_temp.insert(old_node.render_handle, temp_index);
        temp_to_final.push(old_node.render_handle);
    }

    for edge in &old_model.edges {
        let mut kept = edge.clone();
        kept.from_render_handle = *old_render_to_temp.get(&edge.from_render_handle)?;
        kept.to_render_handle = *old_render_to_temp.get(&edge.to_render_handle)?;
        merged_edges.push(kept);
    }

    let root_temp_idx = root_temp_index(&merged_nodes)?;
    {
        let mut layout_builder = GraphBuilder::new(config.clone(), language);
        layout_builder.nodes = std::mem::take(&mut merged_nodes);
        layout_builder.edges = std::mem::take(&mut merged_edges);
        layout_builder.layout_graph(root_temp_idx);
        layout_builder.apply_node_bounds();
        layout_builder.apply_edge_bezier_args();
        merged_nodes = layout_builder.nodes;
        merged_edges = layout_builder.edges;
    }
    remap_node_and_edge_render_handles(&mut merged_nodes, &mut merged_edges, &temp_to_final);

    let mut merged_model = GraphModel {
        nodes: merged_nodes,
        edges: merged_edges,
        ..Default::default()
    };
    merged_model.rebuild_edge_index();
    let merged_graph_index = GraphModelIndex::build(&merged_model);
    let delta = build_graph_delta_with_index(old_index, &merged_model, &merged_graph_index);

    Some(IncrementalGraphDeltaResult {
        model_snapshot: crate::graph::GraphModelSnapshot::owned(merged_model),
        graph_index: merged_graph_index,
        delta,
        relayout_chain: Vec::new(),
        table_cell_patches: Vec::new(),
    })
}

fn impacted_boundary(store: &TreeStore, root: NodeId, edit: &DocumentTextEdit) -> Option<NodeId> {
    let affected = find_affected_node_id_for_edit(store, root, edit)?;
    Some(find_reparse_boundary_id(store, affected, root).unwrap_or(affected))
}

fn find_node_by_render_handle(nodes: &[GraphNode], render_handle: u32) -> Option<&GraphNode> {
    nodes
        .iter()
        .find(|node| node.render_handle == render_handle)
}

fn root_temp_index(nodes: &[GraphNode]) -> Option<u32> {
    nodes
        .iter()
        .find(|node| node.path.is_empty())
        .map(|node| node.render_handle)
}

fn remap_node_and_edge_render_handles(
    nodes: &mut [GraphNode],
    edges: &mut [GraphEdge],
    temp_to_final: &[u32],
) {
    for node in nodes.iter_mut() {
        node.render_handle = temp_to_final[node.render_handle as usize];
    }
    for edge in edges.iter_mut() {
        edge.from_render_handle = temp_to_final[edge.from_render_handle as usize];
        edge.to_render_handle = temp_to_final[edge.to_render_handle as usize];
    }
}

fn build_subtree_model(
    config: BuilderConfig,
    language: GraphLanguage,
    store: &TreeStore,
    root: NodeId,
    root_path: &[PathSeg],
) -> Option<GraphModel> {
    let mut builder = super::graph_builder_preorder::Builder::new(config, language);
    builder.release_view_data_on_cleanup = false;
    builder
        .emit_tree_store_preorder_stack_with_root_path(store, root, root_path)
        .ok()?;
    builder.finish().ok()
}

fn estimated_table_column_width(config: &BuilderConfig, text: &str) -> i32 {
    super::graph_builder::table_graph::estimated_table_column_width_for_config(config, text)
}

fn build_header_table_columns(store: &TreeStore, node_id: NodeId) -> Vec<GraphCell> {
    let mut keys: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let Some(node) = store.get(node_id) else {
        return Vec::new();
    };
    for item_id in &node.content {
        let Some(item) = store.get(*item_id) else {
            continue;
        };
        if item.kind != TreeNodeKind::Mapping {
            continue;
        }
        let mut i = 0;
        while i + 1 < item.content.len() {
            let key_id = item.content[i];
            let Some(_key_node) = store.get(key_id) else {
                i += 2;
                continue;
            };
            let key_text = store.value_string_for(key_id).unwrap_or_default();
            if seen.insert(key_text.clone()) {
                keys.push(key_text);
            }
            i += 2;
        }
    }

    let needs_fallback = header_table_needs_fallback_value_column(store, node_id);
    let value_column_count = if needs_fallback {
        keys.len().max(1) + 1
    } else {
        keys.len().max(1)
    };
    let total_columns = value_column_count + 1;

    let mut cols = Vec::with_capacity(total_columns);
    cols.push(GraphCell {
        text: String::new(),
        path: Vec::new(),
        editable: false,
        ..Default::default()
    });
    for key in &keys {
        cols.push(GraphCell {
            text: key.clone(),
            path: Vec::new(),
            editable: false,
            ..Default::default()
        });
    }
    while cols.len() < total_columns {
        cols.push(GraphCell {
            text: String::new(),
            path: Vec::new(),
            editable: false,
            ..Default::default()
        });
    }
    cols
}

fn header_table_needs_fallback_value_column(store: &TreeStore, node_id: NodeId) -> bool {
    let Some(node) = store.get(node_id) else {
        return false;
    };
    let mut has_mapping_key = false;
    for item_id in &node.content {
        let Some(item) = store.get(*item_id) else {
            continue;
        };
        if item.kind != TreeNodeKind::Mapping {
            continue;
        }
        let mut i = 0;
        while i + 1 < item.content.len() {
            let value_id = item.content[i + 1];
            let Some(value_node) = store.get(value_id) else {
                i += 2;
                continue;
            };
            if matches!(
                value_node.kind,
                TreeNodeKind::Mapping | TreeNodeKind::Sequence
            ) && !is_leaf(value_node)
            {
                has_mapping_key = true;
                break;
            }
            i += 2;
        }
        if has_mapping_key {
            break;
        }
    }
    has_mapping_key
}

fn is_leaf(node: &TreeNode) -> bool {
    if !matches!(node.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence) {
        return true;
    }
    node.content.is_empty()
}

fn cell_from_value_node(
    store: &TreeStore,
    path: &[PathSeg],
    node_id: NodeId,
    node: &TreeNode,
) -> GraphCell {
    GraphCell {
        text: store.value_string_for(node_id).unwrap_or_default(),
        sem_type: node.sem_type.map(|st| st.to_string()),
        path: path.to_vec(),
        value: store.value_string_for(node_id).unwrap_or_default(),
        editable: true,
        source: Some(node_id.index()),
        ..Default::default()
    }
}

fn same_table_column_cells(old_columns: &[GraphCell], new_columns: &[GraphCell]) -> bool {
    if old_columns.len() != new_columns.len() {
        return false;
    }
    old_columns
        .iter()
        .zip(new_columns.iter())
        .all(|(old_col, new_col)| old_col.text == new_col.text)
}

fn table_cell_patch_preserves_geometry(
    config: &BuilderConfig,
    _language: GraphLanguage,
    old_table: &GraphTable,
    row_index: u32,
    column_index: u32,
    patch_cell: &GraphCell,
) -> bool {
    let col_idx = column_index as usize;
    if col_idx >= old_table.columns.len() || col_idx >= old_table.column_widths.len() {
        return false;
    }
    let expected_width = old_table.column_widths[col_idx];
    let mut width = estimated_table_column_width(config, &old_table.columns[col_idx].text);
    let target_row_index = row_index as usize;

    for (row_idx, row) in old_table.rows.iter().enumerate() {
        if col_idx >= row.len() {
            return false;
        }
        let text = if row_idx == target_row_index {
            &patch_cell.text
        } else {
            &row[col_idx].text
        };
        width = width.max(estimated_table_column_width(config, text));
    }
    width = width.min(config.table_column_width);
    width == expected_width
}

fn copy_cell_geometry(cell: &mut GraphCell, old_cell: &GraphCell) {
    cell.bounds = old_cell.bounds;
    cell.text_bounds = old_cell.text_bounds;
    cell.box_args = old_cell.box_args;
    cell.text_args = old_cell.text_args.clone();
    cell.text_args.text = cell.text.clone();
    cell.text_args.editable = cell.editable;
}

fn graph_path_for_node(store: &TreeStore, id: NodeId) -> Option<Vec<PathSeg>> {
    let node = store.get(id)?;
    let Some(parent_id) = node.parent else {
        return Some(Vec::new());
    };
    let mut path = graph_path_for_node(store, parent_id)?;
    if node.is_map_key {
        path.push(PathSeg::Key(store.value_string_for(id).ok()?));
        return Some(path);
    }
    if let Some(key_id) = node.key() {
        path.push(PathSeg::Key(store.value_string_for(key_id).ok()?));
        return Some(path);
    }
    if let Some(index) = node
        .sequence_index()
        .and_then(|value| usize::try_from(value).ok())
    {
        path.push(PathSeg::Index(index));
    }
    Some(path)
}

fn find_node_id_by_graph_path(store: &TreeStore, root: NodeId, path: &[PathSeg]) -> Option<NodeId> {
    let mut current = root;
    for segment in path {
        let node = store.get(current)?;
        current = match (node.kind, segment) {
            (TreeNodeKind::Mapping, PathSeg::Key(key)) => find_map_entry(store, current, &key)?.1,
            (TreeNodeKind::Sequence, PathSeg::Index(value)) => *node.content.get(*value)?,
            _ => return None,
        };
    }
    Some(current)
}

fn find_map_entry(
    store: &TreeStore,
    parent: NodeId,
    expected_key: &str,
) -> Option<(NodeId, NodeId)> {
    let node = store.get(parent)?;
    let mut index = 0;
    while index + 1 < node.content.len() {
        let key_id = node.content[index];
        let value_id = node.content[index + 1];
        let key_node = store.get(key_id)?;
        if key_node.is_map_key && store.value_for(key_id).ok()? == expected_key {
            return Some((key_id, value_id));
        }
        index += 2;
    }
    None
}

// ===========================================================================
// Graph delta computation
// ===========================================================================

/// Diff two complete graph models using prebuilt sidecar indexes.
///
/// Complexity contract: keep this hash-based and linear in model size:
/// `O(old_nodes + old_edges + new_nodes + new_edges)` time with a comparable
/// hash-map working set. Do not regress to nested linear membership scans.
pub(crate) fn build_graph_delta_with_index(
    old_index: &GraphModelIndex,
    new_model: &GraphModel,
    new_index: &GraphModelIndex,
) -> GraphDelta {
    let mut nodes_added = Vec::new();
    let mut nodes_updated = Vec::new();
    let mut nodes_removed = Vec::new();
    let mut edges_added = Vec::new();
    let mut edges_removed = Vec::new();

    for node in &new_model.nodes {
        let stable_id = node_stable_id(node);
        let Some(new_hash) = new_index.node_hash(stable_id) else {
            continue;
        };
        match old_index.node_hash(stable_id) {
            None => nodes_added.push(node.clone()),
            Some(old_hash) if old_hash != new_hash => nodes_updated.push(node.clone()),
            _ => {}
        }
    }

    for (stable_id, render_handle) in old_index.stable_id_render_handles() {
        if new_index.render_handle_for_stable_id(stable_id).is_none() {
            nodes_removed.push(render_handle);
        }
    }

    for edge in &new_model.edges {
        let key = new_index.edge_key_for(edge);
        let Some(new_hash) = new_index.edge_hash(&key) else {
            continue;
        };
        match old_index.edge_hash(&key) {
            None => edges_added.push(edge.clone()),
            Some(old_hash) if old_hash != new_hash => edges_added.push(edge.clone()),
            _ => {}
        }
    }

    for (key, edge, old_hash) in old_index.edge_entries() {
        match new_index.edge_hash(key) {
            None => edges_removed.push(edge.clone()),
            Some(new_hash) if old_hash != new_hash => edges_removed.push(edge.clone()),
            _ => {}
        }
    }

    GraphDelta {
        clear: false,
        nodes_added,
        nodes_updated,
        nodes_removed,
        edges_added,
        edges_removed,
        table_cell_patches: Vec::new(),
    }
}
