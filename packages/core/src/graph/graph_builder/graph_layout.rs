use super::shared;
use super::table_graph;
use super::{BezierArgs, CurveControlPoints, GraphBuilder, GraphEdge, GraphNode, GraphNodeKey};

// ---------------------------------------------------------------------------
// make_edge
// ---------------------------------------------------------------------------
pub(super) fn make_edge(
    from_render_handle: u32,
    from_key: GraphNodeKey,
    from_row: i32,
    to_render_handle: u32,
    to_key: GraphNodeKey,
    to_row: i32,
) -> GraphEdge {
    GraphEdge {
        from_render_handle,
        from_key,
        from_row,
        to_render_handle,
        to_key,
        to_row,
        bezier_args: BezierArgs::default(),
    }
}

// =========================================================================
//  Multi-stage hierarchical layout
// =========================================================================

pub(super) fn layout_graph(builder: &mut GraphBuilder, root_render_handle: u32) {
    let mut model = super::GraphModel {
        nodes: std::mem::take(&mut builder.nodes),
        edges: std::mem::take(&mut builder.edges),
        ..Default::default()
    };
    model.rebuild_edge_index();
    crate::layout::layout_engine::LayoutEngine::new(builder.config.clone())
        .layout_positions(&mut model, root_render_handle);
    builder.nodes = model.nodes;
    builder.edges = model.edges;
}

// =========================================================================
//  getAnchorY  – compute the y-anchor for a bezier curve on a node row
// =========================================================================

pub(super) fn get_anchor_y(builder: &GraphBuilder, node: &GraphNode, row_index: i32) -> i32 {
    if node.kind == super::GraphKind::Table {
        if let Some(ref table) = node.table {
            let border_width = builder.config.node_border_width.max(0);
            let header_offset: i32 = if table.header_height > 0 { 1 } else { 0 };

            if header_offset == 1 && row_index == 0 {
                return node.y + border_width + table.header_height / 2;
            }

            let body_index = row_index - header_offset;
            if body_index >= 0 {
                let row_idx = body_index as usize;
                if row_idx < table.rows.len() {
                    let row_offset =
                        border_width + table.header_height + row_idx as i32 * table.row_height;
                    return node.y + row_offset + table.row_height / 2;
                }
            }

            return node.y + border_width + table.header_height / 2;
        }
    }

    if row_index >= 0 {
        let row_idx = row_index as usize;
        if row_idx < node.rows.len() {
            let row = &node.rows[row_idx];
            return row.abs_bounds.y + row.abs_bounds.height / 2;
        }
    }

    node.y + node.height / 2
}

// =========================================================================
//  getCurveControlPoints
// =========================================================================

pub(super) fn get_curve_control_points(x1: i32, y1: i32, x2: i32, y2: i32) -> CurveControlPoints {
    let distance = (x2 - x1).abs();
    let curve = 40.max(200.min(distance / 2));
    let direction: i32 = if x2 >= x1 { 1 } else { -1 };

    CurveControlPoints {
        c1x: x1 + curve * direction,
        c1y: y1,
        c2x: x2 - curve * direction,
        c2y: y2,
    }
}

// =========================================================================
//  applyEdgeBezierArgs  – for all edges in the builder
// =========================================================================

pub(super) fn apply_edge_bezier_args(builder: &mut GraphBuilder) {
    // We need to look up nodes by render_handle for each edge.
    // Build a temporary lookup vector indexed by render_handle.
    let node_count = builder.nodes.len();
    let mut node_by_handle: Vec<Option<usize>> = vec![None; node_count];
    for (idx, node) in builder.nodes.iter().enumerate() {
        let handle = node.render_handle as usize;
        if handle < node_by_handle.len() {
            node_by_handle[handle] = Some(idx);
        }
    }

    for edge_idx in 0..builder.edges.len() {
        let edge = &builder.edges[edge_idx];
        let from_idx = match node_by_handle.get(edge.from_render_handle as usize) {
            Some(Some(idx)) => *idx,
            _ => continue,
        };
        let to_idx = match node_by_handle.get(edge.to_render_handle as usize) {
            Some(Some(idx)) => *idx,
            _ => continue,
        };

        // Split borrow: read nodes, then write edge
        let from_node = &builder.nodes[from_idx];
        let to_node = &builder.nodes[to_idx];

        let from_y = get_anchor_y(builder, from_node, edge.from_row);
        let to_y = get_anchor_y(builder, to_node, edge.to_row);
        let from_x = from_node.x + from_node.width;
        let to_x = to_node.x;
        let curve = get_curve_control_points(from_x, from_y, to_x, to_y);

        let edge_mut = &mut builder.edges[edge_idx];
        edge_mut.bezier_args = BezierArgs {
            from_x,
            from_y,
            c1x: curve.c1x,
            c1y: curve.c1y,
            c2x: curve.c2x,
            c2y: curve.c2y,
            to_x,
            to_y,
        };
    }
}

// =========================================================================
//  applyEdgeBezierArgsTo  – for a single edge given an external node list
// =========================================================================

pub fn apply_edge_bezier_args_to(
    config: &super::BuilderConfig,
    nodes: &[GraphNode],
    edge: &mut GraphEdge,
) {
    let from_node = shared::find_node_in_list(nodes, edge.from_render_handle);
    let to_node = shared::find_node_in_list(nodes, edge.to_render_handle);

    let (Some(from_node), Some(to_node)) = (from_node, to_node) else {
        return;
    };

    let from_y = get_anchor_y_for_config(config, from_node, edge.from_row);
    let to_y = get_anchor_y_for_config(config, to_node, edge.to_row);
    let from_x = from_node.x + from_node.width;
    let to_x = to_node.x;
    let curve = get_curve_control_points(from_x, from_y, to_x, to_y);

    edge.bezier_args = BezierArgs {
        from_x,
        from_y,
        c1x: curve.c1x,
        c1y: curve.c1y,
        c2x: curve.c2x,
        c2y: curve.c2y,
        to_x,
        to_y,
    };
}

/// Variant of `get_anchor_y` that takes a `BuilderConfig` directly instead of
/// a full `GraphBuilder`, so it can be called from outside the graph_builder
/// module (e.g. from `graph_delta_service`).
fn get_anchor_y_for_config(config: &super::BuilderConfig, node: &GraphNode, row_index: i32) -> i32 {
    if node.kind == super::GraphKind::Table {
        if let Some(ref table) = node.table {
            let border_width = config.node_border_width.max(0);
            let header_offset: i32 = if table.header_height > 0 { 1 } else { 0 };

            if header_offset == 1 && row_index == 0 {
                return node.y + border_width + table.header_height / 2;
            }

            let body_index = row_index - header_offset;
            if body_index >= 0 {
                let row_idx = body_index as usize;
                if row_idx < table.rows.len() {
                    let row_offset =
                        border_width + table.header_height + row_idx as i32 * table.row_height;
                    return node.y + row_offset + table.row_height / 2;
                }
            }

            return node.y + border_width + table.header_height / 2;
        }
    }

    if row_index >= 0 {
        let row_idx = row_index as usize;
        if row_idx < node.rows.len() {
            let row = &node.rows[row_idx];
            return row.abs_bounds.y + row.abs_bounds.height / 2;
        }
    }

    node.y + node.height / 2
}

// =========================================================================
//  applyRowBounds  – set bounds for key/value cells within object/scalar rows
// =========================================================================

pub(super) fn apply_row_bounds(
    config: &super::BuilderConfig,
    rows: &mut [super::GraphRow],
    node_x: i32,
    node_y: i32,
    key_width: i32,
    value_width: i32,
) {
    let row_height = config.row_height;
    let row_padding_x = config.row_padding_x;
    let border_width = config.node_border_width.max(0);
    let row_width = key_width + value_width;

    for row in rows.iter_mut() {
        let local_y = border_width + row.index * row_height;

        shared::set_row_bounds(row, border_width, local_y, row_width, row_height);
        shared::set_row_abs_bounds(
            row,
            node_x + border_width,
            node_y + local_y,
            row_width,
            row_height,
        );
        shared::set_row_cell_bounds(row, 0, 0, row_width, row_height);

        // Key cell (idx = 0)
        {
            let cell_x: i32 = 0;
            let cell_width: i32 = key_width;
            let text_x: i32 = row_padding_x;
            let text_width: i32 = key_width - row_padding_x * 2;
            shared::set_cell_bounds(&mut row.key, cell_x, 0, cell_width, row_height);
            shared::set_text_bounds(&mut row.key, text_x, 0, text_width.max(0), row_height);
            if let Some(cell) = row.cells.get_mut(0) {
                *cell = row.key.clone();
            }
        }

        // Value cell (idx = 1)
        {
            let cell_x: i32 = key_width;
            let cell_width: i32 = value_width;
            let text_x: i32 = row_padding_x;
            let text_width: i32 = value_width - row_padding_x * 2;
            shared::set_cell_bounds(&mut row.value, cell_x, 0, cell_width, row_height);
            shared::set_text_bounds(&mut row.value, text_x, 0, text_width.max(0), row_height);
            if let Some(cell) = row.cells.get_mut(1) {
                *cell = row.value.clone();
            }
        }
    }
}

// =========================================================================
//  applyNodeBoundsTo  – set box / meta / row / table bounds for one node
// =========================================================================

pub(super) fn apply_node_bounds_to(
    config: &super::BuilderConfig,
    node: &mut super::GraphNode,
    row_widths: super::RowColumnWidths,
) {
    let border_width = config.node_border_width.max(0);

    node.box_args = super::BoxArgs {
        x: node.x,
        y: node.y,
        width: node.width,
        height: node.height,
        corner_radius: 0,
    };

    let meta_height = config.row_height;
    let inner_width = match node.kind {
        super::GraphKind::Table => node.table.as_ref().map(|t| t.width).unwrap_or(0),
        _ => row_widths.key + row_widths.value,
    };

    shared::set_cell_bounds(
        &mut node.meta,
        node.x + border_width,
        node.y - meta_height,
        inner_width,
        meta_height,
    );
    shared::set_text_bounds(
        &mut node.meta,
        node.x + border_width + config.row_padding_x,
        node.y - meta_height,
        inner_width - config.row_padding_x * 2,
        meta_height,
    );

    match node.kind {
        super::GraphKind::Object | super::GraphKind::Scalar => {
            apply_row_bounds(
                config,
                &mut node.rows,
                node.x,
                node.y,
                row_widths.key,
                row_widths.value,
            );
        }
        super::GraphKind::Table => {
            if let Some(ref mut table) = node.table {
                table_graph::apply_table_bounds_via_config(config, table);
            }
        }
    }
}

// =========================================================================
//  applyNodeBounds  – for all nodes
// =========================================================================

pub(super) fn apply_node_bounds(builder: &mut GraphBuilder) {
    // Pre-compute row column widths to avoid simultaneous &builder + &mut node borrows.
    let row_widths: Vec<super::RowColumnWidths> = builder
        .nodes
        .iter()
        .map(|node| builder.row_column_widths(&node.rows))
        .collect();

    let config = builder.config.clone();

    for i in 0..builder.nodes.len() {
        apply_node_bounds_to(&config, &mut builder.nodes[i], row_widths[i]);
    }
}
