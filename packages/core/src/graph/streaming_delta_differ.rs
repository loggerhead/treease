use std::collections::HashSet;

use crate::document::protocol::{GraphDelta as ProtocolGraphDelta, GraphNodeData};

use super::graph_builder::{GraphKind, GraphModel};
use super::graph_materialize::MaterializedGraphPatch;
use super::graph_projection_service::{convert_edge, convert_node};
use crate::layout::layout_engine::LayoutChangeSet;

pub(crate) struct StreamingDeltaDiffer;

impl StreamingDeltaDiffer {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn emit_incremental_delta(
        &self,
        model: &GraphModel,
        materialized: &MaterializedGraphPatch,
        layout: &LayoutChangeSet,
        skip_updated_table_handles: &HashSet<u32>,
    ) -> ProtocolGraphDelta {
        let added_handles: HashSet<u32> = materialized.added_handles.iter().copied().collect();
        let mut updated_handles: HashSet<u32> =
            materialized.updated_handles.iter().copied().collect();
        updated_handles.extend(layout.node_handles().iter().copied());

        let mut added: Vec<GraphNodeData> = Vec::with_capacity(materialized.added_handles.len());
        for &handle in &materialized.added_handles {
            let Some(node) = model.nodes.get(handle as usize) else {
                continue;
            };
            let converted = convert_node(node);
            added.push(converted);
        }
        added.sort_by_key(|node| node.render_handle);

        let mut updated: Vec<GraphNodeData> = Vec::with_capacity(updated_handles.len());
        for handle in updated_handles
            .iter()
            .filter(|handle| !added_handles.contains(handle))
        {
            let Some(node) = model.nodes.get(*handle as usize) else {
                continue;
            };
            let table_shape = node.kind == GraphKind::Table || node.table.is_some();
            if table_shape && skip_updated_table_handles.contains(handle) {
                continue;
            }
            let converted = convert_node(node);
            updated.push(converted);
        }
        updated.sort_by_key(|node| node.render_handle);

        let mut emitted_edges = HashSet::new();
        let mut edges = Vec::new();
        for &edge_index in &materialized.added_edge_indexes {
            let Some(candidate) = model.edges.get(edge_index) else {
                continue;
            };
            let key = (
                candidate.from_render_handle,
                candidate.from_row,
                candidate.to_render_handle,
                candidate.to_row,
            );
            if emitted_edges.insert(key) {
                edges.push(convert_edge(candidate));
            }
        }
        for &edge_index in layout.edge_indexes() {
            let Some(candidate) = model.edges.get(edge_index) else {
                continue;
            };
            let key = (
                candidate.from_render_handle,
                candidate.from_row,
                candidate.to_render_handle,
                candidate.to_row,
            );
            if emitted_edges.insert(key) {
                edges.push(convert_edge(candidate));
            }
        }
        edges.sort_by_key(|edge| {
            (
                edge.from_render_handle,
                edge.from_row,
                edge.to_render_handle,
                edge.to_row,
            )
        });

        ProtocolGraphDelta {
            nodes_added: added,
            nodes_updated: updated,
            nodes_removed: Vec::new(),
            edges_added: edges,
            edges_removed: Vec::new(),
            ..ProtocolGraphDelta::default()
        }
    }
}
