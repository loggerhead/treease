use treease_core::document::materialize::materialize;
use treease_core::document::protocol::{
    DocumentInputPlan, GraphDelta, GraphPathSeg, OutputPlan, ProjectionRequest, SnapshotId,
    SnapshotReadResult,
};
use treease_core::document::runtime::{reset_runtime_for_tests, store_snapshot_for_document};
use treease_core::document::snapshot::DocumentSnapshot;
use treease_core::wasm::init_wasm;
use treease_core::wasm_types::SemType;

fn graph_delta_row_value_sem_type(
    delta: &GraphDelta,
    node_path: &[&str],
    row_key: &str,
) -> Option<u32> {
    let node = delta
        .nodes_added
        .iter()
        .find(|node| graph_path_matches(&node.path, node_path))?;
    node.rows.iter().find_map(|row| {
        let key = row.cells.first()?;
        let value = row.cells.get(1)?;
        (key.value == row_key).then_some(value.sem_type)
    })
}

fn graph_path_matches(path: &[GraphPathSeg], expected: &[&str]) -> bool {
    path.len() == expected.len()
        && path
            .iter()
            .zip(expected.iter())
            .all(|(segment, key)| segment.tag == 0 && segment.key == *key)
}

fn graph_delta_contains_row_key(delta: &GraphDelta, row_key: &str) -> bool {
    delta.nodes_added.iter().any(|node| {
        node.rows
            .iter()
            .any(|row| row.cells.first().is_some_and(|cell| cell.value == row_key))
    })
}

fn graph_delta_contains_node_path(delta: &GraphDelta, expected: &[&str]) -> bool {
    delta
        .nodes_added
        .iter()
        .any(|node| graph_path_matches(&node.path, expected))
}

fn assert_graph_json_path_keys_are_strings(value: &serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                assert_graph_json_path_keys_are_strings(item);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(path) = map.get("path").and_then(serde_json::Value::as_array) {
                for segment in path {
                    let key = segment
                        .get("key")
                        .expect("graph path segment should include key");
                    assert!(
                        key.is_string(),
                        "graph path key must serialize as string: {segment:?}"
                    );
                }
            }
            for entry in map.values() {
                assert_graph_json_path_keys_are_strings(entry);
            }
        }
        _ => {}
    }
}

fn store_analysis_snapshot(document_key: &str, snapshot_id: u64, language: &str, source: &str) {
    let materialized = materialize(
        &DocumentInputPlan::SourceText,
        document_key,
        language,
        source,
        false,
        &OutputPlan {
            analysis: true,
            graph: true,
        },
        &[],
        None,
    );
    let mut snapshot = DocumentSnapshot::with_analysis(document_key, materialized.analysis);
    snapshot.snapshot_id = SnapshotId(snapshot_id);
    store_snapshot_for_document(document_key, snapshot, true).expect("snapshot should be stored");
}

#[test]
fn hover_subgraph_projection_builds_ready_subgraph_for_nested_path() {
    init_wasm();
    reset_runtime_for_tests();
    store_analysis_snapshot(
        "doc-hover-nested",
        77,
        "json",
        r#"{"users":[{"name":"Ada"}],"count":1}"#,
    );

    let projection =
        treease_core::document::projection::build_hover_subgraph_projection(&ProjectionRequest {
            snapshot_id: SnapshotId(77),
            path: "$.users[0]".into(),
        })
        .expect("hover projection should build");

    let SnapshotReadResult::Ready { data } = projection else {
        panic!("hover projection should be ready");
    };
    assert!(data.clear);
    let delta = data
        .graph_data
        .as_ref()
        .expect("hover projection should include graph data");
    assert_eq!(
        graph_delta_row_value_sem_type(delta, &[], "name"),
        Some(SemType::Str as u32)
    );
    assert_eq!(graph_delta_row_value_sem_type(delta, &[], "count"), None);
}

#[test]
fn hover_subgraph_projection_builds_ready_subgraph_for_header_table_structured_cell() {
    init_wasm();
    reset_runtime_for_tests();
    store_analysis_snapshot(
        "doc-hover-header-table-cell",
        89,
        "json",
        r#"{"rows":[{"name":"Ada","child":{"x":1}}]}"#,
    );

    let projection =
        treease_core::document::projection::build_hover_subgraph_projection(&ProjectionRequest {
            snapshot_id: SnapshotId(89),
            path: "$.rows[0].child".into(),
        })
        .expect("header-table hover projection should build");

    let SnapshotReadResult::Ready { data } = projection else {
        panic!("header-table hover projection should be ready");
    };
    assert!(data.clear);
    let delta = data
        .graph_data
        .as_ref()
        .expect("hover projection should include graph data");
    assert_eq!(
        graph_delta_row_value_sem_type(delta, &[], "x"),
        Some(SemType::Int as u32)
    );
}

#[test]
fn hover_subgraph_projection_for_header_table_nested_object_stays_inside_requested_subtree() {
    init_wasm();
    reset_runtime_for_tests();
    store_analysis_snapshot(
        "doc-hover-header-table-nested-object",
        90,
        "json",
        include_str!("../../../test/fixtures/json/graph-table-missing-row.1.json"),
    );

    let projection =
        treease_core::document::projection::build_hover_subgraph_projection(&ProjectionRequest {
            snapshot_id: SnapshotId(90),
            path: "$.ApiList[0].AccountLevelTotalLimitConf".into(),
        })
        .expect("header-table nested object hover projection should build");

    let SnapshotReadResult::Ready { data } = projection else {
        panic!("hover projection should be ready");
    };
    let delta = data
        .graph_data
        .as_ref()
        .expect("hover projection should include graph data");

    assert!(graph_delta_contains_row_key(delta, "StrategicAccountLimit"));
    assert!(!graph_delta_contains_row_key(delta, "TemplateVersion"));
    assert!(!graph_delta_contains_node_path(delta, &["ApiList"]));
}

#[test]
fn hover_subgraph_projection_supports_quoted_key_segments() {
    init_wasm();
    reset_runtime_for_tests();
    store_analysis_snapshot(
        "doc-hover-quoted-key",
        88,
        "json",
        r#"{"user profile":{"display name":"Ada"}}"#,
    );

    let projection =
        treease_core::document::projection::build_hover_subgraph_projection(&ProjectionRequest {
            snapshot_id: SnapshotId(88),
            path: r#"$["user profile"]"#.into(),
        })
        .expect("quoted-key hover projection should build");

    let SnapshotReadResult::Ready { data } = projection else {
        panic!("quoted-key hover projection should be ready");
    };
    let delta = data
        .graph_data
        .as_ref()
        .expect("hover projection should include graph data");
    assert_eq!(
        graph_delta_row_value_sem_type(delta, &[], "display name"),
        Some(SemType::Str as u32),
    );
    let json = serde_json::to_value(data).expect("hover projection should serialize");
    assert_graph_json_path_keys_are_strings(&json);
}

#[test]
fn hover_subgraph_projection_missing_snapshot_is_snapshot_not_ready() {
    init_wasm();
    reset_runtime_for_tests();

    let projection =
        treease_core::document::projection::build_hover_subgraph_projection(&ProjectionRequest {
            snapshot_id: SnapshotId(99999),
            path: "$".into(),
        })
        .expect("missing snapshot should not be an error");

    assert!(matches!(projection, SnapshotReadResult::SnapshotNotReady));
}
#[test]
fn hover_subgraph_projection_without_analysis_is_error() {
    init_wasm();
    reset_runtime_for_tests();
    store_snapshot_for_document(
        "doc-hover-no-analysis",
        DocumentSnapshot {
            snapshot_id: SnapshotId(91),
            document_key: "doc-hover-no-analysis".into(),
            analysis: None,
            graph: None,
            incremental: None,
        },
        true,
    )
    .expect("snapshot should be stored");

    let error =
        treease_core::document::projection::build_hover_subgraph_projection(&ProjectionRequest {
            snapshot_id: SnapshotId(91),
            path: "$".into(),
        })
        .expect_err("snapshot without analysis should fail");

    assert_eq!(error, "no analysis");
}
