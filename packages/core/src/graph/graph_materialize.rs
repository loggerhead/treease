use std::collections::HashSet;

use super::graph_builder::{
    BuilderConfig, GraphCell, GraphEdge, GraphKind, GraphModel, GraphRow, PathSeg,
    SequencePresentation,
};
use super::graph_shape::{
    NodeShapeBuilder, NodeShapePresentation, build_aligned_row_from_mapping,
    build_headerless_inline_row, build_object_rows_from_mapping, header_table_row_has_new_column,
};
use super::graph_topology::{
    DirtyEdge, DirtySet, GraphTopology, SequencePresentationState, TableRowRef,
};
use crate::tree::{TreeNodeKind, TreeStore};

#[derive(Debug, Default)]
pub(crate) struct MaterializedGraphPatch {
    pub added_handles: Vec<u32>,
    pub updated_handles: Vec<u32>,
    pub added_edges: Vec<DirtyEdge>,
    pub added_edge_indexes: Vec<usize>,
    pub rebuilt_table_handles: Vec<u32>,
    pub deferred_table_handles: Vec<u32>,
    pub table_row_touches: Vec<TableRowTouch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableRowTouch {
    pub table_handle: u32,
    pub row_indexes: Vec<usize>,
}

pub(crate) fn materialize_into_current_model(
    topology: &mut GraphTopology,
    model: &mut GraphModel,
    store: &TreeStore,
    dirty: &DirtySet,
    shape_builder: &NodeShapeBuilder<'_>,
    config: &BuilderConfig,
) -> MaterializedGraphPatch {
    let mut result = MaterializedGraphPatch::default();
    let mut rebuilt_tables = HashSet::new();
    let mut row_touched_tables = HashSet::new();

    for &handle in dirty.added_handles() {
        let Some(slot) = topology.slot(handle).cloned() else {
            continue;
        };
        let Some(presentation) = node_shape_presentation(topology, store, slot.node_id) else {
            continue;
        };
        let Some(node) = shape_builder.build_node_from_store(
            store,
            slot.node_id,
            &slot.path,
            slot.depth,
            handle,
            presentation,
        ) else {
            continue;
        };
        if model.nodes.len() == handle as usize {
            model.nodes.push(node.clone());
        } else if let Some(existing) = model.nodes.get_mut(handle as usize) {
            *existing = node.clone();
        } else {
            continue;
        }
        if let Some(slot) = topology.slot_mut(handle) {
            slot.shape = Some(node);
        }
        result.added_handles.push(handle);
    }

    let table_rows = dirty.table_rows();
    let mut row_start = 0;
    while row_start < table_rows.len() {
        let table_handle = table_rows[row_start].table_handle;
        let mut row_end = row_start + 1;
        while row_end < table_rows.len() && table_rows[row_end].table_handle == table_handle {
            row_end += 1;
        }

        if result.added_handles.contains(&table_handle) {
            row_start = row_end;
            continue;
        }
        result.table_row_touches.push(TableRowTouch {
            table_handle,
            row_indexes: table_rows[row_start..row_end]
                .iter()
                .map(|row| row.row_index)
                .collect(),
        });
        row_touched_tables.insert(table_handle);
        if rebuilt_tables.contains(&table_handle) {
            mark_updated(&mut result.updated_handles, table_handle);
            row_start = row_end;
            continue;
        }

        let update_result = apply_table_row_updates(
            topology,
            model,
            store,
            &table_rows[row_start..row_end],
            shape_builder,
            config,
        );
        if update_result.rebuilt {
            rebuilt_tables.insert(table_handle);
            mark_updated(&mut result.rebuilt_table_handles, table_handle);
        }
        if update_result.deferred_replacement {
            mark_updated(&mut result.deferred_table_handles, table_handle);
        }
        mark_updated(&mut result.updated_handles, table_handle);
        row_start = row_end;
    }

    for &handle in dirty.shape_handles() {
        if result.added_handles.contains(&handle) || rebuilt_tables.contains(&handle) {
            continue;
        }
        if row_touched_tables.contains(&handle) {
            continue;
        }
        let Some(slot) = topology.slot(handle).cloned() else {
            continue;
        };
        let Some(current) = model.nodes.get(handle as usize).cloned() else {
            continue;
        };
        let rebuilt = if current.kind == GraphKind::Object {
            let rows = build_object_rows_from_mapping(store, slot.node_id, &current.path);
            if rows.is_empty() {
                node_shape_presentation(topology, store, slot.node_id).and_then(|presentation| {
                    shape_builder.rebuild_node_from_store(
                        store,
                        slot.node_id,
                        &current,
                        presentation,
                    )
                })
            } else {
                let mut node = current.clone();
                shape_builder.install_object_rows(&mut node, rows);
                Some(node)
            }
        } else {
            node_shape_presentation(topology, store, slot.node_id).and_then(|presentation| {
                shape_builder.rebuild_node_from_store(store, slot.node_id, &current, presentation)
            })
        };
        let Some(rebuilt) = rebuilt else {
            continue;
        };
        if same_logical_node(&current, &rebuilt) {
            if let Some(slot) = topology.slot_mut(handle) {
                slot.shape = Some(current);
            }
            continue;
        }
        if let Some(existing) = model.nodes.get_mut(handle as usize) {
            *existing = rebuilt.clone();
        }
        if let Some(slot) = topology.slot_mut(handle) {
            slot.shape = Some(rebuilt);
        }
        mark_updated(&mut result.updated_handles, handle);
    }

    for &edge in dirty.added_edges() {
        insert_missing_edge(model, edge, &mut result);
    }

    result.added_handles.sort_unstable();
    result.updated_handles.sort_unstable();
    result
        .added_edges
        .sort_by_key(|edge| (edge.from, edge.from_row, edge.to, edge.to_row));
    result.rebuilt_table_handles.sort_unstable();
    result.rebuilt_table_handles.dedup();
    result.deferred_table_handles.sort_unstable();
    result.deferred_table_handles.dedup();
    result
}

struct TableRowUpdateContext {
    parent_path: Vec<PathSeg>,
    parent_columns: Vec<GraphCell>,
    column_widths: Vec<i32>,
    is_header_table: bool,
    table_is_open: bool,
    table_node_kind: TreeNodeKind,
}

#[derive(Debug, Clone, Copy, Default)]
struct TableRowUpdateResult {
    rebuilt: bool,
    deferred_replacement: bool,
}

fn apply_table_row_updates(
    topology: &mut GraphTopology,
    model: &mut GraphModel,
    store: &TreeStore,
    rows: &[TableRowRef],
    shape_builder: &NodeShapeBuilder<'_>,
    config: &BuilderConfig,
) -> TableRowUpdateResult {
    let Some(&first_row) = rows.first() else {
        return TableRowUpdateResult::default();
    };
    let Some(context) = table_row_update_context(topology, model, store, first_row) else {
        return TableRowUpdateResult::default();
    };

    let mut applied = false;
    let mut deferred_replacement = false;
    for &row in rows {
        debug_assert_eq!(row.table_handle, first_row.table_handle);
        debug_assert_eq!(row.table_node_id, first_row.table_node_id);

        if context.is_header_table
            && header_table_row_has_new_column(&context.parent_columns, store, row.row_node_id)
        {
            if !context.table_is_open {
                return TableRowUpdateResult {
                    rebuilt: rebuild_table_node(topology, model, store, row, shape_builder),
                    deferred_replacement,
                };
            }
            deferred_replacement = true;
        }

        let Some(next_row) = build_table_row_update(store, row, &context) else {
            continue;
        };
        if table_row_requires_rebuild(config, &context.column_widths, &next_row) {
            if !context.table_is_open {
                return TableRowUpdateResult {
                    rebuilt: rebuild_table_node(topology, model, store, row, shape_builder),
                    deferred_replacement,
                };
            }
            deferred_replacement = true;
        }
        applied |= apply_built_table_row(model, row, next_row, config);
    }

    if applied && let Some(graph_node) = model.nodes.get_mut(first_row.table_handle as usize) {
        sync_table_size(config, graph_node);
    }
    TableRowUpdateResult {
        rebuilt: false,
        deferred_replacement,
    }
}

fn table_row_update_context(
    topology: &GraphTopology,
    model: &GraphModel,
    store: &TreeStore,
    row: TableRowRef,
) -> Option<TableRowUpdateContext> {
    let graph_node = model.nodes.get(row.table_handle as usize)?;
    let table = graph_node.table.as_ref()?;
    let table_node = store.get(row.table_node_id)?;

    Some(TableRowUpdateContext {
        parent_path: graph_node.path.clone(),
        parent_columns: table.columns.clone(),
        column_widths: table.column_widths.clone(),
        is_header_table: matches!(
            topology.sequence_presentation(row.table_node_id),
            Some(SequencePresentationState::HeaderTable)
        ),
        table_is_open: !table_node.sequence_closed(),
        table_node_kind: table_node.kind,
    })
}

fn build_table_row_update(
    store: &TreeStore,
    row: TableRowRef,
    context: &TableRowUpdateContext,
) -> Option<GraphRow> {
    if context.is_header_table
        && store
            .get(row.row_node_id)
            .is_some_and(|node| node.kind == TreeNodeKind::Mapping)
    {
        build_aligned_row_from_mapping(
            store,
            row.row_node_id,
            &context.parent_path,
            &context.parent_columns,
            row.row_index as i32,
        )
    } else if context.table_node_kind == TreeNodeKind::Sequence {
        build_headerless_inline_row(store, row.row_node_id, &context.parent_path)
    } else {
        None
    }
}

fn apply_built_table_row(
    model: &mut GraphModel,
    row: TableRowRef,
    next_row: GraphRow,
    config: &BuilderConfig,
) -> bool {
    let Some(graph_node) = model.nodes.get_mut(row.table_handle as usize) else {
        return false;
    };
    let Some(table) = graph_node.table.as_mut() else {
        return false;
    };

    if row.row_index < table.rows.len() {
        table.rows[row.row_index] = next_row;
    } else if row.row_index == table.rows.len() {
        table.rows.push(next_row);
        table.count = table.rows.len() as i32;
    } else {
        return false;
    }

    super::graph_builder::table_graph::apply_row_bounds_at_index(config, table, row.row_index);
    true
}

fn same_logical_node(
    left: &super::graph_builder::GraphNode,
    right: &super::graph_builder::GraphNode,
) -> bool {
    left.kind == right.kind
        && left.depth == right.depth
        && left.width == right.width
        && left.height == right.height
        && left.path == right.path
        && same_rows_logical(&left.rows, &right.rows)
        && left.table == right.table
}

fn same_rows_logical(
    left: &[super::graph_builder::GraphRow],
    right: &[super::graph_builder::GraphRow],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.index == right.index
                && left.cells.len() == right.cells.len()
                && left.cells.iter().zip(&right.cells).all(|(left, right)| {
                    left.text == right.text
                        && left.value == right.value
                        && left.format_text == right.format_text
                        && left.sem_type == right.sem_type
                        && left.path == right.path
                        && left.editable == right.editable
                })
        })
}

fn rebuild_table_node(
    topology: &mut GraphTopology,
    model: &mut GraphModel,
    store: &TreeStore,
    row: super::graph_topology::TableRowRef,
    shape_builder: &NodeShapeBuilder<'_>,
) -> bool {
    let Some(current) = model.nodes.get(row.table_handle as usize) else {
        return false;
    };
    let Some(presentation) = node_shape_presentation(topology, store, row.table_node_id) else {
        return false;
    };
    let Some(rebuilt) =
        shape_builder.rebuild_node_from_store(store, row.table_node_id, current, presentation)
    else {
        return false;
    };
    if let Some(existing) = model.nodes.get_mut(row.table_handle as usize) {
        *existing = rebuilt.clone();
    }
    if let Some(slot) = topology.slot_mut(row.table_handle) {
        slot.shape = Some(rebuilt);
    }
    true
}

fn node_shape_presentation(
    topology: &GraphTopology,
    store: &TreeStore,
    node_id: crate::tree::NodeId,
) -> Option<NodeShapePresentation> {
    if store.get(node_id)?.kind != TreeNodeKind::Sequence {
        return Some(NodeShapePresentation::NonTable);
    }
    let is_root = topology
        .root_handle()
        .and_then(|handle| topology.slot(handle))
        .is_some_and(|slot| slot.node_id == node_id);
    if is_root && store.get(node_id)?.content.is_empty() {
        return Some(NodeShapePresentation::NonTable);
    }
    match topology.sequence_presentation(node_id) {
        Some(SequencePresentationState::HeaderTable) => Some(NodeShapePresentation::Table(
            SequencePresentation::HeaderTable,
        )),
        Some(SequencePresentationState::HeaderlessTable) => Some(NodeShapePresentation::Table(
            SequencePresentation::HeaderlessTable,
        )),
        Some(
            SequencePresentationState::EmptyOpen
            | SequencePresentationState::EmptyClosed
            | SequencePresentationState::PendingHeaderSchema,
        )
        | None => None,
    }
}

fn table_row_requires_rebuild(
    config: &BuilderConfig,
    column_widths: &[i32],
    row: &super::graph_builder::GraphRow,
) -> bool {
    if row.cells.len() != column_widths.len() {
        return true;
    }
    row.cells.iter().enumerate().any(|(column_index, cell)| {
        let required_width =
            super::graph_builder::table_graph::estimated_table_column_width_for_config(
                config, &cell.text,
            )
            .min(config.table_column_width);
        required_width > column_widths[column_index]
    })
}

fn sync_table_size(config: &BuilderConfig, node: &mut super::graph_builder::GraphNode) {
    let Some(table) = node.table.as_mut() else {
        return;
    };
    table.count = table.rows.len() as i32;
    let base_rows = table.count.max(1);
    table.total_height = table.header_height + table.row_height * base_rows;
    table.view_height = if table.header_height == 0 {
        table.total_height
    } else {
        config.table_max_height.min(table.total_height)
    };
    node.width = table.width + config.node_border_width * 2;
    node.height = table.view_height + config.node_border_width * 2;
    node.box_args.width = node.width;
    node.box_args.height = node.height;
}

fn mark_updated(handles: &mut Vec<u32>, handle: u32) {
    if !handles.contains(&handle) {
        handles.push(handle);
    }
}

fn insert_missing_edge(
    model: &mut GraphModel,
    edge: DirtyEdge,
    result: &mut MaterializedGraphPatch,
) -> bool {
    let Some(graph_edge) = build_edge(model, &edge) else {
        return false;
    };
    let Some(edge_index) = model.insert_edge_if_missing(graph_edge) else {
        return false;
    };
    result.added_edges.push(edge);
    result.added_edge_indexes.push(edge_index);
    true
}

fn build_edge(model: &GraphModel, edge: &DirtyEdge) -> Option<GraphEdge> {
    let from = model.nodes.get(edge.from as usize)?;
    let to = model.nodes.get(edge.to as usize)?;
    Some(GraphEdge {
        from_render_handle: edge.from,
        from_key: from.key.clone(),
        from_row: edge.from_row,
        to_render_handle: edge.to,
        to_key: to.key.clone(),
        to_row: edge.to_row,
        bezier_args: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::graph_builder::{
        BezierArgs, GraphEdge, GraphKind, GraphModel, GraphNode, GraphNodeKey, GraphRow,
        GraphTable, PathSeg,
    };

    fn graph_key(handle: u32) -> GraphNodeKey {
        GraphNodeKey {
            stable_id: u64::from(handle) + 1,
            path: vec![PathSeg::Index(handle as usize)],
        }
    }

    fn graph_node(handle: u32) -> GraphNode {
        let key = graph_key(handle);
        GraphNode {
            render_handle: handle,
            stable_id: u64::from(handle) + 1,
            key: key.clone(),
            kind: GraphKind::Scalar,
            depth: handle,
            x: 0,
            y: 0,
            width: 10,
            height: 10,
            box_args: Default::default(),
            path: key.path.clone(),
            meta: Default::default(),
            rows: Vec::new(),
            table: None,
            preorder_first: handle,
            preorder_last: handle,
            source: None,
        }
    }

    fn graph_edge(from: u32, to: u32, from_row: i32, to_row: i32) -> GraphEdge {
        GraphEdge {
            from_render_handle: from,
            from_key: graph_key(from),
            from_row,
            to_render_handle: to,
            to_key: graph_key(to),
            to_row,
            bezier_args: BezierArgs::default(),
        }
    }

    #[test]
    fn insert_missing_edge_skips_existing_edges_and_keeps_row_identity() {
        let mut model = GraphModel {
            nodes: vec![graph_node(0), graph_node(1)],
            edges: vec![graph_edge(0, 1, 0, 0)],
            ..Default::default()
        };
        model.rebuild_edge_index();
        let mut patch = MaterializedGraphPatch::default();

        assert!(!insert_missing_edge(
            &mut model,
            DirtyEdge {
                from: 0,
                to: 1,
                from_row: 0,
                to_row: 0,
            },
            &mut patch,
        ));
        assert!(insert_missing_edge(
            &mut model,
            DirtyEdge {
                from: 0,
                to: 1,
                from_row: 1,
                to_row: 0,
            },
            &mut patch,
        ));

        assert_eq!(model.edges.len(), 2);
        assert_eq!(model.edge_index_len(), 2);
        assert_eq!(
            patch.added_edges,
            vec![DirtyEdge {
                from: 0,
                to: 1,
                from_row: 1,
                to_row: 0,
            }]
        );
        assert_eq!(patch.added_edge_indexes, vec![1]);
    }

    #[test]
    fn sync_table_size_expands_headerless_tables_with_child_nodes() {
        let mut node = graph_node(0);
        node.kind = GraphKind::Table;
        node.table = Some(GraphTable {
            rows: vec![GraphRow::default(); 5],
            width: 40,
            header_height: 0,
            row_height: 20,
            ..GraphTable::default()
        });
        let mut config = crate::graph::graph_builder::default_config();
        config.table_max_height = 40;

        sync_table_size(&config, &mut node);

        let table = node.table.expect("table exists");
        assert_eq!(table.total_height, 100);
        assert_eq!(table.view_height, 100);
        assert_eq!(node.height, 100 + config.node_border_width * 2);
    }
}
#[cfg(test)]
mod test_util {
    use crate::document::protocol::{GraphEdgeData, GraphNodeData};
    use crate::graph::graph_builder::GraphModel;
    use crate::graph::graph_projection_service::{convert_edge, convert_node};

    #[derive(Debug, Default)]
    pub(crate) struct MaterializedGraphBatch {
        pub offset: u32,
        pub nodes_added: Vec<GraphNodeData>,
        pub edges_added: Vec<GraphEdgeData>,
    }

    pub(crate) fn append_batch_into_current_model(
        model: &mut GraphModel,
        batch: GraphModel,
        skip_set: &[bool],
    ) -> MaterializedGraphBatch {
        let offset = model.nodes.len() as u32;
        let mut result = MaterializedGraphBatch {
            offset,
            ..MaterializedGraphBatch::default()
        };

        for (batch_index, mut node) in batch.nodes.into_iter().enumerate() {
            if skip_set.get(batch_index).copied().unwrap_or(false) {
                continue;
            }
            node.render_handle += offset;
            node.preorder_first += offset;
            node.preorder_last += offset;
            result.nodes_added.push(convert_node(&node));
            model.nodes.push(node);
        }

        for mut edge in batch.edges {
            let from = edge.from_render_handle as usize;
            let to = edge.to_render_handle as usize;
            if skip_set.get(from).copied().unwrap_or(false)
                || skip_set.get(to).copied().unwrap_or(false)
            {
                continue;
            }
            edge.from_render_handle += offset;
            edge.to_render_handle += offset;
            result.edges_added.push(convert_edge(&edge));
            model.edges.push(edge);
        }
        model.rebuild_edge_index();

        result
    }
}

#[cfg(test)]
pub(crate) use test_util::*;
