use treease_core::core::graph_builder::{GraphKind, GraphModel, GraphNode, GraphRow, PathSeg};
use treease_core::core::{
    BuilderConfig, DocumentTextEdit, GraphBuilder, GraphFragmentIndex, GraphLanguage,
    TreeNodeKind as CoreTreeNodeKind, build_incremental_graph_delta,
    compute_ancestor_relayout_chain, default_config,
};
use treease_core::formats::DecodedDocument;
use treease_core::formats::{Decode, JsonDecoder};
use treease_core::operators::{NodeKind, SemType, TreeNode};

fn decoded_root_to_graph_tree(decoded: &DecodedDocument) -> TreeNode {
    core_node_to_graph_tree(&decoded.store, decoded.root)
}

fn core_node_to_graph_tree(
    store: &treease_core::core::TreeStore,
    id: treease_core::core::NodeId,
) -> TreeNode {
    let source = store.get(id).expect("decoded node should exist");
    TreeNode {
        kind: match source.kind {
            CoreTreeNodeKind::Sequence => NodeKind::Sequence,
            CoreTreeNodeKind::Mapping => NodeKind::Mapping,
            CoreTreeNodeKind::Scalar | CoreTreeNodeKind::Unknown => NodeKind::Scalar,
            CoreTreeNodeKind::Alias => NodeKind::Alias,
        },
        sequence_closed: source.sequence_closed(),
        sem_type: source.sem_type.map(|sem_type| match sem_type {
            treease_core::core::SemType::Nil => SemType::Nil,
            treease_core::core::SemType::Str => SemType::Str,
            treease_core::core::SemType::Int => SemType::Int,
            treease_core::core::SemType::Float => SemType::Float,
            treease_core::core::SemType::Boolean => SemType::Boolean,
            treease_core::core::SemType::Map => SemType::Map,
            treease_core::core::SemType::Seq => SemType::Seq,
        }),
        tag: source.tag.to_string_value(),
        value: store.value_string_for(id).unwrap_or_default(),
        start_byte: source.start_byte,
        end_byte: source.end_byte,
        content: source
            .content
            .iter()
            .map(|child| core_node_to_graph_tree(store, *child))
            .collect(),
        leading_content: store.leading_content_for(id).unwrap_or_default().to_owned(),
        is_map_key: source.is_map_key,
        sequence_index: source.sequence_index().map(|index| index as i64),
        anchor: store.anchor_for(id).unwrap_or_default().to_owned(),
        head_comment: store.head_comment_for(id).unwrap_or_default().to_owned(),
        line_comment: store.line_comment_for(id).unwrap_or_default().to_owned(),
        foot_comment: store.foot_comment_for(id).unwrap_or_default().to_owned(),
        document: source.document,
        filename: store.filename_for(id).unwrap_or_default().to_owned(),
        line: source.line,
        column: source.column,
        file_index: store.file_index_for(id).unwrap_or_default(),
        encode_separate: source.encode_separate(),
        evaluate_together: source.evaluate_together(),
        ..TreeNode::default()
    }
}

fn scalar_node(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn map_key_node(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: SemType::Str.tag().to_owned(),
        value: value.to_owned(),
        is_map_key: true,
        ..TreeNode::default()
    }
}

fn mapping_node(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        content.push(map_key_node(key));
        content.push(value);
    }
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.tag().to_owned(),
        content,
        ..TreeNode::default()
    }
}

fn nested_model() -> GraphModel {
    let root = mapping_node(vec![
        (
            "a",
            mapping_node(vec![("x", mapping_node(vec![("leaf", scalar_node("1"))]))]),
        ),
        ("b", mapping_node(vec![("y", scalar_node("2"))])),
    ]);
    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    builder.build(&root)
}

// ---------------------------------------------------------------------------
// Helpers aligned with graph_layout.zig
// ---------------------------------------------------------------------------

fn build_from_json(json: &str) -> GraphModel {
    build_from_json_with_config(json, default_config())
}

fn build_from_json_with_config(json: &str, config: BuilderConfig) -> GraphModel {
    let decoded = JsonDecoder
        .decode_str(json)
        .expect("json decode should succeed");
    let root = decoded_root_to_graph_tree(&decoded);
    let mut builder = GraphBuilder::new(config, GraphLanguage::Json);
    builder.build(&root)
}

fn web_default_config() -> BuilderConfig {
    BuilderConfig {
        key_width: 300,
        value_width: 500,
        row_height: 18,
        row_padding_x: 20,
        row_padding_y: 1,
        v_gap: 60,
        h_gap: 60,
        table_header_height: 26,
        table_row_height: 28,
        table_column_width: 500,
        ..default_config()
    }
}

fn subtree_bottom(model: &GraphModel, render_handle: u32) -> i32 {
    let node = model
        .nodes
        .iter()
        .find(|n| n.render_handle == render_handle)
        .expect("node should exist");
    let mut bottom = node.y + node.height;
    for edge in &model.edges {
        if edge.from_render_handle != render_handle {
            continue;
        }
        bottom = bottom.max(subtree_bottom(model, edge.to_render_handle));
    }
    bottom
}

fn node_by_render_handle(nodes: &[GraphNode], render_handle: u32) -> Option<&GraphNode> {
    nodes.iter().find(|n| n.render_handle == render_handle)
}

fn node_by_root_key<'a>(nodes: &'a [GraphNode], key: &str) -> Option<&'a GraphNode> {
    nodes
        .iter()
        .find(|node| node.path.len() == 1 && matches!(&node.path[0], PathSeg::Key(k) if k == key))
}

fn has_updated_node(nodes: &[GraphNode], render_handle: u32) -> bool {
    nodes.iter().any(|n| n.render_handle == render_handle)
}

fn row_looks_expandable(row: &GraphRow) -> bool {
    let value_text = &row.value.text;
    if value_text.is_empty() {
        return false;
    }
    let first = value_text.as_bytes()[0];
    first == b'{' || first == b'['
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

fn assert_edge_node_refs_valid(model: &GraphModel) {
    for edge in &model.edges {
        assert!(
            (edge.from_render_handle as usize) < model.nodes.len(),
            "from_render_handle out of bounds"
        );
        assert!(
            (edge.to_render_handle as usize) < model.nodes.len(),
            "to_render_handle out of bounds"
        );
        assert_ne!(
            edge.from_render_handle, edge.to_render_handle,
            "edge should not be self-referential"
        );
    }
}

fn assert_node_identity_and_size(model: &GraphModel) {
    for (idx, node) in model.nodes.iter().enumerate() {
        assert_eq!(
            node.render_handle, idx as u32,
            "render_handle should match index"
        );
        assert!(node.width > 0, "node width should be positive");
        assert!(node.height > 0, "node height should be positive");
    }
}

fn assert_node_size(model: &GraphModel) {
    for node in &model.nodes {
        assert!(node.width > 0, "node width should be positive");
        assert!(node.height > 0, "node height should be positive");
    }
}

fn assert_depth_x_alignment(model: &GraphModel) {
    for (idx, node) in model.nodes.iter().enumerate() {
        for previous in &model.nodes[..idx] {
            if previous.depth == node.depth {
                assert_eq!(previous.x, node.x, "nodes at same depth should have same x");
                break;
            }
        }
    }
}

fn assert_sibling_row_ordering(model: &GraphModel, parent_id: u32) {
    for (i, edge_a) in model.edges.iter().enumerate() {
        if edge_a.from_render_handle != parent_id {
            continue;
        }
        let child_a = node_by_render_handle(&model.nodes, edge_a.to_render_handle)
            .expect("child a should exist");

        for (j, edge_b) in model.edges.iter().enumerate() {
            if i == j {
                continue;
            }
            if edge_b.from_render_handle != parent_id {
                continue;
            }
            let child_b = node_by_render_handle(&model.nodes, edge_b.to_render_handle)
                .expect("child b should exist");
            if edge_a.from_row < edge_b.from_row {
                assert!(child_a.y <= child_b.y);
                assert!(child_a.y + child_a.height <= child_b.y + child_b.height);
            }
        }
    }
}

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

/// Assert bezier from_y is at the midpoint of the parent's value cell (规则:
/// "起点 y 为父节点对应 value 单元格的中点"), and to_y is at the midpoint of
/// the child's first row (规则: "终点 y 为子节点首个 row 的中点").
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
/// 按 docs/web/layout-pipeline.md Graph layout 规则计算锚点 y：
/// - Table：header row 为 header 高度中点，body row 为对应 body row 中点
/// - Object/Scalar：使用 row.abs_bounds.y + height/2（abs_bounds 已包含 node.y）
/// 按 docs/web/layout-pipeline.md Graph layout 规则计算锚点 y。
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

fn assert_layout_relations(model: &GraphModel) {
    assert_edge_node_refs_valid(model);
    assert_node_identity_and_size(model);
    assert_depth_x_alignment(model);
    assert_edge_row_contract(model);
    assert_edge_from_row_matches_parent_key(model);
    assert_edge_depth_contract(model);
    assert_bezier_contract(model);

    for edge in &model.edges {
        let parent = node_by_render_handle(&model.nodes, edge.from_render_handle)
            .expect("parent should exist");
        let child =
            node_by_render_handle(&model.nodes, edge.to_render_handle).expect("child should exist");
        assert!(
            parent.x < child.x,
            "parent.x ({}) < child.x ({})",
            parent.x,
            child.x
        );
        assert!(
            child.y >= parent.y,
            "child.y ({}) should be >= parent.y ({})",
            child.y,
            parent.y
        );
    }

    for edge in &model.edges {
        assert_sibling_row_ordering(model, edge.from_render_handle);
    }
}

fn assert_incremental_layout_relations(model: &GraphModel) {
    assert_node_size(model);
    assert_depth_x_alignment(model);
    assert_edge_row_contract(model);
    assert_edge_from_row_matches_parent_key(model);
    assert_edge_depth_contract(model);
    assert_bezier_contract(model);

    for edge in &model.edges {
        let parent = node_by_render_handle(&model.nodes, edge.from_render_handle)
            .expect("parent should exist");
        let child =
            node_by_render_handle(&model.nodes, edge.to_render_handle).expect("child should exist");
        assert!(parent.x < child.x);
        assert!(parent.y <= child.y);
    }

    for edge in &model.edges {
        assert_sibling_row_ordering(model, edge.from_render_handle);
    }
}

fn check_layout(json: &str, node_count: usize, edge_count: usize) {
    let graph_model = build_from_json(json);
    assert_eq!(graph_model.nodes.len(), node_count);
    assert_eq!(graph_model.edges.len(), edge_count);

    assert_node_identity_and_size(&graph_model);
    assert_edge_node_refs_valid(&graph_model);

    if edge_count > 0 {
        assert_layout_relations(&graph_model);
    }

    // Determinism: building again should produce identical layout
    let graph_model_again = build_from_json(json);
    assert_eq!(graph_model.nodes.len(), graph_model_again.nodes.len());
    assert_eq!(graph_model.edges.len(), graph_model_again.edges.len());
    assert_node_identity_and_size(&graph_model_again);
    assert_edge_node_refs_valid(&graph_model_again);

    for (node, again) in graph_model.nodes.iter().zip(graph_model_again.nodes.iter()) {
        assert_eq!(node.render_handle, again.render_handle);
        assert_eq!(node.depth, again.depth);
        assert_eq!(node.kind, again.kind);
        assert_eq!(node.x, again.x);
        assert_eq!(node.y, again.y);
        assert_eq!(node.width, again.width);
        assert_eq!(node.height, again.height);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn graph_layout_fragment_index_resolves_parent_stable_id_for_nested_nodes() {
    let model = nested_model();
    let index = GraphFragmentIndex::build(&model);
    let path_a = [PathSeg::Key("a".to_owned())];
    assert_layout_relations(&model);
    let path_ax = [PathSeg::Key("a".to_owned()), PathSeg::Key("x".to_owned())];

    let a_stable_id = index
        .stable_id_for_path(&path_a)
        .expect("path a should be indexed");
    let ax_stable_id = index
        .stable_id_for_path(&path_ax)
        .expect("path a.x should be indexed");
    let a_fragment = index
        .get_by_stable_id(a_stable_id)
        .expect("path a fragment should exist");
    let ax_fragment = index
        .get_by_stable_id(ax_stable_id)
        .expect("path a.x fragment should exist");

    assert_eq!(ax_fragment.parent_stable_id, Some(a_stable_id));
    assert_eq!(a_fragment.path, path_a);
    assert_eq!(ax_fragment.path, path_ax);
}

#[test]
fn graph_layout_relayout_chain_walks_changed_fragment_to_root_without_duplicates() {
    let model = nested_model();
    let index = GraphFragmentIndex::build(&model);
    let path_a = [PathSeg::Key("a".to_owned())];
    assert_layout_relations(&model);
    let path_ax = [PathSeg::Key("a".to_owned()), PathSeg::Key("x".to_owned())];

    let a_stable_id = index
        .stable_id_for_path(&path_a)
        .expect("path a should be indexed");
    let ax_stable_id = index
        .stable_id_for_path(&path_ax)
        .expect("path a.x should be indexed");
    let root_stable_id = model.nodes[0].stable_id;

    let chain = compute_ancestor_relayout_chain(&index, &[ax_stable_id, a_stable_id]);

    assert_eq!(chain, vec![ax_stable_id, a_stable_id, root_stable_id]);
}

// ---------------------------------------------------------------------------
// Test 1: view layout: representative shapes
// ---------------------------------------------------------------------------

#[test]
fn view_layout_representative_shapes() {
    let cases = [
        ("6", 1, 0),
        ("{}", 1, 0),
        ("[]", 1, 0),
        (
            r#"{
  "int64": 12345678987654321,
  "key": "value"
}"#,
            1,
            0,
        ),
        (
            r#"{
  "int64": 12345678987654321,
  "key": "value",
  "array": {"a": 1, "b": 2}
}"#,
            2,
            1,
        ),
        (
            r#"{
  "int64": 12345678987654321,
  "key": "value",
  "array": [12345678987654321, 0.1234567891111111111]
}"#,
            2,
            1,
        ),
        ("[12345678987654321, 0.1234567891111111111]", 1, 0),
        (r#"[{"a": 1}, {"b": 2}]"#, 1, 0),
        ("[[1, 1], [2, 2]]", 3, 2),
    ];

    for (input, node_count, edge_count) in cases {
        check_layout(input, node_count, edge_count);
    }
}

// ---------------------------------------------------------------------------
// Test 2: view layout: language example json ordering
// ---------------------------------------------------------------------------

#[test]
fn view_layout_language_example_json_ordering() {
    let language_example_json = include_str!("../../../test/fixtures/json/sample.1.json");
    let model = build_from_json(language_example_json);
    assert_layout_relations(&model);
}

// ---------------------------------------------------------------------------
// Test 3: view layout: multi child parent keeps row-order monotonic
// ---------------------------------------------------------------------------

#[test]
fn view_layout_multi_child_parent_keeps_row_order_monotonic() {
    let json = r#"{
  "first": {"a": 1},
  "middle": 2,
  "last": [1, 2]
}"#;

    check_layout(json, 3, 2);

    let model = build_from_json(json);

    let mut first_child_y: Option<i32> = None;
    let mut last_child_y: Option<i32> = None;
    for edge in &model.edges {
        if edge.from_render_handle != 0 {
            continue;
        }
        let child = &model.nodes[edge.to_render_handle as usize];
        if edge.from_row == 0 {
            first_child_y = Some(child.y);
        }
        if edge.from_row == 2 {
            last_child_y = Some(child.y);
        }
    }

    let first = first_child_y.expect("first child y should be set");
    let last = last_child_y.expect("last child y should be set");
    assert!(first <= last);
}

// ---------------------------------------------------------------------------
// docs/core/index.md rule 2: 第一个子节点 y = 父节点 y
// ---------------------------------------------------------------------------

#[test]
fn view_layout_first_child_y_equals_parent_y() {
    // 父节点有多个 key，首个 value 是容器（生成首个子节点）。
    // 文档规则 2 要求第一个子节点 y 与父节点 y 相同。
    let json = r#"{
  "first": {"a": 1},
  "second": {"b": 2},
  "third": {"c": 3}
}"#;

    let model = build_from_json(json);
    assert_layout_relations(&model);

    for parent in &model.nodes {
        // 找到该父节点 from_row 最小的那条出边对应的子节点。
        let mut first_edge: Option<&treease_core::core::graph_builder::GraphEdge> = None;
        for edge in &model.edges {
            if edge.from_render_handle != parent.render_handle {
                continue;
            }
            match first_edge {
                Some(prev) if prev.from_row <= edge.from_row => {}
                _ => first_edge = Some(edge),
            }
        }
        let Some(edge) = first_edge else {
            continue;
        };
        let child = node_by_render_handle(&model.nodes, edge.to_render_handle)
            .expect("first child should exist");
        assert_eq!(
            child.y, parent.y,
            "first child (handle {}) y should equal parent (handle {}) y",
            child.render_handle, parent.render_handle
        );
    }
}
// ---------------------------------------------------------------------------

#[test]
fn view_layout_same_parent_siblings_keep_a_single_vgap() {
    let json = r#"{
  "first": {"nested": {"leaf": 1}},
  "second": {"value": 2}
}"#;

    let model = build_from_json(json);
    assert_layout_relations(&model);

    let mut first_child: Option<&GraphNode> = None;
    let mut second_child: Option<&GraphNode> = None;
    for edge in &model.edges {
        if edge.from_render_handle != 0 {
            continue;
        }
        let child = &model.nodes[edge.to_render_handle as usize];
        if edge.from_row == 0 {
            first_child = Some(child);
        }
        if edge.from_row == 1 {
            second_child = Some(child);
        }
    }

    let first = first_child.expect("first child should exist");
    let second = second_child.expect("second child should exist");
    let expected_gap = default_config().v_gap;
    assert_eq!(
        subtree_bottom(&model, first.render_handle) + expected_gap,
        second.y
    );
}

// ---------------------------------------------------------------------------
// Test 5: view layout: sibling spacing ignores previous subtree depth
// ---------------------------------------------------------------------------

#[test]
fn view_layout_sibling_spacing_ignores_previous_subtree_depth() {
    let json = r#"{
  "first": {"nested": {"deep": {"leaf": 1}}},
  "second": {"nested": {"value": 2}}
}"#;

    let model = build_from_json(json);
    assert_layout_relations(&model);

    let mut first_child: Option<&GraphNode> = None;
    let mut second_child: Option<&GraphNode> = None;
    for edge in &model.edges {
        if edge.from_render_handle != 0 {
            continue;
        }
        let child = &model.nodes[edge.to_render_handle as usize];
        if edge.from_row == 0 {
            first_child = Some(child);
        }
        if edge.from_row == 1 {
            second_child = Some(child);
        }
    }

    let first = first_child.expect("first child should exist");
    let second = second_child.expect("second child should exist");
    let expected_gap = default_config().v_gap;
    assert_eq!(
        subtree_bottom(&model, first.render_handle) + expected_gap,
        second.y
    );
}

// ---------------------------------------------------------------------------
// Test 6: view layout: mixed siblings keep equal vGap
// ---------------------------------------------------------------------------

#[test]
fn view_layout_mixed_siblings_keep_equal_vgap() {
    let json = r##"{
  "object": {"nested": {"leaf": 1}},
  "table_without_header": ["a", {"name": "bob"}, [3, 4]],
  "preview": {"color": "#4f46e5", "time": "2026-04-13T10:00:00Z"}
}"##;

    let model = build_from_json(json);
    assert_layout_relations(&model);

    let mut object_child: Option<&GraphNode> = None;
    let mut no_header_child: Option<&GraphNode> = None;
    let mut preview_child: Option<&GraphNode> = None;
    for edge in &model.edges {
        if edge.from_render_handle != 0 {
            continue;
        }
        let child = &model.nodes[edge.to_render_handle as usize];
        if edge.from_row == 0 {
            object_child = Some(child);
        }
        if edge.from_row == 1 {
            no_header_child = Some(child);
        }
        if edge.from_row == 2 {
            preview_child = Some(child);
        }
    }

    let object = object_child.expect("object child should exist");
    let no_header = no_header_child.expect("no-header child should exist");
    let preview = preview_child.expect("preview child should exist");

    assert_eq!(object.kind, GraphKind::Object);
    assert_eq!(no_header.kind, GraphKind::Table);
    assert_eq!(preview.kind, GraphKind::Object);

    let expected_gap = default_config().v_gap;
    assert_eq!(
        subtree_bottom(&model, object.render_handle) + expected_gap,
        no_header.y
    );
    assert_eq!(
        subtree_bottom(&model, no_header.render_handle) + expected_gap,
        preview.y
    );
}

// ---------------------------------------------------------------------------
// Test 7: view layout: equivalent key order keeps topology contracts
// ---------------------------------------------------------------------------

#[test]
fn view_layout_equivalent_key_order_keeps_topology_contracts() {
    let json_a = r#"{
  "a": {"x": 1},
  "b": [1, 2]
}"#;
    let json_b = r#"{
  "b": [1, 2],
  "a": {"x": 1}
}"#;

    let model_a = build_from_json(json_a);
    let model_b = build_from_json(json_b);

    assert_eq!(model_a.nodes.len(), model_b.nodes.len());
    assert_eq!(model_a.edges.len(), model_b.edges.len());
    assert_eq!(model_a.edges.len(), 2);

    assert_layout_relations(&model_a);
    assert_layout_relations(&model_b);

    let mut a_object_count = 0;
    let mut a_table_count = 0;
    for edge in &model_a.edges {
        if edge.from_render_handle != 0 {
            continue;
        }
        let child = &model_a.nodes[edge.to_render_handle as usize];
        if child.kind == GraphKind::Object {
            a_object_count += 1;
        }
        if child.kind == GraphKind::Table {
            a_table_count += 1;
        }
    }

    let mut b_object_count = 0;
    let mut b_table_count = 0;
    for edge in &model_b.edges {
        if edge.from_render_handle != 0 {
            continue;
        }
        let child = &model_b.nodes[edge.to_render_handle as usize];
        if child.kind == GraphKind::Object {
            b_object_count += 1;
        }
        if child.kind == GraphKind::Table {
            b_table_count += 1;
        }
    }

    assert_eq!(a_object_count, 1);
    assert_eq!(a_table_count, 1);
    assert_eq!(b_object_count, 1);
    assert_eq!(b_table_count, 1);
}

// ---------------------------------------------------------------------------
// Test 8: view layout: expandable row set equals edge.from_row set
// ---------------------------------------------------------------------------

#[test]
fn view_layout_expandable_row_set_equals_edge_from_row_set() {
    let json = r#"{
  "obj": {"x": 1},
  "arr": [1, 2],
  "scalar": 3
}"#;

    let model = build_from_json(json);
    assert_layout_relations(&model);
    assert_eq!(model.nodes.len(), 3);
    assert_eq!(model.edges.len(), 2);

    let root = &model.nodes[0];

    let mut expandable_rows: Vec<i32> = Vec::new();
    for (idx, row) in root.rows.iter().enumerate() {
        if row_looks_expandable(row) {
            expandable_rows.push(idx as i32);
        }
    }

    let mut edge_rows: Vec<i32> = Vec::new();
    for edge in &model.edges {
        if edge.from_render_handle != 0 {
            continue;
        }
        edge_rows.push(edge.from_row);
    }

    assert_eq!(expandable_rows.len(), edge_rows.len());

    for row_idx in &expandable_rows {
        assert!(edge_rows.contains(row_idx));
    }
    for row_idx in &edge_rows {
        assert!(expandable_rows.contains(row_idx));
    }
}

// ---------------------------------------------------------------------------
// Test 9: view layout: core default config matches web graph defaults
// ---------------------------------------------------------------------------

#[test]
fn view_layout_core_default_config_matches_web_graph_defaults() {
    let config = default_config();
    assert_eq!(config.key_width, 300);
    assert_eq!(config.value_width, 500);
    assert_eq!(config.row_height, 18);
    assert_eq!(config.row_padding_x, 20);
    assert_eq!(config.row_padding_y, 1);
    assert_eq!(config.v_gap, 60);
    assert_eq!(config.h_gap, 60);
    assert_eq!(config.table_row_height, 28);
    assert_eq!(config.table_header_height, 26);
    assert_eq!(config.table_column_width, 500);
    assert_eq!(config.avg_char_width_x10, 72);
    assert_eq!(config.font_size, 12);
}

// ---------------------------------------------------------------------------
// Test 10: view layout: custom hGap changes child x spacing
// ---------------------------------------------------------------------------

#[test]
fn view_layout_custom_hgap_changes_child_x_spacing() {
    let mut compact = web_default_config();
    compact.h_gap = 12;
    let mut spacious = web_default_config();
    spacious.h_gap = 84;

    let json = r#"{
  "outer": {"inner": 1},
  "label": "hello"
}"#;

    let compact_h_gap = compact.h_gap;
    let spacious_h_gap = spacious.h_gap;
    let compact_model = build_from_json_with_config(json, compact);
    let spacious_model = build_from_json_with_config(json, spacious);
    assert_layout_relations(&compact_model);
    assert_layout_relations(&spacious_model);

    let compact_child = compact_model
        .nodes
        .iter()
        .find(|n| n.depth == 1)
        .expect("compact child should exist");
    let spacious_child = spacious_model
        .nodes
        .iter()
        .find(|n| n.depth == 1)
        .expect("spacious child should exist");

    assert_eq!(
        spacious_h_gap - compact_h_gap,
        spacious_child.x - compact_child.x
    );
}

// ---------------------------------------------------------------------------
// Test 11: view layout: graph-table-missing-row fixture siblings keep one web vGap
// ---------------------------------------------------------------------------

#[test]
fn view_layout_graph_table_missing_row_fixture_siblings_keep_one_web_vgap() {
    let fixture_json = include_str!("../../../test/fixtures/json/graph-table-missing-row.1.json");
    let model = build_from_json_with_config(fixture_json, web_default_config());
    assert_layout_relations(&model);
    assert!(model.nodes.len() > 1);

    let mut api_list: Option<&GraphNode> = None;
    let mut other_depth_one: Option<&GraphNode> = None;
    for node in &model.nodes {
        if node.depth != 1 {
            continue;
        }
        if node.path.len() == 1 && matches!(&node.path[0], PathSeg::Key(k) if k == "ApiList") {
            api_list = Some(node);
            continue;
        }
        if other_depth_one.is_none() {
            other_depth_one = Some(node);
        }
    }

    let api = api_list.expect("ApiList node should exist");
    let other = other_depth_one.expect("other depth-1 node should exist");

    let (first, second) = if api.y <= other.y {
        (api, other)
    } else {
        (other, api)
    };

    assert_eq!(
        subtree_bottom(&model, first.render_handle) + web_default_config().v_gap,
        second.y
    );
}

#[test]
fn view_layout_trajectory_fixture_edges_follow_bezier_contract() {
    let fixture_json = include_str!("../../../test/fixtures/json/trajectory.1.json");
    let model = build_from_json_with_config(fixture_json, web_default_config());

    assert!(
        !model.edges.is_empty(),
        "trajectory fixture should produce graph edges"
    );
    assert_bezier_contract(&model);
}

#[test]
fn trajectory_fixture_is_obfuscated() {
    let fixture_json = include_str!("../../../test/fixtures/json/trajectory.1.json");

    assert!(
        !fixture_json.contains("130862393"),
        "trajectory fixture should not retain raw task ids"
    );
    assert!(
        !fixture_json.contains("智能马桶"),
        "trajectory fixture should not retain raw business copy"
    );
    assert!(
        fixture_json.contains("\\\"media_id\\\""),
        "trajectory fixture should preserve nested json keys inside escaped payloads"
    );
    assert!(
        fixture_json.contains("mime_type"),
        "trajectory fixture should preserve nested json fragment keys inside escaped payloads"
    );
}

#[test]
fn graph_delta_service_emits_changed_trajectory_edges_after_subtree_growth() {
    let old_source = include_str!("../../../test/fixtures/json/trajectory.1.json");
    let root_step_anchor = old_source
        .find("\"root_step\":{")
        .expect("trajectory fixture should contain root_step");
    let basic_info_anchor = old_source[root_step_anchor..]
        .find("\"basic_info\":{")
        .expect("trajectory fixture should contain root_step.basic_info")
        + root_step_anchor;
    let duration_key = "\"duration\":\"";
    let duration_start = old_source[basic_info_anchor..]
        .find(duration_key)
        .expect("trajectory fixture should contain root_step.basic_info.duration")
        + basic_info_anchor
        + duration_key.len();
    let insert_at = old_source[duration_start..]
        .find('"')
        .expect("root_step.basic_info.duration should be a string value")
        + duration_start
        + 1;
    let insertion = ",\n      \"ended_at\": \"2025-02-24T00:00:00Z\"";
    let new_source = format!(
        "{}{}{}",
        &old_source[..insert_at],
        insertion,
        &old_source[insert_at..]
    );

    let old_decoded = JsonDecoder
        .decode_str(old_source)
        .expect("old trajectory json should decode");
    let new_decoded = JsonDecoder
        .decode_str(&new_source)
        .expect("new trajectory json should decode");

    let old_root = decoded_root_to_graph_tree(&old_decoded);

    let mut old_builder = GraphBuilder::new(web_default_config(), GraphLanguage::Json);
    let old_model = old_builder.build(&old_root);

    let edit = DocumentTextEdit {
        start_byte: insert_at as u32,
        old_end_byte: insert_at as u32,
        new_end_byte: (insert_at + insertion.len()) as u32,
        replacement: insertion.to_owned(),
    };

    let actual = build_incremental_graph_delta(
        &old_model,
        &new_decoded.store,
        new_decoded.root,
        &edit,
        web_default_config(),
        GraphLanguage::Json,
    )
    .expect("trajectory subtree growth should stay incremental");
    let actual_model = actual.model_snapshot.materialize();

    assert!(
        actual
            .delta
            .edges_added
            .iter()
            .any(|edge| edge.from_render_handle != edge.to_render_handle),
        "trajectory edit must move at least one edge to cover the regression"
    );
    assert_incremental_layout_relations(&actual_model);
}

// ---------------------------------------------------------------------------
// Test 12: view layout: large headerless table sibling spacing uses subtree
// bottom
// ---------------------------------------------------------------------------

#[test]
fn view_layout_large_headerless_table_sibling_spacing_uses_subtree_bottom() {
    let json = include_str!("../../../test/fixtures/json/large_headerless_table.1.json");

    let model = build_from_json(json);
    assert_layout_relations(&model);

    assert_eq!(model.nodes.len(), 5);
    assert_eq!(model.edges.len(), 4);

    let mut ae: Option<&GraphNode> = None;
    let mut af: Option<&GraphNode> = None;
    let mut ae_prices: Option<&GraphNode> = None;
    for node in &model.nodes {
        if node.path.len() != 1 && node.path.len() != 2 {
            continue;
        }
        if node.path.len() == 1 {
            if let PathSeg::Key(k) = &node.path[0] {
                if k == "AE" {
                    ae = Some(node);
                }
                if k == "AF" {
                    af = Some(node);
                }
            }
        }
        if node.path.len() == 2 {
            if let (PathSeg::Key(k0), PathSeg::Key(k1)) = (&node.path[0], &node.path[1]) {
                if k0 == "AE" && k1 == "Prices" {
                    ae_prices = Some(node);
                }
            }
        }
    }

    let ae_node = ae.expect("AE node should exist");
    let af_node = af.expect("AF node should exist");
    let prices_node = ae_prices.expect("AE.Prices node should exist");

    let prices_table = prices_node
        .table
        .as_ref()
        .expect("Prices should have a table");
    assert_eq!(prices_node.kind, GraphKind::Table);
    assert_eq!(prices_table.header_height, 0);
    assert!(prices_table.rows.len() > 50);
    assert_eq!(
        subtree_bottom(&model, ae_node.render_handle) + default_config().v_gap,
        af_node.y
    );
}

// ---------------------------------------------------------------------------
// Test 13: graph delta service reuses unaffected sibling render ids on scalar
// edit
// ---------------------------------------------------------------------------

#[test]
fn graph_delta_service_reuses_unaffected_sibling_render_ids_on_scalar_edit() {
    let old_source = r#"{"a":{"x":1},"b":{"y":2}}"#;
    let new_source = r#"{"a":{"x":9},"b":{"y":2}}"#;

    let old_decoded = JsonDecoder
        .decode_str(old_source)
        .expect("old json decode should succeed");
    let new_decoded = JsonDecoder
        .decode_str(new_source)
        .expect("new json decode should succeed");

    let old_root = decoded_root_to_graph_tree(&old_decoded);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let old_model = builder.build(&old_root);
    assert_layout_relations(&old_model);
    let old_index = GraphFragmentIndex::build(&old_model);

    let scalar_offset = old_source.find('1').expect("should find '1' in old source") as u32;
    let edit = DocumentTextEdit {
        start_byte: scalar_offset,
        old_end_byte: scalar_offset + 1,
        new_end_byte: scalar_offset + 1,
        replacement: "9".to_owned(),
    };

    let rebuilt = build_incremental_graph_delta(
        &old_model,
        &new_decoded.store,
        new_decoded.root,
        &edit,
        default_config(),
        GraphLanguage::Json,
    );
    assert!(rebuilt.is_some(), "delta service should return a result");

    let result = rebuilt.unwrap();
    let model = result.model_snapshot.materialize();
    assert_incremental_layout_relations(&model);
    assert!(!result.delta.clear);

    let path_b = [PathSeg::Key("b".to_owned())];
    let b_stable_id = old_index
        .stable_id_for_path(&path_b)
        .expect("path b should be indexed");
    let old_b_fragment = old_index
        .get_by_stable_id(b_stable_id)
        .expect("b fragment should exist");

    let new_b = model
        .nodes
        .iter()
        .find(|node| node.path.len() == 1 && matches!(&node.path[0], PathSeg::Key(k) if k == "b"))
        .expect("new b node should exist");

    assert_eq!(old_b_fragment.render_handle, new_b.render_handle);
    assert!(
        !result
            .delta
            .nodes_removed
            .contains(&old_b_fragment.render_handle)
    );
}

#[test]
fn graph_delta_service_rebuilds_subtree_for_special_key_paths() {
    let old_source = r#"{"x[y]":{"v":1},"plain":{"y":2}}"#;
    let new_source = r#"{"x[y]":{"v":9},"plain":{"y":2}}"#;

    let old_decoded = JsonDecoder
        .decode_str(old_source)
        .expect("old json decode should succeed");
    let new_decoded = JsonDecoder
        .decode_str(new_source)
        .expect("new json decode should succeed");

    let old_root = decoded_root_to_graph_tree(&old_decoded);
    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let old_model = builder.build(&old_root);
    assert_layout_relations(&old_model);
    let old_index = GraphFragmentIndex::build(&old_model);

    let scalar_offset = old_source.find('1').expect("should find '1' in old source") as u32;
    let edit = DocumentTextEdit {
        start_byte: scalar_offset,
        old_end_byte: scalar_offset + 1,
        new_end_byte: scalar_offset + 1,
        replacement: "9".to_owned(),
    };

    let rebuilt = build_incremental_graph_delta(
        &old_model,
        &new_decoded.store,
        new_decoded.root,
        &edit,
        default_config(),
        GraphLanguage::Json,
    );
    assert!(
        rebuilt.is_some(),
        "delta service should keep special-key subtree incremental"
    );

    let result = rebuilt.unwrap();
    let model = result.model_snapshot.materialize();
    assert_incremental_layout_relations(&model);
    let special_path = [PathSeg::Key("x[y]".to_owned())];
    let special_stable_id = old_index
        .stable_id_for_path(&special_path)
        .expect("special-key path should be indexed");
    let old_special = old_index
        .get_by_stable_id(special_stable_id)
        .expect("special-key fragment should exist");
    let new_special = model
        .nodes
        .iter()
        .find(|node| node.path.as_slice() == special_path.as_slice())
        .expect("rebuilt model should keep special-key path");

    assert_eq!(old_special.render_handle, new_special.render_handle);
}

// ---------------------------------------------------------------------------
// Test 14: graph delta service preserves layout constraints after scalar table
// width changes
// ---------------------------------------------------------------------------

#[test]
fn graph_delta_service_preserves_layout_constraints_after_scalar_table_width_changes() {
    let old_source = r#"{
  "object": {
    "int": 42,
    "float": 0.125,
    "bool": true,
    "nil": null,
    "arr": [],
    "obj0": {}
  },
  "table_without_header": ["a", "b", "c"],
  "table_with_header": [
    {"h1": 11, "h2": 12, "h3": 13},
    {"h1": 21, "h2": 22, "h3": 23}
  ]
}"#;
    let new_source = r#"{
  "object": {
    "int": 42,
    "float": 0.125,
    "bool": true,
    "nil": null,
    "arr": [],
    "obj0": {}
  },
  "table_without_header": ["a1", "b", "c"],
  "table_with_header": [
    {"h1": 11, "h2": 12, "h3": 13},
    {"h1": 21, "h2": 22, "h3": 23}
  ]
}"#;

    let old_decoded = JsonDecoder
        .decode_str(old_source)
        .expect("old json decode should succeed");
    let new_decoded = JsonDecoder
        .decode_str(new_source)
        .expect("new json decode should succeed");

    let old_root = decoded_root_to_graph_tree(&old_decoded);

    let mut builder = GraphBuilder::new(web_default_config(), GraphLanguage::Json);
    let old_model = builder.build(&old_root);
    assert_layout_relations(&old_model);

    // Find the byte offset of "a" in the old source (the value to edit)
    let value_offset = old_source.find("\"a\"").expect("should find '\"a\"'") as u32 + 2;
    let edit = DocumentTextEdit {
        start_byte: value_offset,
        old_end_byte: value_offset,
        new_end_byte: value_offset + 1,
        replacement: "1".to_owned(),
    };

    let rebuilt = build_incremental_graph_delta(
        &old_model,
        &new_decoded.store,
        new_decoded.root,
        &edit,
        web_default_config(),
        GraphLanguage::Json,
    );
    assert!(rebuilt.is_some(), "delta service should return a result");

    let result = rebuilt.unwrap();
    let model = result.model_snapshot.materialize();
    assert_incremental_layout_relations(&model);

    let table_without_header = node_by_root_key(&model.nodes, "table_without_header")
        .expect("table_without_header should exist");
    let table_with_header = node_by_root_key(&model.nodes, "table_with_header")
        .expect("table_with_header should exist");

    assert_eq!(table_with_header.depth, table_without_header.depth);
    assert_eq!(table_with_header.x, table_without_header.x);
    assert!(has_updated_node(
        &result.delta.nodes_updated,
        table_without_header.render_handle
    ));
}

// ---------------------------------------------------------------------------
// Test 15: graph delta service updates header table scalar cell without full
// graph rebuild
// ---------------------------------------------------------------------------

#[test]
fn graph_delta_service_updates_header_table_scalar_cell_without_full_graph_rebuild() {
    let old_source =
        r#"[{"name":"Adeel Solangi","age":1},{"name":"Adeel Solangi with suffix","age":2}]"#;
    let new_source = r#"[{"name":"hi","age":1},{"name":"Adeel Solangi with suffix","age":2}]"#;

    let old_decoded = JsonDecoder
        .decode_str(old_source)
        .expect("old json decode should succeed");
    let new_decoded = JsonDecoder
        .decode_str(new_source)
        .expect("new json decode should succeed");

    let old_root = decoded_root_to_graph_tree(&old_decoded);
    let new_root = decoded_root_to_graph_tree(&new_decoded);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let old_model = builder.build(&old_root);
    assert_layout_relations(&old_model);
    let old_index = GraphFragmentIndex::build(&old_model);

    let value_offset = old_source.find("Adeel Solangi").expect("should find text") as u32;
    let edit = DocumentTextEdit {
        start_byte: value_offset,
        old_end_byte: value_offset + "Adeel Solangi".len() as u32,
        new_end_byte: value_offset + "hi".len() as u32,
        replacement: "hi".to_owned(),
    };

    let cell_path = [PathSeg::Index(0), PathSeg::Key("name".to_owned())];
    assert!(old_index.find_table_cell_by_path(&cell_path).is_some());

    let rebuilt = build_incremental_graph_delta(
        &old_model,
        &new_decoded.store,
        new_decoded.root,
        &edit,
        default_config(),
        GraphLanguage::Json,
    );
    assert!(rebuilt.is_some(), "delta service should return a result");

    let result = rebuilt.unwrap();
    let model = result.model_snapshot.materialize();
    assert_incremental_layout_relations(&model);
    assert!(!result.delta.clear);
    assert_eq!(result.delta.nodes_added.len(), 0);
    assert_eq!(result.delta.nodes_removed.len(), 0);
    assert_eq!(result.delta.nodes_updated.len(), 0);
    assert_eq!(result.delta.table_cell_patches.len(), 1);
    assert_eq!(result.delta.table_cell_patches[0].row_index, 0);
    assert_eq!(result.delta.table_cell_patches[0].column_index, 1);
    assert_eq!(result.delta.table_cell_patches[0].cell.text, "hi");
    assert_eq!(result.table_cell_patches.len(), 1);
    assert_eq!(result.table_cell_patches[0].row_index, 0);
    assert_eq!(result.table_cell_patches[0].column_index, 1);
    assert_eq!(result.table_cell_patches[0].cell.text, "hi");
    assert_eq!(result.table_cell_patches[0].cell.value, "hi");

    let table = model.nodes[0]
        .table
        .as_ref()
        .expect("root should have a table");
    assert_eq!(table.rows[0][1].text, "hi");
    assert_eq!(table.rows[0][1].value, "hi");
    assert!(table.rows[0][1].source.is_some());

    // Full rebuild should match
    let mut full_builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let full_model = full_builder.build(&new_root);
    assert_layout_relations(&full_model);
    let full_table = full_model.nodes[0]
        .table
        .as_ref()
        .expect("full root should have a table");
    assert_eq!(full_table.rows[0][1].text, table.rows[0][1].text);
    assert_eq!(full_table.column_widths.len(), table.column_widths.len());
}

// ---------------------------------------------------------------------------
// Test 16: graph delta service falls back when header table schema changes
// ---------------------------------------------------------------------------

#[test]
fn graph_delta_service_falls_back_when_header_table_schema_changes() {
    let old_source = r#"[{"name":"amy","age":1}]"#;
    let new_source = r#"[{"name":"ada","age":1,"city":"sf"}]"#;

    let old_decoded = JsonDecoder
        .decode_str(old_source)
        .expect("old json decode should succeed");
    let new_decoded = JsonDecoder
        .decode_str(new_source)
        .expect("new json decode should succeed");

    let old_root = decoded_root_to_graph_tree(&old_decoded);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let old_model = builder.build(&old_root);
    assert_layout_relations(&old_model);

    let value_offset = old_source.find("amy").expect("should find 'amy'") as u32;
    let edit = DocumentTextEdit {
        start_byte: value_offset,
        old_end_byte: value_offset + "amy".len() as u32,
        new_end_byte: value_offset + "ada".len() as u32,
        replacement: "ada".to_owned(),
    };

    let rebuilt = build_incremental_graph_delta(
        &old_model,
        &new_decoded.store,
        new_decoded.root,
        &edit,
        default_config(),
        GraphLanguage::Json,
    );
    assert!(
        rebuilt.is_none(),
        "delta service should return None when schema changes"
    );
}

#[test]
fn layout_engine_matches_graph_builder_full_layout() {
    let via_builder = build_from_json(
        r#"{"left":{"wide_label":{"x":1},"narrow":{"y":2}},"right":[[1,2],[3,4]],"tail":{"z":5}}"#,
    );
    let mut via_engine = via_builder.clone();
    for node in &mut via_engine.nodes {
        node.x = -999;
        node.y = -999;
    }

    treease_core::core::layout_engine::LayoutEngine::new(default_config())
        .layout_full(&mut via_engine, 0);

    let builder_positions: Vec<(u32, i32, i32)> = via_builder
        .nodes
        .iter()
        .map(|node| (node.render_handle, node.x, node.y))
        .collect();
    let engine_positions: Vec<(u32, i32, i32)> = via_engine
        .nodes
        .iter()
        .map(|node| (node.render_handle, node.x, node.y))
        .collect();

    assert_eq!(engine_positions, builder_positions);
}

#[test]
fn example_simple_json_graph_layout_matches_loading_skeleton() {
    let model = build_from_json(include_str!("../../../example/simple.json"));

    let nodes: Vec<(u32, i32, i32, i32, i32)> = model
        .nodes
        .iter()
        .map(|node| (node.render_handle, node.x, node.y, node.width, node.height))
        .collect();
    let edges: Vec<(u32, i32, u32, i32, i32, i32, i32, i32, i32, i32, i32, i32)> = model
        .edges
        .iter()
        .map(|edge| {
            (
                edge.from_render_handle,
                edge.from_row,
                edge.to_render_handle,
                edge.to_row,
                edge.bezier_args.from_x,
                edge.bezier_args.from_y,
                edge.bezier_args.c1x,
                edge.bezier_args.c1y,
                edge.bezier_args.c2x,
                edge.bezier_args.c2y,
                edge.bezier_args.to_x,
                edge.bezier_args.to_y,
            )
        })
        .collect();

    assert_eq!(
        nodes,
        vec![
            (0, 0, 0, 248, 74),
            (1, 308, 0, 154, 110),
            (2, 308, 170, 110, 56),
            (3, 308, 286, 242, 84),
            (4, 308, 430, 592, 128),
            (5, 960, 430, 556, 38),
        ]
    );
    assert_eq!(
        edges,
        vec![
            (0, 0, 1, 0, 248, 10, 288, 10, 268, 10, 308, 10),
            (0, 1, 2, 0, 248, 28, 288, 28, 268, 180, 308, 180),
            (0, 2, 3, 0, 248, 46, 288, 46, 268, 300, 308, 300),
            (4, 6, 5, 0, 900, 548, 940, 548, 920, 440, 960, 440),
            (0, 3, 4, 0, 248, 64, 288, 64, 268, 440, 308, 440),
        ]
    );
}
