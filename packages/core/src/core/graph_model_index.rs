use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use super::graph_builder::{
    GraphCell, GraphEdge, GraphModel, GraphNode, GraphNodeKey, GraphRow, GraphTable, PathSeg,
};
use super::graph_fragment_index::{FragmentInfo, GraphFragmentIndex, TableCellImpact};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct StableEdgeKey {
    pub from_stable_id: u64,
    pub from_row: i32,
    pub to_stable_id: u64,
    pub to_row: i32,
}

#[derive(Debug, Clone)]
struct IndexedEdge {
    edge: GraphEdge,
    hash: u64,
}

#[derive(Debug, Clone)]
pub struct GraphModelIndex {
    node_hashes: HashMap<u64, u64>,
    render_handles_by_stable_id: HashMap<u64, u32>,
    stable_ids_by_render_handle: HashMap<u32, u64>,
    edges: HashMap<StableEdgeKey, IndexedEdge>,
    parent_by_to_render_handle: HashMap<u32, u32>,
    fragment_index: GraphFragmentIndex,
}

impl GraphModelIndex {
    pub fn build(model: &GraphModel) -> Self {
        let mut node_hashes = HashMap::with_capacity(model.nodes.len());
        let mut render_handles_by_stable_id = HashMap::with_capacity(model.nodes.len());
        let mut stable_ids_by_render_handle = HashMap::with_capacity(model.nodes.len());

        for node in &model.nodes {
            let stable_id = node_stable_id(node);
            node_hashes.insert(stable_id, hash_graph_node(node));
            render_handles_by_stable_id.insert(stable_id, node.render_handle);
            stable_ids_by_render_handle.insert(node.render_handle, stable_id);
        }
        let mut parent_by_to_render_handle = HashMap::with_capacity(model.edges.len());

        let mut edges = HashMap::with_capacity(model.edges.len());
        for edge in &model.edges {
            let key = stable_edge_key_for(&stable_ids_by_render_handle, edge);
            if edge.from_render_handle != edge.to_render_handle {
                parent_by_to_render_handle.insert(edge.to_render_handle, edge.from_render_handle);
            }
            edges.insert(
                key,
                IndexedEdge {
                    edge: edge.clone(),
                    hash: hash_edge(edge),
                },
            );
        }

        Self {
            node_hashes,
            render_handles_by_stable_id,
            stable_ids_by_render_handle,
            edges,
            parent_by_to_render_handle,
            fragment_index: GraphFragmentIndex::build(model),
        }
    }

    pub fn fragment_index(&self) -> &GraphFragmentIndex {
        &self.fragment_index
    }

    pub fn stable_id_for_path(&self, path: &[PathSeg]) -> Option<u64> {
        self.fragment_index.stable_id_for_path(path)
    }

    pub fn fragment_for_stable_id(&self, stable_id: u64) -> Option<&FragmentInfo> {
        self.fragment_index.get_by_stable_id(stable_id)
    }

    pub fn find_table_cell_by_path(&self, path: &[PathSeg]) -> Option<&TableCellImpact> {
        self.fragment_index.find_table_cell_by_path(path)
    }

    pub(crate) fn node_hash(&self, stable_id: u64) -> Option<u64> {
        self.node_hashes.get(&stable_id).copied()
    }

    pub(crate) fn render_handle_for_stable_id(&self, stable_id: u64) -> Option<u32> {
        self.render_handles_by_stable_id.get(&stable_id).copied()
    }

    pub(crate) fn edge_key_for(&self, edge: &GraphEdge) -> StableEdgeKey {
        stable_edge_key_for(&self.stable_ids_by_render_handle, edge)
    }

    pub(crate) fn edge_hash(&self, key: &StableEdgeKey) -> Option<u64> {
        self.edges.get(key).map(|entry| entry.hash)
    }

    pub(crate) fn stable_id_render_handles(&self) -> impl Iterator<Item = (u64, u32)> + '_ {
        self.render_handles_by_stable_id
            .iter()
            .map(|(stable_id, render_handle)| (*stable_id, *render_handle))
    }

    pub(crate) fn edge_entries(
        &self,
    ) -> impl Iterator<Item = (&StableEdgeKey, &GraphEdge, u64)> + '_ {
        self.edges
            .iter()
            .map(|(key, entry)| (key, &entry.edge, entry.hash))
    }

    /// Patch a single node's hash, render handle mappings, and fragment index
    /// without rebuilding edges. Used for table-cell in-place updates.
    pub fn patch_node_without_edge_changes(&mut self, node: &GraphNode) {
        let stable_id = node_stable_id(node);
        self.node_hashes.insert(stable_id, hash_graph_node(node));
        self.render_handles_by_stable_id
            .insert(stable_id, node.render_handle);
        self.stable_ids_by_render_handle
            .insert(node.render_handle, stable_id);
        self.fragment_index.patch_fragment_from_node(node);
    }

    fn parent_stable_id_for_render_handle(&self, render_handle: u32) -> Option<u64> {
        self.parent_by_to_render_handle
            .get(&render_handle)
            .and_then(|parent_render_handle| {
                self.stable_ids_by_render_handle.get(parent_render_handle)
            })
            .copied()
    }

    #[cfg(test)]
    fn parent_render_handle_for_test(&self, render_handle: u32) -> Option<u32> {
        self.parent_by_to_render_handle.get(&render_handle).copied()
    }
    // ── Incremental update API ───────────────────────────────────────────

    /// Remove all traces of the subtree rooted at `subtree_root_stable_id`.
    pub fn remove_subtree(&mut self, subtree_root_stable_id: u64) {
        let subtree_ids: Vec<u64> = self
            .fragment_index
            .collect_subtree_stable_ids(subtree_root_stable_id);

        // Fallback to full rebuild if subtree is large
        if subtree_ids.len() > self.node_hashes.len() / 2 {
            *self = GraphModelIndex::build(&self.to_model());
            return;
        }

        // Collect render handles BEFORE removing mappings (they're needed for edge cleanup)
        let removed_handles: HashSet<u32> = subtree_ids
            .iter()
            .filter_map(|sid| self.render_handles_by_stable_id.get(sid).copied())
            .collect();

        // Remove node_hashes and render handle mappings
        for sid in &subtree_ids {
            self.node_hashes.remove(sid);
            if let Some(rh) = self.render_handles_by_stable_id.remove(sid) {
                self.stable_ids_by_render_handle.remove(&rh);
            }
        }

        // Remove edges connected to the removed subtree
        self.edges.retain(|_, entry| {
            !removed_handles.contains(&entry.edge.from_render_handle)
                && !removed_handles.contains(&entry.edge.to_render_handle)
        });
        self.parent_by_to_render_handle
            .retain(|to, from| !removed_handles.contains(to) && !removed_handles.contains(from));

        // Delegate to fragment_index
        self.fragment_index.remove_subtree(subtree_root_stable_id);
    }

    /// Insert or update fragments for the given nodes. Updates node hashes,
    /// render handle mappings, and the fragment index.
    pub fn insert_or_update_fragments(&mut self, nodes: &[GraphNode]) {
        for node in nodes {
            let stable_id = node_stable_id(node);
            let new_hash = hash_graph_node(node);

            self.node_hashes.insert(stable_id, new_hash);
            self.render_handles_by_stable_id
                .insert(stable_id, node.render_handle);
            self.stable_ids_by_render_handle
                .insert(node.render_handle, stable_id);

            let parent_stable_id = self.parent_stable_id_for_render_handle(node.render_handle);

            // Build and insert fragment (separate borrow scopes)
            let fragment = self
                .fragment_index
                .build_fragment_from_node(node, parent_stable_id);
            self.fragment_index.insert_fragment(fragment);
        }
    }

    /// render handles (checking both from and to) and inserts new edges.

    pub fn replace_edges_for_subtree(&mut self, new_edges: &[GraphEdge]) {
        let subtree_handles: HashSet<u32> = new_edges
            .iter()
            .flat_map(|e| [e.from_render_handle, e.to_render_handle])
            .collect();

        self.edges.retain(|_, entry| {
            !subtree_handles.contains(&entry.edge.from_render_handle)
                && !subtree_handles.contains(&entry.edge.to_render_handle)
        });
        self.parent_by_to_render_handle
            .retain(|to, from| !subtree_handles.contains(to) && !subtree_handles.contains(from));

        for edge in new_edges {
            if edge.from_render_handle != edge.to_render_handle {
                self.parent_by_to_render_handle
                    .insert(edge.to_render_handle, edge.from_render_handle);
            }
            let key = stable_edge_key_for(&self.stable_ids_by_render_handle, edge);
            self.edges.insert(
                key,
                IndexedEdge {
                    edge: edge.clone(),
                    hash: hash_edge(edge),
                },
            );
        }
    }

    /// Removes the old subtree, inserts rebuilt subtree data, and replaces
    /// edges. Falls back to full `GraphModelIndex::build(&model)` if the
    /// incremental update cannot be safely applied.
    ///
    /// `impact_stable_id` is the root of the changed subtree.
    /// `impacted_stable_ids` contains all stable IDs that were in the old subtree.
    pub fn incremental_update(
        &mut self,
        model: &GraphModel,
        impact_stable_id: u64,
        impacted_stable_ids: &[u64],
    ) {
        let impacted_set: HashSet<u64> = impacted_stable_ids.iter().copied().collect();

        // Remove old subtree data
        self.remove_subtree(impact_stable_id);

        // Insert rebuilt subtree nodes where the stable ID was in the old subtree
        // or is brand new (not in old index).
        let rebuilt_nodes: Vec<&GraphNode> = model
            .nodes
            .iter()
            .filter(|n| {
                let sid = node_stable_id(n);
                impacted_set.contains(&sid) || self.render_handle_for_stable_id(sid).is_none()
            })
            .collect();

        // Collect rebuilt subtree edges from the model
        let rebuilt_render_handles: HashSet<u32> =
            rebuilt_nodes.iter().map(|n| n.render_handle).collect();
        let rebuilt_edges: Vec<GraphEdge> = model
            .edges
            .iter()
            .filter(|e| {
                rebuilt_render_handles.contains(&e.from_render_handle)
                    || rebuilt_render_handles.contains(&e.to_render_handle)
            })
            .cloned()
            .collect();

        // Apply incremental updates: edges first so parent_by_to_render_handle is populated before fragment lookup
        let rebuilt_node_owned: Vec<GraphNode> = rebuilt_nodes.into_iter().cloned().collect();
        self.replace_edges_for_subtree(&rebuilt_edges);
        self.insert_or_update_fragments(&rebuilt_node_owned);
    }

    /// Rebuild a model from current index state (fallback for incremental failure).
    fn to_model(&self) -> GraphModel {
        let nodes: Vec<GraphNode> = self
            .fragment_index
            .iter()
            .map(|f| {
                let key = GraphNodeKey {
                    stable_id: f.stable_id,
                    path: f.path.clone(),
                };
                GraphNode {
                    render_handle: f.render_handle,
                    stable_id: f.stable_id,
                    key,
                    kind: f.kind,
                    depth: f.depth,
                    x: f.x,
                    y: f.y,
                    width: f.width,
                    height: f.height,
                    box_args: super::graph_builder::BoxArgs::default(),
                    path: f.path.clone(),
                    meta: super::graph_builder::GraphCell::default(),
                    rows: Vec::new(),
                    table: None,
                    preorder_first: f.render_handle,
                    preorder_last: f.render_handle,
                    source: None,
                }
            })
            .collect();
        let mut model = GraphModel {
            nodes,
            edges: self
                .edges
                .values()
                .map(|entry| entry.edge.clone())
                .collect(),
            ..Default::default()
        };
        model.rebuild_edge_index();
        model
    }
}
pub(crate) fn node_stable_id(node: &GraphNode) -> u64 {
    if node.stable_id != 0 {
        node.stable_id
    } else {
        u64::from(node.render_handle)
    }
}

pub(crate) fn stable_edge_key_for(
    stable_ids: &HashMap<u32, u64>,
    edge: &GraphEdge,
) -> StableEdgeKey {
    StableEdgeKey {
        from_stable_id: stable_ids
            .get(&edge.from_render_handle)
            .copied()
            .unwrap_or(edge.from_render_handle as u64),
        from_row: edge.from_row,
        to_stable_id: stable_ids
            .get(&edge.to_render_handle)
            .copied()
            .unwrap_or(edge.to_render_handle as u64),
        to_row: edge.to_row,
    }
}

pub(crate) fn hash_graph_node(node: &GraphNode) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    node_stable_id(node).hash(&mut state);
    node.kind.hash(&mut state);
    node.depth.hash(&mut state);
    node.x.hash(&mut state);
    node.y.hash(&mut state);
    node.width.hash(&mut state);
    node.height.hash(&mut state);
    node.path.hash(&mut state);
    hash_graph_cell(&mut state, &node.meta);
    for row in &node.rows {
        hash_graph_row(&mut state, row);
    }
    hash_graph_table(&mut state, node.table.as_ref());
    state.finish()
}

fn hash_graph_cell(state: &mut impl Hasher, cell: &GraphCell) {
    cell.text.hash(state);
    cell.sem_type.hash(state);
    cell.path.hash(state);
    cell.value.hash(state);
    cell.editable.hash(state);
    hash_cell_bounds(state, cell.bounds);
    hash_cell_bounds(state, cell.text_bounds);
    hash_box_args(state, cell.box_args);
    cell.source.hash(state);
}

fn hash_graph_row(state: &mut impl Hasher, row: &GraphRow) {
    row.index.hash(state);
    hash_cell_bounds(state, row.bounds);
    hash_cell_bounds(state, row.abs_bounds);
    hash_cell_bounds(state, row.cell_bounds);
    hash_box_args(state, row.box_args);
    hash_box_args(state, row.cell_box_args);
    for cell in &row.cells {
        hash_graph_cell(state, cell);
    }
}

fn hash_graph_table(state: &mut impl Hasher, table: Option<&GraphTable>) {
    if let Some(table) = table {
        table.columns.len().hash(state);
        for cell in &table.columns {
            hash_graph_cell(state, cell);
        }
        table.column_widths.hash(state);
        for row in &table.rows {
            hash_graph_row(state, row);
        }
        table.width.hash(state);
        table.total_height.hash(state);
        table.view_height.hash(state);
        table.header_height.hash(state);
        table.row_height.hash(state);
        table.key.hash(state);
        table.count.hash(state);
        table.source.hash(state);
    } else {
        0u8.hash(state);
    }
}

fn hash_cell_bounds(state: &mut impl Hasher, bounds: super::graph_builder::CellBounds) {
    bounds.x.hash(state);
    bounds.y.hash(state);
    bounds.width.hash(state);
    bounds.height.hash(state);
}

fn hash_box_args(state: &mut impl Hasher, args: super::graph_builder::BoxArgs) {
    args.x.hash(state);
    args.y.hash(state);
    args.width.hash(state);
    args.height.hash(state);
    args.corner_radius.hash(state);
}

pub(crate) fn hash_edge(edge: &GraphEdge) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    edge.from_render_handle.hash(&mut state);
    edge.from_key.hash(&mut state);
    edge.from_row.hash(&mut state);
    edge.to_render_handle.hash(&mut state);
    edge.to_key.hash(&mut state);
    edge.to_row.hash(&mut state);
    edge.bezier_args.hash(&mut state);
    state.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::graph_builder::{
        GraphCell, GraphEdge, GraphKind, GraphModel, GraphNode, GraphNodeKey, PathSeg,
    };

    fn key(stable_id: u64, path: Vec<PathSeg>) -> GraphNodeKey {
        GraphNodeKey { stable_id, path }
    }

    fn node(render_handle: u32, stable_id: u64, path: Vec<PathSeg>, depth: u32) -> GraphNode {
        GraphNode {
            render_handle,
            stable_id,
            key: key(stable_id, path.clone()),
            kind: GraphKind::Object,
            depth,
            x: 0,
            y: 0,
            width: 10,
            height: 10,
            box_args: Default::default(),
            path,
            meta: GraphCell::default(),
            rows: Vec::new(),
            table: None,
            preorder_first: render_handle,
            preorder_last: render_handle,
            source: None,
        }
    }

    fn edge(from: &GraphNode, to: &GraphNode) -> GraphEdge {
        GraphEdge {
            from_render_handle: from.render_handle,
            from_key: from.key.clone(),
            from_row: 0,
            to_render_handle: to.render_handle,
            to_key: to.key.clone(),
            to_row: 0,
            bezier_args: Default::default(),
        }
    }

    #[test]
    fn build_records_parent_by_to_render_handle() {
        let root = node(1, 10, vec![PathSeg::Key("root".to_owned())], 0);
        let child = node(
            2,
            20,
            vec![
                PathSeg::Key("root".to_owned()),
                PathSeg::Key("child".to_owned()),
            ],
            1,
        );
        let model = GraphModel {
            nodes: vec![root.clone(), child.clone()],
            edges: vec![edge(&root, &child)],
            ..Default::default()
        };

        let index = GraphModelIndex::build(&model);

        assert_eq!(
            index.parent_render_handle_for_test(child.render_handle),
            Some(root.render_handle)
        );
        assert_eq!(
            index.parent_render_handle_for_test(root.render_handle),
            None
        );
        assert_eq!(
            index
                .fragment_for_stable_id(child.stable_id)
                .and_then(|fragment| fragment.parent_stable_id),
            Some(root.stable_id)
        );
    }

    #[test]
    fn incremental_update_preserves_fragment_parents_like_full_build() {
        let root = node(1, 10, vec![PathSeg::Key("root".to_owned())], 0);
        let branch = node(
            2,
            20,
            vec![
                PathSeg::Key("root".to_owned()),
                PathSeg::Key("branch".to_owned()),
            ],
            1,
        );
        let old_leaf = node(
            3,
            30,
            vec![
                PathSeg::Key("root".to_owned()),
                PathSeg::Key("branch".to_owned()),
                PathSeg::Key("old".to_owned()),
            ],
            2,
        );
        let new_leaf = node(
            4,
            40,
            vec![
                PathSeg::Key("root".to_owned()),
                PathSeg::Key("branch".to_owned()),
                PathSeg::Key("new".to_owned()),
            ],
            2,
        );

        let old_model = GraphModel {
            nodes: vec![root.clone(), branch.clone(), old_leaf.clone()],
            edges: vec![edge(&root, &branch), edge(&branch, &old_leaf)],
            ..Default::default()
        };
        let new_model = GraphModel {
            nodes: vec![root.clone(), branch.clone(), new_leaf.clone()],
            edges: vec![edge(&root, &branch), edge(&branch, &new_leaf)],
            ..Default::default()
        };

        let mut incremental = GraphModelIndex::build(&old_model);
        incremental.incremental_update(
            &new_model,
            branch.stable_id,
            &[branch.stable_id, old_leaf.stable_id],
        );
        let full = GraphModelIndex::build(&new_model);

        assert_eq!(
            incremental
                .fragment_for_stable_id(new_leaf.stable_id)
                .and_then(|fragment| fragment.parent_stable_id),
            full.fragment_for_stable_id(new_leaf.stable_id)
                .and_then(|fragment| fragment.parent_stable_id)
        );
        assert_eq!(
            incremental.parent_render_handle_for_test(new_leaf.render_handle),
            Some(branch.render_handle)
        );
    }
}
