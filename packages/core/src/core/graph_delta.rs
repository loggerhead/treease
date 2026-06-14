use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use super::graph_model::{GraphEdge, GraphModel, GraphNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphTableCellPatch<'a> {
    pub table_render_handle: u32,
    pub row_index: u32,
    pub column_index: u32,
    pub cell: super::graph_model::GraphCell<'a>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphDelta<'a> {
    pub clear: bool,
    pub nodes_added: Vec<GraphNode<'a>>,
    pub nodes_updated: Vec<GraphNode<'a>>,
    pub nodes_removed: Vec<u32>,
    pub edges_added: Vec<GraphEdge>,
    pub edges_removed: Vec<GraphEdge>,
    pub table_cell_patches: Vec<GraphTableCellPatch<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StableEdgeKey {
    from_stable_id: u64,
    from_row: i32,
    to_stable_id: u64,
    to_row: i32,
}

pub fn build_graph_delta<'a>(
    old_model: Option<&GraphModel<'a>>,
    new_model: &GraphModel<'a>,
) -> GraphDelta<'a> {
    let Some(old_model) = old_model else {
        return GraphDelta {
            clear: true,
            nodes_added: new_model.nodes.clone(),
            nodes_updated: Vec::new(),
            nodes_removed: Vec::new(),
            edges_added: new_model.edges.clone(),
            edges_removed: Vec::new(),
            table_cell_patches: Vec::new(),
        };
    };

    let mut old_hashes = HashMap::new();
    let mut old_render_handles = HashMap::new();
    let mut old_node_stable_ids = HashMap::new();
    let mut old_edges = HashMap::new();

    for node in &old_model.nodes {
        let stable_id = stable_node_key(node);
        old_hashes.insert(stable_id, hash_graph_node(node));
        old_render_handles.insert(stable_id, node.render_handle);
        old_node_stable_ids.insert(node.render_handle, stable_id);
    }
    for edge in &old_model.edges {
        old_edges.insert(stable_edge_key_for(&old_node_stable_ids, edge), *edge);
    }

    let mut nodes_added = Vec::new();
    let mut nodes_updated = Vec::new();
    let mut nodes_removed = Vec::new();
    let mut edges_added = Vec::new();
    let mut edges_removed = Vec::new();
    let mut new_ids = HashMap::new();
    let mut new_node_stable_ids = HashMap::new();

    for node in &new_model.nodes {
        let stable_id = stable_node_key(node);
        new_ids.insert(stable_id, ());
        new_node_stable_ids.insert(node.render_handle, stable_id);
        match old_hashes.get(&stable_id) {
            None => nodes_added.push(*node),
            Some(old_hash) if *old_hash != hash_graph_node(node) => nodes_updated.push(*node),
            _ => {}
        }
    }

    for (stable_id, render_handle) in old_render_handles {
        if !new_ids.contains_key(&stable_id) {
            nodes_removed.push(render_handle);
        }
    }

    let mut new_edges = HashMap::new();
    for edge in &new_model.edges {
        let key = stable_edge_key_for(&new_node_stable_ids, edge);
        new_edges.insert(key, *edge);
        match old_edges.get(&key) {
            None => edges_added.push(*edge),
            Some(old_edge) if hash_edge(old_edge) != hash_edge(edge) => edges_added.push(*edge),
            _ => {}
        }
    }

    for (key, edge) in old_edges {
        match new_edges.get(&key) {
            None => edges_removed.push(edge),
            Some(new_edge) if hash_edge(&edge) != hash_edge(new_edge) => edges_removed.push(edge),
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

fn stable_node_key(node: &GraphNode<'_>) -> u64 {
    if node.stable_id != 0 {
        node.stable_id
    } else {
        u64::from(node.render_handle)
    }
}

fn stable_node_id_for_render_handle(stable_ids: &HashMap<u32, u64>, render_handle: u32) -> u64 {
    stable_ids
        .get(&render_handle)
        .copied()
        .unwrap_or(u64::from(render_handle))
}

fn stable_edge_key_for(stable_ids: &HashMap<u32, u64>, edge: &GraphEdge) -> StableEdgeKey {
    StableEdgeKey {
        from_stable_id: stable_node_id_for_render_handle(stable_ids, edge.from_render_handle),
        from_row: edge.from_row,
        to_stable_id: stable_node_id_for_render_handle(stable_ids, edge.to_render_handle),
        to_row: edge.to_row,
    }
}

fn hash_graph_node(node: &GraphNode<'_>) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    stable_node_key(node).hash(&mut state);
    node.kind.hash(&mut state);
    node.depth.hash(&mut state);
    node.x.hash(&mut state);
    node.y.hash(&mut state);
    node.width.hash(&mut state);
    node.height.hash(&mut state);
    hash_path(&mut state, node.path);
    hash_graph_cell(&mut state, &node.meta);
    for row in node.rows {
        hash_graph_row(&mut state, row);
    }
    if let Some(table) = node.table {
        hash_graph_table(&mut state, &table);
    }
    state.finish()
}

fn hash_graph_cell(state: &mut impl Hasher, cell: &super::graph_model::GraphCell<'_>) {
    cell.text.hash(state);
    cell.value.hash(state);
    cell.format_text.hash(state);
    hash_path(state, cell.path);
    cell.sem_type.hash(state);
    cell.editable.hash(state);
    cell.bounds.hash(state);
    cell.text_bounds.hash(state);
    cell.box_args.hash(state);
    cell.text_args.hash(state);
}

fn hash_graph_row(state: &mut impl Hasher, row: &super::graph_model::GraphRow<'_>) {
    row.index.hash(state);
    row.bounds.hash(state);
    row.abs_bounds.hash(state);
    row.cell_bounds.hash(state);
    row.box_args.hash(state);
    row.cell_box_args.hash(state);
    for cell in row.cells {
        hash_graph_cell(state, cell);
    }
}

fn hash_graph_table(state: &mut impl Hasher, table: &super::graph_model::GraphTable<'_>) {
    for cell in table.columns {
        hash_graph_cell(state, cell);
    }
    table.column_widths.hash(state);
    for row in table.rows {
        hash_graph_row(state, row);
    }
    table.width.hash(state);
    table.total_height.hash(state);
    table.view_height.hash(state);
    table.header_height.hash(state);
    table.row_height.hash(state);
    table.key.hash(state);
    table.count.hash(state);
}

fn hash_path(state: &mut impl Hasher, path: &[crate::wasm_types::PathSeg<'_>]) {
    path.hash(state);
}

fn hash_edge(edge: &GraphEdge) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    edge.from_render_handle.hash(&mut state);
    edge.from_row.hash(&mut state);
    edge.to_render_handle.hash(&mut state);
    edge.to_row.hash(&mut state);
    edge.bezier_args.hash(&mut state);
    state.finish()
}
