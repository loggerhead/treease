use crate::operators::TreeNode;

use super::node_meta::display_text_for_node;
use super::shared::{append_path, value_node_builds_child};
use super::{
    GraphBuilder, GraphCell, GraphKind, GraphRow, PathSeg, RowColumnWidths, graph_kind_for_node,
};

/// Build rows for scalar and object nodes.
pub(super) fn build_rows(
    builder: &GraphBuilder,
    node: &TreeNode,
    path: &[PathSeg],
) -> Vec<GraphRow> {
    match graph_kind_for_node(node) {
        GraphKind::Scalar => build_scalar_rows(builder, node, path),
        GraphKind::Object => build_object_rows(builder, node, path),
        GraphKind::Table => Vec::new(),
    }
}

/// Build children for object (mapping) nodes.
/// Only value nodes that pass `value_node_builds_child` create child graph nodes.
pub(super) fn build_object_children(
    builder: &mut GraphBuilder,
    node: &TreeNode,
    depth: u32,
    parent_render_handle: u32,
    path: &[PathSeg],
) {
    let mut i = 0;
    while i + 1 < node.content.len() {
        let key_node = &node.content[i];
        let value_node = &node.content[i + 1];
        let row_index = (i / 2) as i32;
        if !value_node_builds_child(value_node) {
            i += 2;
            continue;
        }
        let child_path = append_path(path, PathSeg::Key(key_node.value.clone()));
        let child_render_handle = builder.build_node(value_node, depth + 1, &child_path);
        let parent_key = builder.nodes[parent_render_handle as usize].key.clone();
        let child_key = builder.nodes[child_render_handle as usize].key.clone();
        builder.edges.push(builder.make_edge(
            parent_render_handle,
            parent_key,
            row_index,
            child_render_handle,
            child_key,
            0,
        ));
        i += 2;
    }
}

// ── scalar rows ──────────────────────────────────────────────────

/// Build a single row for a scalar node without an extra "value" label.
fn build_scalar_rows(builder: &GraphBuilder, node: &TreeNode, path: &[PathSeg]) -> Vec<GraphRow> {
    let key_cell = empty_cell(path);
    let value_cell = node_value_cell(builder, node, path, true);
    vec![GraphRow {
        index: 0,
        key: key_cell.clone(),
        value: value_cell.clone(),
        cells: vec![key_cell, value_cell],
        ..GraphRow::default()
    }]
}

// ── object rows ──────────────────────────────────────────────────

/// Build rows for an object (mapping) node: one row per key-value pair.
/// Uses `append_path` to create a child path with the key segment.
fn build_object_rows(builder: &GraphBuilder, node: &TreeNode, path: &[PathSeg]) -> Vec<GraphRow> {
    let pair_count = node.content.len() / 2;
    let mut rows = Vec::with_capacity(pair_count);
    let mut i = 0;
    while i + 1 < node.content.len() {
        let key_node = &node.content[i];
        let value_node = &node.content[i + 1];
        let cell_path = append_path(path, PathSeg::Key(key_node.value.clone()));
        let key_cell = key_node_cell(key_node, &cell_path, true);
        let value_cell = value_node_cell(builder, value_node, &cell_path, true);
        rows.push(GraphRow {
            index: (i / 2) as i32,
            key: key_cell.clone(),
            value: value_cell.clone(),
            cells: vec![key_cell, value_cell],
            ..GraphRow::default()
        });
        i += 2;
    }
    rows
}

// ── cell constructors ────────────────────────────────────────────

/// Build a label cell (no source node, just display text).
fn label_cell(path: &[PathSeg], label: &str, editable: bool) -> GraphCell {
    GraphCell {
        text: label.to_string(),
        sem_type: None,
        path: path.to_vec(),
        value: String::new(),
        editable,
        ..GraphCell::default()
    }
}

/// Build an empty cell (no text, not editable).
fn empty_cell(path: &[PathSeg]) -> GraphCell {
    label_cell(path, "", false)
}

/// Build a cell for a map key node.
fn key_node_cell(node: &TreeNode, path: &[PathSeg], editable: bool) -> GraphCell {
    GraphCell {
        text: node.value.clone(),
        sem_type: node.resolved_sem_type().map(|st| st.tag().to_owned()),
        path: path.to_vec(),
        value: node.value.clone(),
        editable,
        source: Some(node as *const TreeNode as usize),
        ..GraphCell::default()
    }
}

/// Build a cell for a map value node (uses `display_text_for_node` for
/// collection summary text).
fn value_node_cell(
    builder: &GraphBuilder,
    node: &TreeNode,
    path: &[PathSeg],
    editable: bool,
) -> GraphCell {
    GraphCell {
        text: display_text_for_node(builder, node),
        sem_type: node.resolved_sem_type().map(|st| st.tag().to_owned()),
        path: path.to_vec(),
        value: node.value.clone(),
        editable,
        source: Some(node as *const TreeNode as usize),
        ..GraphCell::default()
    }
}

/// Build a cell for a standalone node value (same structure as value_node_cell).
fn node_value_cell(
    builder: &GraphBuilder,
    node: &TreeNode,
    path: &[PathSeg],
    editable: bool,
) -> GraphCell {
    value_node_cell(builder, node, path, editable)
}

// ── column widths ────────────────────────────────────────────────

/// Compute key/value column widths from rows.
/// Key and value widths are computed independently.
pub(super) fn row_column_widths(builder: &GraphBuilder, rows: &[GraphRow]) -> RowColumnWidths {
    let has_visible_key = rows.iter().any(|row| !row.key.text.is_empty());
    let mut key = if has_visible_key {
        estimated_column_width(builder, "")
    } else {
        0
    };
    let mut value = estimated_column_width(builder, "");
    for row in rows {
        if has_visible_key {
            key = key.max(estimated_column_width(builder, &row.key.text));
        }
        value = value.max(estimated_column_width(builder, &row.value.text));
    }
    key = key.min(builder.config.key_width);
    value = value.min(builder.config.value_width);
    RowColumnWidths {
        key: if has_visible_key { key.max(1) } else { 0 },
        value: value.max(1),
    }
}

fn estimated_column_width(builder: &GraphBuilder, text: &str) -> i32 {
    let avg_x10 = builder.config.avg_char_width_x10.max(1);
    let padding = builder.config.row_padding_x * 2;
    let min_char = ((avg_x10 + 5) / 10).max(1);
    let min_width = padding + min_char;
    let text_len = text.chars().count() as i32;
    let content = (text_len * avg_x10 + 5) / 10;
    min_width.max(content + padding)
}
