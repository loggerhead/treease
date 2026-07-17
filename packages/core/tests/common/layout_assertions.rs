// Shared graph layout assertion helpers, included via `include!` from test files.
// 这些函数验证 docs/contracts/layout.md Graph layout 规则：
//   - 节点生成规则（Mapping→Object, Sequence→Table, 其他→Scalar）
//   - 根节点始终生成图节点
//   - 只有非空 Mapping/Sequence 生成子节点
//   - 贝塞尔曲线锚点规则
//   - Object/Table/Scalar 节点内部结构
//   - X/Y 坐标计算规则
// NOTE: 不在此文件中添加 use 语句，以避免 include! 到多个测试文件时与各文件现有的
// use 冲突。所有类型引用使用全限定路径或依赖父文件已导入的路径。
// 父文件需要确保导入了：treease_core::core::graph_builder::{GraphKind, GraphModel, GraphNode, GraphRow, PathSeg}
// 以及 treease_core::core::{BuilderConfig, default_config}
// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn node_by_render_handle<'a>(nodes: &'a [GraphNode], render_handle: u32) -> Option<&'a GraphNode> {
    // Fast path: full-tree builds keep `render_handle == index into nodes`, so
    // probe that slot first and fall back to a linear scan only for
    // incremental/streaming models where handles may be sparse.
    if let Some(node) = nodes.get(render_handle as usize) {
        if node.render_handle == render_handle {
            return Some(node);
        }
    }
    nodes.iter().find(|n| n.render_handle == render_handle)
}

fn rows_for_node(node: &GraphNode) -> &[GraphRow] {
    &node.rows
}

fn body_row_count(node: &GraphNode) -> usize {
    if node.kind == GraphKind::Table {
        if let Some(table) = &node.table {
            return table.rows.len();
        }
    }
    node.rows.len()
}

fn edge_body_row(node: &GraphNode, edge_row: i32) -> i32 {
    if node.kind == GraphKind::Table {
        if let Some(table) = &node.table {
            if table.header_height > 0 {
                return edge_row - 1;
            }
        }
    }
    edge_row
}

/// 按 docs/contracts/layout.md Graph layout 规则计算锚点 y。
/// Table 使用 header/body row 高度从 node.y 独立推导。
/// Object/Scalar 从 node.y + 配置参数独立计算（不用 row.abs_bounds，避免循环验证）。
fn computed_anchor_y(node: &GraphNode, row_index: i32) -> i32 {
    let config = default_config();
    let border_width = config.node_border_width.max(0);
    if node.kind == GraphKind::Table {
        if let Some(table) = &node.table {
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
    // 独立计算非 Table 行中点，使用与 apply_row_bounds 一致的公式：
    //   local_y = border_width + row.index * row_height
    //   abs_y = node.y + local_y
    //   anchor = abs_y + row_height / 2
    if row_index >= 0 {
        let row_idx = row_index as usize;
        if row_idx < node.rows.len() {
            let local_y = border_width + row_idx as i32 * config.row_height;
            let abs_y = node.y + local_y;
            return abs_y + config.row_height / 2;
        }
    }
    node.y + node.height / 2
}

// ---------------------------------------------------------------------------
// Individual assertion helpers
// ---------------------------------------------------------------------------

/// 轻量版：只检查 size > 0（增量布局中 render_handle 可能不连续）
fn assert_node_size(model: &GraphModel) {
    for node in &model.nodes {
        assert!(node.width > 0, "node width should be positive");
        assert!(node.height > 0, "node height should be positive");
    }
}

/// 规则：同一深度的节点 x 坐标相同（由 level_meta 保证）
fn assert_depth_x_alignment(model: &GraphModel) {
    // 按 depth 记录首个出现的 x，O(N) 校验同深度同 x。
    let mut depth_x: std::collections::HashMap<u32, i32> = std::collections::HashMap::new();
    for node in &model.nodes {
        match depth_x.get(&node.depth) {
            Some(&x) => assert_eq!(x, node.x, "nodes at same depth should have same x"),
            None => {
                depth_x.insert(node.depth, node.x);
            }
        }
    }
}

/// 规则：兄弟节点的 y 坐标单调递增（按 from_row 排序后线性校验）。
/// 一次性遍历所有父节点，避免对每条 edge 重复 O(E^2) 比较。
fn assert_all_sibling_row_ordering(model: &GraphModel) {
    let index = node_index(model);
    // parent render_handle -> 该父节点的所有出边 (from_row, child_handle)
    let mut groups: std::collections::HashMap<u32, Vec<(i32, u32)>> =
        std::collections::HashMap::new();
    for edge in &model.edges {
        groups
            .entry(edge.from_render_handle)
            .or_default()
            .push((edge.from_row, edge.to_render_handle));
    }
    for siblings in groups.values_mut() {
        siblings.sort_by_key(|(from_row, _)| *from_row);
        for window in siblings.windows(2) {
            let (row_a, child_a_handle) = window[0];
            let (row_b, child_b_handle) = window[1];
            if row_a >= row_b {
                continue;
            }
            let child_a = index[&child_a_handle];
            let child_b = index[&child_b_handle];
            assert!(child_a.y <= child_b.y);
            assert!(child_a.y + child_a.height <= child_b.y + child_b.height);
        }
    }
}

/// 构建 render_handle -> &GraphNode 索引，避免重复线性查找。
fn node_index(model: &GraphModel) -> std::collections::HashMap<u32, &GraphNode> {
    model
        .nodes
        .iter()
        .map(|node| (node.render_handle, node))
        .collect()
}

/// 规则：edge.from_row 必须在父节点有效范围内；to_row 同理
fn assert_edge_row_contract(model: &GraphModel) {
    for edge in &model.edges {
        let parent = node_by_render_handle(&model.nodes, edge.from_render_handle)
            .expect("parent should exist");
        let child =
            node_by_render_handle(&model.nodes, edge.to_render_handle).expect("child should exist");
        let child_rows = rows_for_node(child);
        let parent_body_row = edge_body_row(parent, edge.from_row);
        let child_body_row = edge_body_row(child, edge.to_row);

        assert!(edge.from_row >= 0);
        assert!(edge.to_row >= 0);
        assert!(parent_body_row >= 0);
        assert!((parent_body_row as usize) < body_row_count(parent));

        if child.kind != GraphKind::Table && !child_rows.is_empty() {
            assert!(child_body_row >= 0);
            assert!((child_body_row as usize) < child_rows.len());
        } else {
            assert_eq!(edge.to_row, 0);
        }
    }
}

/// 规则：edge.from_row 对应父节点行的 key 应与 child path 最后一段匹配
fn assert_edge_from_row_matches_parent_key(model: &GraphModel) {
    for edge in &model.edges {
        let parent = node_by_render_handle(&model.nodes, edge.from_render_handle)
            .expect("parent should exist");
        let child =
            node_by_render_handle(&model.nodes, edge.to_render_handle).expect("child should exist");
        let parent_rows = rows_for_node(parent);
        let body_row = edge_body_row(parent, edge.from_row);
        if body_row < 0 {
            continue;
        }
        if (body_row as usize) >= parent_rows.len() {
            continue;
        }
        let row = &parent_rows[body_row as usize];
        let row_key = &row.key.text;
        if row_key.is_empty() {
            continue;
        }
        if child.path.is_empty() {
            continue;
        }
        let last_seg = &child.path[child.path.len() - 1];
        if let PathSeg::Key(k) = last_seg {
            assert_eq!(row_key, k, "edge from_row key should match child path key");
        }
    }
}

/// 规则：child.depth == parent.depth + 1
fn assert_edge_depth_contract(model: &GraphModel) {
    for edge in &model.edges {
        let parent = node_by_render_handle(&model.nodes, edge.from_render_handle)
            .expect("parent should exist");
        let child =
            node_by_render_handle(&model.nodes, edge.to_render_handle).expect("child should exist");
        assert_eq!(
            parent.depth + 1,
            child.depth,
            "child depth should be parent depth + 1"
        );
    }
}

/// 规则："起点 y 为父节点对应 value 单元格的中点；
///         终点 y 为子节点首个 row 的中点"
/// 同时验证 from_x = parent.right, to_x = child.left
fn assert_bezier_contract(model: &GraphModel) {
    for edge in &model.edges {
        let parent = node_by_render_handle(&model.nodes, edge.from_render_handle)
            .expect("parent should exist");
        let child =
            node_by_render_handle(&model.nodes, edge.to_render_handle).expect("child should exist");
        assert_eq!(parent.x + parent.width, edge.bezier_args.from_x);
        assert_eq!(child.x, edge.bezier_args.to_x);
        assert!(edge.bezier_args.from_x < edge.bezier_args.to_x);

        let expected_from_y = computed_anchor_y(parent, edge.from_row);
        assert_eq!(
            edge.bezier_args.from_y, expected_from_y,
            "from_y for edge {}->{} should be midpoint of parent's value cell at row {}",
            edge.from_render_handle, edge.to_render_handle, edge.from_row
        );

        let expected_to_y = computed_anchor_y(child, edge.to_row);
        assert_eq!(
            edge.bezier_args.to_y, expected_to_y,
            "to_y for edge {}->{} should be midpoint of child's first row at row {}",
            edge.from_render_handle, edge.to_render_handle, edge.to_row
        );
    }
}

/// 轻量版布局验证（增量/流式构建中 render_handle 可能不连续，跳过 identity 检查）
fn assert_incremental_layout_relations(model: &GraphModel) {
    assert_node_size(model);
    // 规则：同一深度的节点 x 坐标相同（由 level_meta / finish() 的 normalize_depth_x 保证）
    assert_depth_x_alignment(model);
    assert_edge_row_contract(model);
    assert_edge_from_row_matches_parent_key(model);
    assert_edge_depth_contract(model);
    assert_bezier_contract(model);

    let index = node_index(model);
    for edge in &model.edges {
        let parent = index[&edge.from_render_handle];
        let child = index[&edge.to_render_handle];
        assert!(parent.x < child.x, "parent.x ({}) < child.x ({}) failed", parent.x, child.x);
        assert!(child.y >= parent.y,
            "child.y ({}) should be >= parent.y ({})", child.y, parent.y);
    }

    assert_all_sibling_row_ordering(model);
}
