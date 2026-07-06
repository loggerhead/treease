use treease_core::core::graph_builder::{GraphModel, GraphNode, GraphRow, SequencePresentation};
use treease_core::core::{
    BuilderConfig, GraphBuilder, GraphKind, GraphLanguage, PathSeg, TreeStore, default_config,
    find_node_by_path, path_seg_index, path_seg_key,
};
use treease_core::formats::{Decode, JsonDecoder};
use treease_core::operators::{NodeId as CompatNodeId, NodeKind, SemType, TreeNode};

const LARGE_GRAPH_HOVER_FIXTURE: &str = include_str!("../../../test/fixtures/json/2mb.1.json");

fn scalar_node(value: &str) -> TreeNode {
    TreeNode {
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn typed_scalar_node(sem_type: SemType, value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(sem_type),
        tag: sem_type.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn map_key_node(value: &str) -> TreeNode {
    TreeNode {
        value: value.to_owned(),
        is_map_key: true,
        ..TreeNode::default()
    }
}

fn mapping_node(entries: &[(&str, &str)]) -> TreeNode {
    let mut node = TreeNode {
        kind: NodeKind::Mapping,
        ..TreeNode::default()
    };
    for (key, value) in entries {
        node.content.push(map_key_node(key));
        node.content.push(scalar_node(value));
    }
    node
}

fn mapping_node_with_values(entries: &[(&str, TreeNode)]) -> TreeNode {
    let mut node = TreeNode {
        kind: NodeKind::Mapping,
        ..TreeNode::default()
    };
    for (key, value) in entries {
        node.content.push(map_key_node(key));
        node.content.push(value.clone());
    }
    node
}

fn sequence_node(items: &[TreeNode]) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        content: items.to_vec(),
        ..TreeNode::default()
    }
}

fn compat_tree_from_core(store: &TreeStore, node_id: treease_core::core::NodeId) -> TreeNode {
    let source = store.get(node_id).unwrap();
    let mut out = TreeNode {
        kind: match source.kind {
            treease_core::core::TreeNodeKind::Sequence => NodeKind::Sequence,
            treease_core::core::TreeNodeKind::Mapping => NodeKind::Mapping,
            treease_core::core::TreeNodeKind::Scalar => NodeKind::Scalar,
            treease_core::core::TreeNodeKind::Alias => NodeKind::Alias,
            treease_core::core::TreeNodeKind::Unknown => NodeKind::Unknown,
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
        value: store.value_string_for(node_id).unwrap_or_default(),
        start_byte: source.start_byte,
        end_byte: source.end_byte,
        anchor: store.anchor_for(node_id).unwrap_or_default().to_owned(),
        alias: source.alias().map(|id| CompatNodeId(id.index())),
        head_comment: store
            .head_comment_for(node_id)
            .unwrap_or_default()
            .to_owned(),
        line_comment: store
            .line_comment_for(node_id)
            .unwrap_or_default()
            .to_owned(),
        foot_comment: store
            .foot_comment_for(node_id)
            .unwrap_or_default()
            .to_owned(),
        parent: source.parent.map(|id| CompatNodeId(id.index())),
        key: source.key().map(|id| CompatNodeId(id.index())),
        sequence_index: source.sequence_index().map(|index| index as i64),
        leading_content: store
            .leading_content_for(node_id)
            .unwrap_or_default()
            .to_owned(),
        document: source.document,
        filename: store.filename_for(node_id).unwrap_or_default().to_owned(),
        line: source.line,
        column: source.column,
        file_index: store.file_index_for(node_id).unwrap_or_default(),
        is_map_key: source.is_map_key,
        encode_separate: source.encode_separate(),
        evaluate_together: source.evaluate_together(),
        ..TreeNode::default()
    };
    out.content = source
        .content
        .iter()
        .map(|child_id| compat_tree_from_core(store, *child_id))
        .collect();
    out
}

fn estimated_width(config: &BuilderConfig, text: &str) -> i32 {
    let avg_x10 = config.avg_char_width_x10.max(1);
    let padding = config.row_padding_x * 2;
    let min_char = ((avg_x10 + 5) / 10).max(1);
    let min_width = padding + min_char;
    let text_len = text.chars().count() as i32;
    let content = (text_len * avg_x10 + 5) / 10;
    min_width.max(content + padding)
}

fn table_estimated_width(config: &BuilderConfig, text: &str) -> i32 {
    let avg_x10 = config.avg_char_width_x10.max(1);
    let padding = config.row_padding_x * 2;
    let min_char = ((avg_x10 + 5) / 10).max(1);
    let min_width = padding + min_char;
    let text_len = text.chars().count() as i32 + 1;
    let content = (text_len * avg_x10 + 5) / 10;
    min_width.max(content + padding)
}

fn max_width(config: &BuilderConfig, texts: &[&str], limit: i32) -> i32 {
    let mut width = estimated_width(config, "");
    for t in texts {
        width = width.max(estimated_width(config, t));
    }
    width.min(limit)
}

// Graph layout 规则验证（docs/layout-pipeline.md）
include!("common/layout_assertions.rs");

#[test]
fn graph_builder_object_rows_pair_mapping_keys_with_following_values() {
    let root = mapping_node(&[("name", "Ada"), ("lang", "Rust")]);
    let builder = GraphBuilder::new(default_config(), GraphLanguage::Json);

    let graph_node = builder.build_node_only(&root, 0, &[], 0);

    assert_eq!(graph_node.rows.len(), 2);
    assert_eq!(graph_node.rows[0].key.text, "name");
    assert_eq!(graph_node.rows[0].value.text, "Ada");
    assert_eq!(graph_node.rows[0].cells.len(), 2);
    assert_eq!(graph_node.rows[0].cells[0].text, "name");
    assert_eq!(graph_node.rows[0].cells[1].text, "Ada");
    assert_eq!(graph_node.rows[1].key.text, "lang");
    assert_eq!(graph_node.rows[1].value.text, "Rust");
    assert_eq!(graph_node.rows[1].cells.len(), 2);
    assert_eq!(graph_node.rows[1].cells[0].text, "lang");
    assert_eq!(graph_node.rows[1].cells[1].text, "Rust");
    assert!(graph_node.rows[0].value.editable);
}

#[test]
fn graph_builder_object_rows_keep_non_scalar_values_editable() {
    let root = TreeNode {
        kind: NodeKind::Mapping,
        content: vec![map_key_node("meta"), mapping_node(&[("name", "Ada")])],
        ..TreeNode::default()
    };
    let builder = GraphBuilder::new(default_config(), GraphLanguage::Json);

    let graph_node = builder.build_node_only(&root, 0, &[], 0);

    assert_eq!(graph_node.rows.len(), 1);
    assert_eq!(graph_node.rows[0].key.text, "meta");
    assert_eq!(graph_node.rows[0].cells.len(), 2);
    assert_eq!(graph_node.rows[0].cells[0].text, "meta");
    assert!(graph_node.rows[0].value.editable);
}

#[test]
fn graph_builder_table_includes_index_column_before_mapping_keys() {
    let root = TreeNode {
        kind: NodeKind::Sequence,
        content: vec![
            mapping_node(&[("id", "1"), ("name", "Ada")]),
            mapping_node(&[("id", "2"), ("name", "Grace")]),
        ],
        ..TreeNode::default()
    };
    let builder = GraphBuilder::new(default_config(), GraphLanguage::Json);

    let graph_node = builder.build_node_only(&root, 0, &[], 0);
    let table = graph_node.table.expect("sequence should render as table");
    let headers: Vec<_> = table
        .columns
        .iter()
        .map(|cell| cell.text.as_str())
        .collect();
    let first_row: Vec<_> = table.rows[0]
        .iter()
        .map(|cell| cell.text.as_str())
        .collect();

    assert_eq!(headers, ["", "id", "name"]);
    assert_eq!(first_row, ["0", "1", "Ada"]);
    assert_eq!(table.width, table.column_widths.iter().sum::<i32>());
}

#[test]
fn graph_builder_scalar_mapping_values_do_not_build_child_nodes() {
    let root = mapping_node(&[("name", "Ada")]);
    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);

    let model = builder.build(&root);

    assert_incremental_layout_relations(&model);
    assert_eq!(model.nodes.len(), 1);
}

#[test]
fn graph_builder_stable_id_depends_on_path_not_scalar_text() {
    let builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let path = [PathSeg::Key("value".to_owned())];

    let left = builder.build_node_only(&scalar_node("one"), 0, &path, 0);
    let right = builder.build_node_only(&scalar_node("two"), 0, &path, 0);

    assert_eq!(left.stable_id, right.stable_id);
    assert_eq!(left.key.stable_id, right.key.stable_id);
}

#[test]
fn graph_builder_sequence_presentation_is_header_table_for_mapping_rows_only() {
    let builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let header_table = TreeNode {
        kind: NodeKind::Sequence,
        content: vec![mapping_node(&[("id", "1")]), mapping_node(&[("id", "2")])],
        ..TreeNode::default()
    };
    let headerless = TreeNode {
        kind: NodeKind::Sequence,
        content: vec![scalar_node("one"), scalar_node("two")],
        ..TreeNode::default()
    };

    assert_eq!(
        builder.sequence_presentation(&header_table),
        SequencePresentation::HeaderTable
    );
    assert_eq!(
        builder.sequence_presentation(&headerless),
        SequencePresentation::HeaderlessTable
    );
}

#[test]
fn graph_builder_headerless_table_uses_index_and_value_body_cells() {
    let builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let root = TreeNode {
        kind: NodeKind::Sequence,
        content: vec![scalar_node("one"), scalar_node("two")],
        ..TreeNode::default()
    };

    let graph_node = builder.build_node_only(&root, 0, &[], 0);
    let table = graph_node
        .table
        .expect("headerless sequence should render as table");

    assert_eq!(table.columns.len(), 0);
    assert_eq!(table.rows[0][0].text, "0");
    assert_eq!(table.rows[0][1].text, "one");
    assert_eq!(table.rows[1][0].text, "1");
    assert_eq!(table.rows[1][1].text, "two");
}

#[test]
fn graph_builder_build_assigns_render_handles_and_layout_x_positions() {
    let root = TreeNode {
        kind: NodeKind::Sequence,
        content: vec![
            scalar_node("head"),
            mapping_node(&[("name", "Ada")]),
            scalar_node("tail"),
        ],
        ..TreeNode::default()
    };
    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);

    let model = builder.build(&root);

    assert_incremental_layout_relations(&model);
    for (index, node) in model.nodes.iter().enumerate() {
        assert_eq!(node.render_handle, index as u32);
        if node.depth == 0 {
            assert_eq!(node.x, 0);
        } else {
            assert!(node.x >= default_config().h_gap);
        }
        assert!(node.width > 0);
        assert!(node.height > 0);
    }
}

#[test]
fn graph_builder_edges_connect_parent_box_to_child_box() {
    let root = TreeNode {
        kind: NodeKind::Sequence,
        content: vec![
            scalar_node("head"),
            mapping_node(&[("name", "Ada")]),
            scalar_node("tail"),
        ],
        ..TreeNode::default()
    };
    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);

    let model = builder.build(&root);

    assert_incremental_layout_relations(&model);
    assert!(!model.edges.is_empty());
    for edge in &model.edges {
        let parent = &model.nodes[edge.from_render_handle as usize];
        let child = &model.nodes[edge.to_render_handle as usize];
        assert_eq!(child.depth, parent.depth + 1);
        assert_eq!(edge.bezier_args.from_x, parent.x + parent.width);
        assert_eq!(edge.bezier_args.to_x, child.x);
        assert!(edge.bezier_args.from_x < edge.bezier_args.to_x);
        assert!(edge.bezier_args.c1x >= edge.bezier_args.from_x);
        assert!(edge.bezier_args.c2x <= edge.bezier_args.to_x);
        // c1y == from_y (被贝塞尔控制点生成函数直接赋值为 y1)
        let expected_c1y = computed_anchor_y(parent, edge.from_row);
        assert_eq!(
            edge.bezier_args.c1y, expected_c1y,
            "c1y for edge {}->{} should be midpoint of parent's value cell at row {}",
            edge.from_render_handle, edge.to_render_handle, edge.from_row
        );
        // c2y == to_y
        let expected_c2y = computed_anchor_y(child, edge.to_row);
        assert_eq!(
            edge.bezier_args.c2y, expected_c2y,
            "c2y for edge {}->{} should be midpoint of child's first row at row {}",
            edge.from_render_handle, edge.to_render_handle, edge.to_row
        );
    }
}

// ---------------------------------------------------------------------------
// graph_builder derives table columns and binds cells to compute nodes
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_derives_table_columns_and_binds_cells_to_compute_nodes() {
    let v1 = typed_scalar_node(SemType::Int, "1");
    let v2 = typed_scalar_node(SemType::Int, "2");
    let v3 = typed_scalar_node(SemType::Int, "3");
    let v4 = typed_scalar_node(SemType::Int, "4");

    let row1 = mapping_node_with_values(&[("a", v1.clone()), ("b", v2.clone())]);
    let row2 = mapping_node_with_values(&[("b", v3.clone()), ("c", v4.clone())]);
    let root = sequence_node(&[row1, row2]);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let model = builder.build(&root);

    assert_incremental_layout_relations(&model);
    assert_eq!(model.nodes.len(), 1);
    let table_node = &model.nodes[0];
    assert_eq!(table_node.kind, GraphKind::Table);
    assert!(table_node.table.is_some());

    let table = table_node.table.as_ref().unwrap();
    // 4 columns: "" (index), "a", "b", "c"
    assert_eq!(table.columns.len(), 4);
    assert_eq!(table.columns[0].text, "");
    assert_eq!(table.columns[1].text, "a");
    assert_eq!(table.columns[2].text, "b");
    assert_eq!(table.columns[3].text, "c");
    assert_eq!(table.columns.len(), table.column_widths.len());
    let width_sum: i32 = table.column_widths.iter().sum();
    assert_eq!(width_sum, table.width);

    assert_eq!(table.rows.len(), 2);
    // Row 0: cell at column "a" has text "1", source points to v1
    assert_eq!(table.rows[0][1].text, "1");
    assert_eq!(table.rows[0][1].sem_type, Some("!!int".to_string()));
    assert!(table.rows[0][1].source.is_some());
    // Row 0: missing header-table field is rendered as miss and is not clickable
    assert_eq!(table.rows[0][3].text, "miss");
    assert!(table.rows[0][3].source.is_none());
    assert!(table.rows[0][3].path.is_empty());
    assert_eq!(table.rows[0][3].value, "miss");
    assert_eq!(table.rows[0][3].sem_type, Some("!!str".to_string()));
    // Row 1: missing header-table field is rendered as miss and is not clickable
    assert_eq!(table.rows[1][1].text, "miss");
    assert!(table.rows[1][1].source.is_none());
    assert!(table.rows[1][1].path.is_empty());
    assert_eq!(table.rows[1][1].value, "miss");
    assert_eq!(table.rows[1][1].sem_type, Some("!!str".to_string()));
    // Row 1: cell at column "c" has text "4", source points to v4
    assert_eq!(table.rows[1][3].text, "4");
    assert!(table.rows[1][3].source.is_some());
}

// ---------------------------------------------------------------------------
// graph_builder expands table children for subtree panels when enabled
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_expands_table_children_for_subtree_panels_when_enabled() {
    let team_name = scalar_node("beta");
    let team = mapping_node_with_values(&[("name", team_name)]);
    let region_id = typed_scalar_node(SemType::Int, "2");
    let region = mapping_node_with_values(&[("id", region_id)]);
    let row = mapping_node_with_values(&[("team", team), ("region", region)]);
    let root = sequence_node(&[row]);
    let path = [PathSeg::Key("Groups".to_owned())];

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    builder.expand_table_children = true;
    let result = builder.build_subtree(&root, &path);

    assert_incremental_layout_relations(&result);
    assert_eq!(result.nodes.len(), 4);
    assert_eq!(result.edges.len(), 3);
    assert_eq!(result.nodes[0].kind, GraphKind::Table);
    assert_eq!(result.nodes[1].kind, GraphKind::Object);
    assert_eq!(result.nodes[2].kind, GraphKind::Object);
    assert_eq!(result.nodes[3].kind, GraphKind::Object);

    // Find the edge from the table node (render_handle 0)
    let table_edge = result
        .edges
        .iter()
        .find(|e| e.from_render_handle == 0)
        .expect("should have edge from table");
    assert_eq!(table_edge.from_row, 1);
    assert_eq!(table_edge.to_render_handle, 1);

    // Child node paths include "Groups" prefix
    assert_eq!(result.nodes[1].path.len(), 2);
    assert!(matches!(&result.nodes[1].path[0], PathSeg::Key(k) if k == "Groups"));
    assert!(matches!(result.nodes[1].path[1], PathSeg::Index(0)));

    assert_eq!(result.nodes[2].path.len(), 3);
    assert_eq!(result.nodes[3].path.len(), 3);
    assert!(matches!(&result.nodes[2].path[2], PathSeg::Key(_)));
    assert!(matches!(&result.nodes[3].path[2], PathSeg::Key(_)));

    let child_key_a = match &result.nodes[2].path[2] {
        PathSeg::Key(k) => k.as_str(),
        _ => "",
    };
    let child_key_b = match &result.nodes[3].path[2] {
        PathSeg::Key(k) => k.as_str(),
        _ => "",
    };
    let saw_team = child_key_a == "team" || child_key_b == "team";
    let saw_region = child_key_a == "region" || child_key_b == "region";
    assert!(saw_team);
    assert!(saw_region);
}

#[test]
fn graph_builder_builds_hover_panel_subtree_for_demo_group_like_fixture() {
    let response = mapping_node_with_values(&[
        (
            "ResponseMetadata",
            mapping_node_with_values(&[("RequestId", scalar_node("req-1"))]),
        ),
        (
            "Result",
            mapping_node_with_values(&[("Ok", scalar_node("true"))]),
        ),
    ]);
    let row = mapping_node_with_values(&[
        ("Name", scalar_node("demo")),
        ("Description", scalar_node("")),
        (
            "Request",
            scalar_node("POST /?Action=CopyGeniusProject HTTP/1.1"),
        ),
        ("Response", response),
    ]);
    let demo_group = sequence_node(&[row]);
    let path = [
        PathSeg::Key("ApiList".to_owned()),
        PathSeg::Index(0),
        PathSeg::Key("DemoGroup".to_owned()),
    ];

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    builder.expand_table_children = true;
    let result = builder.build_subtree(&demo_group, &path);

    assert_incremental_layout_relations(&result);
    assert_eq!(result.nodes.len(), 5);
    assert_eq!(result.edges.len(), 4);
    let table = result.nodes[0].table.as_ref().expect("table root expected");
    assert_eq!(result.nodes[0].kind, GraphKind::Table);
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.columns[3].text, "Request");
    assert_eq!(table.columns[4].text, "Response");
    assert!(matches!(result.nodes[1].path[3], PathSeg::Index(0)));
    assert!(matches!(&result.nodes[2].path[4], PathSeg::Key(key) if key == "Response"));
    assert!(matches!(&result.nodes[3].path[5], PathSeg::Key(key) if key == "ResponseMetadata"));
    assert!(matches!(&result.nodes[4].path[5], PathSeg::Key(key) if key == "Result"));
}

#[test]
fn graph_builder_builds_hover_panel_subtree_from_stored_analysis_boundary() {
    let source = r#"{"Result":{"Blocks":[{"TaskError":{"Error":{"Code":500,"Message":"boom"}}}]}}"#;
    let mut decoded = JsonDecoder.decode_str(source).unwrap();
    decoded.store.set_document_analysis(
        "stored-task-error",
        "json",
        decoded.root,
        None,
        source,
        vec![],
        vec![],
        vec![],
        String::new(),
    );

    let path = [
        path_seg_key("Result"),
        path_seg_key("Blocks"),
        path_seg_index(0),
        path_seg_key("TaskError"),
    ];
    let target = find_node_by_path(decoded.root, &path, false, &decoded.store).unwrap();
    let subtree = compat_tree_from_core(&decoded.store, target);
    let graph_path = [
        PathSeg::Key("Result".to_owned()),
        PathSeg::Key("Blocks".to_owned()),
        PathSeg::Index(0),
        PathSeg::Key("TaskError".to_owned()),
    ];

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    builder.expand_table_children = true;
    let result = builder.build_subtree(&subtree, &graph_path);

    assert_incremental_layout_relations(&result);
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.edges.len(), 1);
    assert_eq!(result.nodes[0].kind, GraphKind::Object);
    assert!(matches!(&result.nodes[0].path[3], PathSeg::Key(key) if key == "TaskError"));
    assert!(matches!(&result.nodes[1].path[4], PathSeg::Key(key) if key == "Error"));
}

#[test]
fn graph_builder_matches_chunked_import_boundary_with_concatenated_cached_source() {
    let chunks = [
        r#"{"Result":{"Blocks":["#,
        r#"{"TaskError":{"Error":{"Code":500,"Message":"boom"}}}"#,
        r#"]}}"#,
    ];
    let source = chunks.concat();
    let mut decoded = JsonDecoder.decode_str(&source).unwrap();
    decoded.store.set_document_analysis(
        "chunked-task-error",
        "json",
        decoded.root,
        None,
        &source,
        vec![],
        vec![],
        vec![],
        String::new(),
    );

    let path = [
        path_seg_key("Result"),
        path_seg_key("Blocks"),
        path_seg_index(0),
        path_seg_key("TaskError"),
    ];
    let target = find_node_by_path(decoded.root, &path, false, &decoded.store).unwrap();
    let subtree = compat_tree_from_core(&decoded.store, target);
    let graph_path = [
        PathSeg::Key("Result".to_owned()),
        PathSeg::Key("Blocks".to_owned()),
        PathSeg::Index(0),
        PathSeg::Key("TaskError".to_owned()),
    ];

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    builder.expand_table_children = true;
    let result = builder.build_subtree(&subtree, &graph_path);

    assert_incremental_layout_relations(&result);
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.edges.len(), 1);
    assert_eq!(result.nodes[0].rows[0].key.text, "Error");
    assert_eq!(result.nodes[0].rows[0].value.text, "{2}");
    assert!(matches!(&result.nodes[1].path[4], PathSeg::Key(key) if key == "Error"));
}

#[test]
fn graph_builder_keeps_table_hover_panel_rows_to_single_child_edge_for_large_json_content_nle_draft_list()
 {
    let decoded = JsonDecoder.decode_str(LARGE_GRAPH_HOVER_FIXTURE).unwrap();

    let path = [
        path_seg_key("Result"),
        path_seg_key("Blocks"),
        path_seg_index(0),
        path_seg_key("Content"),
    ];
    let target = find_node_by_path(decoded.root, &path, false, &decoded.store).unwrap();
    let subtree = compat_tree_from_core(&decoded.store, target);
    let graph_path = [
        PathSeg::Key("Result".to_owned()),
        PathSeg::Key("Blocks".to_owned()),
        PathSeg::Index(0),
        PathSeg::Key("Content".to_owned()),
    ];

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    builder.expand_table_children = true;
    let result = builder.build_subtree(&subtree, &graph_path);

    assert_incremental_layout_relations(&result);
    let table_node = result
        .nodes
        .iter()
        .find(|node| {
            node.kind == GraphKind::Table
                && node.path.len() == 5
                && matches!(node.path[4], PathSeg::Key(ref key) if key == "NleDraftList")
        })
        .expect("NleDraftList table node expected");
    let table = table_node.table.as_ref().expect("table expected");
    assert!(!table.rows.is_empty());

    let mut row_edge_counts = std::collections::HashMap::<i32, usize>::new();
    let mut sample_child = None;
    for edge in &result.edges {
        if edge.from_render_handle != table_node.render_handle {
            continue;
        }
        *row_edge_counts.entry(edge.from_row).or_insert(0) += 1;
        if sample_child.is_none() {
            sample_child = result.nodes.get(edge.to_render_handle as usize);
        }
    }

    for row_index in 0..table.rows.len() {
        let row_number = row_index as i32 + 1;
        assert!(row_edge_counts.get(&row_number).copied().unwrap_or(0) <= 1);
    }

    let sample_child = sample_child.expect("table child expected");
    assert_eq!(sample_child.kind, GraphKind::Object);
    assert_eq!(sample_child.path.len(), 6);
    assert!(matches!(sample_child.path[5], PathSeg::Index(_)));
}

#[test]
fn graph_builder_builds_subtree_for_large_json_task_error_hover_panel() {
    let decoded = JsonDecoder.decode_str(LARGE_GRAPH_HOVER_FIXTURE).unwrap();

    let path = [
        path_seg_key("Result"),
        path_seg_key("Blocks"),
        path_seg_index(6),
        path_seg_key("TaskError"),
    ];
    let target = find_node_by_path(decoded.root, &path, false, &decoded.store).unwrap();
    let subtree = compat_tree_from_core(&decoded.store, target);
    let graph_path = [
        PathSeg::Key("Result".to_owned()),
        PathSeg::Key("Blocks".to_owned()),
        PathSeg::Index(6),
        PathSeg::Key("TaskError".to_owned()),
    ];

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    builder.expand_table_children = true;
    let result = builder.build_subtree(&subtree, &graph_path);

    assert_incremental_layout_relations(&result);
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.edges.len(), 1);
    assert_eq!(result.nodes[0].kind, GraphKind::Object);
    assert!(matches!(&result.nodes[0].path[3], PathSeg::Key(key) if key == "TaskError"));
    assert_eq!(result.nodes[1].kind, GraphKind::Object);
    assert!(matches!(&result.nodes[1].path[4], PathSeg::Key(key) if key == "Error"));
}

// ---------------------------------------------------------------------------
// graph_delta keeps unchanged sibling object when earlier scalar becomes object
// ---------------------------------------------------------------------------
#[test]
fn graph_delta_keeps_unchanged_sibling_object_when_earlier_scalar_becomes_object() {
    let old_a = typed_scalar_node(SemType::Int, "1");
    let old_b_y = typed_scalar_node(SemType::Int, "2");
    let old_b = mapping_node_with_values(&[("y", old_b_y)]);
    let old_root = mapping_node_with_values(&[("a", old_a), ("b", old_b)]);

    let new_a_x = typed_scalar_node(SemType::Int, "1");
    let new_a = mapping_node_with_values(&[("x", new_a_x)]);
    let new_b_y = typed_scalar_node(SemType::Int, "2");
    let new_b = mapping_node_with_values(&[("y", new_b_y)]);
    let new_root = mapping_node_with_values(&[("a", new_a), ("b", new_b)]);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let old_model = builder.build(&old_root);
    builder.reset();
    assert_incremental_layout_relations(&old_model);
    let new_model = builder.build(&new_root);

    assert_incremental_layout_relations(&new_model);
    // Simulate graph_delta: compare old and new models.
    // Old: root + child "b" (a is an inline scalar in root's rows)
    // New: root + child "a" + child "b"
    // The "a" node is newly added as a separate object child.
    // The "b" node is unchanged. The root is updated (rows changed).

    // Find added nodes (in new but not in old) by path.
    let old_paths: std::collections::HashSet<Vec<PathSeg>> =
        old_model.nodes.iter().map(|n| n.path.clone()).collect();
    let new_paths: std::collections::HashSet<Vec<PathSeg>> =
        new_model.nodes.iter().map(|n| n.path.clone()).collect();

    // Nodes added: present in new but not old
    let added_paths: Vec<&Vec<PathSeg>> = new_model
        .nodes
        .iter()
        .filter(|n| !old_paths.contains(&n.path))
        .map(|n| &n.path)
        .collect();

    // Nodes removed: present in old but not new
    let removed_paths: Vec<&Vec<PathSeg>> = old_model
        .nodes
        .iter()
        .filter(|n| !new_paths.contains(&n.path))
        .map(|n| &n.path)
        .collect();

    assert_eq!(added_paths.len(), 1);
    assert!(
        matches!(added_paths[0].as_slice(), [PathSeg::Key(k)] if k == "a"),
        "expected added node path to be [Key(\"a\")], got {:?}",
        added_paths[0]
    );
    assert_eq!(removed_paths.len(), 0);
    assert!(new_model.nodes.len() > old_model.nodes.len());
}

// ---------------------------------------------------------------------------
// graph_builder uses dynamic column widths with max cap
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_uses_dynamic_column_widths_with_max_cap() {
    let config = default_config();
    let short_value = scalar_node("0");
    let long_value = scalar_node("empty string as");
    let row1 = mapping_node_with_values(&[("index", short_value), ("empty string as", long_value)]);
    let root = sequence_node(&[row1]);

    let mut builder = GraphBuilder::new(config.clone(), GraphLanguage::Json);
    let model = builder.build(&root);
    let table = model.nodes[0].table.as_ref().expect("should have table");
    assert_incremental_layout_relations(&model);

    assert_eq!(table.column_widths.len(), 3);
    // Column widths should be non-decreasing (index <= "index" <= "empty string as")
    assert!(table.column_widths[1] >= table.column_widths[0]);
    assert!(table.column_widths[2] >= table.column_widths[1]);
    // All widths capped at table_column_width
    for &w in &table.column_widths {
        assert!(w <= config.table_column_width);
    }
}

// ---------------------------------------------------------------------------
// graph_builder uses shared width logic for table index column
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_uses_shared_width_logic_for_table_index_column() {
    let config = default_config();
    let values = ["x", "y", "z", "w", "v", "u", "t", "s", "r", "q", "p", "o"];
    let rows: Vec<TreeNode> = values
        .iter()
        .map(|v| mapping_node_with_values(&[("alpha", scalar_node(v))]))
        .collect();
    let root = sequence_node(&rows);

    let mut builder = GraphBuilder::new(config.clone(), GraphLanguage::Json);
    let model = builder.build(&root);
    let table = model.nodes[0].table.as_ref().expect("should have table");
    assert_incremental_layout_relations(&model);

    // Index column width should match table width for "11" plus one slack character.
    let expected_index_width = table_estimated_width(&config, "11");
    assert_eq!(expected_index_width, table.column_widths[0]);
    assert_eq!(expected_index_width, table.columns[0].box_args.width);
    assert_eq!(expected_index_width, table.rows[0][0].box_args.width);
    assert_eq!(
        table.columns[0].box_args.width,
        table.rows[0][0].box_args.width
    );
    // Header cells include the table border offset; row cells are row-relative.
    assert_eq!(
        table.columns[1].box_args.x,
        table.rows[0][1].box_args.x + config.node_border_width.max(0)
    );
}

#[test]
fn graph_builder_column_width_reserves_one_extra_character() {
    let config = default_config();
    let mut rows = Vec::new();
    for index in 0..=161 {
        rows.push(mapping_node_with_values(&[(
            "index",
            scalar_node(&index.to_string()),
        )]));
    }
    let root = sequence_node(&rows);

    let mut builder = GraphBuilder::new(config.clone(), GraphLanguage::Json);
    let model = builder.build(&root);
    let table = model.nodes[0].table.as_ref().expect("should have table");
    assert_incremental_layout_relations(&model);

    let expected = {
        let avg_x10 = config.avg_char_width_x10.max(1);
        let padding = config.row_padding_x * 2;
        let min_char = ((avg_x10 + 5) / 10).max(1);
        let min_width = padding + min_char;
        let text_len_with_slack = "161".chars().count() as i32 + 1;
        let content = (text_len_with_slack * avg_x10 + 5) / 10;
        min_width.max(content + padding)
    };

    assert_eq!(expected, table.column_widths[0]);
    assert_eq!(expected, table.rows[161][0].box_args.width);
}

// ---------------------------------------------------------------------------
// graph_builder column width considers header text
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_column_width_considers_header_text() {
    let short_value = scalar_node("0");
    let long_value = scalar_node("0");
    let row1 = mapping_node_with_values(&[("index", short_value), ("empty string as", long_value)]);
    let root = sequence_node(&[row1]);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let model = builder.build(&root);
    let table = model.nodes[0].table.as_ref().expect("should have table");
    assert_incremental_layout_relations(&model);

    assert_eq!(table.column_widths.len(), 3);
    // Column widths should account for header text, so "empty string as" >= "index"
    assert!(table.column_widths[1] >= table.column_widths[0]);
    assert!(table.column_widths[2] >= table.column_widths[1]);
}

// ---------------------------------------------------------------------------
// graph_builder computes width and height for object and scalar nodes
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_computes_width_and_height_for_object_and_scalar_nodes() {
    let config = default_config();
    let v1 = typed_scalar_node(SemType::Int, "1");
    let v2 = typed_scalar_node(SemType::Int, "2");
    let root_obj = mapping_node_with_values(&[("a", v1), ("b", v2)]);

    let mut builder = GraphBuilder::new(config.clone(), GraphLanguage::Json);
    let result_obj = builder.build(&root_obj);
    let node_obj = &result_obj.nodes[0];
    assert_incremental_layout_relations(&result_obj);

    assert_eq!(node_obj.kind, GraphKind::Object);
    let obj_key_width = max_width(&config, &["a", "b"], config.key_width);
    let obj_val_width = max_width(&config, &["1", "2"], config.value_width);
    assert_eq!(
        obj_key_width + obj_val_width + config.node_border_width * 2,
        node_obj.width
    );
    let expected_height = config.row_height * 2 + config.node_border_width * 2;
    assert_eq!(expected_height, node_obj.height);

    // Scalar node
    builder.reset();
    let root_scalar = typed_scalar_node(SemType::Int, "7");
    let result_scalar = builder.build(&root_scalar);
    let node_scalar = &result_scalar.nodes[0];
    assert_incremental_layout_relations(&result_scalar);

    assert_eq!(node_scalar.kind, GraphKind::Scalar);
    let scalar_val_width = max_width(&config, &["7"], config.value_width);
    assert_eq!(
        scalar_val_width + config.node_border_width * 2,
        node_scalar.width
    );
    let expected_scalar_height = config.row_height + config.node_border_width * 2;
    assert_eq!(expected_scalar_height, node_scalar.height);
}

#[test]
fn graph_builder_scalar_rows_render_without_value_label() {
    let builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let graph_node = builder.build_node_only(&typed_scalar_node(SemType::Int, "123"), 0, &[], 0);

    assert_eq!(graph_node.kind, GraphKind::Scalar);
    assert_eq!(graph_node.rows.len(), 1);
    assert_eq!(graph_node.rows[0].key.text, "");
    assert_eq!(graph_node.rows[0].value.text, "123");
    assert_eq!(graph_node.rows[0].cells[0].text, "");
    assert_eq!(graph_node.rows[0].cells[1].text, "123");
    assert_eq!(graph_node.rows[0].key.box_args.width, 0);
    assert_eq!(graph_node.rows[0].value.box_args.x, 0);
}

// ---------------------------------------------------------------------------
// graph_builder renders empty containers as scalar summary cells
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_renders_empty_containers_as_scalar_summary_cells() {
    let config = default_config();
    let builder = GraphBuilder::new(config, GraphLanguage::Json);

    // Empty object
    let empty_object = mapping_node_with_values(&[]);
    let empty_object_view = builder.build_node_only(&empty_object, 0, &[], 0);
    assert_eq!(empty_object_view.kind, GraphKind::Scalar);
    assert_eq!(empty_object_view.rows.len(), 1);
    assert_eq!(empty_object_view.rows[0].key.text, "");
    assert_eq!(empty_object_view.rows[0].value.text, "{}");
    assert!(!empty_object_view.rows[0].key.editable);
    assert!(empty_object_view.rows[0].value.editable);
    assert_eq!(empty_object_view.rows[0].key.box_args.width, 0);
    assert_eq!(empty_object_view.rows[0].value.box_args.x, 0);

    // Empty sequence
    let empty_sequence = sequence_node(&[]);
    let empty_sequence_view = builder.build_node_only(&empty_sequence, 0, &[], 0);
    assert_eq!(empty_sequence_view.kind, GraphKind::Scalar);
    assert_eq!(empty_sequence_view.rows.len(), 1);
    assert_eq!(empty_sequence_view.rows[0].key.text, "");
    assert_eq!(empty_sequence_view.rows[0].value.text, "[]");
    assert!(!empty_sequence_view.rows[0].key.editable);
    assert!(empty_sequence_view.rows[0].value.editable);
    assert_eq!(empty_sequence_view.rows[0].key.box_args.width, 0);
    assert_eq!(empty_sequence_view.rows[0].value.box_args.x, 0);
}

// ---------------------------------------------------------------------------
// graph_builder keeps full meta path for leaf nodes
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_keeps_full_meta_path_for_leaf_nodes() {
    let leaf = mapping_node_with_values(&[("value", typed_scalar_node(SemType::Int, "1"))]);
    let path = [
        PathSeg::Key("alpha".to_owned()),
        PathSeg::Key("beta".to_owned()),
        PathSeg::Key("gamma".to_owned()),
        PathSeg::Key("delta".to_owned()),
        PathSeg::Key("epsilon".to_owned()),
    ];

    let builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let view_node = builder.build_node_only(&leaf, 0, &path, 0);

    // Leaf nodes (no children) show the full path
    assert_eq!(view_node.meta.text, "alpha.beta.gamma.delta.epsilon");
}

// ---------------------------------------------------------------------------
// graph_builder truncates branch meta path using only tail segments
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_truncates_branch_meta_path_using_only_tail_segments() {
    let branch_child = mapping_node_with_values(&[("value", typed_scalar_node(SemType::Int, "1"))]);
    let branch = mapping_node_with_values(&[("child", branch_child)]);
    let path = [
        PathSeg::Key("alpha".to_owned()),
        PathSeg::Key("beta".to_owned()),
        PathSeg::Key("gamma".to_owned()),
        PathSeg::Key("delta".to_owned()),
        PathSeg::Key("epsilon".to_owned()),
    ];

    let builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let view_node = builder.build_node_only(&branch, 0, &path, 0);

    // Branch nodes (with children) show truncated path: "...lastSegment"
    assert_eq!(view_node.meta.text, "...epsilon");
}

// ---------------------------------------------------------------------------
// graph_builder meta path truncation does not support head keep config
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_meta_path_truncation_does_not_support_head_keep_config() {
    let branch_child = mapping_node_with_values(&[("value", typed_scalar_node(SemType::Int, "1"))]);
    let branch = mapping_node_with_values(&[("child", branch_child)]);
    let path = [
        PathSeg::Key("alpha".to_owned()),
        PathSeg::Key("beta".to_owned()),
        PathSeg::Key("gamma".to_owned()),
        PathSeg::Key("delta".to_owned()),
        PathSeg::Key("epsilon".to_owned()),
    ];

    let mut config = default_config();
    config.meta_path_min_segments = 3;
    config.meta_path_min_chars = 8;
    config.meta_path_keep_tail_segments = 2;

    let builder = GraphBuilder::new(config, GraphLanguage::Json);
    let view_node = builder.build_node_only(&branch, 0, &path, 0);

    // With keep_tail_segments=2, shows "...delta.epsilon"
    assert_eq!(view_node.meta.text, "...delta.epsilon");
}

// ---------------------------------------------------------------------------
// graph_builder preserves index segments in truncated meta path suffix
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_preserves_index_segments_in_truncated_meta_path_suffix() {
    let branch_child = mapping_node_with_values(&[("value", typed_scalar_node(SemType::Int, "1"))]);
    let branch = mapping_node_with_values(&[("child", branch_child)]);
    let path = [
        PathSeg::Key("root".to_owned()),
        PathSeg::Key("items".to_owned()),
        PathSeg::Index(12),
        PathSeg::Key("name".to_owned()),
    ];

    let mut config = default_config();
    config.meta_path_min_segments = 3;
    config.meta_path_min_chars = 8;
    config.meta_path_keep_tail_segments = 2;

    let builder = GraphBuilder::new(config, GraphLanguage::Json);
    let view_node = builder.build_node_only(&branch, 0, &path, 0);

    // Index segments preserved in truncated suffix
    assert_eq!(view_node.meta.text, "...[12].name");
}

// ---------------------------------------------------------------------------
// graph_builder renders no-header sequence as headerless table with child nodes
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_renders_no_header_sequence_as_headerless_table_with_child_nodes() {
    let scalar_item = typed_scalar_node(SemType::Int, "1");
    let map_value = typed_scalar_node(SemType::Int, "2");
    let map_item = mapping_node_with_values(&[("a", map_value)]);
    let seq_child_a = typed_scalar_node(SemType::Int, "3");
    let seq_child_b = typed_scalar_node(SemType::Int, "4");
    let seq_item = sequence_node(&[seq_child_a, seq_child_b]);
    let root = sequence_node(&[scalar_item, map_item, seq_item]);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let result = builder.build(&root);

    assert_incremental_layout_relations(&result);
    assert_eq!(result.nodes.len(), 3);
    let root_node = &result.nodes[0];
    let table = root_node.table.as_ref().expect("should have table");
    assert_eq!(root_node.kind, GraphKind::Table);
    assert_eq!(root_node.rows.len(), 0);
    assert_eq!(table.header_height, 0);
    assert_eq!(table.columns.len(), 0);
    assert_eq!(table.rows.len(), 3);

    // Row 0: scalar "1"
    assert_eq!(table.rows[0][0].text, "0");
    assert_eq!(table.rows[0][1].text, "1");
    assert_eq!(table.rows[0][1].path.len(), 1);
    assert!(matches!(table.rows[0][1].path[0], PathSeg::Index(0)));

    // Row 1: mapping {a: 2} -> summary "{1}"
    assert_eq!(table.rows[1][0].text, "1");
    assert_eq!(table.rows[1][1].text, "{1}");
    assert!(table.rows[1][1].editable);

    // Row 2: sequence [3, 4] -> summary "[2]"
    assert_eq!(table.rows[2][0].text, "2");
    assert_eq!(table.rows[2][1].text, "[2]");
    assert!(table.rows[2][1].editable);

    // Edges: 2 child edges from the table
    assert_eq!(result.edges.len(), 2);
    assert_eq!(result.edges[0].from_row, 1);
    assert_eq!(result.edges[1].from_row, 2);
    assert_eq!(result.nodes[1].kind, GraphKind::Object);
    assert_eq!(result.nodes[2].kind, GraphKind::Table);
    assert_eq!(result.nodes[1].path.len(), 1);
    assert!(matches!(result.nodes[1].path[0], PathSeg::Index(1)));
    assert_eq!(result.nodes[2].path.len(), 1);
    assert!(matches!(result.nodes[2].path[0], PathSeg::Index(2)));
}

#[test]
fn graph_builder_keeps_scrollable_headerless_table_rows_inline() {
    let row_a = sequence_node(&[typed_scalar_node(SemType::Int, "1")]);
    let row_b = sequence_node(&[typed_scalar_node(SemType::Int, "2")]);
    let row_c = sequence_node(&[typed_scalar_node(SemType::Int, "3")]);
    let row_d = sequence_node(&[typed_scalar_node(SemType::Int, "4")]);
    let root = sequence_node(&[row_a, row_b, row_c, row_d]);
    let mut config = default_config();
    config.table_max_height = config.row_height * 2;

    let mut builder = GraphBuilder::new(config, GraphLanguage::Json);
    let result = builder.build(&root);

    let table = result.nodes[0]
        .table
        .as_ref()
        .expect("headerless sequence should render as table");
    assert!(
        table.total_height > table.view_height,
        "fixture must exercise the scrollable table branch"
    );
    assert_eq!(
        result.nodes.len(),
        1,
        "scrollable table rows must stay inline instead of expanding child graph nodes"
    );
    assert!(
        result.edges.is_empty(),
        "scrollable table rows must not create outgoing child edges"
    );
}

// ---------------------------------------------------------------------------
// graph_builder uses unknown count marker for open sequence summary
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_uses_unknown_count_marker_for_open_sequence_summary() {
    let item = typed_scalar_node(SemType::Int, "1");
    let mut sequence = sequence_node(&[item]);
    sequence.sequence_closed = false;
    let root = mapping_node_with_values(&[("items", sequence)]);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let result = builder.build(&root);

    assert_incremental_layout_relations(&result);
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.nodes[0].kind, GraphKind::Object);
    // Open sequence shows "[?]" as summary
    assert_eq!(result.nodes[0].rows[0].value.text, "[?]");
}

// ---------------------------------------------------------------------------
// graph_builder keeps dedicated fallback value column for heterogeneous header table
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_keeps_dedicated_fallback_value_column_for_heterogeneous_header_table() {
    let map_value = typed_scalar_node(SemType::Int, "1");
    let first_row = mapping_node_with_values(&[("a", map_value)]);
    let scalar_row = typed_scalar_node(SemType::Int, "2");
    let seq_child_a = typed_scalar_node(SemType::Int, "3");
    let seq_child_b = typed_scalar_node(SemType::Int, "4");
    let seq_row = sequence_node(&[seq_child_a, seq_child_b]);
    let root = sequence_node(&[first_row, scalar_row, seq_row]);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let result = builder.build(&root);

    assert_incremental_layout_relations(&result);
    assert_eq!(result.nodes.len(), 1);
    let table_node = &result.nodes[0];
    assert_eq!(table_node.kind, GraphKind::Table);
    let table = table_node.table.as_ref().unwrap();

    // 3 columns: "" (index), "a", "value" (fallback)
    assert_eq!(table.columns.len(), 3);
    assert_eq!(table.columns[1].text, "a");
    assert_eq!(table.columns[2].text, "value");

    // Row 0: mapping {a: 1} -> "1" in column "a", "" in fallback
    assert_eq!(table.rows[0][1].text, "1");
    assert_eq!(table.rows[0][2].text, "");

    // Row 1: scalar "2" keeps the fallback value column; non-mapping rows still leave object columns empty
    assert_eq!(table.rows[1][1].text, "");
    assert_eq!(table.rows[1][2].text, "2");

    // Row 2: sequence [3, 4] keeps the fallback value column; non-mapping rows still leave object columns empty
    assert_eq!(table.rows[2][1].text, "");
    assert_eq!(table.rows[2][2].text, "[2]");
    assert!(table.rows[2][2].editable);
}

// ---------------------------------------------------------------------------
// graph_builder preserves object columns while heterogeneous rows use fallback column
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_preserves_object_columns_while_heterogeneous_rows_use_fallback_column() {
    let first_value = typed_scalar_node(SemType::Int, "1");
    let second_value = typed_scalar_node(SemType::Int, "2");
    let first_row = mapping_node_with_values(&[("a", first_value)]);
    let second_row = mapping_node_with_values(&[("a", second_value)]);
    let scalar_row = typed_scalar_node(SemType::Int, "3");
    let root = sequence_node(&[first_row, second_row, scalar_row]);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let result = builder.build(&root);

    assert_incremental_layout_relations(&result);
    let table = result.nodes[0].table.as_ref().unwrap();
    // 3 columns: "" (index), "a", "value" (fallback)
    assert_eq!(table.columns.len(), 3);
    assert_eq!(table.columns[1].text, "a");
    assert_eq!(table.columns[2].text, "value");

    // Row 0: mapping {a: 1}
    assert_eq!(table.rows[0][1].text, "1");
    assert_eq!(table.rows[0][2].text, "");

    // Row 1: mapping {a: 2}
    assert_eq!(table.rows[1][1].text, "2");
    assert_eq!(table.rows[1][2].text, "");

    // Row 2: scalar "3" -> uses fallback column while object column stays empty for non-mapping rows
    assert_eq!(table.rows[2][1].text, "");
    assert_eq!(table.rows[2][2].text, "3");
}

// ---------------------------------------------------------------------------
// graph_builder keeps fallback value column for empty first mapping header table
// ---------------------------------------------------------------------------
#[test]
fn graph_builder_keeps_fallback_value_column_for_empty_first_mapping_header_table() {
    let empty_map = mapping_node_with_values(&[]);
    let scalar_row = typed_scalar_node(SemType::Int, "2");
    let root = sequence_node(&[empty_map, scalar_row]);

    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::Json);
    let result = builder.build(&root);

    assert_incremental_layout_relations(&result);
    let table = result.nodes[0].table.as_ref().unwrap();
    // 2 columns: "" (index), "value" (fallback, since first mapping is empty)
    assert_eq!(table.columns.len(), 2);
    assert_eq!(table.columns[1].text, "value");
    // Row 1: scalar "2" in fallback column
    assert_eq!(table.rows[1][1].text, "2");
}
