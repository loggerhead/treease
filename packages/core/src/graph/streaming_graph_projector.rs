use std::collections::{HashMap, HashSet};

use crate::document::protocol::{
    GraphBoxArgs, GraphCellData, GraphDelta, GraphNodeData, GraphPathSeg, GraphRowData,
    LayoutPatch, TableCellPatchData, TablePatch,
};
use crate::document::snapshot::IncrementalState;
use crate::graph::graph_materialize::materialize_into_current_model;
use crate::graph::graph_model_index::GraphModelIndex;
use crate::graph::graph_projection_service;
use crate::graph::graph_shape::NodeShapeBuilder;
use crate::graph::graph_topology::{self, GraphTopology, SequencePresentationState};
use crate::layout::layout_engine::{LayoutEngine, LayoutState};
use crate::stream::tree_patch::TreePatch;
use crate::tree::tree_node::NodeId;
use crate::tree::tree_store::TreeStore;

#[cfg(test)]
use super::graph_builder::PathSeg;
use super::graph_builder::{GraphCell, GraphLanguage, GraphModel, GraphRow};
use super::graph_projection_service::{
    convert_box_args, convert_cell, convert_cell_value, convert_path, convert_table,
    graph_language_from_name, str_sem_type_to_u32,
};
use super::streaming_delta_differ::StreamingDeltaDiffer;

/// Build a single column-aligned row from a Mapping NodeId in the streaming

/// Stable key for identifying nodes across chunk boundaries: joined path segments.
#[cfg(test)]
fn path_key(node: &GraphNodeData) -> String {
    node.path
        .iter()
        .map(|s| match s.tag {
            0 => s.key.clone(),
            _ => format!("[{}]", s.index),
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn pending_sequence_presentation(store: &TreeStore, id: NodeId) -> bool {
    matches!(
        graph_topology::sequence_presentation_state(store, id),
        Some(SequencePresentationState::EmptyOpen)
    )
}

fn pending_header_table_schema(store: &TreeStore, id: NodeId) -> bool {
    matches!(
        graph_topology::sequence_presentation_state(store, id),
        Some(SequencePresentationState::PendingHeaderSchema)
    )
}

fn contains_pending_header_table_schema(store: &TreeStore, id: NodeId) -> bool {
    if pending_sequence_presentation(store, id) || pending_header_table_schema(store, id) {
        return true;
    }
    let Some(node) = store.get(id) else {
        return false;
    };
    node.content
        .iter()
        .any(|child| contains_pending_header_table_schema(store, *child))
}

fn push_layout_patch_unique(patches: &mut Vec<LayoutPatch>, patch: LayoutPatch) {
    if !patches.contains(&patch) {
        patches.push(patch);
    }
}

fn table_state_key_from_graph_node(node: &super::graph_builder::GraphNode) -> TableStateKey {
    if node.stable_id != 0 {
        TableStateKey::Stable(node.stable_id)
    } else {
        TableStateKey::Render(node.render_handle)
    }
}

fn table_state_key_for_render_handle(model: &GraphModel, handle: u32) -> TableStateKey {
    model
        .nodes
        .get(handle as usize)
        .map(table_state_key_from_graph_node)
        .unwrap_or(TableStateKey::Render(handle))
}

fn table_state_key_from_node_data(model: &GraphModel, node: &GraphNodeData) -> TableStateKey {
    table_state_key_for_render_handle(model, node.render_handle)
}

fn cell_fingerprint_from_graph_cell(cell: &GraphCell) -> CellFingerprint {
    CellFingerprint {
        sem_type: str_sem_type_to_u32(cell.sem_type.as_deref()),
        is_missing: cell.is_missing,
        path: convert_path(&cell.path),
        text: cell.text.clone(),
        value: convert_cell_value(cell),
        format_text: cell.format_text.clone(),
        box_args: convert_box_args(&cell.box_args),
        text_args: TextArgsFingerprint {
            x: cell.text_bounds.x,
            y: cell.text_bounds.y,
            width: cell.text_bounds.width,
            height: cell.text_bounds.height,
            text_align: cell.text_args.text_align as u8,
            text_vertical_align: cell.text_args.text_vertical_align as u8,
            editable: cell.editable,
        },
    }
}

fn cell_fingerprint_from_node_data(cell: &GraphCellData) -> CellFingerprint {
    CellFingerprint {
        sem_type: cell.sem_type,
        is_missing: cell.is_missing,
        path: cell.path.clone(),
        text: cell.text.clone(),
        value: cell.value.clone(),
        format_text: cell.format_text.clone(),
        box_args: cell.box_args,
        text_args: TextArgsFingerprint {
            x: cell.text_args.x,
            y: cell.text_args.y,
            width: cell.text_args.width,
            height: cell.text_args.height,
            text_align: cell.text_args.text_align,
            text_vertical_align: cell.text_args.text_vertical_align,
            editable: cell.text_args.editable,
        },
    }
}

fn convert_row_and_store_fingerprints(
    row: &GraphRow,
    row_index: usize,
    cells: &mut HashMap<(usize, usize), CellFingerprint>,
) -> GraphRowData {
    let mut converted_cells = Vec::with_capacity(row.cells.len());
    for (column_index, cell) in row.cells.iter().enumerate() {
        cells.insert(
            (row_index, column_index),
            cell_fingerprint_from_graph_cell(cell),
        );
        converted_cells.push(convert_cell(cell));
    }
    GraphRowData {
        index: row.index,
        box_args: convert_box_args(&row.box_args),
        cell_box_args: convert_box_args(&row.cell_box_args),
        cells: converted_cells,
    }
}

#[derive(Debug)]
pub struct ProjectionUpdate {
    pub delta: GraphDelta,
    pub patch_seq: u64,
    pub base_graph_version: u64,
    pub graph_version: u64,
    /// Patch budget counters for monitoring linearity.
    pub new_nodes: usize,
    pub updated_nodes: usize,
    pub new_edges: usize,
    pub removed_edges: usize,
    pub rows_appended: usize,
    pub cells_updated: usize,
    pub layout_summaries: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TableStateKey {
    Stable(u64),
    Render(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextArgsFingerprint {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    text_align: u8,
    text_vertical_align: u8,
    editable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CellFingerprint {
    sem_type: u32,
    is_missing: bool,
    path: Vec<GraphPathSeg>,
    text: String,
    value: String,
    format_text: String,
    box_args: GraphBoxArgs,
    text_args: TextArgsFingerprint,
}

/// Persistent table/layout patch state keyed by stable graph identity.
#[derive(Debug, Clone)]
struct TableState {
    row_count: usize,
    columns: Vec<GraphCellData>,
    cells: HashMap<(usize, usize), CellFingerprint>,
    box_args: crate::document::protocol::GraphBoxArgs,
    deferred_replacement: bool,
}

impl TableState {
    fn new(
        row_count: usize,
        columns: Vec<GraphCellData>,
        cells: HashMap<(usize, usize), CellFingerprint>,
        box_args: crate::document::protocol::GraphBoxArgs,
    ) -> Self {
        Self {
            row_count,
            columns,
            cells,
            box_args,
            deferred_replacement: false,
        }
    }

    fn with_deferred_replacement(mut self, deferred_replacement: bool) -> Self {
        self.deferred_replacement = deferred_replacement;
        self
    }
}

#[derive(Debug, Default)]
struct ProjectionPatchPlan {
    skip_updated_table_handles: HashSet<u32>,
    full_rebuild_table_handles: Vec<u32>,
    deferred_rebuilt_table_handles: HashSet<u32>,
    finalized_table_handles: HashSet<u32>,
    layout_patches: Vec<LayoutPatch>,
    skipped_table_rows: usize,
    skipped_table_cells: usize,
}

#[derive(Debug)]
pub struct StreamingGraphProjector {
    language: GraphLanguage,
    previous_model: Option<GraphModel>,
    topology: GraphTopology,
    layout_state: LayoutState,
    patch_seq: u64,
    graph_version: u64,
    table_states: HashMap<TableStateKey, TableState>,
}

impl StreamingGraphProjector {
    pub fn new(language_name: &str) -> Self {
        Self {
            language: graph_language_from_name(language_name),
            previous_model: None,
            topology: GraphTopology::new(),
            layout_state: LayoutState::default(),
            patch_seq: 0,
            graph_version: 0,
            table_states: HashMap::new(),
        }
    }

    pub(crate) fn from_incremental_state(
        language_name: &str,
        state: &IncrementalState,
    ) -> Option<Self> {
        let previous_model = state
            .graph_model_snapshot
            .as_ref()
            .map(|snapshot| snapshot.materialize())?;
        let topology = state.graph_topology().cloned()?;
        let layout_state = state.layout_state().cloned()?;
        let table_states = table_states_from_model(&previous_model);
        Some(Self {
            language: graph_language_from_name(language_name),
            previous_model: Some(previous_model),
            topology,
            layout_state,
            patch_seq: 0,
            graph_version: 0,
            table_states,
        })
    }

    pub fn update(
        &mut self,
        store: &TreeStore,
        root: NodeId,
        patches: &[TreePatch],
    ) -> Option<ProjectionUpdate> {
        if self.previous_model.is_none() && contains_pending_header_table_schema(store, root) {
            return Some(self.ver(GraphDelta::default()));
        }
        let cfg = graph_projection_service::projection_builder_config().to_graph_builder_config();
        let dirty = if self.previous_model.is_none() && patches.is_empty() {
            self.topology.build_full(store, root, &cfg)
        } else {
            self.topology.apply(store, root, patches, &cfg)
        };
        if dirty.is_empty() {
            return Some(self.ver(GraphDelta::default()));
        }

        let mut model = self.previous_model.take().unwrap_or_default();
        let shape_builder = NodeShapeBuilder::new(&cfg, self.language);
        let materialized = materialize_into_current_model(
            &mut self.topology,
            &mut model,
            store,
            &dirty,
            &shape_builder,
            &cfg,
        );

        let root_handle = self.topology.root_handle().unwrap_or(0);
        let mut layout_seeds = Vec::new();
        layout_seeds.extend_from_slice(&materialized.added_handles);
        layout_seeds.extend_from_slice(&materialized.updated_handles);
        layout_seeds.sort_unstable();
        layout_seeds.dedup();
        let layout = LayoutEngine::new(cfg).layout_changed_region(
            &mut self.layout_state,
            &self.topology,
            &mut model,
            root_handle,
            &layout_seeds,
            &materialized.added_edges,
            &materialized.added_edge_indexes,
        );
        let patch_plan = self.build_projection_patch_plan(
            store,
            patches,
            &model,
            &materialized,
            &layout,
            &dirty,
        );
        let raw_delta = StreamingDeltaDiffer::new().emit_incremental_delta(
            &model,
            &materialized,
            &layout,
            &patch_plan.skip_updated_table_handles,
        );
        let mut delta = self.split_patches(
            &model,
            raw_delta,
            &patch_plan.full_rebuild_table_handles,
            &patch_plan.layout_patches,
        );
        let table_patches = self.collect_table_patches(&model, &materialized, &patch_plan);
        self.previous_model = Some(model);
        delta.table_patches.extend(table_patches);
        Some(self.ver(delta))
    }

    fn build_projection_patch_plan(
        &self,
        store: &TreeStore,
        patches: &[TreePatch],
        model: &GraphModel,
        materialized: &crate::graph::graph_materialize::MaterializedGraphPatch,
        layout: &crate::layout::layout_engine::LayoutChangeSet,
        dirty: &graph_topology::DirtySet,
    ) -> ProjectionPatchPlan {
        let sealed_nodes: HashSet<NodeId> = patches
            .iter()
            .filter_map(|patch| match patch {
                TreePatch::NodeSealed { node_id } => Some(*node_id),
                _ => None,
            })
            .collect();
        let dirty_table_handles: HashSet<u32> = dirty
            .table_rows()
            .iter()
            .map(|row| row.table_handle)
            .collect();

        let added_handles: HashSet<u32> = materialized.added_handles.iter().copied().collect();
        let rebuilt_handles: HashSet<u32> =
            materialized.rebuilt_table_handles.iter().copied().collect();
        let mut candidate_handles: Vec<u32> = materialized.updated_handles.clone();
        candidate_handles.extend(layout.node_handles().iter().copied());
        candidate_handles.sort_unstable();
        candidate_handles.dedup();

        let mut plan = ProjectionPatchPlan::default();
        for &handle in &materialized.rebuilt_table_handles {
            if added_handles.contains(&handle) {
                continue;
            }
            let Some(node) = model.nodes.get(handle as usize) else {
                plan.full_rebuild_table_handles.push(handle);
                continue;
            };
            if node.table.is_none() {
                plan.full_rebuild_table_handles.push(handle);
                continue;
            }
            let key = table_state_key_from_graph_node(node);
            let Some(prev) = self.table_states.get(&key) else {
                plan.full_rebuild_table_handles.push(handle);
                continue;
            };
            let current_box = graph_box_args_from_node(node);
            if current_box != prev.box_args {
                push_layout_patch_unique(
                    &mut plan.layout_patches,
                    LayoutPatch::NodeBoundsUpdated {
                        render_handle: handle,
                        box_args: current_box,
                    },
                );
            }
            plan.skip_updated_table_handles.insert(handle);
            if self.table_is_finalized(store, &sealed_nodes, handle) {
                plan.finalized_table_handles.insert(handle);
            } else {
                plan.deferred_rebuilt_table_handles.insert(handle);
            }
        }

        for &handle in &materialized.deferred_table_handles {
            if added_handles.contains(&handle) {
                continue;
            }
            let Some(node) = model.nodes.get(handle as usize) else {
                continue;
            };
            if node.table.is_none() {
                continue;
            }
            let key = table_state_key_from_graph_node(node);
            let Some(prev) = self.table_states.get(&key) else {
                continue;
            };
            let current_box = graph_box_args_from_node(node);
            if current_box != prev.box_args {
                push_layout_patch_unique(
                    &mut plan.layout_patches,
                    LayoutPatch::NodeBoundsUpdated {
                        render_handle: handle,
                        box_args: current_box,
                    },
                );
            }
            plan.skip_updated_table_handles.insert(handle);
            if self.table_is_finalized(store, &sealed_nodes, handle) {
                plan.finalized_table_handles.insert(handle);
            } else {
                plan.deferred_rebuilt_table_handles.insert(handle);
            }
        }

        if !sealed_nodes.is_empty() {
            for node in &model.nodes {
                if node.table.is_none() || added_handles.contains(&node.render_handle) {
                    continue;
                }
                let key = table_state_key_from_graph_node(node);
                if !self
                    .table_states
                    .get(&key)
                    .is_some_and(|state| state.deferred_replacement)
                {
                    continue;
                }
                if !self.table_is_finalized(store, &sealed_nodes, node.render_handle) {
                    continue;
                }
                let current_box = graph_box_args_from_node(node);
                if let Some(prev) = self.table_states.get(&key)
                    && current_box != prev.box_args
                {
                    push_layout_patch_unique(
                        &mut plan.layout_patches,
                        LayoutPatch::NodeBoundsUpdated {
                            render_handle: node.render_handle,
                            box_args: current_box,
                        },
                    );
                }
                plan.skip_updated_table_handles.insert(node.render_handle);
                plan.finalized_table_handles.insert(node.render_handle);
            }
        }

        plan.full_rebuild_table_handles = materialized
            .rebuilt_table_handles
            .iter()
            .copied()
            .filter(|handle| !plan.skip_updated_table_handles.contains(handle))
            .collect();

        if dirty_table_handles.is_empty() {
            return plan;
        }

        for handle in candidate_handles {
            if added_handles.contains(&handle)
                || rebuilt_handles.contains(&handle)
                || !dirty_table_handles.contains(&handle)
            {
                continue;
            }
            let Some(node) = model.nodes.get(handle as usize) else {
                continue;
            };
            if node.table.is_none() {
                continue;
            }
            let key = table_state_key_from_graph_node(node);
            let Some(prev) = self.table_states.get(&key) else {
                continue;
            };
            let current_box = graph_box_args_from_node(node);
            if current_box.x != prev.box_args.x
                || current_box.y != prev.box_args.y
                || current_box.width != prev.box_args.width
            {
                continue;
            }
            if current_box != prev.box_args {
                push_layout_patch_unique(
                    &mut plan.layout_patches,
                    LayoutPatch::NodeBoundsUpdated {
                        render_handle: handle,
                        box_args: current_box,
                    },
                );
            }
            if let Some(table) = node.table.as_ref() {
                plan.skipped_table_rows += table.rows.len();
                plan.skipped_table_cells +=
                    table.rows.iter().map(|row| row.cells.len()).sum::<usize>();
            }
            plan.skip_updated_table_handles.insert(handle);
        }
        plan
    }

    fn table_is_finalized(
        &self,
        store: &TreeStore,
        sealed_nodes: &HashSet<NodeId>,
        handle: u32,
    ) -> bool {
        let Some(slot) = self.topology.slot(handle) else {
            return false;
        };
        sealed_nodes.contains(&slot.node_id)
            && store
                .get(slot.node_id)
                .is_some_and(|node| node.sequence_closed())
    }

    fn split_patches(
        &mut self,
        model: &GraphModel,
        mut delta: GraphDelta,
        structural_table_handles: &[u32],
        planned_layout_patches: &[LayoutPatch],
    ) -> GraphDelta {
        let mut layout_patches: Vec<LayoutPatch> = Vec::new();
        for patch in planned_layout_patches {
            push_layout_patch_unique(&mut layout_patches, patch.clone());
        }
        let structural_table_handles: HashSet<u32> =
            structural_table_handles.iter().copied().collect();

        let mut kept_added = Vec::with_capacity(delta.nodes_added.len());
        for node in delta.nodes_added.drain(..) {
            if node.table.is_some() {
                kept_added.push(node);
                continue;
            }
            let key = table_state_key_from_node_data(model, &node);
            if let Some(prev) = self.table_states.get(&key) {
                if node.box_args != prev.box_args {
                    push_layout_patch_unique(
                        &mut layout_patches,
                        LayoutPatch::GroupLayoutUpdated {
                            group_handle: node.render_handle,
                            width: node.box_args.width,
                            height: node.box_args.height,
                        },
                    );
                }
            }
            self.table_states
                .insert(key, non_table_state_from_node(&node));
            kept_added.push(node);
        }
        delta.nodes_added = kept_added;

        let mut kept_updated = Vec::with_capacity(delta.nodes_updated.len());
        for node in delta.nodes_updated.drain(..) {
            let key = table_state_key_from_node_data(model, &node);
            let is_structural_table =
                node.table.is_some() && structural_table_handles.contains(&node.render_handle);
            let should_replace_table_for_geometry = node.table.is_some()
                && self.table_states.get(&key).is_some_and(|prev| {
                    node.box_args.x != prev.box_args.x
                        || node.box_args.y != prev.box_args.y
                        || node.box_args.width != prev.box_args.width
                });
            if is_structural_table || should_replace_table_for_geometry {
                if let Some(state) = table_state_from_node_data(&node) {
                    self.table_states.insert(key, state);
                }
                kept_updated.push(node);
                continue;
            }
            if let Some(prev) = self.table_states.get(&key) {
                if node.box_args != prev.box_args {
                    push_layout_patch_unique(
                        &mut layout_patches,
                        LayoutPatch::NodeBoundsUpdated {
                            render_handle: node.render_handle,
                            box_args: node.box_args,
                        },
                    );
                }
            }
            if node.table.is_some() {
                continue;
            }
            self.table_states
                .insert(key, non_table_state_from_node(&node));
            kept_updated.push(node);
        }
        delta.nodes_updated = kept_updated;
        delta.layout_patches = layout_patches;
        delta
    }

    fn collect_table_patches(
        &mut self,
        model: &GraphModel,
        materialized: &crate::graph::graph_materialize::MaterializedGraphPatch,
        plan: &ProjectionPatchPlan,
    ) -> Vec<TablePatch> {
        let mut patches = Vec::new();
        let rebuilt_handles: HashSet<u32> =
            materialized.rebuilt_table_handles.iter().copied().collect();

        for &handle in &materialized.added_handles {
            let Some(node) = model.nodes.get(handle as usize) else {
                continue;
            };
            let Some(table) = node.table.as_ref() else {
                continue;
            };
            let key = table_state_key_from_graph_node(node);
            let mut state = TableState::new(
                table.rows.len(),
                table.columns.iter().map(convert_cell).collect(),
                HashMap::new(),
                graph_box_args_from_node(node),
            );
            patches.push(TablePatch::TableCreated {
                table_handle: handle,
                columns: state.columns.clone(),
            });
            if !table.rows.is_empty() {
                let rows: Vec<_> = table
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(row_index, row)| {
                        convert_row_and_store_fingerprints(row, row_index, &mut state.cells)
                    })
                    .collect();
                patches.push(TablePatch::RowsAppended {
                    table_handle: handle,
                    start_index: 0,
                    rows,
                    total_height: table.total_height,
                    view_height: table.view_height,
                    header_height: table.header_height,
                    row_height: table.row_height,
                });
            }
            self.table_states.insert(key, state);
        }

        let mut finalized_handles: Vec<u32> =
            plan.finalized_table_handles.iter().copied().collect();
        finalized_handles.sort_unstable();
        for table_handle in finalized_handles {
            let Some(node) = model.nodes.get(table_handle as usize) else {
                continue;
            };
            let Some(table) = node.table.as_ref() else {
                continue;
            };
            let key = table_state_key_from_graph_node(node);
            patches.push(TablePatch::TableReplaced {
                table_handle,
                table: convert_table(table),
            });
            self.table_states
                .insert(key, table_state_from_graph_node(node));
        }

        for touch in &materialized.table_row_touches {
            let table_handle = touch.table_handle;
            let Some(node) = model.nodes.get(table_handle as usize) else {
                continue;
            };
            let Some(table) = node.table.as_ref() else {
                continue;
            };
            let key = table_state_key_from_graph_node(node);
            if plan.finalized_table_handles.contains(&table_handle) {
                continue;
            }
            if rebuilt_handles.contains(&table_handle) {
                if !plan.deferred_rebuilt_table_handles.contains(&table_handle) {
                    if !self.table_states.contains_key(&key) {
                        self.table_states
                            .insert(key, table_state_from_graph_node(node));
                    }
                    continue;
                }
                if !self.table_states.contains_key(&key) {
                    self.table_states.insert(
                        key,
                        table_state_from_graph_node(node).with_deferred_replacement(true),
                    );
                    continue;
                }
            }
            let mut state = if let Some(state) = self.table_states.remove(&key) {
                state
            } else {
                table_state_from_graph_node(node)
            };

            let current_columns: Vec<GraphCellData> =
                table.columns.iter().map(convert_cell).collect();
            let defer_structure = plan.deferred_rebuilt_table_handles.contains(&table_handle);
            if !defer_structure && current_columns.len() > state.columns.len() {
                patches.push(TablePatch::ColumnsAdded {
                    table_handle,
                    columns: current_columns[state.columns.len()..].to_vec(),
                });
                let mut new_cells = Vec::new();
                let comparable_rows = state.row_count.min(table.rows.len());
                for row_index in 0..comparable_rows {
                    let Some(row) = table.rows.get(row_index) else {
                        continue;
                    };
                    for column_index in state.columns.len()..row.cells.len() {
                        if let Some(cell) = row.cells.get(column_index) {
                            let fingerprint = cell_fingerprint_from_graph_cell(cell);
                            let converted = convert_cell(cell);
                            state.cells.insert((row_index, column_index), fingerprint);
                            new_cells.push(TableCellPatchData {
                                row_index: row_index as u32,
                                column_index: column_index as u32,
                                cell: converted,
                            });
                        }
                    }
                }
                if !new_cells.is_empty() {
                    patches.push(TablePatch::CellsUpdated {
                        table_handle,
                        cells: new_cells,
                    });
                }
                state.columns = current_columns;
            }

            let dirty_scan_row_limit = state.row_count;
            if table.rows.len() > state.row_count {
                let start_index = state.row_count;
                let rows: Vec<_> = table.rows[start_index..]
                    .iter()
                    .enumerate()
                    .map(|(offset, row)| {
                        let row_index = start_index + offset;
                        convert_row_and_store_fingerprints(row, row_index, &mut state.cells)
                    })
                    .collect();
                patches.push(TablePatch::RowsAppended {
                    table_handle,
                    start_index: start_index as u32,
                    rows,
                    total_height: table.total_height,
                    view_height: table.view_height,
                    header_height: table.header_height,
                    row_height: table.row_height,
                });
                state.row_count = table.rows.len();
            }

            if !defer_structure {
                let mut row_indexes = touch.row_indexes.clone();
                row_indexes.sort_unstable();
                row_indexes.dedup();
                for row_index in row_indexes {
                    if row_index >= dirty_scan_row_limit || row_index >= table.rows.len() {
                        continue;
                    }
                    let Some(row) = table.rows.get(row_index) else {
                        continue;
                    };
                    let mut cells = Vec::new();
                    for (column_index, cell) in row.cells.iter().enumerate() {
                        let fingerprint = cell_fingerprint_from_graph_cell(cell);
                        let key = (row_index, column_index);
                        if state.cells.get(&key) != Some(&fingerprint) {
                            let converted = convert_cell(cell);
                            state.cells.insert(key, fingerprint);
                            cells.push(TableCellPatchData {
                                row_index: row_index as u32,
                                column_index: column_index as u32,
                                cell: converted,
                            });
                        }
                    }
                    if !cells.is_empty() {
                        patches.push(TablePatch::CellsUpdated {
                            table_handle,
                            cells,
                        });
                    }
                }
            }

            state.box_args = graph_box_args_from_node(node);
            state.deferred_replacement |= defer_structure;
            self.table_states.insert(key, state);
        }

        patches
    }

    fn ver(&mut self, d: GraphDelta) -> ProjectionUpdate {
        let b = self.graph_version;
        let new_nodes = d.nodes_added.len();
        let updated_nodes = d.nodes_updated.len();
        let new_edges = d.edges_added.len();
        let removed_edges = d.edges_removed.len();
        let rows_appended = d
            .table_patches
            .iter()
            .filter_map(|p| {
                if let TablePatch::RowsAppended { rows, .. } = p {
                    Some(rows.len())
                } else {
                    None
                }
            })
            .sum();
        let cells_updated = d
            .table_patches
            .iter()
            .filter(|p| matches!(p, TablePatch::CellsUpdated { .. }))
            .count();
        let layout_summaries = d.layout_patches.len();
        self.graph_version += 1;
        self.patch_seq += 1;
        ProjectionUpdate {
            delta: d,
            patch_seq: self.patch_seq,
            base_graph_version: b,
            graph_version: self.graph_version,
            new_nodes,
            updated_nodes,
            new_edges,
            removed_edges,
            rows_appended,
            cells_updated,
            layout_summaries,
        }
    }

    pub fn finalize_layout(&mut self) -> Option<GraphDelta> {
        let model = self.previous_model.as_ref()?;
        if model.nodes.is_empty() {
            return None;
        }

        None
    }

    pub fn take_incremental_state(&mut self) -> Option<IncrementalState> {
        let model = self.previous_model.take()?;
        let index = GraphModelIndex::build(&model);
        Some(
            IncrementalState::resumable()
                .with_graph_state(
                    crate::graph::graph_builder::GraphModelSnapshot::owned(model),
                    index,
                )
                .with_graph_runtime_state(self.topology.clone(), self.layout_state.clone()),
        )
    }
}

fn non_table_state_from_node(node: &GraphNodeData) -> TableState {
    TableState::new(0, Vec::new(), HashMap::new(), node.box_args)
}

fn table_states_from_model(model: &GraphModel) -> HashMap<TableStateKey, TableState> {
    model
        .nodes
        .iter()
        .map(|node| {
            let key = table_state_key_from_graph_node(node);
            let state = if node.table.is_some() {
                table_state_from_graph_node(node)
            } else {
                non_table_state_from_graph_node(node)
            };
            (key, state)
        })
        .collect()
}

fn non_table_state_from_graph_node(node: &super::graph_builder::GraphNode) -> TableState {
    TableState::new(
        0,
        Vec::new(),
        HashMap::new(),
        graph_box_args_from_node(node),
    )
}

fn table_state_from_graph_node(node: &super::graph_builder::GraphNode) -> TableState {
    let table = node
        .table
        .as_ref()
        .expect("table state requires table node");
    let columns: Vec<GraphCellData> = table.columns.iter().map(convert_cell).collect();
    let mut cells = HashMap::new();
    for (row_index, row) in table.rows.iter().enumerate() {
        for (column_index, cell) in row.cells.iter().enumerate() {
            cells.insert(
                (row_index, column_index),
                cell_fingerprint_from_graph_cell(cell),
            );
        }
    }
    TableState::new(
        table.rows.len(),
        columns,
        cells,
        graph_box_args_from_node(node),
    )
}

fn table_state_from_node_data(node: &GraphNodeData) -> Option<TableState> {
    let table = node.table.as_ref()?;
    let columns = table.columns.clone();
    let mut cells = HashMap::new();
    for (row_index, row) in table.rows.iter().enumerate() {
        for (column_index, cell) in row.cells.iter().enumerate() {
            cells.insert(
                (row_index, column_index),
                cell_fingerprint_from_node_data(cell),
            );
        }
    }
    Some(TableState::new(
        table.rows.len(),
        columns,
        cells,
        node.box_args,
    ))
}

fn graph_box_args_from_node(
    node: &super::graph_builder::GraphNode,
) -> crate::document::protocol::GraphBoxArgs {
    crate::document::protocol::GraphBoxArgs {
        x: node.box_args.x,
        y: node.box_args.y,
        width: node.box_args.width,
        height: node.box_args.height,
        corner_radius: node.box_args.corner_radius,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::layout_engine::LayoutMetrics;
    use crate::stream::tree_builder::Builder;

    impl StreamingGraphProjector {
        fn last_layout_metrics(&self) -> LayoutMetrics {
            self.layout_state.metrics()
        }
    }

    fn dec(s: &str) -> Vec<crate::stream::streaming_events::StreamingEvent> {
        crate::stream::decode("json", s).unwrap()
    }

    fn protocol_edge_key(edge: &crate::document::protocol::GraphEdgeData) -> (u32, i32, u32, i32) {
        (
            edge.from_render_handle,
            edge.from_row,
            edge.to_render_handle,
            edge.to_row,
        )
    }

    fn apply_streaming_delta_to_consumer(
        nodes: &mut HashMap<u32, crate::document::protocol::GraphNodeData>,
        edges: &mut HashMap<(u32, i32, u32, i32), crate::document::protocol::GraphEdgeData>,
        delta: &GraphDelta,
    ) {
        for handle in &delta.nodes_removed {
            nodes.remove(handle);
        }
        for node in &delta.nodes_added {
            nodes.insert(node.render_handle, node.clone());
        }
        for node in &delta.nodes_updated {
            nodes.insert(node.render_handle, node.clone());
        }
        for removed in &delta.edges_removed {
            edges.retain(|(from, _, to, _), _| *from != removed.from || *to != removed.to);
        }
        for edge in &delta.edges_added {
            edges.insert(protocol_edge_key(edge), edge.clone());
        }
        for patch in &delta.layout_patches {
            if let LayoutPatch::NodeBoundsUpdated {
                render_handle,
                box_args,
            } = patch
            {
                if let Some(node) = nodes.get_mut(render_handle) {
                    node.box_args = *box_args;
                }
            }
        }
    }

    #[test]
    fn take_incremental_state_moves_projector_model_for_snapshot_resume() {
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");

        decoder.feed(r#"{"a":1,"b":{"c":2}}"#).unwrap();
        for event in &decoder.take_events() {
            builder.push(event).unwrap();
        }
        let patches = builder.take_patches();
        let (store, root) = builder.tree_ref().unwrap();
        projector.update(store, root, &patches).unwrap();
        projector.finalize_layout();

        let state = projector
            .take_incremental_state()
            .expect("projector model should become snapshot incremental state");
        assert!(state.can_resume);
        assert!(state.structural_safe);
        assert!(state.graph_model_snapshot.is_some());
        assert!(state.graph_model_index.is_some());
        assert!(state.graph_topology().is_some());
        assert!(state.layout_state().is_some());
    }

    #[test]
    fn streaming_sequence_waits_for_first_mapping_schema_before_initial_projection() {
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");

        d.feed(r#"{"rows":["#).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let patches = b.take_patches();
        let (s, r) = b.tree_ref().unwrap();
        let pending = p
            .update(s, r, &patches)
            .expect("pending sequence should still emit a clear projection heartbeat");
        assert!(
            pending.delta.nodes_added.is_empty()
                && pending.delta.edges_added.is_empty()
                && pending.delta.table_patches.is_empty(),
            "an open empty sequence must not publish a graph shape before its presentation is known"
        );

        d.feed(r#"{"a":1,"#).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let patches = b.take_patches();
        let (s, r) = b.tree_ref().unwrap();
        let pending_header_schema = p
            .update(s, r, &patches)
            .expect("partial first mapping should keep the table schema pending");
        assert!(
            pending_header_schema.delta.table_patches.is_empty(),
            "a partial first mapping must not commit a header table before its direct values are known"
        );

        d.feed(r#""b":2}"#).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let patches = b.take_patches();
        let (s, r) = b.tree_ref().unwrap();
        let update = p
            .update(s, r, &patches)
            .expect("closed scalar-only first mapping should publish a header table");
        let columns = update
            .delta
            .table_patches
            .iter()
            .find_map(|patch| match patch {
                TablePatch::TableCreated { columns, .. } => Some(columns),
                _ => None,
            })
            .expect("closed scalar-only first mapping should create columns");
        assert!(
            columns.iter().any(|column| column.text == "a"),
            "first key must be published as a table column"
        );
    }

    #[test]
    fn streaming_first_mapping_with_nested_value_publishes_only_a_headerless_table() {
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");

        d.feed(r#"{"rows":[{"a":1,"#).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let patches = b.take_patches();
        let (s, r) = b.tree_ref().unwrap();
        let pending = p.update(s, r, &patches).unwrap();
        assert!(pending.delta.table_patches.is_empty());

        d.feed(r#""nested":{}"#).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let patches = b.take_patches();
        let (s, r) = b.tree_ref().unwrap();
        let headerless = p.update(s, r, &patches).unwrap();
        assert!(headerless.delta.table_patches.iter().any(|patch| matches!(
            patch,
            TablePatch::RowsAppended {
                header_height: 0,
                ..
            }
        )));

        d.feed("}]}").unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let patches = b.take_patches();
        let (s, r) = b.tree_ref().unwrap();
        let completed = p.update(s, r, &patches).unwrap();
        assert!(
            !completed.delta.table_patches.iter().any(|patch| matches!(
                patch,
                TablePatch::TableReplaced { table, .. } if table.header_height > 0
            )),
            "a first mapping that contains a nested value must never flip to a header table"
        );
    }

    #[test]
    fn streaming_object_key_split_then_table_value_emits_incremental_projection() {
        let source = r#"{"batches":[{"index":0}]}"#;
        let split = 5;
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");

        d.feed(&source[..split]).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let patches = b.take_patches();
        let (s, r) = b.tree_ref().unwrap();
        let first = p
            .update(s, r, &patches)
            .expect("first partial object should publish a baseline");
        assert!(
            first.delta.nodes_added.len() <= 1,
            "partial object key must not publish descendant graph shape"
        );

        d.feed(&source[split..]).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let patches = b.take_patches();
        let (s, r) = b.tree_ref().unwrap();
        let second = p
            .update(s, r, &patches)
            .expect("completed key and table value should produce an incremental projection");
        assert!(
            !second.delta.nodes_added.is_empty()
                || !second.delta.edges_added.is_empty()
                || !second.delta.table_patches.is_empty(),
            "completed key and table value should publish graph/table changes"
        );
    }

    #[test]
    fn streaming_headerless_scalar_rows_append_without_rebuilding_existing_nodes() {
        let chunks = [r#"{"items":[1,"#, r#"2,"#];
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");
        let mut updates = Vec::new();

        for chunk in chunks {
            d.feed(chunk).unwrap();
            for e in &d.take_events() {
                b.push(e).unwrap();
            }
            let patches = b.take_patches();
            if let Some((s, r)) = b.tree_ref() {
                if let Some(update) = p.update(s, r, &patches) {
                    updates.push(update);
                }
            }
        }

        let second = updates
            .last()
            .expect("second scalar row should produce a delta");
        assert_eq!(
            second.new_nodes, 0,
            "inline headerless scalar rows must append table rows, not graph nodes"
        );
        assert_eq!(
            second.updated_nodes, 0,
            "appending an inline row must not rebuild existing graph nodes"
        );
        assert_eq!(
            second.removed_edges, 0,
            "appending an inline row must not diff and replace existing edges"
        );
        assert_eq!(
            second.rows_appended, 1,
            "second scalar should append exactly one table row"
        );
    }

    #[test]
    fn streaming_consumer_keeps_same_depth_x_when_root_width_grows_after_existing_table() {
        let chunks = [
            r#"{"a":[{"id":0},{"id":1},{"id":2}]"#,
            r#","this_is_a_very_long_root_key_that_forces_depth_one_column_to_move":{"x":1}}"#,
        ];
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");
        let mut consumer_nodes: HashMap<u32, crate::document::protocol::GraphNodeData> =
            HashMap::new();
        let mut consumer_edges: HashMap<
            (u32, i32, u32, i32),
            crate::document::protocol::GraphEdgeData,
        > = HashMap::new();

        for chunk in chunks {
            decoder.feed(chunk).unwrap();
            for event in &decoder.take_events() {
                builder.push(event).unwrap();
            }
            let patches = builder.take_patches();
            let (store, root) = builder.tree_ref().expect("streaming tree");
            let update = projector
                .update(store, root, &patches)
                .expect("projection update");
            apply_streaming_delta_to_consumer(
                &mut consumer_nodes,
                &mut consumer_edges,
                &update.delta,
            );
        }

        let mut x_by_depth: HashMap<u32, i32> = HashMap::new();
        for node in consumer_nodes.values() {
            if let Some(previous_x) = x_by_depth.insert(node.depth, node.box_args.x) {
                assert_eq!(
                    node.box_args.x,
                    previous_x,
                    "consumer-visible nodes at depth {} must share one x column; nodes={:?}",
                    node.depth,
                    consumer_nodes
                        .values()
                        .map(|node| (
                            node.render_handle,
                            path_key(node),
                            node.depth,
                            node.box_args.x
                        ))
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn streaming_consumer_refreshes_existing_edge_geometry_when_node_bounds_change() {
        let chunks = [
            r#"{"a":[{"id":0},{"id":1},{"id":2}]"#,
            r#","this_is_a_very_long_root_key_that_forces_depth_one_column_to_move":{"x":1}}"#,
        ];
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");
        let mut consumer_nodes: HashMap<u32, crate::document::protocol::GraphNodeData> =
            HashMap::new();
        let mut consumer_edges: HashMap<
            (u32, i32, u32, i32),
            crate::document::protocol::GraphEdgeData,
        > = HashMap::new();

        for chunk in chunks {
            decoder.feed(chunk).unwrap();
            for event in &decoder.take_events() {
                builder.push(event).unwrap();
            }
            let patches = builder.take_patches();
            let (store, root) = builder.tree_ref().expect("streaming tree");
            let update = projector
                .update(store, root, &patches)
                .expect("projection update");
            apply_streaming_delta_to_consumer(
                &mut consumer_nodes,
                &mut consumer_edges,
                &update.delta,
            );
        }

        for edge in consumer_edges.values() {
            let from = consumer_nodes
                .get(&edge.from_render_handle)
                .expect("edge parent node");
            let to = consumer_nodes
                .get(&edge.to_render_handle)
                .expect("edge child node");
            assert_eq!(
                edge.bezier_args.from_x,
                from.box_args.x + from.box_args.width,
                "edge {:?}->{:?} must start at the current parent right edge",
                path_key(from),
                path_key(to),
            );
            assert_eq!(
                edge.bezier_args.to_x,
                to.box_args.x,
                "edge {:?}->{:?} must end at the current child left edge",
                path_key(from),
                path_key(to),
            );
        }
    }

    #[test]
    fn streaming_trajectory_edges_converge_to_current_node_columns() {
        let source = include_str!("../../../../test/fixtures/json/trajectory.1.json");
        let bytes = source.as_bytes();
        let chunk_size = 64 * 1024;
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");
        let mut consumer_nodes: HashMap<u32, crate::document::protocol::GraphNodeData> =
            HashMap::new();
        let mut consumer_edges: HashMap<
            (u32, i32, u32, i32),
            crate::document::protocol::GraphEdgeData,
        > = HashMap::new();

        let mut offset = 0;
        while offset < bytes.len() {
            let end = (offset + chunk_size).min(bytes.len());
            let chunk_end = if end < bytes.len() {
                let mut boundary = end;
                while boundary > offset && bytes[boundary] & 0xC0 == 0x80 {
                    boundary -= 1;
                }
                boundary
            } else {
                end
            };
            let chunk = std::str::from_utf8(&bytes[offset..chunk_end]).expect("utf-8 fixture");
            decoder.feed(chunk).expect("trajectory chunk should decode");
            for event in &decoder.take_events() {
                builder.push(event).expect("trajectory event should build");
            }
            let patches = builder.take_patches();
            let (store, root) = builder.tree_ref().expect("streaming trajectory tree");
            let update = projector
                .update(store, root, &patches)
                .expect("trajectory projection update");
            apply_streaming_delta_to_consumer(
                &mut consumer_nodes,
                &mut consumer_edges,
                &update.delta,
            );
            offset = chunk_end;
        }

        for event in decoder
            .finish_events()
            .expect("trajectory stream should finish")
        {
            builder
                .push(&event)
                .expect("final trajectory event should build");
        }
        let patches = builder.take_patches();
        let (store, root) = builder.tree_ref().expect("finished trajectory tree");
        let update = projector
            .update(store, root, &patches)
            .expect("trajectory close projection update");
        apply_streaming_delta_to_consumer(&mut consumer_nodes, &mut consumer_edges, &update.delta);

        let model = projector
            .previous_model
            .as_ref()
            .expect("trajectory model after close");
        for edge in &model.edges {
            let to = model
                .nodes
                .get(edge.to_render_handle as usize)
                .expect("model edge child");
            assert_eq!(
                edge.bezier_args.to_x, to.box_args.x,
                "model edge {:?}->{:?} must end at the current child left edge",
                edge.from_key.path, edge.to_key.path,
            );
        }

        for edge in consumer_edges.values() {
            let from = consumer_nodes
                .get(&edge.from_render_handle)
                .expect("edge parent node");
            let to = consumer_nodes
                .get(&edge.to_render_handle)
                .expect("edge child node");
            assert_eq!(
                edge.bezier_args.from_x,
                from.box_args.x + from.box_args.width,
                "edge {:?}->{:?} must start at the current parent right edge",
                path_key(from),
                path_key(to),
            );
            assert_eq!(
                edge.bezier_args.to_x,
                to.box_args.x,
                "edge {:?}->{:?} must end at the current child left edge",
                path_key(from),
                path_key(to),
            );
        }
    }

    #[test]
    fn streaming_trajectory_waits_for_agent_steps_presentation() {
        let source = include_str!("../../../../test/fixtures/json/trajectory.1.json");
        let bytes = source.as_bytes();
        let chunk_size = 64 * 1024;
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");
        let mut agent_steps_handle = None;

        let mut assert_update = |update: &ProjectionUpdate| {
            for node in update
                .delta
                .nodes_added
                .iter()
                .chain(&update.delta.nodes_updated)
            {
                if path_key(node) == "agent_steps" {
                    agent_steps_handle = Some(node.render_handle);
                    assert_eq!(
                        node.table.as_ref().map_or(0, |table| table.header_height),
                        0,
                        "agent_steps must not materialize from a provisional header table"
                    );
                }
            }
            let Some(handle) = agent_steps_handle else {
                return;
            };
            for patch in &update.delta.table_patches {
                match patch {
                    TablePatch::RowsAppended {
                        table_handle,
                        header_height,
                        ..
                    } if *table_handle == handle => {
                        assert_eq!(
                            *header_height, 0,
                            "agent_steps rows must not use a provisional header table"
                        );
                    }
                    TablePatch::TableReplaced {
                        table_handle,
                        table,
                    } if *table_handle == handle => {
                        assert_eq!(
                            table.header_height, 0,
                            "agent_steps must not be replaced with a provisional header table"
                        );
                    }
                    _ => {}
                }
            }
        };

        let mut offset = 0;
        while offset < bytes.len() {
            let end = (offset + chunk_size).min(bytes.len());
            let mut chunk_end = end;
            while chunk_end < bytes.len() && chunk_end > offset && bytes[chunk_end] & 0xC0 == 0x80 {
                chunk_end -= 1;
            }
            decoder
                .feed(std::str::from_utf8(&bytes[offset..chunk_end]).expect("utf-8 fixture"))
                .expect("trajectory chunk should decode");
            for event in &decoder.take_events() {
                builder.push(event).expect("trajectory event should build");
            }
            let patches = builder.take_patches();
            let (store, root) = builder.tree_ref().expect("streaming trajectory tree");
            let update = projector
                .update(store, root, &patches)
                .expect("projection update");
            assert_update(&update);
            offset = chunk_end;
        }

        for event in decoder
            .finish_events()
            .expect("trajectory stream should finish")
        {
            builder
                .push(&event)
                .expect("final trajectory event should build");
        }
        let patches = builder.take_patches();
        let (store, root) = builder.tree_ref().expect("finished trajectory tree");
        let update = projector
            .update(store, root, &patches)
            .expect("close projection update");
        assert_update(&update);
    }

    #[test]
    fn streaming_consumer_refreshes_existing_table_meta_when_node_moves() {
        let chunks = [
            r#"{"a":[{"id":0},{"id":1},{"id":2}]"#,
            r#","this_is_a_very_long_root_key_that_forces_depth_one_column_to_move":{"x":1}}"#,
        ];
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");
        let mut consumer_nodes: HashMap<u32, crate::document::protocol::GraphNodeData> =
            HashMap::new();
        let mut consumer_edges: HashMap<
            (u32, i32, u32, i32),
            crate::document::protocol::GraphEdgeData,
        > = HashMap::new();

        for chunk in chunks {
            decoder.feed(chunk).unwrap();
            for event in &decoder.take_events() {
                builder.push(event).unwrap();
            }
            let patches = builder.take_patches();
            let (store, root) = builder.tree_ref().expect("streaming tree");
            let update = projector
                .update(store, root, &patches)
                .expect("projection update");
            apply_streaming_delta_to_consumer(
                &mut consumer_nodes,
                &mut consumer_edges,
                &update.delta,
            );
        }

        let border_width = graph_projection_service::projection_builder_config()
            .to_graph_builder_config()
            .node_border_width
            .max(0);
        let table = consumer_nodes
            .values()
            .find(|node| node.table.is_some() && path_key(node) == "a")
            .expect("consumer must retain the moved $.a table node");
        let meta = table.meta.as_ref().expect("table node must carry meta");
        assert_eq!(
            meta.box_args.x,
            table.box_args.x + border_width,
            "table meta must move with its node; node={:?}",
            path_key(table),
        );
        assert_eq!(
            meta.box_args.y,
            table.box_args.y
                - graph_projection_service::projection_builder_config()
                    .to_graph_builder_config()
                    .row_height,
            "table meta y must stay anchored above its node",
        );
    }

    fn builder_from_source(source: &str) -> Builder {
        let mut b = Builder::new();
        for e in &dec(source) {
            b.push(e).unwrap();
        }
        b
    }

    #[test]
    fn streaming_header_table_waits_for_first_key_before_table_created() {
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");

        d.feed(r#"{"rows":[{"#).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let patches = b.take_patches();
        let (s, r) = b.tree_ref().unwrap();

        let update = p.update(s, r, &patches);
        if let Some(update) = update {
            let created_rows_table = update.delta.table_patches.iter().any(|patch| {
                matches!(
                    patch,
                    TablePatch::TableCreated {
                        columns,
                        ..
                    } if columns
                        .get(1)
                        .is_some_and(|column| column.text == "value")
                )
            });
            assert!(
                !created_rows_table,
                "header table must not publish fallback columns before the first key is known"
            );
        }
    }

    #[test]
    fn projector_produces_initial_delta_for_first_chunk() {
        let b = builder_from_source(r#"{"a":1}"#);
        let (s, r) = b.tree_ref().unwrap();
        let mut p = StreamingGraphProjector::new("json");
        let u = p.update(s, r, &[]).unwrap();
        assert!(!u.delta.nodes_added.is_empty());
        assert_eq!(u.graph_version, 1);
    }

    #[test]
    fn projector_increments_version_across_updates() {
        let b = builder_from_source(r#"{"a":1}"#);
        let (s, r) = b.tree_ref().unwrap();
        let mut p = StreamingGraphProjector::new("json");
        assert_eq!(p.update(s, r, &[]).unwrap().graph_version, 1);
        assert_eq!(p.update(s, r, &[]).unwrap().graph_version, 2);
    }

    #[test]
    fn multi_chunk_growing_tree_maintains_version_chain() {
        let chunks = [r#"{"a":1,"b":"#, r#""hello","c":"#, r#"[1,2,3]}"#];
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        let mut p = StreamingGraphProjector::new("json");
        for (i, ch) in chunks.iter().enumerate() {
            d.feed(ch).unwrap();
            for e in &d.take_events() {
                b.push(e).unwrap();
            }
            if let Some((s, r)) = b.tree_ref() {
                if let Some(u) = p.update(s, r, &[]) {
                    assert_eq!(u.patch_seq, (i + 1) as u64);
                    assert_eq!(u.graph_version, (i + 1) as u64);
                }
            }
        }
    }

    #[test]
    fn empty_graph_data_still_advances_version() {
        let b = builder_from_source(r#"{}"#);
        let (s, r) = b.tree_ref().unwrap();
        let mut p = StreamingGraphProjector::new("json");
        assert_eq!(p.update(s, r, &[]).unwrap().graph_version, 1);
        assert_eq!(p.update(s, r, &[]).unwrap().graph_version, 2);
    }

    #[test]
    fn streaming_three_chunk_json_produces_non_empty_deltas() {
        let source = r#"{"a":1,"b":"hello","c":[1,2,3],"d":{"e":true,"f":null}}"#;
        let n = source.len();
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        let mut p = StreamingGraphProjector::new("json");
        let mut c = 0;
        for ch in [
            &source[..n / 3],
            &source[n / 3..2 * n / 3],
            &source[2 * n / 3..],
        ] {
            d.feed(ch).unwrap();
            for e in &d.take_events() {
                b.push(e).unwrap();
            }
            if let Some((s, r)) = b.tree_ref() {
                if p.update(s, r, &[]).is_some() {
                    c += 1;
                }
            }
        }
        assert!(c >= 2, "got {c}");
    }

    #[test]
    fn streaming_patch_budget_does_not_grow_superlinearly() {
        let mut source = String::from("{\"rows\":[\n");
        for i in 0..100 {
            let comma = if i < 99 { "," } else { "" };
            source.push_str(&format!(
                "  {{\"id\":{i},\"name\":\"item_{i}\",\"value\":{v}}}{comma}\n",
                v = i * 10
            ));
        }
        source.push_str("]}");
        let n = source.len();
        let cs = n / 4;
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        let mut p = StreamingGraphProjector::new("json");
        let (mut ta, mut tu) = (0usize, 0usize);
        let mut o = 0;
        while o < n {
            let e = (o + cs).min(n);
            d.feed(&source[o..e]).unwrap();
            for ev in &d.take_events() {
                b.push(ev).unwrap();
            }
            if let Some((s, r)) = b.tree_ref() {
                if let Some(u) = p.update(s, r, &[]) {
                    ta += u.delta.nodes_added.len();
                    tu += u.delta.nodes_updated.len();
                }
            }
            o = e;
        }
        assert!(tu <= 10 || tu * 10 <= ta, "tu={tu} ta={ta}");
    }

    #[test]
    fn incremental_subtree_merge_with_patches() {
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");
        d.feed(r#"{"a":1,"b":"#).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let p1 = b.take_patches();
        assert!(!p1.is_empty());
        let (s1, r1) = b.tree_ref().unwrap();
        let u1 = p.update(s1, r1, &p1).unwrap();
        assert_eq!(u1.patch_seq, 1);
        d.feed(r#""hello"}"#).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let p2 = b.take_patches();
        assert!(!p2.is_empty());
        let (s2, r2) = b.tree_ref().unwrap();
        let u2 = p.update(s2, r2, &p2).unwrap();
        assert_eq!(u2.patch_seq, 2);
        let has = !u2.delta.nodes_added.is_empty()
            || !u2.delta.nodes_updated.is_empty()
            || !u2.delta.edges_added.is_empty();
        assert!(has, "chunk2 should produce changes");
    }

    #[test]
    fn streaming_chunks_do_not_keep_rebuilding_full_index() {
        let chunks = [r#"{"a0":0,"#, r#""a1":1,"#, r#""a2":2,"#, r#""a3":3,"#];

        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");

        for chunk in chunks {
            d.feed(chunk).unwrap();
            for event in &d.take_events() {
                b.push(event).unwrap();
            }
            let patches = b.take_patches();
            let (store, root) = b.tree_ref().expect("streaming tree");
            let update = p.update(store, root, &patches);
            assert!(update.is_some(), "chunk should produce a projection update");
        }
    }

    #[test]
    fn table_patch_produced_for_growing_table() {
        // Simple JSON array of objects that becomes a table in the graph.
        let source = r#"{"items":[{"k":"a","v":1},{"k":"b","v":2},{"k":"c","v":3}]}"#;
        let n = source.len();
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        let mut p = StreamingGraphProjector::new("json");
        // Feed first half.
        d.feed(&source[..n / 2]).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let (s1, r1) = b.tree_ref().unwrap();
        let u1 = p.update(s1, r1, &[]).unwrap();
        let tp1: usize = u1
            .delta
            .table_patches
            .iter()
            .filter(|p| matches!(p, TablePatch::TableCreated { .. }))
            .count();
        assert!(tp1 > 0, "first chunk should produce TableCreated");

        // Feed second half.
        d.feed(&source[n / 2..]).unwrap();
        for e in &d.take_events() {
            b.push(e).unwrap();
        }
        let (s2, r2) = b.tree_ref().unwrap();
        let u2 = p.update(s2, r2, &[]).unwrap();
        let ra: usize = u2
            .delta
            .table_patches
            .iter()
            .filter(|p| matches!(p, TablePatch::RowsAppended { .. }))
            .count();
        // After splitting, the table should not appear in nodes_added if rows were appended.
        assert!(
            ra > 0 || u2.delta.nodes_added.len() < 10,
            "subsequent chunks should produce RowsAppended or avoid re-adding full table: ra={ra} na={}",
            u2.delta.nodes_added.len()
        );
    }

    /// 红测试：均匀对象数组流式输入后只产生 1 个 Table 节点（即根 Mapping
    /// + 1 个 Table）。当前实现因 populate/skip_set 错位以及行非列对齐，
    /// 会把每个 Mapping 当独立节点写入 model.nodes，导致 N+ 个节点。
    #[test]
    fn streaming_uniform_object_array_produces_single_table_node() {
        const N: usize = 64;
        let mut source = String::from(r#"{"items":["#);
        for i in 0..N {
            if i > 0 {
                source.push(',');
            }
            source.push_str(&format!(r#"{{"id":{i},"name":"x{i}"}}"#));
        }
        source.push_str("]}");

        let n = source.len();
        let cs = (n / 8).max(1);
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");
        let mut o = 0;
        while o < n {
            let e = (o + cs).min(n);
            d.feed(&source[o..e]).unwrap();
            for ev in &d.take_events() {
                b.push(ev).unwrap();
            }
            let patches = b.take_patches();
            if let Some((s, r)) = b.tree_ref() {
                p.update(s, r, &patches);
            }
            o = e;
        }

        let model = p.previous_model.as_ref().expect("model must exist");
        // 期望节点：1 个根 Mapping (root "{...}") + 1 个 Table ("items"=[...])
        // 共 2 个 GraphNode；当前 buggy 实现下会出现 N+ 个 Mapping 节点。
        let table_count = model.nodes.iter().filter(|n| n.table.is_some()).count();
        // 调试用：dump 节点信息
        let dbg = model
            .nodes
            .iter()
            .map(|n| {
                format!(
                    "kind={:?} table={} rows={} ",
                    n.kind,
                    n.table.is_some(),
                    n.rows.len()
                )
            })
            .collect::<Vec<_>>()
            .join("\n  ");
        assert_eq!(
            table_count, 1,
            "should have exactly 1 Table node, got {table_count}\nnodes:\n  {dbg}"
        );
        // 表里应当有 N 行
        let table_node = model
            .nodes
            .iter()
            .find(|n| n.table.is_some())
            .expect("table node");
        let rows = &table_node.table.as_ref().unwrap().rows;
        assert_eq!(
            rows.len(),
            N,
            "table should have N={N} rows, got {}\nall nodes ({} total):\n  {dbg}",
            rows.len(),
            model.nodes.len()
        );
        // model.nodes 总数应当远小于 N（不能为每个 mapping 独立创建节点）
        assert!(
            model.nodes.len() < N,
            "expected far fewer than N={N} nodes, got {} (mapping nodes leaked as independent)",
            model.nodes.len()
        );
    }

    /// 红测试：incr 路径中 RowsAppended 的 row.cells 必须按 parent table.columns
    /// 顺序对齐——cells.len() == columns.len()。
    #[test]
    fn streaming_table_rows_aligned_with_columns() {
        const N: usize = 16;
        let mut source = String::from(r#"{"items":["#);
        for i in 0..N {
            if i > 0 {
                source.push(',');
            }
            source.push_str(&format!(r#"{{"id":{i},"name":"x{i}"}}"#));
        }
        source.push_str("]}");

        let n = source.len();
        let cs = (n / 6).max(1);
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");
        let mut o = 0;
        while o < n {
            let e = (o + cs).min(n);
            d.feed(&source[o..e]).unwrap();
            for ev in &d.take_events() {
                b.push(ev).unwrap();
            }
            let patches = b.take_patches();
            if let Some((s, r)) = b.tree_ref() {
                p.update(s, r, &patches);
            }
            o = e;
        }

        let model = p.previous_model.as_ref().expect("model");
        let table_node = model
            .nodes
            .iter()
            .find(|n| n.table.is_some())
            .expect("table node");
        let table = table_node.table.as_ref().unwrap();
        let col_n = table.columns.len();
        for (i, row) in table.rows.iter().enumerate() {
            assert_eq!(
                row.cells.len(),
                col_n,
                "row {i} cell count {} != column count {col_n}",
                row.cells.len()
            );
        }

        // Position rules (cell box_args use row-local coordinates; absolute Y is
        // row.box_args.y; X is cell.box_args.x).
        // 1. Same row → cells share the same absolute y.
        // 2. Same column → cells share the same x across rows.
        // 3. Right neighbor → strictly larger x.
        // 4. Below neighbor → strictly larger absolute y.
        for (ri, row) in table.rows.iter().enumerate() {
            // Rule 1: same row → same y (cells use row-local y; row.box_args.y
            // is the absolute row y; assert each cell within the row has
            // identical y so they line up horizontally).
            let first_y = row.cells[0].box_args.y;
            for (ci, cell) in row.cells.iter().enumerate() {
                assert_eq!(
                    cell.box_args.y, first_y,
                    "row {ri} col {ci}: cell.y={} != first cell y={first_y}",
                    cell.box_args.y
                );
            }
            // Rule 3: right neighbor strictly larger x.
            for ci in 1..row.cells.len() {
                let prev_x = row.cells[ci - 1].box_args.x;
                let cur_x = row.cells[ci].box_args.x;
                assert!(
                    cur_x > prev_x,
                    "row {ri} col {ci}: x={cur_x} not > prev x={prev_x}",
                );
            }
        }
        // Rule 2: same column → same cell.box_args.x across rows.
        for col_idx in 0..col_n {
            let baseline_x = table.rows[0].cells[col_idx].box_args.x;
            for (ri, row) in table.rows.iter().enumerate().skip(1) {
                assert_eq!(
                    row.cells[col_idx].box_args.x, baseline_x,
                    "row {ri} col {col_idx}: x={} != baseline x={baseline_x}",
                    row.cells[col_idx].box_args.x
                );
            }
        }
        // Rule 4: below row strictly larger absolute y (row.box_args.y).
        for ri in 1..table.rows.len() {
            let prev_y = table.rows[ri - 1].box_args.y;
            let cur_y = table.rows[ri].box_args.y;
            assert!(
                cur_y > prev_y,
                "row {ri}: row.y={cur_y} not > prev row.y={prev_y}"
            );
        }
    }

    /// 回归保护：首次投影遇到 table 节点时，必须同时发出
    /// `TableCreated` 与覆盖全部初始行的 `RowsAppended{ start_index: 0 }`，
    /// 否则 web 端在收到 `clear:1` 后会拿不到 0..N 的行数据。
    #[test]
    fn first_projection_emits_table_created_and_initial_rows() {
        let b = builder_from_source(r#"[{"a":1,"b":2},{"a":3,"b":4},{"a":5,"b":6}]"#);
        let (s, r) = b.tree_ref().unwrap();
        let mut p = StreamingGraphProjector::new("json");
        let update = p
            .update(s, r, &[])
            .expect("first update must produce delta");
        let delta = &update.delta;

        // 找到本次投影里新增的 table 节点。注意：split_patches 会把 table 节点
        // 从 nodes_added 里搬出去，仅保留剥离 rows 后的副本（且 rows=Vec::new()）。
        // 所以 table 节点会出现在 nodes_added 中，table.is_some()，但 rows 为空，
        // 真正的行数据必须以 RowsAppended 形式出现。
        let table_node = delta
            .nodes_added
            .iter()
            .find(|n| n.table.is_some())
            .expect("first projection should expose the streaming table node");
        let table_handle = table_node.render_handle;

        let mut saw_created = false;
        let mut initial_rows: Option<usize> = None;
        for patch in &delta.table_patches {
            match patch {
                crate::document::protocol::TablePatch::TableCreated {
                    table_handle: h, ..
                } if *h == table_handle => {
                    saw_created = true;
                }
                crate::document::protocol::TablePatch::RowsAppended {
                    table_handle: h,
                    start_index,
                    rows,
                    ..
                } if *h == table_handle && *start_index == 0 => {
                    initial_rows = Some(rows.len());
                }
                _ => {}
            }
        }

        assert!(
            saw_created,
            "first projection must emit TableCreated for handle {table_handle}"
        );
        assert_eq!(
            initial_rows,
            Some(3),
            "first projection must emit RowsAppended{{ start_index: 0, rows.len() == 3 }} \
             for handle {table_handle}; got {:?}; full table_patches = {:?}",
            initial_rows,
            delta.table_patches,
        );
    }

    /// 红测试：首次投影产出的 `TableCreated.columns[i].box_args` 必须携带列定位
    /// 信息——`x` 严格按列累加、`width` 非零、跨列 x 严格递增——否则前端
    /// `graph-viewer-render.ts` 用 `column?.boxArgs.x ?? cell.boxArgs.x`
    /// 取列位置时会全部得到 0，导致同一行所有 cell 渲染到第一列。
    #[test]
    fn table_created_columns_carry_box_args() {
        let b =
            builder_from_source(r#"[{"id":1,"name":"a"},{"id":2,"name":"b"},{"id":3,"name":"c"}]"#);
        let (s, r) = b.tree_ref().unwrap();
        let mut p = StreamingGraphProjector::new("json");
        let update = p
            .update(s, r, &[])
            .expect("first update must produce delta");
        let delta = &update.delta;

        let table_node = delta
            .nodes_added
            .iter()
            .find(|n| n.table.is_some())
            .expect("first projection should expose a streaming table node");
        let table_handle = table_node.render_handle;

        let columns = delta
            .table_patches
            .iter()
            .find_map(|patch| match patch {
                crate::document::protocol::TablePatch::TableCreated {
                    table_handle: h,
                    columns,
                } if *h == table_handle => Some(columns.clone()),
                _ => None,
            })
            .expect("must emit TableCreated for table_handle");

        assert!(
            columns.len() >= 2,
            "expected ≥2 columns, got {}",
            columns.len()
        );
        for (i, col) in columns.iter().enumerate() {
            assert!(
                col.box_args.width > 0,
                "column {i} box_args.width must be > 0, got {}",
                col.box_args.width
            );
        }
        for i in 1..columns.len() {
            let prev_x = columns[i - 1].box_args.x;
            let cur_x = columns[i].box_args.x;
            assert!(
                cur_x > prev_x,
                "column {i}: box_args.x={cur_x} must be > prev x={prev_x}"
            );
        }
    }

    /// 回归测试：RowsAppended 必须携带 total_height / view_height / header_height /
    /// row_height，确保 Frontend 在追加行后不需要完整重新发送 Table 节点就能
    /// 更新 viewport 尺寸——否则会出现「table node 下方大片空白，仅显示
    /// 首块 chunk 的行数」的 bug。
    #[test]
    fn rows_appended_carries_table_sizing() {
        let b = builder_from_source(r#"[{"a":1,"b":2},{"a":3,"b":4},{"a":5,"b":6}]"#);
        let (s, r) = b.tree_ref().unwrap();
        let mut p = StreamingGraphProjector::new("json");
        let update = p
            .update(s, r, &[])
            .expect("first update must produce delta");
        let delta = &update.delta;

        let table_node = delta
            .nodes_added
            .iter()
            .find(|n| n.table.is_some())
            .expect("must have a table node in nodes_added");
        let table_handle = table_node.render_handle;

        let rows_appended_patch = delta
            .table_patches
            .iter()
            .find_map(|patch| match patch {
                crate::document::protocol::TablePatch::RowsAppended {
                    table_handle: h,
                    start_index,
                    total_height,
                    view_height,
                    header_height,
                    row_height,
                    ..
                } if *h == table_handle && *start_index == 0 => {
                    Some((*total_height, *view_height, *header_height, *row_height))
                }
                _ => None,
            })
            .expect("must emit RowsAppended with start_index=0 carrying sizing fields");

        let (total_height, view_height, header_height, row_height) = rows_appended_patch;
        assert!(total_height > 0, "total_height must be positive");
        assert!(view_height > 0, "view_height must be positive");
        assert!(
            header_height > 0,
            "header_height must be positive for header table"
        );
        assert!(row_height > 0, "row_height must be positive");
    }

    #[test]
    fn dirty_table_row_emits_only_changed_cells() {
        let b = builder_from_source(r#"[{"a":1,"b":2},{"a":3,"b":4}]"#);
        let (s, r) = b.tree_ref().unwrap();
        let mut p = StreamingGraphProjector::new("json");
        let first = p.update(s, r, &[]).expect("initial projection");
        let table_handle = first
            .delta
            .nodes_added
            .iter()
            .find(|node| node.table.is_some())
            .map(|node| node.render_handle)
            .expect("initial projection should expose table node");

        let mut model = p
            .previous_model
            .as_ref()
            .expect("projection should retain model")
            .clone();
        let table_node = model
            .nodes
            .get_mut(table_handle as usize)
            .expect("table handle should resolve");
        let cell = table_node
            .table
            .as_mut()
            .expect("table payload")
            .rows
            .get_mut(0)
            .expect("first row")
            .cells
            .get_mut(0)
            .expect("first cell");
        cell.text = "9".to_string();
        cell.value = "9".to_string();
        cell.format_text = "9".to_string();

        let materialized = crate::graph::graph_materialize::MaterializedGraphPatch {
            updated_handles: vec![table_handle],
            table_row_touches: vec![crate::graph::graph_materialize::TableRowTouch {
                table_handle,
                row_indexes: vec![0],
            }],
            ..Default::default()
        };
        let plan = ProjectionPatchPlan::default();
        let patches = p.collect_table_patches(&model, &materialized, &plan);

        assert!(
            patches
                .iter()
                .all(|patch| matches!(patch, TablePatch::CellsUpdated { .. })),
            "dirty row should not emit table creation, columns, or appended rows; patches={patches:?}"
        );
        let changed_cells: Vec<_> = patches
            .iter()
            .flat_map(|patch| match patch {
                TablePatch::CellsUpdated { cells, .. } => cells.as_slice(),
                _ => &[],
            })
            .collect();
        assert_eq!(
            changed_cells.len(),
            1,
            "only the edited cell should be emitted; patches={patches:?}"
        );
        assert_eq!(changed_cells[0].row_index, 0);
        assert_eq!(changed_cells[0].column_index, 0);
        assert_eq!(changed_cells[0].cell.text, "9");
    }

    /// 复现：以 web e2e fixture（1MB-min.json，裸顶层数组，3168 个均匀对象）
    /// 喂给流式投影器，期望产出一个 Table（含 3168 行）而不是上千个独立节点。
    #[test]
    fn streaming_fixture_1mb_min_produces_single_table_node() {
        let source = include_str!("../../../../test/fixtures/json/1MB-min.1.json");
        let bytes = source.as_bytes();
        let chunk_size = crate::stream::chunk_size::select_chunk_size(bytes.len());

        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");

        let mut offset = 0;
        while offset < bytes.len() {
            let end = (offset + chunk_size).min(bytes.len());
            // UTF-8 边界
            let chunk_end = if end < bytes.len() {
                let mut q = end;
                while q > offset && bytes[q] & 0xC0 == 0x80 {
                    q -= 1;
                }
                q
            } else {
                end
            };
            let text = std::str::from_utf8(&bytes[offset..chunk_end]).expect("utf-8");
            d.feed(text).unwrap();
            for ev in &d.take_events() {
                b.push(ev).unwrap();
            }
            let patches = b.take_patches();
            if let Some((s, r)) = b.tree_ref() {
                p.update(s, r, &patches);
            }
            offset = chunk_end;
        }

        let model = p.previous_model.as_ref().expect("model must exist");
        let table_count = model.nodes.iter().filter(|n| n.table.is_some()).count();
        let total_nodes = model.nodes.len();
        let table_node = model
            .nodes
            .iter()
            .find(|n| n.table.is_some())
            .expect("must have a table node");
        let rows_len = table_node.table.as_ref().unwrap().rows.len();

        assert_eq!(
            table_count, 1,
            "fixture should produce exactly 1 Table node, got {table_count}; total_nodes={total_nodes}"
        );
        // fixture 实际有 3168 条记录
        assert_eq!(
            rows_len, 3168,
            "table should have 3168 rows, got {rows_len}"
        );
        // 总节点数应远小于行数
        assert!(
            total_nodes < 64,
            "expected far fewer than 64 nodes (1 table + few siblings), got {total_nodes}"
        );
    }

    #[test]
    fn streaming_big_object_node_keeps_consumer_node_count_bounded() {
        let source = include_str!("../../../../test/fixtures/json/big-object-node.1.json");
        let bytes = source.as_bytes();
        let chunk_size = crate::stream::chunk_size::select_chunk_size(bytes.len());

        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");
        let mut consumer: HashMap<u32, crate::document::protocol::GraphNodeData> = HashMap::new();
        let mut max_consumer_nodes = 0usize;

        let mut offset = 0;
        while offset < bytes.len() {
            let end = (offset + chunk_size).min(bytes.len());
            let chunk_end = if end < bytes.len() {
                let mut q = end;
                while q > offset && bytes[q] & 0xC0 == 0x80 {
                    q -= 1;
                }
                q
            } else {
                end
            };
            let text = std::str::from_utf8(&bytes[offset..chunk_end]).expect("utf-8");
            d.feed(text).unwrap();
            for ev in &d.take_events() {
                b.push(ev).unwrap();
            }
            let patches = b.take_patches();
            if let Some((s, r)) = b.tree_ref() {
                if let Some(u) = p.update(s, r, &patches) {
                    for n in &u.delta.nodes_added {
                        consumer.insert(n.render_handle, n.clone());
                    }
                    for n in &u.delta.nodes_updated {
                        consumer.insert(n.render_handle, n.clone());
                    }
                    for &rh in &u.delta.nodes_removed {
                        consumer.remove(&rh);
                    }
                    max_consumer_nodes = max_consumer_nodes.max(consumer.len());
                }
            }
            offset = chunk_end;
        }

        let final_nodes = p
            .previous_model
            .as_ref()
            .map(|model| model.nodes.len())
            .expect("model must exist");
        assert!(
            max_consumer_nodes <= final_nodes + 1024,
            "streamed consumer graph exploded to {max_consumer_nodes} nodes while final graph only has {final_nodes}",
        );
    }

    /// 复现 Bug 2：2mb.json 拖入时流式渲染中间出现「重复对象节点 + 表格节点
    /// 同时存在」的错误结构。该 fixture 的 Result.Blocks 是 array-of-objects，
    /// 在多 chunk 流式输入下，Builder 在父 Sequence 还未拿到第一条 Mapping
    /// 时会把它当作非表容器先暴露子节点，等后续 chunk 把第一条 Mapping
    /// 拼好后才把 Sequence 提升为 header-table。流式投影器要求：
    /// 不论中间经过怎样的「先非表后表」过渡，**任意时刻** consumer 看到
    /// 的累积视图（== 按顺序 apply 每个 delta 的 nodes_added/updated/removed）
    /// 中，Header Table 节点的行 Mapping 不允许作为独立节点存在。Headerless
    /// Table 的结构项可以展开，因而不适用这个约束。
    #[test]
    fn streaming_fixture_2mb_keeps_promoted_table_clean() {
        let source = include_str!("../../../../test/fixtures/json/2mb.1.json");
        let bytes = source.as_bytes();
        let chunk_size = crate::stream::chunk_size::select_chunk_size(bytes.len());

        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");

        // Consumer-side accumulation: render_handle -> GraphNodeData (the
        // post-split form actually delivered to the web consumer).
        let mut consumer: HashMap<u32, crate::document::protocol::GraphNodeData> = HashMap::new();
        // Tables also exist in `consumer` (with their `table` populated on
        // first projection); we only need a path-set for tables to detect
        // leaked children.
        let mut chunk_idx = 0usize;

        let mut offset = 0;
        while offset < bytes.len() {
            let end = (offset + chunk_size).min(bytes.len());
            let chunk_end = if end < bytes.len() {
                let mut q = end;
                while q > offset && bytes[q] & 0xC0 == 0x80 {
                    q -= 1;
                }
                q
            } else {
                end
            };
            let text = std::str::from_utf8(&bytes[offset..chunk_end]).expect("utf-8");

            d.feed(text).unwrap();
            for ev in &d.take_events() {
                b.push(ev).unwrap();
            }
            let patches = b.take_patches();
            if let Some((s, r)) = b.tree_ref() {
                if let Some(u) = p.update(s, r, &patches) {
                    // Apply consumer-side update (mirrors what
                    // `applyGraphDelta` does on the web side).
                    for n in &u.delta.nodes_added {
                        consumer.insert(n.render_handle, n.clone());
                    }
                    for n in &u.delta.nodes_updated {
                        consumer.insert(n.render_handle, n.clone());
                    }
                    for &rh in &u.delta.nodes_removed {
                        consumer.remove(&rh);
                    }

                    // Only Header Table rows stay folded. Headerless tables expose
                    // structural row values as expandable child nodes.
                    let table_paths: Vec<Vec<crate::document::protocol::GraphPathSeg>> = consumer
                        .values()
                        .filter(|n| {
                            n.table
                                .as_ref()
                                .is_some_and(|table| table.header_height > 0)
                        })
                        .map(|n| n.path.clone())
                        .collect();

                    // For each table, no other node may share path-prefix
                    // with len == table.path.len() + 1.
                    for tp in &table_paths {
                        let leaks: Vec<u32> = consumer
                            .values()
                            .filter(|n| {
                                n.table.is_none()
                                    && n.path.len() == tp.len() + 1
                                    && n.path.starts_with(tp)
                            })
                            .map(|n| n.render_handle)
                            .collect();
                        assert!(
                            leaks.is_empty(),
                            "chunk {chunk_idx}: table at path={tp:?} leaked {} child mapping(s) (handles={:?}) into consumer state \
                             (Bug 2: previously-emitted child objects survived after parent Sequence got promoted to header-table)",
                            leaks.len(),
                            leaks
                        );
                    }
                    let relative_row_leaks: Vec<(u32, String)> = consumer
                        .values()
                        .filter_map(|n| {
                            let pk = path_key(n);
                            if pk == "Content" || pk.starts_with("Content.") {
                                Some((n.render_handle, pk))
                            } else {
                                None
                            }
                        })
                        .collect();
                    assert!(
                        relative_row_leaks.is_empty(),
                        "chunk {chunk_idx}: row-internal nodes were rendered as detached graph roots: {relative_row_leaks:?}"
                    );
                }
            }
            offset = chunk_end;
            chunk_idx += 1;
        }
    }

    #[test]
    fn streaming_fixture_5mb_min_appends_rows_without_replacing_steady_table() {
        #[derive(Debug, Clone, Copy)]
        struct TableUpdateSnapshot {
            chunk_index: usize,
            rows_len: usize,
            box_args: crate::document::protocol::GraphBoxArgs,
            table_node_updated: bool,
            bounds_patched: bool,
            rows_appended: usize,
            table_replaced: bool,
        }

        let source = include_str!("../../../../test/fixtures/json/5MB-min.1.json");
        let bytes = source.as_bytes();
        let chunk_size = crate::stream::chunk_size::select_chunk_size(bytes.len());

        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");

        let mut table_handle: Option<u32> = None;
        let mut snapshots: Vec<TableUpdateSnapshot> = Vec::new();
        let mut chunk_index = 0usize;
        let mut offset = 0usize;

        while offset < bytes.len() {
            let end = (offset + chunk_size).min(bytes.len());
            let chunk_end = if end < bytes.len() {
                let mut q = end;
                while q > offset && bytes[q] & 0xC0 == 0x80 {
                    q -= 1;
                }
                q
            } else {
                end
            };
            let text = std::str::from_utf8(&bytes[offset..chunk_end]).expect("utf-8");
            decoder.feed(text).unwrap();
            for event in &decoder.take_events() {
                builder.push(event).unwrap();
            }
            let patches = builder.take_patches();
            if let Some((store, root)) = builder.tree_ref() {
                let update = projector
                    .update(store, root, &patches)
                    .expect("5MB fixture should always produce projection heartbeat");
                let model = projector
                    .previous_model
                    .as_ref()
                    .expect("projection update should materialize a model");
                let table_node =
                    model.nodes.iter().find(|node| node.table.is_some()).expect(
                        "fixture should always converge to a table node once streaming starts",
                    );
                let handle = table_node.render_handle;
                if let Some(previous_handle) = table_handle {
                    assert_eq!(
                        previous_handle, handle,
                        "table handle must stay stable across streaming updates"
                    );
                } else {
                    table_handle = Some(handle);
                }

                let table_node_updated = update
                    .delta
                    .nodes_updated
                    .iter()
                    .any(|node| node.render_handle == handle);
                let bounds_patched = update.delta.layout_patches.iter().any(|patch| {
                    matches!(
                        patch,
                        LayoutPatch::NodeBoundsUpdated {
                            render_handle,
                            ..
                        } if *render_handle == handle
                    )
                });
                let rows_appended = update
                    .delta
                    .table_patches
                    .iter()
                    .filter_map(|patch| match patch {
                        TablePatch::RowsAppended {
                            table_handle, rows, ..
                        } if *table_handle == handle => Some(rows.len()),
                        _ => None,
                    })
                    .sum();
                let table_replaced = update.delta.table_patches.iter().any(|patch| {
                    matches!(
                        patch,
                        TablePatch::TableReplaced {
                            table_handle,
                            ..
                        } if *table_handle == handle
                    )
                });

                snapshots.push(TableUpdateSnapshot {
                    chunk_index,
                    rows_len: table_node
                        .table
                        .as_ref()
                        .expect("table node must have table payload")
                        .rows
                        .len(),
                    box_args: graph_box_args_from_node(table_node),
                    table_node_updated,
                    bounds_patched,
                    rows_appended,
                    table_replaced,
                });
            }
            offset = chunk_end;
            chunk_index += 1;
        }

        let mut steady_row_chunks = 0usize;
        let bad_steady_row_chunks: Vec<_> = snapshots
            .windows(2)
            .filter_map(|pair| {
                let previous = pair[0];
                let current = pair[1];
                let steady_row_append =
                    current.rows_len > previous.rows_len && current.box_args == previous.box_args;
                if !steady_row_append {
                    return None;
                }
                steady_row_chunks += 1;
                (current.table_node_updated
                    || current.bounds_patched
                    || (current.rows_appended == 0 && !current.table_replaced))
                    .then_some((
                        current.chunk_index,
                        current.rows_len,
                        current.box_args,
                        current.table_node_updated,
                        current.bounds_patched,
                        current.rows_appended,
                        current.table_replaced,
                    ))
            })
            .collect();

        assert!(
            steady_row_chunks > 0,
            "fixture must exercise row growth after table geometry stabilizes; snapshots={snapshots:?}"
        );
        assert!(
            bad_steady_row_chunks.is_empty(),
            "steady table row growth must be delivered as RowsAppended without replacing or moving the table node; bad chunks={bad_steady_row_chunks:?}; snapshots={snapshots:?}"
        );
    }

    /// Accumulate streamed deltas (mirroring the web consumer) and the
    /// close-phase remaining delta, then compare the resulting node path set
    /// against the single-shot baseline projection of the final tree.
    ///
    /// Bug repro: a headerless sequence-of-sequences (`influences:[[a,b],…]`).
    /// During streaming each inner `[a,b]` array is mis-classified and emitted
    /// as a standalone graph node (`$.influences[0]`, `[1]`, …) that the
    /// baseline never produces, and those orphans have no parent edge. The
    /// streamed+closed consumer view must equal the baseline.
    #[test]
    fn streaming_headerless_seq_of_seq_matches_baseline_after_close() {
        // influences is a headerless table (first child is a Sequence, not a
        // Mapping). Use 2-element inner arrays like the real fixture.
        let mut source = String::from(r#"{"influences":["#);
        for i in 0..32 {
            if i > 0 {
                source.push(',');
            }
            source.push_str(&format!("[{}.5,{}.25]", i, i));
        }
        source.push_str("]}");

        // ── Baseline: single-shot projection of the fully-decoded tree. ──
        let baseline_paths: std::collections::BTreeSet<String> = {
            let b = builder_from_source(&source);
            let (s, r) = b.tree_ref().unwrap();
            let delta = crate::graph::graph_projection_service::build_initial_projection_delta(
                s, r, "json",
            );
            delta.nodes_added.iter().map(path_key).collect()
        };

        // ── Streaming: feed in small chunks, accumulate consumer node set. ──
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");
        let mut consumer: HashMap<u32, crate::document::protocol::GraphNodeData> = HashMap::new();

        let bytes = source.as_bytes();
        let chunk = 7usize;
        let mut offset = 0;
        while offset < bytes.len() {
            let end = (offset + chunk).min(bytes.len());
            let text = std::str::from_utf8(&bytes[offset..end]).expect("utf-8");
            d.feed(text).unwrap();
            for ev in &d.take_events() {
                b.push(ev).unwrap();
            }
            let patches = b.take_patches();
            if let Some((s, r)) = b.tree_ref() {
                if let Some(u) = p.update(s, r, &patches) {
                    for n in &u.delta.nodes_added {
                        consumer.insert(n.render_handle, n.clone());
                    }
                    for n in &u.delta.nodes_updated {
                        consumer.insert(n.render_handle, n.clone());
                    }
                    for &rh in &u.delta.nodes_removed {
                        consumer.remove(&rh);
                    }
                }
            }
            offset = end;
        }

        // ── Close: apply the projector's remaining delta (clear:false). ──
        let patches = b.take_patches();
        if let Some((s, r)) = b.tree_ref() {
            if let Some(u) = p.update(s, r, &patches) {
                for n in &u.delta.nodes_added {
                    consumer.insert(n.render_handle, n.clone());
                }
                for n in &u.delta.nodes_updated {
                    consumer.insert(n.render_handle, n.clone());
                }
                for &rh in &u.delta.nodes_removed {
                    consumer.remove(&rh);
                }
            }
        }

        let streamed_paths: std::collections::BTreeSet<String> =
            consumer.values().map(path_key).collect();

        let orphans: Vec<&String> = streamed_paths.difference(&baseline_paths).collect();
        assert!(
            orphans.is_empty(),
            "streamed consumer view produced nodes the baseline never emits \
             (orphan inner-array nodes): {orphans:?}\nbaseline={baseline_paths:?}\nstreamed={streamed_paths:?}"
        );
        assert_eq!(
            streamed_paths, baseline_paths,
            "streamed+closed node set must equal baseline node set"
        );
    }

    #[test]
    fn streaming_large_headerless_seq_rows_keep_structured_children_after_close() {
        let mut source = String::from(r#"{"items":["#);
        for i in 0..60 {
            if i > 0 {
                source.push(',');
            }
            source.push_str(&format!("[{}]", i));
        }
        source.push_str("]}");

        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");
        let mut consumer_nodes: HashMap<u32, crate::document::protocol::GraphNodeData> =
            HashMap::new();
        let mut consumer_edges: HashMap<
            (u32, i32, u32, i32),
            crate::document::protocol::GraphEdgeData,
        > = HashMap::new();

        let bytes = source.as_bytes();
        let chunk = 11usize;
        let mut offset = 0;
        while offset < bytes.len() {
            let end = (offset + chunk).min(bytes.len());
            let text = std::str::from_utf8(&bytes[offset..end]).expect("utf-8");
            decoder.feed(text).unwrap();
            for event in &decoder.take_events() {
                builder.push(event).unwrap();
            }
            let patches = builder.take_patches();
            if let Some((store, root)) = builder.tree_ref()
                && let Some(update) = projector.update(store, root, &patches)
            {
                apply_streaming_delta_to_consumer(
                    &mut consumer_nodes,
                    &mut consumer_edges,
                    &update.delta,
                );
            }
            offset = end;
        }

        let final_model = projector
            .previous_model
            .as_ref()
            .expect("projector should retain the final graph model");
        let items_node = final_model
            .nodes
            .iter()
            .find(|node| {
                node.path.len() == 1
                    && matches!(node.path.first(), Some(crate::graph::graph_builder::PathSeg::Key(key)) if key == "items")
            })
            .expect("items table should be visible");
        let items_handle = items_node.render_handle;
        let table = items_node.table.as_ref().expect("items should be a table");
        let (total_height, view_height) = (table.total_height, table.view_height);
        assert_eq!(
            total_height, view_height,
            "headerless tables expose their full body height"
        );
        let child_paths: Vec<String> = consumer_nodes
            .values()
            .map(path_key)
            .filter(|path| path.starts_with("items."))
            .collect();
        assert_eq!(
            child_paths.len(),
            60,
            "every structured row must remain a graph child"
        );
        let child_edges: Vec<_> = consumer_edges
            .values()
            .filter(|edge| edge.from_render_handle == items_handle)
            .collect();
        assert_eq!(
            child_edges.len(),
            60,
            "every structured row must retain an outgoing graph edge"
        );
    }

    /// Stream a root Mapping with several large scalar arrays (mirrors the
    /// `big-object-node.json` shape: positions/tex0/colors/...). After close,
    /// the streamed model must match the single-shot baseline for:
    ///   1. the parent `$` Mapping's rows — every top-level key (e.g. `tex0`)
    ///      must have a row/cell; none may be dropped.
    ///   2. each child node's `box_args` — sibling nodes stack vertically
    ///      (`tex0` below `positions`), never overlapping.
    #[test]
    fn streaming_root_mapping_children_match_baseline_layout_after_close() {
        // Multiple sibling headerless arrays under the root mapping. Keep them
        // small but >1 element so each becomes its own child graph node.
        let arr = |n: usize| {
            let mut s = String::from("[");
            for i in 0..n {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&format!("{}.5", i));
            }
            s.push(']');
            s
        };
        let source = format!(
            r#"{{"positions":{},"tex0":{},"colors":{},"normals":{}}}"#,
            arr(20),
            arr(16),
            arr(12),
            arr(24),
        );

        // ── Baseline: single-shot model of the fully-decoded tree. ──
        let baseline_model = {
            let b = builder_from_source(&source);
            let (s, r) = b.tree_ref().unwrap();
            crate::graph::graph_projection_service::build_graph_model_for_tree(s, r, "json")
                .expect("baseline model build")
        };
        // path -> (y, height, table_rows) for every baseline node.
        let model_by_path = |m: &crate::graph::graph_builder::GraphModel| {
            m.nodes
                .iter()
                .map(|n| {
                    let path = n
                        .path
                        .iter()
                        .map(|seg| match seg {
                            crate::graph::graph_builder::PathSeg::Key(k) => k.clone(),
                            crate::graph::graph_builder::PathSeg::Index(i) => format!("[{i}]"),
                        })
                        .collect::<Vec<_>>()
                        .join(".");
                    (
                        path,
                        (n.y, n.height, n.table.as_ref().map(|t| t.rows.len())),
                    )
                })
                .collect::<std::collections::BTreeMap<String, (i32, i32, Option<usize>)>>()
        };
        let baseline_by_path = model_by_path(&baseline_model);

        // ── Streaming: small chunks then close. ──
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");

        let bytes = source.as_bytes();
        let chunk = 9usize;
        let mut offset = 0;
        while offset < bytes.len() {
            let end = (offset + chunk).min(bytes.len());
            let text = std::str::from_utf8(&bytes[offset..end]).expect("utf-8");
            d.feed(text).unwrap();
            for ev in &d.take_events() {
                b.push(ev).unwrap();
            }
            let patches = b.take_patches();
            if let Some((s, r)) = b.tree_ref() {
                p.update(s, r, &patches);
            }
            offset = end;
        }
        // Close: remaining delta.
        let patches = b.take_patches();
        if let Some((s, r)) = b.tree_ref() {
            p.update(s, r, &patches);
        }
        // Final layout pass (mirrors the close path in engine/batch.rs).
        p.finalize_layout();

        let streamed_model = p
            .previous_model
            .as_ref()
            .expect("projector must retain a model after close");
        let streamed_by_path = model_by_path(streamed_model);

        // 1. Parent `$` mapping must carry a row per top-level key.
        let baseline_root_rows = baseline_model
            .nodes
            .iter()
            .find(|n| n.path.is_empty())
            .map(|n| n.rows.len())
            .unwrap_or(0);
        let streamed_root_rows = streamed_model
            .nodes
            .iter()
            .find(|n| n.path.is_empty())
            .map(|n| n.rows.len())
            .unwrap_or(0);
        assert_eq!(
            streamed_root_rows, baseline_root_rows,
            "root `$` mapping lost rows: baseline has {baseline_root_rows} rows \
             (one per key incl. tex0), streamed has {streamed_root_rows}"
        );

        // 2. Every child node must exist at the same y/height as baseline
        //    (tex0 stacked below positions, never overlapping).
        for (path, (base_y, base_h, base_rows)) in &baseline_by_path {
            let Some((stream_y, stream_h, stream_rows)) = streamed_by_path.get(path) else {
                panic!("streamed model is missing node at path {path:?}");
            };
            assert_eq!(
                stream_rows, base_rows,
                "node {path:?} table rows mismatch: baseline={base_rows:?} streamed={stream_rows:?}"
            );
            assert_eq!(
                (*stream_y, *stream_h),
                (*base_y, *base_h),
                "node {path:?} layout mismatch: baseline (y={base_y},h={base_h}) \
                 streamed (y={stream_y},h={stream_h}) — sibling nodes overlapping"
            );
        }
    }

    /// Bug 2 minimal staged repro: split chunks so that the parent Sequence
    /// is first emitted as a *non-table* (empty content), then the next chunk
    /// adds the first child Mapping and tree-side promotes the Sequence to
    /// a header-table. The projector must not leave the Mapping as an
    /// independent sibling in the consumer's node set.
    #[test]
    fn streaming_table_promotion_mid_stream_does_not_leak_child_mappings() {
        // Chunk boundary chosen so chunk 1 stops right after `[`, leaving
        // the Sequence empty; chunk 2 introduces the first Mapping element.
        let chunks = [
            r#"{"items":["#,
            r#"{"a":1,"b":2},"#,
            r#"{"a":3,"b":4},"#,
            r#"{"a":5,"b":6}]}"#,
        ];

        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");

        let mut consumer: HashMap<u32, crate::document::protocol::GraphNodeData> = HashMap::new();

        for (idx, ch) in chunks.iter().enumerate() {
            d.feed(ch).unwrap();
            for ev in &d.take_events() {
                b.push(ev).unwrap();
            }
            let patches = b.take_patches();
            let Some((s, r)) = b.tree_ref() else {
                continue;
            };
            let Some(u) = p.update(s, r, &patches) else {
                continue;
            };

            for n in &u.delta.nodes_added {
                consumer.insert(n.render_handle, n.clone());
            }
            for n in &u.delta.nodes_updated {
                consumer.insert(n.render_handle, n.clone());
            }
            for &rh in &u.delta.nodes_removed {
                consumer.remove(&rh);
            }

            let table_paths: Vec<Vec<crate::document::protocol::GraphPathSeg>> = consumer
                .values()
                .filter(|n| n.table.is_some())
                .map(|n| n.path.clone())
                .collect();

            for tp in &table_paths {
                let leaks: Vec<(u32, Vec<crate::document::protocol::GraphPathSeg>)> = consumer
                    .values()
                    .filter(|n| {
                        n.table.is_none() && n.path.len() == tp.len() + 1 && n.path.starts_with(tp)
                    })
                    .map(|n| (n.render_handle, n.path.clone()))
                    .collect();
                assert!(
                    leaks.is_empty(),
                    "after chunk {idx} ({:?}): table path={tp:?} leaked {} child mapping(s): {:?}",
                    ch,
                    leaks.len(),
                    leaks,
                );
            }
        }

        // Also sanity-check the final state: consumer must contain exactly
        // 1 table node for "items" and no independent Mapping nodes for its
        // 3 elements.
        let table_count = consumer.values().filter(|n| n.table.is_some()).count();
        assert_eq!(
            table_count, 1,
            "final consumer state must have exactly 1 table; got {table_count} (consumer={consumer:#?})"
        );
        let items_table = consumer
            .values()
            .find(|n| n.table.is_some())
            .expect("items table");
        let item_path = &items_table.path;
        let leaked_finals: Vec<u32> = consumer
            .values()
            .filter(|n| {
                n.table.is_none()
                    && n.path.len() == item_path.len() + 1
                    && n.path.starts_with(item_path)
            })
            .map(|n| n.render_handle)
            .collect();
        assert!(
            leaked_finals.is_empty(),
            "final state still leaks {} mapping(s) under items: {leaked_finals:?}",
            leaked_finals.len()
        );
    }

    /// Repro for the `complex.json` drag-in bug: a root Mapping whose trailing
    /// keys are scalars (true/false/""/null) following large containers. Those
    /// scalar values must stay inline in the root's value cells — never become
    /// standalone floating graph nodes with broken edges. After streaming +
    /// close, the model must match the single-shot baseline exactly: same node
    /// count, same paths, and the root keeps one row per key.
    #[test]
    fn streaming_root_trailing_scalars_match_baseline_after_close() {
        let source = r#"{"empty array":[],"obj":{"a":1,"b":2},"items":[1,2,3],"t":true,"f":false,"empty string":"","n":null}"#;

        let baseline_model = {
            let b = builder_from_source(source);
            let (s, r) = b.tree_ref().unwrap();
            crate::graph::graph_projection_service::build_graph_model_for_tree(s, r, "json")
                .expect("baseline model build")
        };
        let baseline_paths: std::collections::BTreeSet<String> = baseline_model
            .nodes
            .iter()
            .map(|n| {
                n.path
                    .iter()
                    .map(|seg| match seg {
                        PathSeg::Key(k) => k.clone(),
                        PathSeg::Index(i) => format!("[{i}]"),
                    })
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .collect();

        // Stream in small chunks so the root keys arrive across boundaries.
        let mut d = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut b = Builder::new();
        b.enable_patches();
        let mut p = StreamingGraphProjector::new("json");

        let bytes = source.as_bytes();
        let chunk = 7usize;
        let mut offset = 0;
        while offset < bytes.len() {
            let end = (offset + chunk).min(bytes.len());
            let text = std::str::from_utf8(&bytes[offset..end]).expect("utf-8");
            d.feed(text).unwrap();
            for ev in &d.take_events() {
                b.push(ev).unwrap();
            }
            let patches = b.take_patches();
            if let Some((s, r)) = b.tree_ref() {
                p.update(s, r, &patches);
            }
            offset = end;
        }
        let patches = b.take_patches();
        if let Some((s, r)) = b.tree_ref() {
            p.update(s, r, &patches);
        }
        p.finalize_layout();

        let streamed_model = p
            .previous_model
            .as_ref()
            .expect("projector must retain a model after close");
        let streamed_paths: std::collections::BTreeSet<String> = streamed_model
            .nodes
            .iter()
            .map(|n| {
                n.path
                    .iter()
                    .map(|seg| match seg {
                        PathSeg::Key(k) => k.clone(),
                        PathSeg::Index(i) => format!("[{i}]"),
                    })
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .collect();

        // Core bug: scalar values must never surface as standalone nodes.
        // Every node baseline omits as inline (scalars, nested scalars) must be
        // absent from the streamed model too.
        for scalar_path in ["t", "f", "n", "empty string", "obj.a", "obj.b"] {
            assert!(
                !streamed_paths.contains(scalar_path),
                "scalar key {scalar_path:?} leaked as a standalone graph node; \
                 streamed paths = {streamed_paths:?}"
            );
        }
        assert_eq!(
            streamed_paths, baseline_paths,
            "streamed model must exactly match baseline; baseline={baseline_paths:?} streamed={streamed_paths:?}",
        );

        // Root `$` keeps one row per key (7 keys).
        let baseline_root_rows = baseline_model
            .nodes
            .iter()
            .find(|n| n.path.is_empty())
            .map(|n| n.rows.len())
            .unwrap_or(0);
        let streamed_root_rows = streamed_model
            .nodes
            .iter()
            .find(|n| n.path.is_empty())
            .map(|n| n.rows.len())
            .unwrap_or(0);
        assert_eq!(
            streamed_root_rows, baseline_root_rows,
            "root `$` mapping must keep one row per key"
        );

        // Every edge must connect real existing nodes (no broken/orphan edges).
        for edge in &streamed_model.edges {
            assert!(
                streamed_model
                    .nodes
                    .iter()
                    .any(|n| n.render_handle == edge.from_render_handle),
                "edge from_render_handle {} has no node",
                edge.from_render_handle
            );
            assert!(
                streamed_model
                    .nodes
                    .iter()
                    .any(|n| n.render_handle == edge.to_render_handle),
                "edge to_render_handle {} has no node",
                edge.to_render_handle
            );
        }
    }

    #[test]
    fn baseline_headerless_table_nested_items_remain_expandable_in_main_graph() {
        let source =
            r#"{"rows":[{"name":"a","meta":{"score":1}},{"name":"b","meta":{"score":2}}]}"#;
        let b = builder_from_source(source);
        let (store, root) = b.tree_ref().unwrap();
        let model =
            crate::graph::graph_projection_service::build_graph_model_for_tree(store, root, "json")
                .expect("baseline model build");

        let table_path = vec![PathSeg::Key("rows".to_string())];
        let table = model
            .nodes
            .iter()
            .find(|node| node.path == table_path)
            .and_then(|node| node.table.as_ref())
            .expect("rows table exists");
        let expandable_items: Vec<Vec<PathSeg>> = model
            .nodes
            .iter()
            .filter(|node| {
                node.table.is_none()
                    && node.path.len() == table_path.len() + 1
                    && node.path.starts_with(&table_path)
            })
            .map(|node| node.path.clone())
            .collect();

        assert_eq!(table.header_height, 0);
        assert_eq!(
            expandable_items,
            vec![
                vec![PathSeg::Key("rows".to_string()), PathSeg::Index(0)],
                vec![PathSeg::Key("rows".to_string()), PathSeg::Index(1)],
            ],
            "headerless structural items must remain expandable",
        );
    }

    #[test]
    fn prompt_diff_events_streaming_edges_keep_nested_value_under_second_item() {
        let source = include_str!("../../../../test/fixtures/json/prompt_diff_events.1.json");
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(true);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");
        let mut nodes = HashMap::new();
        let mut edges = HashMap::new();

        let chunk_size = crate::stream::chunk_size::select_chunk_size(source.len());
        for chunk in source.as_bytes().chunks(chunk_size) {
            decoder.feed(std::str::from_utf8(chunk).unwrap()).unwrap();
            for event in &decoder.take_events() {
                builder.push(event).unwrap();
            }
            let patches = builder.take_patches();
            if let Some((store, root)) = builder.tree_ref()
                && let Some(update) = projector.update(store, root, &patches)
            {
                apply_streaming_delta_to_consumer(&mut nodes, &mut edges, &update.delta);
            }
        }
        for event in decoder.finish_events().unwrap() {
            builder.push(&event).unwrap();
        }
        let patches = builder.take_patches();
        let (store, root) = builder.tree_ref().expect("streaming fixture tree");
        if let Some(update) = projector.update(store, root, &patches) {
            apply_streaming_delta_to_consumer(&mut nodes, &mut edges, &update.delta);
        }

        let handle_for = |path: &[PathSeg]| {
            let expected = path
                .iter()
                .map(|segment| match segment {
                    PathSeg::Key(key) => key.clone(),
                    PathSeg::Index(index) => format!("[{index}]"),
                })
                .collect::<Vec<_>>()
                .join(".");
            nodes
                .values()
                .find(|node| path_key(node) == expected)
                .map(|node| node.render_handle)
                .unwrap_or_else(|| panic!("missing graph node for path {path:?}"))
        };
        let root_handle = handle_for(&[]);
        let item_handle = handle_for(&[PathSeg::Index(1)]);
        let value_handle = handle_for(&[PathSeg::Index(1), PathSeg::Key("value".to_owned())]);

        assert!(edges.values().any(|edge| {
            edge.from_render_handle == root_handle
                && edge.to_render_handle == item_handle
                && edge.from_row == 1
        }));
        assert!(
            edges.values().any(|edge| {
                edge.from_render_handle == item_handle && edge.to_render_handle == value_handle
            }),
            "nodes={:?}, edges={:?}",
            nodes
                .values()
                .map(|node| (node.render_handle, path_key(node)))
                .collect::<Vec<_>>(),
            edges
                .values()
                .map(|edge| (
                    edge.from_render_handle,
                    edge.from_row,
                    edge.to_render_handle,
                    edge.to_row
                ))
                .collect::<Vec<_>>()
        );
        assert!(!edges.values().any(|edge| {
            edge.from_render_handle == root_handle && edge.to_render_handle == value_handle
        }));
    }

    #[test]
    fn streaming_non_root_empty_containers_match_baseline_without_extra_nodes() {
        let source = r#"{"empty_array":[],"empty_object":{},"values":[1,2],"object":{"x":1}}"#;
        let baseline_model = {
            let b = builder_from_source(source);
            let (store, root) = b.tree_ref().unwrap();
            crate::graph::graph_projection_service::build_graph_model_for_tree(store, root, "json")
                .expect("baseline model build")
        };
        let baseline_paths: std::collections::BTreeSet<String> = baseline_model
            .nodes
            .iter()
            .map(|node| {
                node.path
                    .iter()
                    .map(|seg| match seg {
                        PathSeg::Key(key) => key.clone(),
                        PathSeg::Index(index) => format!("[{index}]"),
                    })
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .collect();

        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");

        let bytes = source.as_bytes();
        let mut offset = 0;
        while offset < bytes.len() {
            let end = (offset + 6).min(bytes.len());
            let text = std::str::from_utf8(&bytes[offset..end]).expect("utf-8");
            decoder.feed(text).unwrap();
            for event in &decoder.take_events() {
                builder.push(event).unwrap();
            }
            let patches = builder.take_patches();
            if let Some((store, root)) = builder.tree_ref() {
                projector.update(store, root, &patches);
            }
            offset = end;
        }

        let patches = builder.take_patches();
        if let Some((store, root)) = builder.tree_ref() {
            projector.update(store, root, &patches);
        }
        projector.finalize_layout();

        let streamed_model = projector
            .previous_model
            .as_ref()
            .expect("projector should retain model");
        let streamed_paths: std::collections::BTreeSet<String> = streamed_model
            .nodes
            .iter()
            .map(|node| {
                node.path
                    .iter()
                    .map(|seg| match seg {
                        PathSeg::Key(key) => key.clone(),
                        PathSeg::Index(index) => format!("[{index}]"),
                    })
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .collect();

        assert_eq!(
            streamed_paths, baseline_paths,
            "non-root empty containers must remain inline; baseline={baseline_paths:?} streamed={streamed_paths:?}",
        );
    }

    #[test]
    fn streaming_header_table_column_growth_replaces_table_with_table_patch() {
        let chunks = [r#"{"rows":[{"a":1},"#, r#"{"a":2,"b":3}]}"#];
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");
        let mut updates = Vec::new();

        for chunk in chunks {
            decoder.feed(chunk).unwrap();
            for event in &decoder.take_events() {
                builder.push(event).unwrap();
            }
            let patches = builder.take_patches();
            let (store, root) = builder.tree_ref().expect("streaming tree");
            updates.push(
                projector
                    .update(store, root, &patches)
                    .expect("projection update"),
            );
        }

        let second = updates.last().expect("second update");
        assert!(
            !second.delta.table_patches.iter().any(|patch| matches!(
                patch,
                TablePatch::ColumnsAdded { .. } | TablePatch::CellsUpdated { .. }
            )),
            "column growth is deferred until table replacement, not expressed as column/cell patches; delta={:?}",
            second.delta,
        );
        assert!(
            !second
                .delta
                .nodes_updated
                .iter()
                .any(|node| node.table.is_some()),
            "column growth must not send a full table node through nodes_updated; delta={:?}",
            second.delta,
        );
        let table = second
            .delta
            .table_patches
            .iter()
            .find_map(|patch| match patch {
                TablePatch::TableReplaced { table, .. } => Some(table),
                _ => None,
            })
            .expect("table close must replace the table via table patch");
        assert!(
            table.columns.iter().any(|column| column.text == "b"),
            "replacement table must contain the new column; columns={:?}",
            table.columns,
        );
        assert!(
            table
                .rows
                .first()
                .is_some_and(|row| row.cells.len() == table.columns.len()),
            "replacement rows must be aligned to replacement columns; table={:?}",
            table,
        );
    }

    #[test]
    fn streaming_header_table_same_schema_rows_do_not_rebuild_table_per_row() {
        let chunks = [
            r#"{"rows":[{"a":1,"b":2}"#,
            r#",{"a":3,"b":4},{"a":5,"b":6},{"a":7,"b":8}]}"#,
        ];
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");

        decoder.feed(chunks[0]).unwrap();
        for event in &decoder.take_events() {
            builder.push(event).unwrap();
        }
        let patches = builder.take_patches();
        let (store, root) = builder.tree_ref().expect("streaming tree");
        projector
            .update(store, root, &patches)
            .expect("initial projection");

        decoder.feed(chunks[1]).unwrap();
        for event in &decoder.take_events() {
            builder.push(event).unwrap();
        }
        let patches = builder.take_patches();
        let (store, root) = builder.tree_ref().expect("streaming tree");
        let second = projector
            .update(store, root, &patches)
            .expect("incremental projection");

        assert_eq!(
            second.rows_appended, 3,
            "same-schema rows should append as rows; delta={:?}",
            second.delta,
        );
        assert!(
            !second
                .delta
                .nodes_updated
                .iter()
                .any(|node| node.table.is_some()),
            "same-schema row append must not send a full table node; delta={:?}",
            second.delta,
        );
    }

    #[test]
    fn streaming_height_only_table_growth_uses_table_and_layout_patches() {
        let chunks = [
            r#"{"rows":[{"a":1}"#,
            r#",{"a":2},{"a":3},{"a":4},{"a":5}]}"#,
        ];
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");

        decoder.feed(chunks[0]).unwrap();
        for event in &decoder.take_events() {
            builder.push(event).unwrap();
        }
        let patches = builder.take_patches();
        let (store, root) = builder.tree_ref().expect("streaming tree");
        let first = projector
            .update(store, root, &patches)
            .expect("initial projection");
        let table_handle = first
            .delta
            .nodes_added
            .iter()
            .find(|node| node.table.is_some())
            .map(|node| node.render_handle)
            .expect("initial projection should add a table");

        decoder.feed(chunks[1]).unwrap();
        for event in &decoder.take_events() {
            builder.push(event).unwrap();
        }
        let patches = builder.take_patches();
        let (store, root) = builder.tree_ref().expect("streaming tree");
        let second = projector
            .update(store, root, &patches)
            .expect("incremental projection");

        assert!(
            !second
                .delta
                .nodes_updated
                .iter()
                .any(|node| node.render_handle == table_handle && node.table.is_some()),
            "height-only table growth should not replace the full table node; delta={:?}",
            second.delta,
        );
        assert!(
            second.delta.table_patches.iter().any(|patch| matches!(
                patch,
                TablePatch::RowsAppended {
                    table_handle: h,
                    rows,
                    ..
                } if *h == table_handle && !rows.is_empty()
            )),
            "height-only table growth must append rows; delta={:?}",
            second.delta,
        );
        assert!(
            second.delta.layout_patches.iter().any(|patch| matches!(
                patch,
                LayoutPatch::NodeBoundsUpdated {
                    render_handle,
                    ..
                } if *render_handle == table_handle
            )),
            "height-only table growth must carry a node bounds layout patch; delta={:?}",
            second.delta,
        );
    }

    #[test]
    fn materializer_appends_batch_with_dense_handles() {
        let b = builder_from_source(r#"{"items":[1,2],"object":{"x":1}}"#);
        let (store, root) = b.tree_ref().unwrap();
        let batch =
            crate::graph::graph_projection_service::build_graph_model_for_tree(store, root, "json")
                .expect("batch model");
        let mut model = GraphModel::default();

        let materialized = crate::graph::graph_materialize::append_batch_into_current_model(
            &mut model,
            batch,
            &[],
        );

        assert_eq!(materialized.offset, 0);
        assert_eq!(materialized.nodes_added.len(), model.nodes.len());
        for (index, node) in model.nodes.iter().enumerate() {
            assert_eq!(
                node.render_handle, index as u32,
                "materialized model must remain dense append-only"
            );
        }
        assert_eq!(materialized.edges_added.len(), model.edges.len());
    }

    #[test]
    fn streaming_tail_append_replays_layout_suffix_instead_of_whole_model() {
        let mut source_prefix = String::from("{");
        for index in 0..256 {
            if index > 0 {
                source_prefix.push(',');
            }
            source_prefix.push_str(&format!(r#""k{index}":[0,1]"#));
        }
        source_prefix.push(',');
        let source_tail = r#""tail":[0,1]}"#;

        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");

        decoder.feed(&source_prefix).unwrap();
        for event in &decoder.take_events() {
            builder.push(event).unwrap();
        }
        let patches = builder.take_patches();
        let (store, root) = builder.tree_ref().expect("prefix tree");
        projector
            .update(store, root, &patches)
            .expect("prefix projection");
        let prefix_node_count = projector
            .previous_model
            .as_ref()
            .expect("prefix model")
            .nodes
            .len();
        assert!(
            prefix_node_count > 200,
            "test must build a large enough graph to catch whole-model layout replay"
        );

        decoder.feed(source_tail).unwrap();
        for event in &decoder.take_events() {
            builder.push(event).unwrap();
        }
        let patches = builder.take_patches();
        let (store, root) = builder.tree_ref().expect("complete tree");
        projector
            .update(store, root, &patches)
            .expect("tail projection");

        let metrics = projector.last_layout_metrics();
        assert!(
            metrics.y_events_replayed < prefix_node_count,
            "tail append should replay a DFS suffix, not the whole model; metrics={metrics:?}, prefix_node_count={prefix_node_count}"
        );
        assert!(
            metrics.x_width_touches < prefix_node_count / 4,
            "tail append should update X width stats from touched handles, not scan every node; metrics={metrics:?}, prefix_node_count={prefix_node_count}"
        );
    }

    #[test]
    fn streaming_deltas_never_remove_and_close_layout_is_noop() {
        let chunks = [
            r#"{"rows":["#,
            r#"{"a":1},"#,
            r#"{"a":2,"b":{"nested":true}}],"tail":false}"#,
        ];
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(false);
        let mut builder = Builder::new();
        builder.enable_patches();
        let mut projector = StreamingGraphProjector::new("json");

        for chunk in chunks {
            decoder.feed(chunk).unwrap();
            for event in &decoder.take_events() {
                builder.push(event).unwrap();
            }
            let patches = builder.take_patches();
            let (store, root) = builder.tree_ref().expect("streaming tree");
            let update = projector
                .update(store, root, &patches)
                .expect("projection update");
            assert!(
                update.delta.nodes_removed.is_empty() && update.delta.edges_removed.is_empty(),
                "streaming graph build must be monotonic; delta={:?}",
                update.delta,
            );
        }

        assert!(
            projector.finalize_layout().is_none(),
            "close must not produce layout correction delta",
        );
    }
}
