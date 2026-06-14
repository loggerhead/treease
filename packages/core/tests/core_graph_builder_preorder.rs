use treease_core::core::graph_builder::{
    GraphCell, GraphEdge, GraphModel, GraphNode, GraphRow, GraphTable,
};
use treease_core::core::{GraphBuilderPreorder, GraphKind, GraphLanguage, PathSeg, default_config};
use treease_core::operators::{NodeKind, SemType, TreeNode};
use treease_core::stream::streaming_events::{Meta, StreamingEvent};
use treease_core::stream::streaming_json::StreamingParser;

fn scalar_node(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn meta_with_path(path: &str) -> Meta {
    Meta {
        path: path.to_string(),
        ..Meta::default()
    }
}

fn meta_with_path_and_sem(path: &str, sem_type: treease_core::core::sem_type::SemType) -> Meta {
    Meta {
        path: path.to_string(),
        sem_type: Some(sem_type),
        ..Meta::default()
    }
}

fn find_node_by_single_key_path(nodes: &[GraphNode], key: &str) -> Option<GraphNode> {
    for node in nodes {
        if node.path.len() != 1 {
            continue;
        }
        if let Some(PathSeg::Key(k)) = node.path.first() {
            if k == key {
                return Some(node.clone());
            }
        }
    }
    None
}

fn find_node_by_key_path(nodes: &[GraphNode], keys: &[&str]) -> Option<GraphNode> {
    for node in nodes {
        if node.path.len() != keys.len() {
            continue;
        }
        let mut matches = true;
        for (i, key) in keys.iter().enumerate() {
            if let Some(PathSeg::Key(k)) = node.path.get(i) {
                if k != *key {
                    matches = false;
                    break;
                }
            } else {
                matches = false;
                break;
            }
        }
        if matches {
            return Some(node.clone());
        }
    }
    None
}

fn subtree_bottom(nodes: &[GraphNode], edges: &[GraphEdge], node_id: u32) -> i32 {
    let node = &nodes[node_id as usize];
    let mut bottom = node.y + node.height;
    for edge in edges {
        if edge.from_render_handle != node_id {
            continue;
        }
        bottom = bottom.max(subtree_bottom(nodes, edges, edge.to_render_handle));
    }
    bottom
}

fn find_table_cell<'a>(table: &'a GraphTable, row_index: usize, column: &str) -> &'a GraphCell {
    let column_index = table
        .columns
        .iter()
        .position(|cell| cell.text == column)
        .unwrap_or_else(|| panic!("missing table column {column}"));
    &table.rows[row_index][column_index]
}

// Graph layout 规则验证（docs/layout-pipeline.md）
include!("common/layout_assertions.rs");

#[test]
fn graph_builder_preorder_builds_scalar_node_and_initial_delta() {
    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    let delta = builder.build_from_tree(&scalar_node("7")).unwrap();
    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);

    assert_eq!(model.nodes.len(), 1);
    assert_eq!(model.edges.len(), 0);
    assert!(!delta.clear);
    assert_eq!(delta.nodes_added.len(), 1);
    assert_eq!(delta.nodes_updated.len(), 0);
    assert_eq!(delta.nodes_removed.len(), 0);
    assert_eq!(delta.edges_added.len(), 0);
    assert_eq!(delta.edges_removed.len(), 0);
}

#[test]
fn graph_builder_preorder_flush_consumes_pending_delta() {
    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    let first = builder.build_from_tree(&scalar_node("7")).unwrap();
    let second = builder.flush().unwrap();

    assert!(!first.clear);
    assert_eq!(first.nodes_added.len(), 1);
    assert!(!first.nodes_added.is_empty());
    assert_eq!(second.nodes_added.len(), 0);
    assert_eq!(second.nodes_updated.len(), 0);
    assert_eq!(second.nodes_removed.len(), 0);
    assert_eq!(second.edges_added.len(), 0);
    assert_eq!(second.edges_removed.len(), 0);
}

// ---------------------------------------------------------------------------
// Test: uses event path for sequence mapping cells
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_uses_event_path_for_sequence_mapping_cells() {
    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    builder
        .on_event(&StreamingEvent::SeqStart(meta_with_path("$")))
        .unwrap();

    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$[0]",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "Action".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "GetCreateCost".to_string(),
            meta: meta_with_path_and_sem("$[0].Action", treease_core::core::sem_type::SemType::Str),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "Description".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "\u{67e5}\u{8be2}\u{521b}\u{70b9}\u{6d88}\u{8017}\u{9884}\u{4f30}".to_string(),
            meta: meta_with_path_and_sem(
                "$[0].Description",
                treease_core::core::sem_type::SemType::Str,
            ),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();

    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$[1]",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "Action".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "ListGeniusProject".to_string(),
            meta: meta_with_path_and_sem("$[1].Action", treease_core::core::sem_type::SemType::Str),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    assert!(model.nodes.len() >= 1);

    let table_node = model
        .nodes
        .iter()
        .find(|n| n.kind == GraphKind::Table && n.table.is_some())
        .expect("table node not found");
    let table = table_node.table.as_ref().unwrap();
    assert_eq!(table.rows.len(), 2);

    // Row 0
    assert_eq!(table.rows[0][1].text, "GetCreateCost");
    assert_eq!(
        table.rows[0][1].path,
        vec![PathSeg::Index(0), PathSeg::Key("Action".to_string())]
    );
    assert_eq!(
        table.rows[0][2].text,
        "\u{67e5}\u{8be2}\u{521b}\u{70b9}\u{6d88}\u{8017}\u{9884}\u{4f30}"
    );
    assert_eq!(
        table.rows[0][2].path,
        vec![PathSeg::Index(0), PathSeg::Key("Description".to_string())]
    );

    // Row 1
    assert_eq!(table.rows[1][1].text, "ListGeniusProject");
    assert_eq!(
        table.rows[1][1].path,
        vec![PathSeg::Index(1), PathSeg::Key("Action".to_string())]
    );
    assert_eq!(table.rows[1][2].text, "");
    assert_eq!(
        table.rows[1][2].path,
        vec![PathSeg::Index(1), PathSeg::Key("Description".to_string())]
    );
}

// ---------------------------------------------------------------------------
// Test: reconstructs paths from split streamed json fixture
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_reconstructs_paths_from_split_streamed_json_fixture() {
    const FIXTURE: &str =
        include_str!("../../../test/fixtures/json/graph-table-missing-row.1.json");

    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    let mut parser = StreamingParser::with_path_emission(true, true);
    let split_at = FIXTURE.len() / 2;
    parser.feed(&FIXTURE[..split_at]).unwrap();
    parser.feed(&FIXTURE[split_at..]).unwrap();
    let events = parser.finish().unwrap();

    for event in &events {
        builder.on_event(event).unwrap();
    }

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    let table_node = model
        .nodes
        .iter()
        .find(|n| {
            n.kind == GraphKind::Table
                && n.table.is_some()
                && n.path.len() == 1
                && matches!(n.path.first(), Some(PathSeg::Key(k)) if k == "ApiList")
        })
        .expect("ApiList table not found");
    let table = table_node.table.as_ref().unwrap();
    assert!(table.rows.len() > 20);

    // Row 18 Action
    let row18_action = &table.rows[18][1];
    assert_eq!(row18_action.text, "DssQqltltKniu");
    assert_eq!(
        row18_action.path,
        vec![
            PathSeg::Key("ApiList".to_string()),
            PathSeg::Index(18),
            PathSeg::Key("Action".to_string()),
        ]
    );

    // Row 19 Action
    let row19_action = &table.rows[19][1];
    assert_eq!(row19_action.text, "AdxvXnovykFcxgxao");
    assert_eq!(
        row19_action.path,
        vec![
            PathSeg::Key("ApiList".to_string()),
            PathSeg::Index(19),
            PathSeg::Key("Action".to_string()),
        ]
    );

    // Row 19 Description
    let row19_desc = find_table_cell(table, 19, "Description");
    assert_eq!(
        row19_desc.text,
        "\u{5143}\u{4e0b}\u{51b3}\u{59d4}\u{95ee}\u{5165}\u{89c1}\u{590d}"
    );
    assert_eq!(
        row19_desc.path,
        vec![
            PathSeg::Key("ApiList".to_string()),
            PathSeg::Index(19),
            PathSeg::Key("Description".to_string()),
        ]
    );

    // Row 20 Action
    let row20_action = &table.rows[20][1];
    assert_eq!(row20_action.text, "UsdugeCsspejrwsov");
    assert_eq!(
        row20_action.path,
        vec![
            PathSeg::Key("ApiList".to_string()),
            PathSeg::Index(20),
            PathSeg::Key("Action".to_string()),
        ]
    );
}

// ---------------------------------------------------------------------------
// Test: works with stream decoder split fixture
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_works_with_stream_decoder_split_fixture() {
    const FIXTURE: &str =
        include_str!("../../../test/fixtures/json/graph-table-missing-row.1.json");

    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    let mut parser = StreamingParser::with_path_emission(true, true);
    let split_at = FIXTURE.len() / 2;
    parser.feed(&FIXTURE[..split_at]).unwrap();
    parser.feed(&FIXTURE[split_at..]).unwrap();

    let events = parser.finish().unwrap();
    for event in &events {
        builder.on_event(event).unwrap();
    }

    let delta = builder.finish_delta().unwrap();
    assert!(!delta.nodes_added.is_empty() || !delta.nodes_updated.is_empty());

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    let table_node = model
        .nodes
        .iter()
        .find(|n| {
            n.kind == GraphKind::Table
                && n.table.is_some()
                && n.path.len() == 1
                && matches!(n.path.first(), Some(PathSeg::Key(k)) if k == "ApiList")
        })
        .expect("ApiList table not found");
    let table = table_node.table.as_ref().unwrap();
    assert!(table.rows.len() > 20);

    let row18_action = &table.rows[18][1];
    assert_eq!(row18_action.text, "DssQqltltKniu");
    assert_eq!(
        row18_action.path,
        vec![
            PathSeg::Key("ApiList".to_string()),
            PathSeg::Index(18),
            PathSeg::Key("Action".to_string()),
        ]
    );

    let row19_action = &table.rows[19][1];
    assert_eq!(row19_action.text, "AdxvXnovykFcxgxao");
    assert_eq!(
        row19_action.path,
        vec![
            PathSeg::Key("ApiList".to_string()),
            PathSeg::Index(19),
            PathSeg::Key("Action".to_string()),
        ]
    );
}

// ---------------------------------------------------------------------------
// Test: still finishes after state move like stream_view_finish
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_still_finishes_after_state_move_like_stream_view_finish() {
    const FIXTURE: &str =
        include_str!("../../../test/fixtures/json/graph-table-missing-row.1.json");

    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    let mut parser = StreamingParser::with_path_emission(true, true);
    let split_at = FIXTURE.len() / 2;
    parser.feed(&FIXTURE[..split_at]).unwrap();
    let _ = builder.flush().unwrap();
    parser.feed(&FIXTURE[split_at..]).unwrap();
    let _ = builder.flush().unwrap();

    // Move builder and parser (simulates state move)
    let mut moved_builder = builder;
    let mut moved_parser = parser;

    let events = moved_parser.finish().unwrap();
    for event in &events {
        moved_builder.on_event(event).unwrap();
    }

    let _ = moved_builder.finish_delta().unwrap();

    let model = moved_builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    let table_node = model
        .nodes
        .iter()
        .find(|n| {
            n.kind == GraphKind::Table
                && n.table.is_some()
                && n.path.len() == 1
                && matches!(n.path.first(), Some(PathSeg::Key(k)) if k == "ApiList")
        })
        .expect("ApiList table not found");
    let table = table_node.table.as_ref().unwrap();
    assert!(table.rows.len() > 20);

    let row18_action = &table.rows[18][1];
    assert_eq!(row18_action.text, "DssQqltltKniu");
}

// ---------------------------------------------------------------------------
// Test: refresh dedupes pending node and edge updates
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_refresh_dedupes_pending_node_and_edge_updates() {
    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "a".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$.a",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "b".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "1".to_string(),
            meta: meta_with_path_and_sem("$.a.b", treease_core::core::sem_type::SemType::Int),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();

    let _ = builder.flush().unwrap();

    // Get a reference to the child node (index 1) and mark it updated twice
    let child = builder.nodes()[1].clone();
    builder.mark_node_updated(&child).unwrap();
    builder.mark_node_updated(&child).unwrap();
    builder.update_edges_for_node(child.render_handle).unwrap();
    builder.update_edges_for_node(child.render_handle).unwrap();

    let delta = builder.flush().unwrap();
    assert_eq!(delta.nodes_updated.len(), 1);
    assert_eq!(delta.edges_added.len(), 1);
    assert_eq!(delta.edges_removed.len(), 1);
}

// ---------------------------------------------------------------------------
// Test: defers repeated sequence refresh until flush
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_defers_repeated_sequence_refresh_until_flush() {
    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    builder
        .on_event(&StreamingEvent::SeqStart(meta_with_path("$")))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$[0]",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "name".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "alice".to_string(),
            meta: meta_with_path_and_sem("$[0].name", treease_core::core::sem_type::SemType::Str),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();
    let _ = builder.flush().unwrap();

    // Add second row
    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$[1]",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "name".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "bob".to_string(),
            meta: meta_with_path_and_sem("$[1].name", treease_core::core::sem_type::SemType::Str),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();

    // Flush should apply deferred refresh and produce updated table node.
    // The Zig test also checks:
    //   builder.deferred_refresh_nodes.items.len == 1 before flush
    //   builder.deferred_refresh_nodes.items[0] == 0
    //   builder.deferred_refresh_nodes.items.len == 0 after flush
    // In Rust, `deferred_refresh_nodes` is a private field of the Builder
    // struct, so the count cannot be inspected directly. The assertions
    // below on the flush delta indirectly verify that the deferred refresh
    // was applied (the updated table node appears with 2 rows).
    let delta = builder.flush().unwrap();
    assert_eq!(delta.nodes_updated.len(), 1);
    let updated = &delta.nodes_updated[0];
    assert_eq!(updated.render_handle, 0);
    assert_eq!(updated.kind, GraphKind::Table);
    let table = updated.table.as_ref().unwrap();
    assert_eq!(table.rows.len(), 2);
    assert_eq!(table.rows[1][1].text, "bob");
    assert!(updated.box_args.width > 0);

    // Add a third row after the first deferred refresh drain. The same parent
    // render handle must be allowed to re-enter the deferred queue.
    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$[2]",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "name".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "cora".to_string(),
            meta: meta_with_path_and_sem("$[2].name", treease_core::core::sem_type::SemType::Str),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();

    let second_delta = builder.flush().unwrap();
    assert_eq!(second_delta.nodes_updated.len(), 1);
    let second_updated = &second_delta.nodes_updated[0];
    let second_table = second_updated.table.as_ref().unwrap();
    assert_eq!(second_table.rows.len(), 3);
    assert_eq!(second_table.rows[2][1].text, "cora");
}

// ---------------------------------------------------------------------------
// Test: renders no-header sequence as headerless table with visible child node
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_renders_no_header_sequence_as_headerless_table_with_visible_child_node() {
    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    builder
        .on_event(&StreamingEvent::SeqStart(meta_with_path("$")))
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "1".to_string(),
            meta: meta_with_path_and_sem("$[0]", treease_core::core::sem_type::SemType::Int),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$[1]",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "name".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "bob".to_string(),
            meta: meta_with_path_and_sem("$[1].name", treease_core::core::sem_type::SemType::Str),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();
    builder
        .on_event(&StreamingEvent::SeqEnd(Meta::default()))
        .unwrap();

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(model.edges.len(), 1);

    let table = model.nodes[0].table.as_ref().unwrap();
    assert_eq!(model.nodes[0].kind, GraphKind::Table);
    assert_eq!(model.nodes[1].kind, GraphKind::Object);
    assert_eq!(table.header_height, 0);
    assert_eq!(table.rows.len(), 2);
    assert_eq!(table.rows[0][0].text, "0");
    assert_eq!(table.rows[0][1].text, "1");
    assert_eq!(table.rows[1][0].text, "1");
    assert_eq!(table.rows[1][1].text, "{1}");
    assert_eq!(model.edges[0].from_row, 1);
    assert_eq!(model.nodes[1].path.len(), 1);
    assert!(matches!(model.nodes[1].path[0], PathSeg::Index(1)));
}

// ---------------------------------------------------------------------------
// Test: keeps open no-header sequence as headerless table during streaming
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_keeps_open_no_header_sequence_as_headerless_table_during_streaming() {
    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    builder
        .on_event(&StreamingEvent::SeqStart(meta_with_path("$")))
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "1".to_string(),
            meta: meta_with_path_and_sem("$[0]", treease_core::core::sem_type::SemType::Int),
        })
        .unwrap();

    let delta = builder.flush().unwrap();
    assert!(!delta.nodes_added.is_empty() || !delta.nodes_updated.is_empty());

    // Find the sequence node in added or updated
    let sequence_node = delta
        .nodes_added
        .iter()
        .chain(delta.nodes_updated.iter())
        .find(|n| n.path.is_empty() && n.kind == GraphKind::Table)
        .expect("sequence table node not found");

    let table = sequence_node.table.as_ref().unwrap();
    assert_eq!(sequence_node.kind, GraphKind::Table);
    assert_eq!(table.header_height, 0);
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0][0].text, "0");
    assert_eq!(table.rows[0][1].text, "1");
}

// ---------------------------------------------------------------------------
// Test: shows unknown count for open table parent during streaming
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_shows_unknown_count_for_open_table_parent_during_streaming() {
    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "items".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::SeqStart(meta_with_path_and_sem(
            "$.items",
            treease_core::core::sem_type::SemType::Seq,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$.items[0]",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "name".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "alice".to_string(),
            meta: meta_with_path_and_sem(
                "$.items[0].name",
                treease_core::core::sem_type::SemType::Str,
            ),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();

    let delta = builder.flush().unwrap();
    assert!(!delta.nodes_added.is_empty() || !delta.nodes_updated.is_empty());

    // Find the parent object node
    let parent = delta
        .nodes_added
        .iter()
        .chain(delta.nodes_updated.iter())
        .find(|n| n.path.is_empty() && n.kind == GraphKind::Object)
        .expect("parent object node not found");

    assert_eq!(parent.kind, GraphKind::Object);
    assert_eq!(parent.rows.len(), 1);
    assert_eq!(parent.rows[0].key.text, "items");
    assert_eq!(parent.rows[0].value.text, "[?]");
}

// ---------------------------------------------------------------------------
// Test: keeps header table inline without leaking first row child node
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_keeps_header_table_inline_without_leaking_first_row_child_node() {
    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);

    builder
        .on_event(&StreamingEvent::SeqStart(meta_with_path("$")))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$[0]",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "name".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "bob".to_string(),
            meta: meta_with_path_and_sem("$[0].name", treease_core::core::sem_type::SemType::Str),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "2".to_string(),
            meta: meta_with_path_and_sem("$[1]", treease_core::core::sem_type::SemType::Int),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::SeqEnd(Meta::default()))
        .unwrap();

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    assert_eq!(model.nodes.len(), 1);
    assert_eq!(model.edges.len(), 0);
    assert_eq!(model.nodes[0].kind, GraphKind::Table);
    let table = model.nodes[0].table.as_ref().unwrap();
    assert_eq!(table.rows.len(), 2);
    assert_eq!(table.rows[0][1].text, "bob");
    assert_eq!(table.rows[1][2].text, "2");
}

// ---------------------------------------------------------------------------
// Test: keeps edge row after empty container sibling
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_keeps_edge_row_after_empty_container_sibling() {
    let json = r#"{
  "DataType": [],
  "CommonErrorCode": [{"$ref": "InvalidParam"}]
}"#;

    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);
    let mut parser = StreamingParser::with_path_emission(true, true);
    parser.feed(json).unwrap();
    let events = parser.finish().unwrap();
    for event in &events {
        builder.on_event(event).unwrap();
    }

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(model.edges.len(), 1);

    let root = &model.nodes[0];
    assert_eq!(root.kind, GraphKind::Object);
    assert_eq!(root.rows.len(), 2);
    assert_eq!(root.rows[0].key.text, "DataType");
    assert_eq!(root.rows[1].key.text, "CommonErrorCode");
    assert_eq!(root.rows[0].value.text, "[]");
    assert_eq!(root.rows[1].value.text, "[1]");

    assert_eq!(model.edges[0].from_row, 1);
    assert_eq!(model.nodes[1].kind, GraphKind::Table);
    assert_eq!(model.nodes[1].path.len(), 1);
    assert!(matches!(
        &model.nodes[1].path[0],
        PathSeg::Key(k) if k == "CommonErrorCode"
    ));
}

// ---------------------------------------------------------------------------
// Test: keeps object and headerless edges while header tables stay inline
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_keeps_object_and_headerless_edges_while_header_tables_stay_inline() {
    let json = r#"{
  "obj": {"child": {"x": 1}},
  "items": [1, {"x": 2}],
  "rows": [{"name": "Ada", "child": {"x": 3}}]
}"#;

    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);
    let mut parser = StreamingParser::with_path_emission(true, true);
    parser.feed(json).unwrap();
    let events = parser.finish().unwrap();
    for event in &events {
        builder.on_event(event).unwrap();
    }

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);

    let obj_node = find_node_by_single_key_path(&model.nodes, "obj").expect("obj node not found");
    let obj_child =
        find_node_by_key_path(&model.nodes, &["obj", "child"]).expect("obj.child node not found");
    assert!(model.edges.iter().any(|edge| {
        edge.from_render_handle == obj_node.render_handle
            && edge.to_render_handle == obj_child.render_handle
            && edge.from_row == 0
    }));

    let items_node =
        find_node_by_single_key_path(&model.nodes, "items").expect("items table node not found");
    assert_eq!(items_node.kind, GraphKind::Table);
    let items_edge = model
        .edges
        .iter()
        .find(|edge| edge.from_render_handle == items_node.render_handle)
        .expect("headerless table should expose a child edge");
    let items_child = &model.nodes[items_edge.to_render_handle as usize];
    assert_eq!(items_edge.from_row, 1);
    assert_eq!(items_child.kind, GraphKind::Object);
    assert_eq!(items_child.path.len(), 2);
    assert!(matches!(&items_child.path[0], PathSeg::Key(key) if key == "items"));
    assert!(matches!(items_child.path[1], PathSeg::Index(1)));

    let rows_node =
        find_node_by_single_key_path(&model.nodes, "rows").expect("rows table node not found");
    assert_eq!(rows_node.kind, GraphKind::Table);
    let rows_table = rows_node
        .table
        .as_ref()
        .expect("rows table payload missing");
    assert_eq!(rows_table.rows.len(), 1);
    assert_eq!(rows_table.rows[0][2].text, "{1}");
    assert!(
        !model
            .edges
            .iter()
            .any(|edge| edge.from_render_handle == rows_node.render_handle),
        "header table rows should stay inline in the main graph"
    );
}

// ---------------------------------------------------------------------------
// Test: keeps one vGap below table sibling in finished model
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_keeps_one_vgap_below_table_sibling_in_finished_model() {
    let mut config = default_config();
    config.v_gap = 24;
    config.table_header_height = 10;
    config.table_row_height = 30;

    let json = r#"{
  "first": [{"name": "alice"}, {"name": "bob"}],
  "second": {"value": 1}
}"#;

    let v_gap = config.v_gap;
    let mut builder = GraphBuilderPreorder::new(config, GraphLanguage::Json);
    let mut parser = StreamingParser::with_path_emission(true, true);
    parser.feed(json).unwrap();
    let events = parser.finish().unwrap();
    for event in &events {
        builder.on_event(event).unwrap();
    }

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    let first = find_node_by_single_key_path(&model.nodes, "first").expect("first node not found");
    let second =
        find_node_by_single_key_path(&model.nodes, "second").expect("second node not found");

    assert_eq!(first.kind, GraphKind::Table);
    assert_eq!(first.y + first.height + v_gap, second.y);
}

#[test]
fn graph_builder_preorder_clamps_table_node_height_to_view_height() {
    let mut config = default_config();
    config.table_max_height = 60;
    let mut builder = GraphBuilderPreorder::new(config.clone(), GraphLanguage::Json);

    let json = r#"[
  {"name":"row-0"},
  {"name":"row-1"},
  {"name":"row-2"},
  {"name":"row-3"},
  {"name":"row-4"},
  {"name":"row-5"}
]"#;

    let mut parser = StreamingParser::with_path_emission(true, true);
    parser.feed(json).unwrap();
    let events = parser.finish().unwrap();
    for event in &events {
        builder.on_event(event).unwrap();
    }

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    let table_node = model
        .nodes
        .iter()
        .find(|n| n.kind == GraphKind::Table && n.table.is_some())
        .expect("table node not found");
    let table = table_node.table.as_ref().unwrap();

    assert!(table.total_height > table.view_height);
    assert_eq!(table.view_height, config.table_max_height);
    assert_eq!(
        table_node.height,
        table.view_height + config.node_border_width * 2,
        "table node height should use clipped view height rather than total content height"
    );
}

// ---------------------------------------------------------------------------
// Test: keeps one vGap below table sibling after streamed finish
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_keeps_one_vgap_below_table_sibling_after_streamed_finish() {
    let mut config = default_config();
    config.v_gap = 24;
    config.table_header_height = 10;
    config.table_row_height = 30;

    let v_gap = config.v_gap;
    let mut builder = GraphBuilderPreorder::new(config, GraphLanguage::Json);

    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "first".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::SeqStart(meta_with_path_and_sem(
            "$.first",
            treease_core::core::sem_type::SemType::Seq,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$.first[0]",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "name".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "alice".to_string(),
            meta: meta_with_path_and_sem(
                "$.first[0].name",
                treease_core::core::sem_type::SemType::Str,
            ),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();
    let _ = builder.flush().unwrap();

    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$.first[1]",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "name".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "bob".to_string(),
            meta: meta_with_path_and_sem(
                "$.first[1].name",
                treease_core::core::sem_type::SemType::Str,
            ),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();
    builder
        .on_event(&StreamingEvent::SeqEnd(Meta::default()))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "second".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapStart(meta_with_path_and_sem(
            "$.second",
            treease_core::core::sem_type::SemType::Map,
        )))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapKey {
            value: "value".to_string(),
            meta: Meta::default(),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::Scalar {
            value: "1".to_string(),
            meta: meta_with_path_and_sem(
                "$.second.value",
                treease_core::core::sem_type::SemType::Int,
            ),
        })
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();
    builder
        .on_event(&StreamingEvent::MapEnd(Meta::default()))
        .unwrap();

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    let first = find_node_by_single_key_path(&model.nodes, "first").expect("first node not found");
    let second =
        find_node_by_single_key_path(&model.nodes, "second").expect("second node not found");

    assert_eq!(first.kind, GraphKind::Table);
    assert_eq!(first.y + first.height + v_gap, second.y);
}

// ---------------------------------------------------------------------------
// Test: keeps headerless-table sibling below previous subtree
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_keeps_headerless_table_sibling_below_previous_subtree() {
    let json = r#"{
  "AE": {
    "Prices": ["3.49","7.49","10.99","14.49","18.49","22","25.49","29.49","33","36.49","40.49","44","47.49","51","55","58.99","62","66","69.95","73","77","80.99","84","88","91.99","95","99","102.99","105.99","109.99","113.99","117.99","120.99","124.99","128.99","131.99","135.99","139.99","142.99","146.99","150.99","153.99","157.99","161.99","164.99","168.99","172.99","175.99","179.99","183.99","200","219.99","239","259","274.99","294.99","309.99","329.99","349","369","404.99","439.99","459","479","509.99","550","589","619.99","639.99","659.99","699","739","769.99","809","849","879.99","919","1099.99","1289.99","1469.99","1649.99","1839.99","2199.99","2549.99","2949.99","3299.99","3649.99"]
  },
  "AF": {
    "Prices": ["0.99","1.99","2.99","3.99","4.99","5.99","6.99","7.99","8.99","9.99","10.99","11.99","12.99","13.99","14.99","15.99","16.99","17.99","18.99","19.99","20.99","21.99","22.99","23.99","24.99","25.99","26.99","27.99","28.99","29.99","30.99","31.99","32.99","33.99","34.99","35.99","36.99","37.99","38.99","39.99","40.99","41.99","42.99","43.99","44.99","45.99","46.99","47.99","48.99","49.99","54.99","59.99","64.99","69.99","74.99","79.99","84.99","89.99","94.99","99.99","109.99","119.99","124.99","129.99","139.99","149.99","159.99","169.99","174.99","179.99","189.99","199.99","209.99","219.99","229.99","239.99","249.99","299.99","349.99","399.99","449.99","499.99","599.99","699.99","799.99","899.99","999.99"]
  }
}"#;

    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);
    let mut parser = StreamingParser::with_path_emission(true, true);
    parser.feed(json).unwrap();
    let events = parser.finish().unwrap();
    for event in &events {
        builder.on_event(event).unwrap();
    }

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);
    let ae = find_node_by_single_key_path(&model.nodes, "AE").expect("AE node not found");
    let af = find_node_by_single_key_path(&model.nodes, "AF").expect("AF node not found");
    let ae_prices =
        find_node_by_key_path(&model.nodes, &["AE", "Prices"]).expect("AE.Prices node not found");

    let prices_table = ae_prices.table.as_ref().unwrap();
    assert_eq!(ae_prices.kind, GraphKind::Table);
    assert_eq!(prices_table.header_height, 0);
    assert!(prices_table.rows.len() > 50);
    assert_eq!(
        subtree_bottom(&model.nodes, &model.edges, ae.render_handle) + default_config().v_gap,
        af.y
    );
}

// ---------------------------------------------------------------------------
// Test: root-level array of mapping creates header table with indexed cell paths
// ---------------------------------------------------------------------------

#[test]
fn graph_builder_preorder_root_level_mapping_array_has_indexed_cell_paths() {
    let json = r#"[
  {"name": "Alice", "language": "en", "id": 1},
  {"name": "Bob", "language": "fr", "id": 2}
]"#;

    let mut builder = GraphBuilderPreorder::new(default_config(), GraphLanguage::Json);
    let mut parser = StreamingParser::with_path_emission(true, true);
    parser.feed(json).unwrap();
    let events = parser.finish().unwrap();
    for event in &events {
        builder.on_event(event).unwrap();
    }

    let model = builder.finish().unwrap();
    assert_incremental_layout_relations(&model);

    // Root-level array → single table node with empty path
    let table_node = model
        .nodes
        .iter()
        .find(|n| n.kind == GraphKind::Table && n.table.is_some() && n.path.is_empty())
        .expect("root-level table node with empty path not found");

    let table = table_node.table.as_ref().unwrap();
    assert_eq!(table.rows.len(), 2, "should have 2 rows");

    // Row 0
    assert_eq!(
        table.rows[0].len(),
        4,
        "row 0 should have 4 cells (index + 3 data columns)"
    );
    assert_eq!(table.rows[0][0].text, "0", "index cell");
    assert_eq!(table.rows[0][1].text, "Alice", "name column text");
    assert_eq!(
        table.rows[0][1].path,
        vec![PathSeg::Index(0), PathSeg::Key("name".to_string())],
        "cell path for row 0 name"
    );
    assert_eq!(
        table.rows[0][2].path,
        vec![PathSeg::Index(0), PathSeg::Key("language".to_string())],
        "cell path for row 0 language"
    );
    // Row 1 — column order: index(0), name(1), language(2), id(3)
    assert_eq!(
        table.rows[1][2].text, "fr",
        "language column text for row 1"
    );
    assert_eq!(table.rows[1][3].text, "2", "id column text for row 1");
    assert_eq!(
        table.rows[1][3].path,
        vec![PathSeg::Index(1), PathSeg::Key("id".to_string())],
        "cell path for row 1 id"
    );
}
