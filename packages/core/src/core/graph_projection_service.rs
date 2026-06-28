use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::document::protocol::{
    GraphBoxArgs, GraphCellData, GraphDelta, GraphEdgeData, GraphEdgeRemoved, GraphNodeData,
    GraphPathSeg, GraphRowData, GraphTableData, GraphTextArgs, TableCellPatchData, TablePatch,
};

use super::{
    GraphModelIndex, NodeId, SemType, TreeStore, graph_builder as core_graph_builder,
    graph_builder_preorder, graph_topology::GraphTopology, lang_spec::lang_from_name,
    layout_engine::LayoutState,
};

#[derive(Debug, Clone)]
pub(crate) struct CachedProjectionModel {
    pub(crate) model_snapshot: crate::core::GraphModelSnapshot,
    pub(crate) index: GraphModelIndex,
    pub(crate) topology: Option<GraphTopology>,
    pub(crate) layout_state: Option<LayoutState>,
}

static PROJECTION_MODEL_CACHE: LazyLock<Mutex<HashMap<String, CachedProjectionModel>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static PROJECTION_BUILDER_CONFIG: LazyLock<Mutex<BuilderConfigState>> =
    LazyLock::new(|| Mutex::new(BuilderConfigState::default()));

#[derive(Debug, Clone, Copy)]
pub(crate) struct BuilderConfigState {
    pub(crate) key_width: i32,
    pub(crate) value_width: i32,
    pub(crate) row_height: i32,
    pub(crate) row_padding_x: i32,
    pub(crate) row_padding_y: i32,
    pub(crate) node_border_width: i32,
    pub(crate) v_gap: i32,
    pub(crate) h_gap: i32,
    pub(crate) table_max_height: i32,
    pub(crate) table_row_height: i32,
    pub(crate) table_header_height: i32,
    pub(crate) table_column_width: i32,
    pub(crate) avg_char_width_x10: i32,
    pub(crate) font_size: i32,
    pub(crate) meta_path_min_segments: i32,
    pub(crate) meta_path_min_chars: i32,
    pub(crate) meta_path_keep_tail_segments: i32,
    pub(crate) corner_radius: i32,
    pub(crate) expand_header_table_rows: bool,
}

impl Default for BuilderConfigState {
    fn default() -> Self {
        Self {
            key_width: 300,
            value_width: 500,
            row_height: 18,
            row_padding_x: 20,
            row_padding_y: 1,
            node_border_width: 1,
            v_gap: 60,
            h_gap: 60,
            table_max_height: 1000,
            table_row_height: 28,
            table_header_height: 26,
            table_column_width: 500,
            avg_char_width_x10: 72,
            font_size: 12,
            meta_path_min_segments: 4,
            meta_path_min_chars: 28,
            meta_path_keep_tail_segments: 1,
            expand_header_table_rows: false,
            corner_radius: 4,
        }
    }
}

impl BuilderConfigState {
    pub(crate) fn to_graph_builder_config(self) -> core_graph_builder::BuilderConfig {
        core_graph_builder::BuilderConfig {
            key_width: self.key_width,
            value_width: self.value_width,
            row_height: self.row_height,
            row_padding_x: self.row_padding_x,
            row_padding_y: self.row_padding_y,
            node_border_width: self.node_border_width,
            v_gap: self.v_gap,
            h_gap: self.h_gap,
            table_max_height: self.table_max_height,
            table_row_height: self.table_row_height,
            table_header_height: self.table_header_height,
            table_column_width: self.table_column_width,
            avg_char_width_x10: self.avg_char_width_x10,
            font_size: self.font_size,
            meta_path_min_segments: self.meta_path_min_segments,
            meta_path_min_chars: self.meta_path_min_chars,
            meta_path_keep_tail_segments: self.meta_path_keep_tail_segments,
            expand_header_table_rows: self.expand_header_table_rows,
        }
    }
}

pub(crate) fn reset_builder_config() {
    if let Ok(mut config) = PROJECTION_BUILDER_CONFIG.lock() {
        *config = BuilderConfigState::default();
    }
}

pub(crate) fn set_builder_config(state: BuilderConfigState) {
    if let Ok(mut config) = PROJECTION_BUILDER_CONFIG.lock() {
        *config = state;
    }
}

pub(crate) fn projection_builder_config() -> BuilderConfigState {
    PROJECTION_BUILDER_CONFIG
        .lock()
        .map(|config| *config)
        .unwrap_or_default()
}

pub(crate) fn store_projection_model_cache(
    document_key: &str,
    model: core_graph_builder::GraphModel,
) {
    let index = GraphModelIndex::build(&model);
    store_projection_model_snapshot_cache_with_index(
        document_key,
        crate::core::GraphModelSnapshot::owned(model),
        index,
    );
}

pub(crate) fn store_projection_model_cache_with_runtime_state(
    document_key: &str,
    model: core_graph_builder::GraphModel,
    topology: GraphTopology,
    layout_state: LayoutState,
) {
    let index = GraphModelIndex::build(&model);
    if let Ok(mut cache) = PROJECTION_MODEL_CACHE.lock() {
        cache.insert(
            document_key.to_owned(),
            CachedProjectionModel {
                model_snapshot: crate::core::GraphModelSnapshot::owned(model),
                index,
                topology: Some(topology),
                layout_state: Some(layout_state),
            },
        );
    }
}

pub(crate) fn store_projection_model_snapshot_cache_with_index(
    document_key: &str,
    model_snapshot: crate::core::GraphModelSnapshot,
    index: GraphModelIndex,
) {
    if let Ok(mut cache) = PROJECTION_MODEL_CACHE.lock() {
        cache.insert(
            document_key.to_owned(),
            CachedProjectionModel {
                model_snapshot,
                index,
                topology: None,
                layout_state: None,
            },
        );
    }
}

pub(crate) fn get_projection_model_cache_entry(
    document_key: &str,
) -> Option<CachedProjectionModel> {
    PROJECTION_MODEL_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(document_key).cloned())
}

fn str_sem_type_to_u32(sem: Option<&str>) -> u32 {
    match sem {
        Some("map") | Some("!!map") => SemType::Map as u32,
        Some("seq") | Some("!!seq") => SemType::Seq as u32,
        Some("str") | Some("!!str") => SemType::Str as u32,
        Some("int") | Some("!!int") => SemType::Int as u32,
        Some("float") | Some("!!float") => SemType::Float as u32,
        Some("boolean") | Some("bool") | Some("!!bool") => SemType::Boolean as u32,
        Some("nil") | Some("null") | Some("!!null") => SemType::Nil as u32,
        _ => SemType::Str as u32,
    }
}

fn convert_path_seg(seg: &core_graph_builder::PathSeg) -> GraphPathSeg {
    match seg {
        core_graph_builder::PathSeg::Key(key) => GraphPathSeg {
            tag: 0,
            key: key.clone(),
            index: 0,
        },
        core_graph_builder::PathSeg::Index(idx) => GraphPathSeg {
            tag: 1,
            key: String::new(),
            index: *idx as i32,
        },
    }
}

fn convert_path(path: &[core_graph_builder::PathSeg]) -> Vec<GraphPathSeg> {
    path.iter().map(convert_path_seg).collect()
}

fn convert_box_args(b: &core_graph_builder::BoxArgs) -> GraphBoxArgs {
    GraphBoxArgs {
        x: b.x,
        y: b.y,
        width: b.width,
        height: b.height,
        corner_radius: b.corner_radius,
    }
}
fn convert_bezier_args(
    b: &core_graph_builder::BezierArgs,
) -> crate::document::protocol::GraphBezierArgsData {
    crate::document::protocol::GraphBezierArgsData {
        from_x: b.from_x,
        from_y: b.from_y,
        c1x: b.c1x,
        c1y: b.c1y,
        c2x: b.c2x,
        c2y: b.c2y,
        to_x: b.to_x,
        to_y: b.to_y,
    }
}
fn convert_cell_value(cell: &core_graph_builder::GraphCell) -> String {
    match cell.sem_type.as_deref() {
        Some("!!map" | "!!seq") if cell.value.is_empty() => cell.text.clone(),
        _ => cell.value.clone(),
    }
}

pub(crate) fn convert_cell(cell: &core_graph_builder::GraphCell) -> GraphCellData {
    let value = convert_cell_value(cell);
    GraphCellData {
        sem_type: str_sem_type_to_u32(cell.sem_type.as_deref()),
        is_missing: cell.is_missing,
        path: convert_path(&cell.path),
        text: cell.text.clone(),
        value,
        format_text: cell.format_text.clone(),
        box_args: convert_box_args(&cell.box_args),
        text_args: GraphTextArgs {
            x: cell.text_bounds.x,
            y: cell.text_bounds.y,
            width: cell.text_bounds.width,
            height: cell.text_bounds.height,
            text: cell.text.clone(),
            text_align: cell.text_args.text_align as u8,
            text_vertical_align: cell.text_args.text_vertical_align as u8,
            editable: cell.editable,
        },
    }
}

pub(crate) fn convert_row(row: &core_graph_builder::GraphRow) -> GraphRowData {
    GraphRowData {
        index: row.index,
        box_args: convert_box_args(&row.box_args),
        cell_box_args: convert_box_args(&row.cell_box_args),
        cells: row.cells.iter().map(convert_cell).collect(),
    }
}

fn convert_table(table: &core_graph_builder::GraphTable) -> GraphTableData {
    GraphTableData {
        columns: table.columns.iter().map(convert_cell).collect(),
        rows: table.rows.iter().map(convert_row).collect(),
        header_height: table.header_height,
        total_height: table.total_height,
        view_height: table.view_height,
        row_height: table.row_height,
    }
}

pub(crate) fn convert_node(node: &core_graph_builder::GraphNode) -> GraphNodeData {
    GraphNodeData {
        render_handle: node.render_handle,
        kind: node.kind as i32,
        path: convert_path(&node.path),
        depth: node.depth,
        box_args: GraphBoxArgs {
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            corner_radius: 0,
        },
        meta: Some(convert_cell(&node.meta)),
        rows: node.rows.iter().map(convert_row).collect(),
        table: node.table.as_ref().map(convert_table),
    }
}
pub(crate) fn convert_edge(edge: &core_graph_builder::GraphEdge) -> GraphEdgeData {
    GraphEdgeData {
        from_render_handle: edge.from_render_handle,
        from_kind: 1,
        from_path: convert_path(&edge.from_key.path),
        from_row: edge.from_row,
        to_render_handle: edge.to_render_handle,
        to_kind: 1,
        to_path: convert_path(&edge.to_key.path),
        to_row: edge.to_row,
        bezier_args: convert_bezier_args(&edge.bezier_args),
        bezier_from_x: edge.bezier_args.from_x,
        bezier_from_y: edge.bezier_args.from_y,
        bezier_c1x: edge.bezier_args.c1x,
        bezier_c1y: edge.bezier_args.c1y,
        bezier_c2x: edge.bezier_args.c2x,
        bezier_c2y: edge.bezier_args.c2y,
        bezier_to_x: edge.bezier_args.to_x,
        bezier_to_y: edge.bezier_args.to_y,
    }
}

pub(crate) fn model_to_graph_delta(model: &core_graph_builder::GraphModel) -> GraphDelta {
    GraphDelta {
        nodes_added: model.nodes.iter().map(convert_node).collect(),
        nodes_updated: Vec::new(),
        nodes_removed: Vec::new(),
        edges_added: model.edges.iter().map(convert_edge).collect(),
        edges_removed: Vec::new(),
        ..Default::default()
    }
}

pub(crate) fn to_document_graph_delta(delta: &graph_builder_preorder::GraphDelta) -> GraphDelta {
    let table_patches: Vec<TablePatch> = delta
        .table_cell_patches
        .iter()
        .map(|patch| TablePatch::CellsUpdated {
            table_handle: patch.table_render_handle,
            cells: vec![TableCellPatchData {
                row_index: patch.row_index,
                column_index: patch.column_index,
                cell: convert_cell(&patch.cell),
            }],
        })
        .collect();
    GraphDelta {
        nodes_added: delta.nodes_added.iter().map(convert_node).collect(),
        nodes_updated: delta.nodes_updated.iter().map(convert_node).collect(),
        nodes_removed: delta.nodes_removed.clone(),
        edges_added: delta.edges_added.iter().map(convert_edge).collect(),
        edges_removed: delta
            .edges_removed
            .iter()
            .map(|edge| GraphEdgeRemoved {
                from: edge.from_render_handle,
                to: edge.to_render_handle,
            })
            .collect(),
        table_patches,
        ..Default::default()
    }
}

pub(crate) fn build_initial_projection_delta(
    store: &TreeStore,
    root: NodeId,
    language: &str,
    document_key: Option<&str>,
) -> GraphDelta {
    match build_graph_model_for_tree_with_runtime_state(store, root, language) {
        Ok(build) => {
            let delta = model_to_graph_delta(&build.model);
            if let Some(key) = document_key {
                store_projection_model_cache_with_runtime_state(
                    key,
                    build.model,
                    build.topology,
                    build.layout_state,
                );
            }
            delta
        }
        Err(_) => GraphDelta::default(),
    }
}

/// Build graph delta for hover subgraph previews (table rows expanded as individual nodes).
pub(crate) fn build_hover_subgraph_delta(
    store: &TreeStore,
    root: NodeId,
    language: &str,
) -> GraphDelta {
    let mut config = projection_builder_config().to_graph_builder_config();
    config.table_max_height = i32::MAX;
    config.expand_header_table_rows = true;
    let adapter = super::full_layout_adapter::FullLayoutAdapter::new(
        config,
        graph_language_from_name(language),
    );
    adapter
        .build_model(store, root, &[])
        .map(|model| model_to_graph_delta(&model))
        .unwrap_or_default()
}

/// Build a full graph model for the supplied subtree.
///
/// Complexity contract: this is a rebuild path, not an incremental one. The
/// cost should stay proportional to the subtree size:
/// `O(subtree_nodes + emitted_graph_nodes_and_edges)`.
/// Do not call it from per-cell/per-span hot paths.
#[cfg(test)]
pub(crate) fn build_graph_model_for_tree(
    store: &TreeStore,
    root: NodeId,
    language: &str,
) -> Result<core_graph_builder::GraphModel, String> {
    build_graph_model_for_subtree(store, root, language, &[])
}

#[cfg(test)]
pub(crate) fn build_graph_model_for_subtree(
    store: &TreeStore,
    root: NodeId,
    language: &str,
    root_path: &[core_graph_builder::PathSeg],
) -> Result<core_graph_builder::GraphModel, String> {
    let config = projection_builder_config().to_graph_builder_config();
    let adapter = super::full_layout_adapter::FullLayoutAdapter::new(
        config,
        graph_language_from_name(language),
    );
    adapter.build_model(store, root, root_path)
}

pub(crate) fn build_graph_model_for_tree_with_runtime_state(
    store: &TreeStore,
    root: NodeId,
    language: &str,
) -> Result<super::full_layout_adapter::FullGraphBuild, String> {
    let config = projection_builder_config().to_graph_builder_config();
    let adapter = super::full_layout_adapter::FullLayoutAdapter::new(
        config,
        graph_language_from_name(language),
    );
    adapter.build_model_with_runtime_state(store, root, &[])
}
pub(crate) fn graph_language_from_name(language: &str) -> core_graph_builder::GraphLanguage {
    let normalized = language.trim().to_ascii_lowercase();
    match lang_from_name(&normalized).map(|spec| spec.name) {
        Some("json") => core_graph_builder::GraphLanguage::Json,
        Some("yaml") => core_graph_builder::GraphLanguage::Yaml,
        Some("toml") => core_graph_builder::GraphLanguage::Toml,
        _ => core_graph_builder::GraphLanguage::Json,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{Decode, JsonDecoder};

    #[test]
    fn initial_projection_preserves_scalar_cell_editable_flags() {
        let decoded = JsonDecoder
            .decode_str(r#"{"user":{"name":"Alice","role":"admin"},"count":42}"#)
            .expect("json fixture should decode");
        let delta = build_initial_projection_delta(&decoded.store, decoded.root, "json", None);
        let cell = delta
            .nodes_added
            .iter()
            .flat_map(|node| node.rows.iter())
            .flat_map(|row| row.cells.iter())
            .find(|cell| {
                cell.text == "Alice"
                    && cell.path.len() == 2
                    && cell.path[0].tag == 0
                    && cell.path[0].key == "user"
                    && cell.path[1].tag == 0
                    && cell.path[1].key == "name"
            })
            .expect("user.name scalar value cell should be present");

        assert!(cell.text_args.editable);
    }
}
