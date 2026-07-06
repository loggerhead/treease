use std::collections::HashMap;
use std::collections::HashSet;

use crate::language::SemType;
use crate::operators::NodeKind;

use super::graph_builder::GraphNode;
use super::graph_builder::{GraphKind, GraphModel, GraphTable, PathSeg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentInfo {
    pub stable_id: u64,
    pub render_handle: u32,
    pub parent_stable_id: Option<u64>,
    pub path: Vec<PathSeg>,
    pub depth: u32,
    pub kind: GraphKind,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub bottom: i32,
    pub value_kind: Option<NodeKind>,
}

pub type GraphFragment = FragmentInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCellImpact {
    pub table_stable_id: u64,
    pub table_render_handle: u32,
    pub row_index: u32,
    pub column_index: u32,
    pub column_key: String,
    pub cell_path: Vec<PathSeg>,
    pub value_kind: NodeKind,
    pub is_header_table: bool,
    pub is_editable_scalar_value: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GraphFragmentIndex {
    by_stable_id: HashMap<u64, FragmentInfo>,
    by_path: HashMap<Vec<PathSeg>, u64>,
    table_cells: Vec<TableCellImpact>,
    children_by_parent: HashMap<u64, Vec<u64>>,
}

impl GraphFragmentIndex {
    pub fn build(model: &GraphModel) -> Self {
        let mut by_stable_id = HashMap::with_capacity(model.nodes.len());
        let mut by_path = HashMap::with_capacity(model.nodes.len());
        let mut table_cells = Vec::new();
        let render_to_stable = model
            .nodes
            .iter()
            .map(|node| (node.render_handle, node_stable_id(node)))
            .collect::<HashMap<_, _>>();
        let mut parent_by_render = HashMap::with_capacity(model.edges.len());
        for edge in &model.edges {
            let stable_id = render_to_stable
                .get(&edge.from_render_handle)
                .copied()
                .unwrap_or(u64::from(edge.from_render_handle));
            parent_by_render.insert(edge.to_render_handle, stable_id);
        }
        let bottom_by_render = fragment_bottoms_by_render_handle(model);
        for node in &model.nodes {
            let stable_id = node_stable_id(node);
            let fragment = FragmentInfo {
                stable_id,
                render_handle: node.render_handle,
                parent_stable_id: parent_by_render.get(&node.render_handle).copied(),
                path: node.path.clone(),
                depth: node.depth,
                kind: node.kind,
                x: node.x,
                y: node.y,
                width: node.width,
                height: node.height,
                bottom: bottom_by_render
                    .get(&node.render_handle)
                    .copied()
                    .unwrap_or(node.y + node.height),
                value_kind: None,
            };
            if let Some(table) = &node.table {
                index_table_cells(stable_id, node.render_handle, table, &mut table_cells);
            }
            by_path.insert(fragment.path.clone(), fragment.stable_id);
            by_stable_id.insert(fragment.stable_id, fragment);
        }
        let mut children_by_parent: HashMap<u64, Vec<u64>> =
            HashMap::with_capacity(model.edges.len());
        for fragment in by_stable_id.values() {
            if let Some(parent_sid) = fragment.parent_stable_id {
                children_by_parent
                    .entry(parent_sid)
                    .or_default()
                    .push(fragment.stable_id);
            }
        }
        Self {
            by_stable_id,
            by_path,
            table_cells,
            children_by_parent,
        }
    }

    pub fn get_by_stable_id(&self, stable_id: u64) -> Option<&FragmentInfo> {
        self.by_stable_id.get(&stable_id)
    }

    /// Returns the stable IDs of direct children for a given parent stable ID.
    pub fn children_for_stable_id(&self, stable_id: u64) -> Option<&[u64]> {
        self.children_by_parent
            .get(&stable_id)
            .map(|v| v.as_slice())
    }

    /// Recursively collects all stable IDs in the subtree rooted at `stable_id`,
    /// including the root itself.
    pub fn collect_subtree_stable_ids(&self, stable_id: u64) -> Vec<u64> {
        let mut result = Vec::new();
        self.collect_subtree_ids_recursive(stable_id, &mut result);
        result
    }

    fn collect_subtree_ids_recursive(&self, stable_id: u64, out: &mut Vec<u64>) {
        out.push(stable_id);
        if let Some(children) = self.children_by_parent.get(&stable_id) {
            for &child_sid in children {
                self.collect_subtree_ids_recursive(child_sid, out);
            }
        }
    }

    /// Recursively collects all render handles in the subtree rooted at `stable_id`.
    /// Root included, leaves included. Returns handles in DFS order.
    pub fn collect_subtree_render_handles(&self, stable_id: u64) -> Vec<u32> {
        let stable_ids = self.collect_subtree_stable_ids(stable_id);
        stable_ids
            .into_iter()
            .filter_map(|sid| self.by_stable_id.get(&sid).map(|f| f.render_handle))
            .collect()
    }

    pub fn stable_id_for_path(&self, path: &[PathSeg]) -> Option<u64> {
        self.by_path.get(path).copied()
    }

    pub fn find_table_cell_by_path(&self, path: &[PathSeg]) -> Option<&TableCellImpact> {
        self.table_cells.iter().find(|cell| cell.cell_path == path)
    }

    pub fn indexed_table_cell_count(&self) -> usize {
        self.table_cells.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &FragmentInfo> {
        self.by_stable_id.values()
    }

    // ── Incremental update API ───────────────────────────────────────────

    /// Remove all traces of the subtree rooted at `stable_id`.
    /// Removes fragments, path entries, table cells, and child adjacency.
    /// Does NOT refresh ancestor bottoms — caller should call
    /// `refresh_ancestor_bottoms()` if needed.
    pub fn remove_subtree(&mut self, stable_id: u64) {
        // Collect all stable_ids in the subtree via DFS
        let mut to_remove = Vec::new();
        self.collect_subtree_ids_recursive_map(stable_id, &mut to_remove);

        // Collect paths to remove (separate from mutable borrow)
        let paths_to_remove: Vec<Vec<PathSeg>> = to_remove
            .iter()
            .filter_map(|sid| self.by_stable_id.get(sid).map(|f| f.path.clone()))
            .collect();

        for path in &paths_to_remove {
            self.by_path.remove(path);
        }

        // Remove from children_by_parent entries that pointed to removed ids
        let removed_set: HashSet<u64> = to_remove.iter().copied().collect();
        self.children_by_parent.retain(|_, children| {
            children.retain(|c| !removed_set.contains(c));
            !children.is_empty()
        });

        // Remove from children_by_parent (key entries) and by_stable_id
        for sid in &to_remove {
            self.children_by_parent.remove(sid);
            self.by_stable_id.remove(sid);
        }

        // Remove table cells whose table_stable_id is in the removed set
        self.table_cells
            .retain(|cell| !removed_set.contains(&cell.table_stable_id));
    }

    /// Insert a single fragment into the index.
    pub fn insert_fragment(&mut self, fragment: FragmentInfo) {
        let stable_id = fragment.stable_id;
        if let Some(parent_sid) = fragment.parent_stable_id {
            self.children_by_parent
                .entry(parent_sid)
                .or_default()
                .push(stable_id);
        }
        self.by_path
            .insert(fragment.path.clone(), fragment.stable_id);
        self.by_stable_id.insert(fragment.stable_id, fragment);
    }

    /// Refresh `bottom` values up the ancestor chain from `leaf_stable_id`.
    /// Returns the number of ancestors updated (0 if unchanged).
    ///
    /// Note: this currently requires the full model to recalculate subtree_bottom.
    /// In a full incremental implementation, this would use cached child bottoms.
    pub fn refresh_ancestor_bottom(&mut self, leaf_stable_id: u64) -> u32 {
        let mut count = 0;

        // Build ancestor chain separately
        let ancestor_chain: Vec<u64> = {
            let mut chain = Vec::new();
            let mut cursor = self
                .by_stable_id
                .get(&leaf_stable_id)
                .and_then(|f| f.parent_stable_id);
            while let Some(sid) = cursor {
                chain.push(sid);
                cursor = self.by_stable_id.get(&sid).and_then(|f| f.parent_stable_id);
            }
            chain
        };

        for ancestor_sid in &ancestor_chain {
            // Compute max child bottom from current children
            let max_child_bottom = self
                .children_by_parent
                .get(ancestor_sid)
                .map(|kids| {
                    kids.iter()
                        .filter_map(|kid_sid| self.by_stable_id.get(kid_sid))
                        .map(|kid| kid.bottom)
                        .max()
                        .unwrap_or(0)
                })
                .unwrap_or(0);

            let old_bottom = self
                .by_stable_id
                .get(ancestor_sid)
                .map(|f| f.bottom)
                .unwrap_or(0);

            let new_bottom = max_child_bottom.max(
                self.by_stable_id
                    .get(ancestor_sid)
                    .map(|f| f.y + f.height)
                    .unwrap_or(0),
            );

            if new_bottom != old_bottom {
                if let Some(fragment) = self.by_stable_id.get_mut(ancestor_sid) {
                    fragment.bottom = new_bottom;
                    count += 1;
                }
            } else {
                break;
            }
        }

        count
    }

    /// Patch a fragment's geometry and metadata from an updated node.
    /// Updates render handle, path, depth, kind, position, and table cells.
    pub fn patch_fragment_from_node(&mut self, node: &GraphNode) {
        let stable_id = crate::graph::graph_model_index::node_stable_id(node);
        let Some(existing) = self.by_stable_id.get_mut(&stable_id) else {
            return;
        };
        existing.render_handle = node.render_handle;
        existing.path = node.path.clone();
        existing.depth = node.depth;
        existing.kind = node.kind;
        existing.x = node.x;
        existing.y = node.y;
        existing.width = node.width;
        existing.height = node.height;
        existing.bottom = node.y + node.height;
        self.by_path.insert(existing.path.clone(), stable_id);
        self.table_cells
            .retain(|cell| cell.table_stable_id != stable_id);
        if let Some(table) = &node.table {
            index_table_cells(stable_id, node.render_handle, table, &mut self.table_cells);
        }
    }

    /// Build a `FragmentInfo` from a `GraphNode` and its parent stable ID.
    pub fn build_fragment_from_node(
        &self,
        node: &GraphNode,
        parent_stable_id: Option<u64>,
    ) -> FragmentInfo {
        let stable_id = node_stable_id(node);
        FragmentInfo {
            stable_id,
            render_handle: node.render_handle,
            parent_stable_id,
            path: node.path.clone(),
            depth: node.depth,
            kind: node.kind,
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            bottom: node.y + node.height,
            value_kind: None,
        }
    }

    fn collect_subtree_ids_recursive_map(&self, stable_id: u64, out: &mut Vec<u64>) {
        out.push(stable_id);
        if let Some(children) = self.children_by_parent.get(&stable_id) {
            for &child_sid in children {
                self.collect_subtree_ids_recursive_map(child_sid, out);
            }
        }
    }
}

fn node_stable_id(node: &super::graph_builder::GraphNode) -> u64 {
    if node.stable_id != 0 {
        node.stable_id
    } else {
        u64::from(node.render_handle)
    }
}

fn fragment_bottoms_by_render_handle(model: &GraphModel) -> HashMap<u32, i32> {
    let node_bottoms = model
        .nodes
        .iter()
        .map(|node| (node.render_handle, node.y + node.height))
        .collect::<HashMap<_, _>>();
    let mut children_by_parent: HashMap<u32, Vec<u32>> = HashMap::with_capacity(model.edges.len());
    for edge in &model.edges {
        if node_bottoms.contains_key(&edge.from_render_handle)
            && node_bottoms.contains_key(&edge.to_render_handle)
        {
            children_by_parent
                .entry(edge.from_render_handle)
                .or_default()
                .push(edge.to_render_handle);
        }
    }
    let mut bottoms = HashMap::with_capacity(model.nodes.len());
    let mut visiting = HashSet::new();
    for node in &model.nodes {
        let bottom = fragment_bottom_for_render_handle(
            node.render_handle,
            &node_bottoms,
            &children_by_parent,
            &mut bottoms,
            &mut visiting,
        );
        bottoms.insert(node.render_handle, bottom);
    }
    bottoms
}
fn fragment_bottom_for_render_handle(
    render_handle: u32,
    node_bottoms: &HashMap<u32, i32>,
    children_by_parent: &HashMap<u32, Vec<u32>>,
    bottoms: &mut HashMap<u32, i32>,
    visiting: &mut HashSet<u32>,
) -> i32 {
    if let Some(bottom) = bottoms.get(&render_handle).copied() {
        return bottom;
    }
    let own_bottom = node_bottoms.get(&render_handle).copied().unwrap_or(0);
    if !visiting.insert(render_handle) {
        return own_bottom;
    }
    let mut bottom = own_bottom;
    if let Some(children) = children_by_parent.get(&render_handle) {
        for child in children {
            bottom = bottom.max(fragment_bottom_for_render_handle(
                *child,
                node_bottoms,
                children_by_parent,
                bottoms,
                visiting,
            ));
        }
    }
    visiting.remove(&render_handle);
    bottoms.insert(render_handle, bottom);
    bottom
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::graph_builder::{
        BezierArgs, GraphEdge, GraphKind, GraphModel, GraphNode, GraphNodeKey, PathSeg,
    };

    fn graph_key(handle: u32) -> GraphNodeKey {
        GraphNodeKey {
            stable_id: u64::from(handle),
            path: vec![PathSeg::Key(format!("node-{handle}"))],
        }
    }

    fn graph_node(handle: u32, y: i32, height: i32) -> GraphNode {
        let key = graph_key(handle);
        GraphNode {
            render_handle: handle,
            stable_id: u64::from(handle),
            key: key.clone(),
            kind: GraphKind::Scalar,
            depth: 0,
            x: 0,
            y,
            width: 10,
            height,
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

    fn graph_edge(from: u32, to: u32) -> GraphEdge {
        GraphEdge {
            from_render_handle: from,
            from_key: graph_key(from),
            from_row: 0,
            to_render_handle: to,
            to_key: graph_key(to),
            to_row: 0,
            bezier_args: BezierArgs::default(),
        }
    }

    #[test]
    fn fragment_bottoms_by_render_handle_computes_branch_bottoms_from_child_adjacency() {
        let model = GraphModel {
            nodes: vec![
                graph_node(1, 0, 10),
                graph_node(2, 20, 10),
                graph_node(3, 80, 10),
                graph_node(4, 35, 10),
            ],
            edges: vec![graph_edge(1, 2), graph_edge(1, 3), graph_edge(2, 4)],
            ..Default::default()
        };

        let bottoms = fragment_bottoms_by_render_handle(&model);

        assert_eq!(bottoms.get(&1), Some(&90));
        assert_eq!(bottoms.get(&2), Some(&45));
        assert_eq!(bottoms.get(&3), Some(&90));
        assert_eq!(bottoms.get(&4), Some(&45));
    }

    #[test]
    fn graph_fragment_index_build_reads_precomputed_bottoms() {
        let model = GraphModel {
            nodes: vec![
                graph_node(1, 0, 10),
                graph_node(2, 20, 10),
                graph_node(3, 80, 10),
                graph_node(4, 35, 10),
            ],
            edges: vec![graph_edge(1, 2), graph_edge(1, 3), graph_edge(2, 4)],
            ..Default::default()
        };

        let index = GraphFragmentIndex::build(&model);

        assert_eq!(index.get_by_stable_id(1).unwrap().bottom, 90);
        assert_eq!(index.get_by_stable_id(2).unwrap().bottom, 45);
        assert_eq!(index.get_by_stable_id(3).unwrap().bottom, 90);
        assert_eq!(index.get_by_stable_id(4).unwrap().bottom, 45);
    }

    #[test]
    fn fragment_bottoms_by_render_handle_bounds_cycles_to_known_node_bottoms() {
        let model = GraphModel {
            nodes: vec![graph_node(1, 0, 10), graph_node(2, 20, 10)],
            edges: vec![graph_edge(1, 2), graph_edge(2, 1)],
            ..Default::default()
        };

        let bottoms = fragment_bottoms_by_render_handle(&model);

        assert_eq!(bottoms.get(&1), Some(&30));
        assert_eq!(bottoms.get(&2), Some(&30));
    }
}
fn index_table_cells(
    table_stable_id: u64,
    table_render_handle: u32,
    table: &GraphTable,
    table_cells: &mut Vec<TableCellImpact>,
) {
    let is_header_table = table.header_height > 0 && !table.columns.is_empty();
    if !is_header_table {
        return;
    }

    for (row_index, row) in table.rows.iter().enumerate() {
        for (column_index, cell) in row.iter().enumerate() {
            if column_index == 0 {
                continue;
            }
            let Some(column) = table.columns.get(column_index) else {
                continue;
            };
            if !cell.editable {
                continue;
            }
            let Some(value_kind) = cell_source_kind(cell) else {
                continue;
            };
            if value_kind != NodeKind::Scalar {
                continue;
            }
            table_cells.push(TableCellImpact {
                table_stable_id,
                table_render_handle,
                row_index: row_index as u32,
                column_index: column_index as u32,
                column_key: column.text.clone(),
                cell_path: cell.path.clone(),
                value_kind,
                is_header_table,
                is_editable_scalar_value: cell.editable,
            });
        }
    }
}

fn cell_source_kind(cell: &super::graph_builder::GraphCell) -> Option<NodeKind> {
    if let Some(sem_type) = cell.sem_type.as_deref().and_then(SemType::from_string) {
        return Some(match sem_type {
            SemType::Map => NodeKind::Mapping,
            SemType::Seq => NodeKind::Sequence,
            SemType::Str | SemType::Int | SemType::Float | SemType::Boolean | SemType::Nil => {
                NodeKind::Scalar
            }
        });
    }
    let source = cell.source? as *const crate::operators::TreeNode;
    // SAFETY: graph cells only store raw pointers created from live TreeNode references.
    let node = unsafe { source.as_ref() }?;
    Some(node.kind)
}
