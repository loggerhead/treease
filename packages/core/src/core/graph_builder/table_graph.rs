use crate::operators::{NodeKind, TreeNode};

use super::node_meta::display_text_for_node;
use super::sequence_graph::sequence_presentation;
use super::shared::{
    append_path, header_table_needs_fallback_value_column, infer_header_table_column_sem_type,
};
use super::{GraphBuilder, GraphCell, GraphRow, GraphTable, PathSeg, SequencePresentation};
use crate::core::graph_identity::canonical_path_key;

pub(super) fn build_table(builder: &GraphBuilder, node: &TreeNode, path: &[PathSeg]) -> GraphTable {
    debug_assert_eq!(node.kind, NodeKind::Sequence);
    match sequence_presentation(node) {
        SequencePresentation::HeaderTable => build_header_table(builder, node, path),
        SequencePresentation::HeaderlessTable => build_headerless_table(builder, node, path),
    }
}

fn build_header_table(builder: &GraphBuilder, node: &TreeNode, path: &[PathSeg]) -> GraphTable {
    let columns = build_header_table_columns(builder, node);
    let rows = build_table_rows(builder, node, path, &columns);
    let column_widths = table_column_widths(builder, &columns, &rows);
    let table_width: i32 = column_widths.iter().sum();
    let row_count = node.content.len() as i32;
    let base_rows = if row_count == 0 { 1 } else { row_count };
    let header_height = builder.config.table_header_height;
    let row_height = builder.config.table_row_height;
    let total_height = header_height + row_height * base_rows;
    let view_height = builder.config.table_max_height.min(total_height);
    let key = canonical_path_key(path);
    let mut table = GraphTable {
        columns,
        rows,
        column_widths,
        width: table_width,
        total_height,
        view_height,
        header_height,
        row_height,
        key,
        count: row_count,
        source: Some(node as *const TreeNode as usize),
    };
    apply_table_bounds(builder, &mut table);
    table
}

fn build_headerless_table(builder: &GraphBuilder, node: &TreeNode, path: &[PathSeg]) -> GraphTable {
    let rows = build_indexed_sequence_rows(builder, node, path);
    let widths = row_column_widths(builder, &rows);
    let column_widths = vec![widths.key, widths.value];
    let row_count = node.content.len() as i32;
    let base_rows = if row_count == 0 { 1 } else { row_count };
    let row_height = builder.config.row_height;
    let total_height = row_height * base_rows;
    let view_height = builder.config.table_max_height.min(total_height);
    let key = canonical_path_key(path);
    let mut table = GraphTable {
        columns: Vec::new(),
        rows,
        column_widths,
        width: widths.key + widths.value,
        total_height,
        view_height,
        header_height: 0,
        row_height,
        key,
        count: row_count,
        source: Some(node as *const TreeNode as usize),
    };
    apply_table_bounds(builder, &mut table);
    table
}

pub(crate) fn estimated_table_column_width_for_config(
    config: &super::BuilderConfig,
    text: &str,
) -> i32 {
    let avg_x10 = config.avg_char_width_x10.max(1);
    let padding = config.row_padding_x * 2;
    let min_char = ((avg_x10 + 5) / 10).max(1);
    let min_width = padding + min_char;
    let text_len = text.chars().count() as i32 + 1;
    let content = (text_len * avg_x10 + 5) / 10;
    (min_width).max(content + padding)
}

pub(crate) fn estimated_table_column_width(builder: &GraphBuilder, text: &str) -> i32 {
    estimated_table_column_width_for_config(&builder.config, text)
}

pub(super) fn row_column_widths(
    builder: &GraphBuilder,
    rows: &[GraphRow],
) -> super::RowColumnWidths {
    let mut key_width = estimated_table_column_width(builder, "");
    let mut value_width = estimated_table_column_width(builder, "");
    for row in rows {
        for (idx, cell) in row.cells.iter().enumerate() {
            if idx == 0 {
                key_width = key_width.max(estimated_table_column_width(builder, &cell.text));
            } else if idx == 1 {
                value_width = value_width.max(estimated_table_column_width(builder, &cell.text));
            }
        }
    }
    key_width = key_width.min(builder.config.key_width);
    value_width = value_width.min(builder.config.value_width);
    super::RowColumnWidths {
        key: key_width.max(1),
        value: value_width.max(1),
    }
}

pub(super) fn table_column_widths(
    builder: &GraphBuilder,
    columns: &[GraphCell],
    rows: &[GraphRow],
) -> Vec<i32> {
    let mut widths: Vec<i32> = columns
        .iter()
        .map(|col| estimated_table_column_width(builder, &col.text))
        .collect();
    for row in rows {
        for (idx, cell) in row.cells.iter().enumerate() {
            if idx < widths.len() {
                widths[idx] = widths[idx].max(estimated_table_column_width(builder, &cell.text));
            }
        }
    }
    for w in &mut widths {
        *w = (*w).min(builder.config.table_column_width);
    }
    widths
}

pub(super) fn build_header_table_columns(
    _builder: &GraphBuilder,
    node: &TreeNode,
) -> Vec<GraphCell> {
    let mut keys: Vec<String> = Vec::new();
    for item in &node.content {
        if item.kind != NodeKind::Mapping {
            continue;
        }
        let mut i = 0;
        while i + 1 < item.content.len() {
            let key_node = &item.content[i];
            if !keys.contains(&key_node.value) {
                keys.push(key_node.value.clone());
            }
            i += 2;
        }
    }

    let needs_fallback = header_table_needs_fallback_value_column(node);
    let value_column_count = if needs_fallback {
        keys.len().max(0) + 1
    } else {
        keys.len().max(1)
    };
    let mut cols = Vec::with_capacity(value_column_count + 1);

    // Column 0: index column (empty header)
    cols.push(GraphCell {
        text: String::new(),
        sem_type: None,
        path: Vec::new(),
        value: String::new(),
        editable: false,
        source: None,
        ..GraphCell::default()
    });

    if keys.is_empty() {
        cols.push(GraphCell {
            text: "value".to_string(),
            sem_type: None,
            path: Vec::new(),
            value: String::new(),
            editable: false,
            source: None,
            ..GraphCell::default()
        });
        return cols;
    }

    for k in &keys {
        cols.push(GraphCell {
            text: k.clone(),
            sem_type: infer_header_table_column_sem_type(node, k),
            path: Vec::new(),
            value: String::new(),
            editable: false,
            source: None,
            ..GraphCell::default()
        });
    }

    if needs_fallback {
        cols.push(GraphCell {
            text: "value".to_string(),
            sem_type: None,
            path: Vec::new(),
            value: String::new(),
            editable: false,
            source: None,
            ..GraphCell::default()
        });
    }

    cols
}

pub(super) fn build_table_rows(
    builder: &GraphBuilder,
    node: &TreeNode,
    path: &[PathSeg],
    columns: &[GraphCell],
) -> Vec<GraphRow> {
    let has_fallback = header_table_needs_fallback_value_column(node);
    let fallback_col_idx: Option<usize> = if has_fallback {
        Some(columns.len() - 1)
    } else {
        None
    };
    let object_column_end: usize = if has_fallback {
        columns.len() - 1
    } else {
        columns.len()
    };

    let mut rows: Vec<GraphRow> = Vec::with_capacity(node.content.len());
    for (idx, item) in node.content.iter().enumerate() {
        let item_path = append_path(path, PathSeg::Index(idx));
        let mut cells: Vec<GraphCell> = Vec::with_capacity(columns.len());
        let index_text = idx.to_string();
        cells.push(index_cell(item, &item_path, &index_text));

        if item.kind == NodeKind::Mapping {
            let mut c: usize = 1;
            while c < object_column_end {
                let col = &columns[c];
                let value_node = find_mapping_value(item, &col.text);
                let cell_path = append_path(&item_path, PathSeg::Key(col.text.clone()));
                if let Some(v) = value_node {
                    cells.push(cell_from_node_value(builder, v, &cell_path));
                } else {
                    cells.push(missing_field_cell(col));
                }
                c += 1;
            }
            if let Some(fallback_idx) = fallback_col_idx {
                // Fill remaining slots up to fallback_idx with empty cells
                while cells.len() < fallback_idx {
                    cells.push(empty_cell(&item_path));
                }
                cells.push(empty_cell(&item_path));
            }
        } else {
            let cell = cell_from_node_value(builder, item, &item_path);
            if let Some(fallback_idx) = fallback_col_idx {
                // Fill slots 1..fallback_idx with empty cells
                while cells.len() < fallback_idx {
                    cells.push(empty_cell(&item_path));
                }
                cells.push(cell);
                // Fill remaining slots after fallback_idx
                while cells.len() < columns.len() {
                    cells.push(empty_cell(&item_path));
                }
            } else {
                if columns.len() > 1 {
                    cells.push(cell);
                }
                let mut c: usize = 2;
                while c < columns.len() {
                    cells.push(empty_cell(&item_path));
                    c += 1;
                }
            }
        }

        // Ensure we have exactly columns.len() cells
        while cells.len() < columns.len() {
            cells.push(empty_cell(&item_path));
        }

        rows.push(graph_table_row(idx as i32, cells));
    }
    rows
}

pub(super) fn find_mapping_value<'a>(map_node: &'a TreeNode, key: &str) -> Option<&'a TreeNode> {
    let mut i = 0;
    while i + 1 < map_node.content.len() {
        let key_node = &map_node.content[i];
        if key_node.value == key {
            return Some(&map_node.content[i + 1]);
        }
        i += 2;
    }
    None
}

// ---- helpers ----

fn empty_cell(path: &[PathSeg]) -> GraphCell {
    GraphCell {
        text: String::new(),
        sem_type: None,
        path: path.to_vec(),
        value: String::new(),
        editable: false,
        source: None,
        ..GraphCell::default()
    }
}

fn missing_field_cell(column: &GraphCell) -> GraphCell {
    GraphCell {
        text: "miss".to_string(),
        sem_type: column.sem_type.clone(),
        is_missing: true,
        path: Vec::new(),
        value: "miss".to_string(),
        editable: false,
        source: None,
        ..GraphCell::default()
    }
}

fn index_cell(node: &TreeNode, path: &[PathSeg], text: &str) -> GraphCell {
    GraphCell {
        text: text.to_string(),
        sem_type: None,
        path: path.to_vec(),
        value: text.to_string(),
        editable: false,
        source: Some(node as *const TreeNode as usize),
        ..GraphCell::default()
    }
}

fn cell_from_node_value(builder: &GraphBuilder, node: &TreeNode, path: &[PathSeg]) -> GraphCell {
    let text = display_text_for_node(builder, node);
    GraphCell {
        text,
        sem_type: node.resolved_sem_type().map(|st| st.tag().to_owned()),
        path: path.to_vec(),
        value: node.value.clone(),
        editable: true,
        source: Some(node as *const TreeNode as usize),
        ..GraphCell::default()
    }
}

fn build_indexed_sequence_rows(
    builder: &GraphBuilder,
    node: &TreeNode,
    path: &[PathSeg],
) -> Vec<GraphRow> {
    let mut rows: Vec<GraphRow> = Vec::with_capacity(node.content.len());
    for (idx, item) in node.content.iter().enumerate() {
        let item_path = append_path(path, PathSeg::Index(idx));
        let index_text = idx.to_string();
        let key_cell = index_cell(item, &item_path, &index_text);
        let value_cell = cell_from_node_value(builder, item, &item_path);
        rows.push(GraphRow {
            index: idx as i32,
            key: key_cell.clone(),
            value: value_cell.clone(),
            cells: vec![key_cell, value_cell],
            ..GraphRow::default()
        });
    }
    rows
}

// ---- bounds application ----

pub(super) fn apply_table_bounds(builder: &GraphBuilder, table: &mut GraphTable) {
    apply_table_bounds_via_config(&builder.config, table)
}

/// Variant of `apply_table_bounds` that takes a `BuilderConfig` directly,
/// so it can be called from contexts that don't have a full `GraphBuilder`.
pub(crate) fn apply_table_bounds_via_config(config: &super::BuilderConfig, table: &mut GraphTable) {
    let border_width = config.node_border_width.max(0);
    let header_height = table.header_height;
    let row_padding_x = config.row_padding_x;
    let column_widths = &table.column_widths;

    // Header cells
    let mut col_x: i32 = border_width;
    for (idx, cell) in table.columns.iter_mut().enumerate() {
        let col_width = column_widths[idx];
        super::shared::set_cell_bounds(cell, col_x, border_width, col_width, header_height);
        super::shared::set_text_bounds(
            cell,
            row_padding_x,
            0,
            (col_width - row_padding_x * 2).max(0),
            header_height,
        );
        col_x += col_width;
    }

    // Body rows: each row is independent, delegate to single-row helper.
    for row_idx in 0..table.rows.len() {
        apply_row_bounds_at_index(config, table, row_idx);
    }
}

/// Apply bounds to a single body row by index. Body row layout has no
/// cross-row dependency (column widths are stable, y is a pure function of
/// row index), so streaming append paths can call this for each new row in
/// O(cols) instead of re-flowing the whole table.
pub(crate) fn apply_row_bounds_at_index(
    config: &super::BuilderConfig,
    table: &mut GraphTable,
    row_idx: usize,
) {
    let border_width = config.node_border_width.max(0);
    let header_height = table.header_height;
    let row_height = table.row_height;
    let row_padding_x = config.row_padding_x;
    let row_y = border_width + header_height + row_idx as i32 * row_height;
    let row_width: i32 = table.column_widths.iter().sum();
    let row = &mut table.rows[row_idx];
    super::shared::set_row_bounds(row, border_width, row_y, row_width, row_height);
    super::shared::set_row_abs_bounds(row, border_width, row_y, row_width, row_height);
    super::shared::set_row_cell_bounds(row, 0, 0, row_width, row_height);
    let mut row_x: i32 = 0;
    for (col_idx, cell) in row.cells.iter_mut().enumerate() {
        let col_width = table.column_widths[col_idx];
        super::shared::set_cell_bounds(cell, row_x, 0, col_width, row_height);
        super::shared::set_text_bounds(
            cell,
            row_padding_x,
            0,
            (col_width - row_padding_x * 2).max(0),
            row_height,
        );
        row_x += col_width;
    }
}

fn graph_table_row(index: i32, cells: Vec<GraphCell>) -> GraphRow {
    let key = cells.first().cloned().unwrap_or_default();
    let value = cells.get(1).cloned().unwrap_or_default();
    GraphRow {
        index,
        key,
        value,
        cells,
        ..GraphRow::default()
    }
}
