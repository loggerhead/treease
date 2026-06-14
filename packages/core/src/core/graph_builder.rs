use crate::operators::{NodeKind, TreeNode};
use std::{
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
};

use super::graph_identity::stable_node_id;
use super::lang_spec::FormatLanguage;

pub mod graph_layout;
pub mod node_meta;
pub mod scalar_object_graph;
pub mod sequence_graph;
pub mod shared;
pub mod table_graph;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphLanguage {
    None,
    Json,
    Yaml,
    Toml,
    Unknown,
}

impl GraphLanguage {
    /// Parse a language name string into a `GraphLanguage`.
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "json" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "toml" => Self::Toml,
            "" => Self::None,
            _ => Self::Unknown,
        }
    }
}

impl From<FormatLanguage> for GraphLanguage {
    fn from(fl: FormatLanguage) -> Self {
        match fl {
            FormatLanguage::Json => Self::Json,
            FormatLanguage::Yaml => Self::Yaml,
            FormatLanguage::Toml => Self::Toml,
            FormatLanguage::Python | FormatLanguage::Javascript | FormatLanguage::Csv => {
                Self::Unknown
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderConfig {
    pub key_width: i32,
    pub value_width: i32,
    pub row_height: i32,
    pub row_padding_x: i32,
    pub row_padding_y: i32,
    pub node_border_width: i32,
    pub v_gap: i32,
    pub h_gap: i32,
    pub table_max_height: i32,
    pub table_row_height: i32,
    pub table_header_height: i32,
    pub table_column_width: i32,
    pub avg_char_width_x10: i32,
    pub font_size: i32,
    pub meta_path_min_segments: i32,
    pub meta_path_min_chars: i32,
    pub meta_path_keep_tail_segments: i32,
    pub expand_header_table_rows: bool,
}

pub fn default_config() -> BuilderConfig {
    BuilderConfig {
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
        expand_header_table_rows: false,
        meta_path_keep_tail_segments: 1,
    }
}

impl Default for BuilderConfig {
    fn default() -> Self {
        default_config()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathSeg {
    Key(String),
    Index(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphKind {
    Scalar,
    Object,
    Table,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GraphNodeKey {
    pub stable_id: u64,
    pub path: Vec<PathSeg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoxArgs {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub corner_radius: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct BezierArgs {
    pub from_x: i32,
    pub from_y: i32,
    pub c1x: i32,
    pub c1y: i32,
    pub c2x: i32,
    pub c2y: i32,
    pub to_x: i32,
    pub to_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TextVerticalAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct TextArgs {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text: String,
    pub text_align: TextAlign,
    pub text_vertical_align: TextVerticalAlign,
    pub editable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GraphCell {
    pub text: String,
    pub sem_type: Option<String>,
    pub path: Vec<PathSeg>,
    pub value: String,
    pub editable: bool,
    pub bounds: CellBounds,
    pub text_bounds: CellBounds,
    pub box_args: BoxArgs,
    pub text_args: TextArgs,
    pub source: Option<usize>,
    pub format_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GraphRow {
    pub index: i32,
    pub key: GraphCell,
    pub value: GraphCell,
    pub cells: Vec<GraphCell>,
    pub bounds: CellBounds,
    pub abs_bounds: CellBounds,
    pub cell_bounds: CellBounds,
    pub box_args: BoxArgs,
    pub cell_box_args: BoxArgs,
}

impl Deref for GraphRow {
    type Target = [GraphCell];

    fn deref(&self) -> &Self::Target {
        &self.cells
    }
}

impl DerefMut for GraphRow {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cells
    }
}

impl<'a> IntoIterator for &'a GraphRow {
    type Item = &'a GraphCell;
    type IntoIter = std::slice::Iter<'a, GraphCell>;

    fn into_iter(self) -> Self::IntoIter {
        self.cells.iter()
    }
}

impl<'a> IntoIterator for &'a mut GraphRow {
    type Item = &'a mut GraphCell;
    type IntoIter = std::slice::IterMut<'a, GraphCell>;

    fn into_iter(self) -> Self::IntoIter {
        self.cells.iter_mut()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GraphTable {
    pub columns: Vec<GraphCell>,
    pub rows: Vec<GraphRow>,
    pub column_widths: Vec<i32>,
    pub width: i32,
    pub total_height: i32,
    pub view_height: i32,
    pub header_height: i32,
    pub row_height: i32,
    pub key: String,
    pub count: i32,
    pub source: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNode {
    pub render_handle: u32,
    pub stable_id: u64,
    pub key: GraphNodeKey,
    pub kind: GraphKind,
    pub depth: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub box_args: BoxArgs,
    pub path: Vec<PathSeg>,
    pub meta: GraphCell,
    pub rows: Vec<GraphRow>,
    pub table: Option<GraphTable>,
    /// Preorder traversal range: first render_handle in this subtree.
    pub preorder_first: u32,
    /// Preorder traversal range: last render_handle in this subtree.
    pub preorder_last: u32,
    /// Raw pointer to the source TreeNode (for ancestry checks).
    /// Only valid while the source tree is alive.
    pub source: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GraphEdge {
    pub from_render_handle: u32,
    pub from_key: GraphNodeKey,
    pub from_row: i32,
    pub to_render_handle: u32,
    pub to_key: GraphNodeKey,
    pub to_row: i32,
    pub bezier_args: BezierArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GraphEdgeKey {
    pub from: u32,
    pub to: u32,
    pub from_row: i32,
    pub to_row: i32,
}

impl GraphEdgeKey {
    pub(crate) fn from_edge(edge: &GraphEdge) -> Self {
        Self {
            from: edge.from_render_handle,
            to: edge.to_render_handle,
            from_row: edge.from_row,
            to_row: edge.to_row,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct GraphEdgeIndex {
    keys: HashMap<GraphEdgeKey, usize>,
    by_from: Vec<Vec<usize>>,
    by_to: Vec<Vec<usize>>,
}

impl GraphEdgeIndex {
    fn clear(&mut self) {
        self.keys.clear();
        self.by_from.clear();
        self.by_to.clear();
    }

    fn ensure_node_capacity(&mut self, len: usize) {
        self.by_from.resize_with(len, Vec::new);
        self.by_to.resize_with(len, Vec::new);
    }

    fn insert(&mut self, edge_index: usize, edge: &GraphEdge) -> bool {
        let key = GraphEdgeKey::from_edge(edge);
        if self.keys.contains_key(&key) {
            return false;
        }
        self.keys.insert(key, edge_index);
        let from = edge.from_render_handle as usize;
        let to = edge.to_render_handle as usize;
        self.ensure_node_capacity(from.max(to) + 1);
        self.by_from[from].push(edge_index);
        self.by_to[to].push(edge_index);
        true
    }

    fn contains(&self, edge: &GraphEdge) -> bool {
        self.keys.contains_key(&GraphEdgeKey::from_edge(edge))
    }

    fn incident_to_handles(&self, handles: &[u32]) -> Vec<usize> {
        let mut seen = HashSet::new();
        for &handle in handles {
            let index = handle as usize;
            if let Some(edges) = self.by_from.get(index) {
                seen.extend(edges.iter().copied());
            }
            if let Some(edges) = self.by_to.get(index) {
                seen.extend(edges.iter().copied());
            }
        }
        let mut indexes: Vec<_> = seen.into_iter().collect();
        indexes.sort_unstable();
        indexes
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.keys.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GraphModel {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub(crate) edge_index: GraphEdgeIndex,
}

impl GraphModel {
    pub fn from_parts(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> Self {
        let mut model = Self {
            nodes,
            edges,
            ..Default::default()
        };
        model.rebuild_edge_index();
        model
    }

    pub(crate) fn rebuild_edge_index(&mut self) {
        self.edge_index.clear();
        self.edge_index.ensure_node_capacity(self.nodes.len());
        for (edge_index, edge) in self.edges.iter().enumerate() {
            self.edge_index.insert(edge_index, edge);
        }
    }

    pub(crate) fn insert_edge_if_missing(&mut self, edge: GraphEdge) -> Option<usize> {
        if self.edge_index.contains(&edge) {
            return None;
        }
        let edge_index = self.edges.len();
        self.edges.push(edge);
        let inserted = self.edge_index.insert(
            edge_index,
            self.edges.get(edge_index).expect("edge just pushed"),
        );
        inserted.then_some(edge_index)
    }

    pub(crate) fn edge_indexes_incident_to_handles(&self, handles: &[u32]) -> Vec<usize> {
        self.edge_index.incident_to_handles(handles)
    }

    #[cfg(test)]
    pub(crate) fn edge_index_len(&self) -> usize {
        self.edge_index.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequencePresentation {
    HeaderTable,
    HeaderlessTable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RowColumnWidths {
    pub key: i32,
    pub value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CurveControlPoints {
    pub c1x: i32,
    pub c1y: i32,
    pub c2x: i32,
    pub c2y: i32,
}

#[derive(Debug, Clone)]
pub struct GraphBuilder {
    pub config: BuilderConfig,
    pub language: GraphLanguage,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub expand_table_children: bool,
}

impl GraphBuilder {
    pub fn new(config: BuilderConfig, language: GraphLanguage) -> Self {
        Self {
            config,
            language,
            nodes: Vec::new(),
            edges: Vec::new(),
            expand_table_children: false,
        }
    }

    pub fn reset(&mut self) {
        self.nodes.clear();
        self.edges.clear();
    }

    pub fn build(&mut self, root: &TreeNode) -> GraphModel {
        self.build_subtree(root, &[])
    }

    pub fn build_subtree(&mut self, root: &TreeNode, path: &[PathSeg]) -> GraphModel {
        self.reset();
        self.build_node(root, 0, path);
        self.layout_graph(0);
        self.apply_node_bounds();
        self.apply_edge_bezier_args();
        let mut model = GraphModel {
            nodes: self.nodes.clone(),
            edges: self.edges.clone(),
            ..Default::default()
        };
        model.rebuild_edge_index();
        model
    }

    pub fn build_node_only(
        &self,
        node: &TreeNode,
        depth: u32,
        path: &[PathSeg],
        render_handle: u32,
    ) -> GraphNode {
        let kind = graph_kind_for_node(node);
        let rows = self.build_rows(node, path);
        let row_widths = self.row_column_widths(&rows);
        let table = if kind == GraphKind::Table {
            Some(self.build_table(node, path))
        } else {
            None
        };
        let inner_width = table
            .as_ref()
            .map(|value| value.width)
            .unwrap_or(row_widths.key + row_widths.value);
        let inner_height = table
            .as_ref()
            .map(|value| value.view_height)
            .unwrap_or_else(|| (rows.len() as i32).max(1) * self.config.row_height);
        let width = inner_width + self.config.node_border_width * 2;
        let height = inner_height + self.config.node_border_width * 2;
        let x = depth as i32 * (self.config.value_width + self.config.h_gap);
        let y = render_handle as i32 * (self.config.row_height + self.config.v_gap / 2);
        let key = graph_node_key(kind, path);
        let meta = node_meta::make_meta_cell(self, node, path, x, y, inner_width);

        GraphNode {
            render_handle,
            stable_id: key.stable_id,
            key,
            kind,
            depth,
            x,
            y,
            width,
            height,
            box_args: BoxArgs {
                x,
                y,
                width,
                height,
                corner_radius: 0,
            },
            path: path.to_vec(),
            meta,
            rows,
            table,
            preorder_first: render_handle,
            preorder_last: render_handle,
            source: Some(node as *const TreeNode as usize),
        }
    }

    pub fn sequence_presentation(&self, node: &TreeNode) -> SequencePresentation {
        sequence_graph::sequence_presentation(node)
    }

    pub fn layout_graph(&mut self, root_render_handle: u32) {
        graph_layout::layout_graph(self, root_render_handle);
    }

    pub fn apply_edge_bezier_args(&mut self) {
        graph_layout::apply_edge_bezier_args(self);
    }

    pub fn make_edge(
        &self,
        from_render_handle: u32,
        from_key: GraphNodeKey,
        from_row: i32,
        to_render_handle: u32,
        to_key: GraphNodeKey,
        to_row: i32,
    ) -> GraphEdge {
        graph_layout::make_edge(
            from_render_handle,
            from_key,
            from_row,
            to_render_handle,
            to_key,
            to_row,
        )
    }

    /// Build a graph node and its children recursively.
    pub(super) fn build_node(&mut self, node: &TreeNode, depth: u32, path: &[PathSeg]) -> u32 {
        let render_handle = self.nodes.len() as u32;
        let graph_node = self.build_node_only(node, depth, path, render_handle);
        let kind = graph_node.kind;
        self.nodes.push(graph_node);

        match kind {
            GraphKind::Object => {
                if node.kind == NodeKind::Mapping {
                    scalar_object_graph::build_object_children(
                        self,
                        node,
                        depth,
                        render_handle,
                        path,
                    );
                }
            }
            GraphKind::Table => {
                if node.kind == NodeKind::Sequence {
                    let presentation = self.sequence_presentation(node);
                    if shared::sequence_table_children_can_expand(self, node)
                        && (presentation == SequencePresentation::HeaderlessTable
                            || self.expand_table_children)
                    {
                        self.build_table_children_impl(node, depth, render_handle, path);
                    }
                }
            }
            GraphKind::Scalar => {}
        }

        render_handle
    }

    /// Build children for table (sequence) nodes.
    fn build_table_children_impl(
        &mut self,
        node: &TreeNode,
        depth: u32,
        parent_render_handle: u32,
        path: &[PathSeg],
    ) {
        let presentation = self.sequence_presentation(node);
        let header_offset: i32 = if presentation == SequencePresentation::HeaderTable {
            1
        } else {
            0
        };

        for (idx, item) in node.content.iter().enumerate() {
            let row_index = idx as i32 + header_offset;
            let item_path = shared::append_path(path, PathSeg::Index(idx));

            if presentation == SequencePresentation::HeaderlessTable {
                if !shared::value_node_builds_child(item) {
                    continue;
                }
                let child_handle = self.build_node(item, depth + 1, &item_path);
                let parent_key = self.nodes[parent_render_handle as usize].key.clone();
                let child_key = self.nodes[child_handle as usize].key.clone();
                self.edges.push(self.make_edge(
                    parent_render_handle,
                    parent_key,
                    row_index,
                    child_handle,
                    child_key,
                    0,
                ));
                continue;
            }

            if item.kind == NodeKind::Mapping {
                if !mapping_row_builds_child(item) {
                    continue;
                }
                let child_handle = self.build_node(item, depth + 1, &item_path);
                let parent_key = self.nodes[parent_render_handle as usize].key.clone();
                let child_key = self.nodes[child_handle as usize].key.clone();
                self.edges.push(self.make_edge(
                    parent_render_handle,
                    parent_key,
                    row_index,
                    child_handle,
                    child_key,
                    0,
                ));
                continue;
            }

            if !shared::value_node_builds_child(item) {
                continue;
            }
            let child_handle = self.build_node(item, depth + 1, &item_path);
            let parent_key = self.nodes[parent_render_handle as usize].key.clone();
            let child_key = self.nodes[child_handle as usize].key.clone();
            self.edges.push(self.make_edge(
                parent_render_handle,
                parent_key,
                row_index,
                child_handle,
                child_key,
                0,
            ));
        }
    }

    fn build_rows(&self, node: &TreeNode, path: &[PathSeg]) -> Vec<GraphRow> {
        scalar_object_graph::build_rows(self, node, path)
    }

    fn build_table(&self, node: &TreeNode, path: &[PathSeg]) -> GraphTable {
        table_graph::build_table(self, node, path)
    }

    pub(super) fn row_column_widths(&self, rows: &[GraphRow]) -> RowColumnWidths {
        scalar_object_graph::row_column_widths(self, rows)
    }

    pub fn apply_node_bounds(&mut self) {
        graph_layout::apply_node_bounds(self);
    }

    /// Apply bounds (box, meta, row, table) to a single node.
    /// Public for use by graph_builder_preorder.
    pub fn apply_node_bounds_to(&self, node: &mut GraphNode) {
        let row_widths = self.row_column_widths(&node.rows);
        graph_layout::apply_node_bounds_to(&self.config, node, row_widths);
    }

    /// Apply bezier args to a single edge given an external node list.
    /// Public for use by graph_builder_preorder.
    pub fn apply_edge_bezier_args_to(&self, nodes: &[GraphNode], edge: &mut GraphEdge) {
        graph_layout::apply_edge_bezier_args_to(&self.config, nodes, edge);
    }

    /// Install a fresh set of object rows on an existing Object/Scalar node
    /// and recompute its width, height, box, and per-row bounds — preserving
    /// the node's `x`/`y`. Used by the streaming projector to keep a parent
    /// Mapping node's `rows` in sync when new key/value children arrive in a
    /// later chunk (the incremental path emits the child node but must also
    /// refresh the parent container's rows). Mirrors the height/width formula
    /// in `build_node_only`.
    pub fn install_object_rows(&self, node: &mut GraphNode, rows: Vec<GraphRow>) {
        let row_widths = self.row_column_widths(&rows);
        let inner_width = row_widths.key + row_widths.value;
        let inner_height = (rows.len() as i32).max(1) * self.config.row_height;
        node.rows = rows;
        node.width = inner_width + self.config.node_border_width * 2;
        node.height = inner_height + self.config.node_border_width * 2;
        self.apply_node_bounds_to(node);
    }

    /// Free any owned data associated with a node (e.g. drop cloned strings).
    /// In Rust this is a no-op since we use owned types, but we keep the
    pub fn free_node_owned_data(&self, _node: &GraphNode, _path: Option<&[PathSeg]>) {
        // Owned data is automatically freed by Rust's drop semantics.
    }
}

pub(crate) fn graph_kind_for_node(node: &TreeNode) -> GraphKind {
    match node.kind {
        NodeKind::Mapping => GraphKind::Object,
        NodeKind::Sequence => GraphKind::Table,
        _ => GraphKind::Scalar,
    }
}

pub(crate) fn graph_node_key(kind: GraphKind, path: &[PathSeg]) -> GraphNodeKey {
    GraphNodeKey {
        stable_id: stable_node_id(kind, path),
        path: path.to_vec(),
    }
}

/// Check whether a mapping row (header table item) has any value child that
/// would produce a sub-graph node.
fn mapping_row_builds_child(item: &TreeNode) -> bool {
    debug_assert_eq!(item.kind, NodeKind::Mapping);
    let mut i = 0;
    while i + 1 < item.content.len() {
        let value_node = &item.content[i + 1];
        if shared::value_node_builds_child(value_node) {
            return true;
        }
        i += 2;
    }
    false
}

use super::graph_builder_preorder::GraphTableCellPatch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphModelSnapshot {
    Owned(GraphModel),
    TableCellOverlay {
        base: Box<GraphModelSnapshot>,
        patches: Vec<GraphTableCellPatch>,
    },
}

impl GraphModelSnapshot {
    pub fn owned(model: GraphModel) -> Self {
        Self::Owned(model)
    }

    pub fn with_table_cell_patch(self, patch: GraphTableCellPatch) -> Self {
        match self {
            Self::TableCellOverlay { base, mut patches } => {
                upsert_patch(&mut patches, patch);
                Self::TableCellOverlay { base, patches }
            }
            other => Self::TableCellOverlay {
                base: Box::new(other),
                patches: vec![patch],
            },
        }
    }

    pub fn materialize(&self) -> GraphModel {
        let mut model = match self {
            Self::Owned(model) => model.clone(),
            Self::TableCellOverlay { base, patches } => {
                let mut model = base.materialize();
                apply_patches_to_model(&mut model, patches);
                model
            }
        };
        model.rebuild_edge_index();
        model
    }

    pub fn patched_node(&self, render_handle: u32) -> Option<GraphNode> {
        let mut model = self.materialize();
        model
            .nodes
            .drain(..)
            .find(|node| node.render_handle == render_handle)
    }
}

fn upsert_patch(patches: &mut Vec<GraphTableCellPatch>, patch: GraphTableCellPatch) {
    if let Some(existing) = patches.iter_mut().find(|existing| {
        existing.table_render_handle == patch.table_render_handle
            && existing.row_index == patch.row_index
            && existing.column_index == patch.column_index
    }) {
        *existing = patch;
        return;
    }
    patches.push(patch);
}

fn apply_patches_to_model(model: &mut GraphModel, patches: &[GraphTableCellPatch]) {
    for patch in patches {
        let Some(node) = model
            .nodes
            .iter_mut()
            .find(|node| node.render_handle == patch.table_render_handle)
        else {
            continue;
        };
        let Some(table) = node.table.as_mut() else {
            continue;
        };
        let row_idx = patch.row_index as usize;
        let col_idx = patch.column_index as usize;
        let Some(row) = table.rows.get_mut(row_idx) else {
            continue;
        };
        let Some(cell) = row.cells.get_mut(col_idx) else {
            continue;
        };
        *cell = patch.cell.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::graph_builder::{
        GraphCell, GraphKind, GraphModel, GraphNode, GraphNodeKey, GraphRow, GraphTable, PathSeg,
    };
    use crate::core::graph_builder_preorder::GraphTableCellPatch;

    #[test]
    fn graph_model_insert_edge_if_missing_uses_persistent_edge_index() {
        let mut model = GraphModel::default();
        model.nodes.push(graph_node_for_edge_index_test(0));
        model.nodes.push(graph_node_for_edge_index_test(1));

        let first = graph_edge_for_edge_index_test(0, 1, 0, 0);
        let duplicate = graph_edge_for_edge_index_test(0, 1, 0, 0);
        let second_row = graph_edge_for_edge_index_test(0, 1, 1, 0);

        assert_eq!(model.insert_edge_if_missing(first), Some(0));
        assert_eq!(model.insert_edge_if_missing(duplicate), None);
        assert_eq!(model.insert_edge_if_missing(second_row), Some(1));
        assert_eq!(model.edges.len(), 2);
        assert_eq!(model.edge_index_len(), 2);
        assert_eq!(model.edge_indexes_incident_to_handles(&[0]), vec![0, 1]);
    }

    fn graph_node_for_edge_index_test(handle: u32) -> GraphNode {
        GraphNode {
            render_handle: handle,
            stable_id: handle as u64,
            key: GraphNodeKey {
                stable_id: handle as u64,
                path: vec![PathSeg::Index(handle as usize)],
            },
            kind: GraphKind::Scalar,
            depth: handle,
            x: 0,
            y: 0,
            width: 10,
            height: 10,
            box_args: super::BoxArgs::default(),
            path: vec![PathSeg::Index(handle as usize)],
            meta: GraphCell::default(),
            rows: Vec::new(),
            table: None,
            preorder_first: handle,
            preorder_last: handle,
            source: None,
        }
    }

    fn graph_edge_for_edge_index_test(from: u32, to: u32, from_row: i32, to_row: i32) -> GraphEdge {
        GraphEdge {
            from_render_handle: from,
            from_key: GraphNodeKey {
                stable_id: from as u64,
                path: vec![PathSeg::Index(from as usize)],
            },
            from_row,
            to_render_handle: to,
            to_key: GraphNodeKey {
                stable_id: to as u64,
                path: vec![PathSeg::Index(to as usize)],
            },
            to_row,
            bezier_args: Default::default(),
        }
    }

    fn table_node() -> GraphNode {
        let cell = GraphCell {
            text: "old".to_owned(),
            value: "old".to_owned(),
            editable: true,
            ..GraphCell::default()
        };
        GraphNode {
            render_handle: 7,
            stable_id: 70,
            key: GraphNodeKey {
                stable_id: 70,
                path: vec![PathSeg::Key("rows".to_owned())],
            },
            kind: GraphKind::Table,
            depth: 0,
            x: 0,
            y: 0,
            width: 100,
            height: 40,
            box_args: Default::default(),
            path: vec![PathSeg::Key("rows".to_owned())],
            meta: GraphCell::default(),
            rows: Vec::new(),
            table: Some(GraphTable {
                columns: vec![GraphCell::default()],
                rows: vec![GraphRow {
                    cells: vec![cell],
                    ..GraphRow::default()
                }],
                column_widths: vec![100],
                width: 100,
                total_height: 40,
                view_height: 40,
                header_height: 20,
                row_height: 20,
                key: "rows".to_owned(),
                count: 1,
                source: None,
            }),
            preorder_first: 7,
            preorder_last: 7,
            source: None,
        }
    }

    #[test]
    fn table_cell_overlay_materializes_patched_cell() {
        let model = GraphModel {
            nodes: vec![table_node()],
            edges: Vec::new(),
            ..Default::default()
        };
        let patch = GraphTableCellPatch {
            table_render_handle: 7,
            row_index: 0,
            column_index: 0,
            cell: GraphCell {
                text: "new".to_owned(),
                value: "new".to_owned(),
                editable: true,
                ..GraphCell::default()
            },
        };

        let snapshot = GraphModelSnapshot::owned(model.clone()).with_table_cell_patch(patch);
        let materialized = snapshot.materialize();

        assert_eq!(
            model.nodes[0].table.as_ref().unwrap().rows[0].cells[0].text,
            "old"
        );
        assert_eq!(
            materialized.nodes[0].table.as_ref().unwrap().rows[0].cells[0].text,
            "new"
        );
        assert!(materialized.edges.is_empty());
    }

    #[test]
    fn table_cell_overlay_replaces_existing_patch_for_same_cell() {
        let model = GraphModel {
            nodes: vec![table_node()],
            edges: Vec::new(),
            ..Default::default()
        };
        let first = GraphTableCellPatch {
            table_render_handle: 7,
            row_index: 0,
            column_index: 0,
            cell: GraphCell {
                text: "first".to_owned(),
                value: "first".to_owned(),
                ..GraphCell::default()
            },
        };
        let second = GraphTableCellPatch {
            table_render_handle: 7,
            row_index: 0,
            column_index: 0,
            cell: GraphCell {
                text: "second".to_owned(),
                value: "second".to_owned(),
                ..GraphCell::default()
            },
        };

        let snapshot = GraphModelSnapshot::owned(model)
            .with_table_cell_patch(first)
            .with_table_cell_patch(second);
        let materialized = snapshot.materialize();

        assert_eq!(
            materialized.nodes[0].table.as_ref().unwrap().rows[0].cells[0].text,
            "second"
        );
    }
}
