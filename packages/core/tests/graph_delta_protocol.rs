use treease_core::document::job::{advance_job, start_job};
use treease_core::document::materialize;
use treease_core::document::metrics::DocumentEngineMetrics;
use treease_core::document::protocol::{
    AdvanceInput, DocumentEvent, DocumentInputPlan, DocumentJobKind, DocumentJobSettings,
    DocumentJobSpec, OutputPlan,
};
/// Tracer — verifies GraphDelta serialization round-trip and graph_api conversion.
use treease_core::document::protocol::{
    GraphBezierArgsData, GraphBoxArgs, GraphCellData, GraphDelta, GraphEdgeData, GraphNodeData,
    GraphPathSeg, GraphRowData, GraphTableData, GraphTextArgs, ProjectionDelta,
};
use treease_core::document::runtime::DocumentRuntime;
use treease_core::wasm_types::SemType;
const GRAPH_UI_FIXTURE: &str = r#"{
  "object": { "arr0": [1, 2], "obj0": { "x": 1 } },
  "table_without_header": ["a", "b", "c"],
  "table_with_header": [
    { "h1": 11, "h2": 12, "h3": 13 },
    { "h1": 21, "h2": 22, "h3": 23 }
  ],
  "preview": "hello"
}"#;
const ONE_MB_MIN_JSON_FIXTURE: &str = include_str!("../../../test/fixtures/json/1MB-min.1.json");
const STREAM_CHUNK_SIZE: usize = 64 * 1024;

#[test]
fn graph_delta_round_trip_json() {
    let delta = GraphDelta {
        nodes_added: vec![GraphNodeData {
            render_handle: 1,
            kind: 0, // Scalar
            path: vec![GraphPathSeg {
                tag: 0, // Key
                key: "profile".into(),
                index: 0,
            }],
            depth: 1,
            box_args: GraphBoxArgs {
                x: 10,
                y: 20,
                width: 100,
                height: 22,
                corner_radius: 4,
            },
            meta: None,
            rows: vec![],
            table: None,
        }],
        nodes_updated: vec![],
        nodes_removed: vec![],
        edges_added: vec![],
        edges_removed: vec![],
        table_patches: vec![],
        layout_patches: vec![],
    };

    let json = serde_json::to_string(&delta).expect("serialize GraphDelta");

    // Verify JSON contains expected structure
    assert!(json.contains("nodesAdded"));
    assert!(json.contains("\"renderHandle\":1"));
    assert!(json.contains("\"kind\":0"));
    assert!(json.contains("\"key\":\"profile\""));
    assert!(json.contains("\"x\":10"));
    assert!(json.contains("\"y\":20"));
    assert!(json.contains("\"width\":100"));
    assert!(json.contains("\"height\":22"));
    assert!(!json.contains("sourceRow"));
    assert!(!json.contains("sourceColumn"));

    // Round-trip deserialization
    let parsed: GraphDelta = serde_json::from_str(&json).expect("deserialize GraphDelta");
    assert_eq!(parsed.nodes_added.len(), 1);
    assert_eq!(parsed.nodes_added[0].render_handle, 1);
    assert_eq!(parsed.nodes_added[0].path[0].key, "profile");
}

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
fn graph_delta_node_any<'a>(
    delta: &'a GraphDelta,
    node_path: &[&str],
) -> Option<&'a GraphNodeData> {
    delta
        .nodes_added
        .iter()
        .chain(delta.nodes_updated.iter())
        .find(|node| graph_path_matches(&node.path, node_path))
}

fn graph_node_json(delta: &GraphDelta, node_path: &[&str]) -> Option<serde_json::Value> {
    let node = graph_delta_node_any(delta, node_path)?;
    serde_json::to_value(node).ok()
}

fn graph_delta_row_value_cell_json(
    delta: &GraphDelta,
    node_path: &[&str],
    row_key: &str,
) -> Option<serde_json::Value> {
    let node = graph_node_json(delta, node_path)?;
    let rows = node.get("rows")?.as_array()?;
    rows.iter().find_map(|row| {
        let cells = row.get("cells")?.as_array()?;
        let key = cells.first()?.get("value")?.as_str()?;
        (key == row_key).then(|| cells.get(1).cloned()).flatten()
    })
}

fn graph_delta_row_cell_string(
    delta: &GraphDelta,
    node_path: &[&str],
    row_key: &str,
    field: &str,
) -> Option<String> {
    graph_delta_row_value_cell_json(delta, node_path, row_key)?
        .get(field)?
        .as_str()
        .map(str::to_owned)
}

fn graph_delta_row_text(delta: &GraphDelta, node_path: &[&str], row_key: &str) -> Option<String> {
    graph_delta_row_cell_string(delta, node_path, row_key, "text")
}

fn graph_delta_row_value(delta: &GraphDelta, node_path: &[&str], row_key: &str) -> Option<String> {
    graph_delta_row_cell_string(delta, node_path, row_key, "value")
}

fn graph_delta_row_text_args_text(
    delta: &GraphDelta,
    node_path: &[&str],
    row_key: &str,
) -> Option<String> {
    graph_delta_row_value_cell_json(delta, node_path, row_key)?
        .get("textArgs")?
        .get("text")?
        .as_str()
        .map(str::to_owned)
}

fn graph_delta_table_column_texts_json(
    delta: &GraphDelta,
    node_path: &[&str],
) -> Option<Vec<String>> {
    let node = graph_node_json(delta, node_path)?;
    let columns = node.get("table")?.get("columns")?.as_array()?;
    columns
        .iter()
        .map(|column| column.get("text")?.as_str().map(|text| text.to_string()))
        .collect()
}

fn json_graph_path_matches(path: &serde_json::Value, expected: &[&str]) -> bool {
    let Some(segments) = path.as_array() else {
        return false;
    };
    segments.len() == expected.len()
        && segments.iter().zip(expected.iter()).all(|(segment, key)| {
            segment.get("tag").and_then(serde_json::Value::as_i64) == Some(0)
                && segment.get("key").and_then(serde_json::Value::as_str) == Some(*key)
        })
}

fn graph_delta_edge_json(
    delta: &GraphDelta,
    from_path: &[&str],
    from_row: i64,
    to_path: &[&str],
) -> Option<serde_json::Value> {
    let delta_json = serde_json::to_value(delta).ok()?;
    let edges = delta_json.get("edgesAdded")?.as_array()?;
    edges
        .iter()
        .find(|edge| {
            edge.get("fromRow").and_then(serde_json::Value::as_i64) == Some(from_row)
                && edge
                    .get("fromPath")
                    .is_some_and(|path| json_graph_path_matches(path, from_path))
                && edge
                    .get("toPath")
                    .is_some_and(|path| json_graph_path_matches(path, to_path))
        })
        .cloned()
}

/// Assert edge has valid camelCase bezierArgs in protocol delta.
/// 验证 from_y 在父节点范围内，to_y 在子节点范围内（对应 docs/core/index.md
/// "起点 y 为父节点对应 value 单元格的中点；终点 y 为子节点首个 row 的中点"）。
fn assert_graph_delta_edge_with_camel_bezier(
    delta: &GraphDelta,
    from_path: &[&str],
    from_row: i64,
    to_path: &[&str],
) {
    let Some(edge) = graph_delta_edge_json(delta, from_path, from_row, to_path) else {
        panic!("expected graph edge from {from_path:?} row {from_row} to {to_path:?}");
    };
    let bezier = edge
        .get("bezierArgs")
        .and_then(serde_json::Value::as_object)
        .expect("edge should expose nested camelCase bezierArgs");
    for field in ["fromX", "fromY", "c1x", "c1y", "c2x", "c2y", "toX", "toY"] {
        assert!(
            bezier
                .get(field)
                .and_then(serde_json::Value::as_i64)
                .is_some(),
            "bezierArgs.{field} should be numeric: {bezier:?}"
        );
    }
    let from_x = bezier
        .get("fromX")
        .and_then(serde_json::Value::as_i64)
        .unwrap();
    let to_x = bezier
        .get("toX")
        .and_then(serde_json::Value::as_i64)
        .unwrap();
    assert!(to_x > from_x, "edge should point left-to-right: {bezier:?}");

    // 验证 from_y 在父节点 box_args 范围内（对应 "起点 y 为父节点…的中点"）
    let from_y = bezier
        .get("fromY")
        .and_then(serde_json::Value::as_i64)
        .unwrap();
    let to_y = bezier
        .get("toY")
        .and_then(serde_json::Value::as_i64)
        .unwrap();
    if let Some(parent_json) = graph_node_json(delta, from_path) {
        if let Some(box_args) = parent_json.get("boxArgs") {
            let p_y = box_args
                .get("y")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let p_h = box_args
                .get("height")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            assert!(
                from_y >= p_y && from_y <= p_y + p_h,
                "from_y {} should be within parent box y={}..{}: {bezier:?}",
                from_y,
                p_y,
                p_y + p_h,
            );
        }
    }
    if let Some(child_json) = graph_node_json(delta, to_path) {
        if let Some(box_args) = child_json.get("boxArgs") {
            let c_y = box_args
                .get("y")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let c_h = box_args
                .get("height")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            assert!(
                to_y >= c_y && to_y <= c_y + c_h,
                "to_y {} should be within child box y={}..{}: {bezier:?}",
                to_y,
                c_y,
                c_y + c_h,
            );
        }
    }
}

fn assert_graph_ui_contract(delta: &GraphDelta) {
    for (row_key, expected) in [
        ("object", "{2}"),
        ("table_without_header", "[3]"),
        ("table_with_header", "[2]"),
    ] {
        assert_eq!(
            graph_delta_row_text(delta, &[], row_key).as_deref(),
            Some(expected),
            "{row_key} display text should be the structured summary"
        );
        assert_eq!(
            graph_delta_row_value(delta, &[], row_key).as_deref(),
            Some(expected),
            "{row_key} value should preserve the structured summary for Graph UI consumers"
        );
        assert_eq!(
            graph_delta_row_text_args_text(delta, &[], row_key).as_deref(),
            Some(expected),
            "{row_key} textArgs.text should drive the rendered cell text"
        );
    }
    assert_eq!(
        graph_delta_table_column_texts_json(delta, &["table_with_header"]),
        Some(vec![
            "".to_string(),
            "h1".to_string(),
            "h2".to_string(),
            "h3".to_string(),
        ])
    );

    assert_graph_delta_edge_with_camel_bezier(delta, &[], 0, &["object"]);
    assert_graph_delta_edge_with_camel_bezier(delta, &[], 1, &["table_without_header"]);
    assert_graph_delta_edge_with_camel_bezier(delta, &[], 2, &["table_with_header"]);
    assert_graph_delta_edge_with_camel_bezier(delta, &["object"], 0, &["object", "arr0"]);
    assert_graph_delta_edge_with_camel_bezier(delta, &["object"], 1, &["object", "obj0"]);

    let delta_json = serde_json::to_value(delta).expect("graph delta should serialize");
    let edges = delta_json
        .get("edgesAdded")
        .and_then(serde_json::Value::as_array)
        .expect("graph delta should expose edgesAdded");
    assert!(
        !edges.iter().any(|edge| {
            edge.get("fromPath")
                .is_some_and(|path| json_graph_path_matches(path, &["table_with_header"]))
        }),
        "header tables should not expose main-graph edges to structured cell children"
    );
}

fn graph_protocol_path_text(path: &[GraphPathSeg]) -> String {
    let text = path
        .iter()
        .map(|segment| {
            if segment.tag == 0 && !segment.key.is_empty() {
                segment.key.clone()
            } else {
                format!("[{}]", segment.index)
            }
        })
        .collect::<Vec<_>>()
        .join(".");
    if text.is_empty() {
        "$".to_string()
    } else {
        text
    }
}

fn graph_box_key(box_args: &GraphBoxArgs) -> (i32, i32, i32, i32) {
    (box_args.x, box_args.y, box_args.width, box_args.height)
}

fn collect_graph_layout_violations(nodes: &[GraphNodeData]) -> Vec<String> {
    let mut violations = Vec::new();
    let mut node_box_by_bounds: std::collections::HashMap<(i32, i32, i32, i32), &GraphNodeData> =
        std::collections::HashMap::new();

    for node in nodes {
        if node.box_args.width <= 0 || node.box_args.height <= 0 {
            continue;
        }
        let key = graph_box_key(&node.box_args);
        if let Some(first) = node_box_by_bounds.get(&key) {
            if first.render_handle != node.render_handle {
                violations.push(format!(
                    "node-overlap: {}#{}, {}#{} share box {:?}",
                    graph_protocol_path_text(&first.path),
                    first.render_handle,
                    graph_protocol_path_text(&node.path),
                    node.render_handle,
                    key,
                ));
            }
        } else {
            node_box_by_bounds.insert(key, node);
        }
    }

    for node in nodes {
        let node_path = graph_protocol_path_text(&node.path);
        let Some(table) = &node.table else {
            continue;
        };

        for (row_index, pair) in table.rows.windows(2).enumerate() {
            let previous = &pair[0].box_args;
            let current = &pair[1].box_args;
            if current.y < previous.y + previous.height {
                violations.push(format!(
                    "table-row-overlap: {node_path} row {} {:?} overlaps row {} {:?}",
                    row_index,
                    graph_box_key(previous),
                    row_index + 1,
                    graph_box_key(current),
                ));
            }
        }

        for (column_index, pair) in table.columns.windows(2).enumerate() {
            let previous = &pair[0].box_args;
            let current = &pair[1].box_args;
            if current.x < previous.x + previous.width {
                violations.push(format!(
                    "table-column-overlap: {node_path} column {} {:?} overlaps column {} {:?}",
                    column_index,
                    graph_box_key(previous),
                    column_index + 1,
                    graph_box_key(current),
                ));
            }
        }

        for (row_index, row) in table.rows.iter().enumerate() {
            for (column_index, pair) in row.cells.windows(2).enumerate() {
                let previous = &pair[0].box_args;
                let current = &pair[1].box_args;
                if current.x < previous.x + previous.width {
                    violations.push(format!(
                        "table-cell-overlap: {node_path} row {row_index} cell {} {:?} overlaps cell {} {:?}",
                        column_index,
                        graph_box_key(previous),
                        column_index + 1,
                        graph_box_key(current),
                    ));
                }
            }
            for (column_index, cell) in row.cells.iter().enumerate() {
                if cell.box_args.width > row.box_args.width
                    || cell.box_args.height > row.box_args.height
                {
                    violations.push(format!(
                        "table-cell-exceeds-row: {node_path} row {row_index} cell {column_index} {:?} exceeds row {:?}",
                        graph_box_key(&cell.box_args),
                        graph_box_key(&row.box_args),
                    ));
                }
            }
        }
    }

    violations
}

fn split_utf8_chunks(text: &str, chunk_size: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = (start + chunk_size).min(text.len());
        while end > start && !text.is_char_boundary(end) {
            end -= 1;
        }
        chunks.push(&text[start..end]);
        start = end;
    }
    chunks
}

#[derive(Default)]
struct GraphProjectionState {
    nodes: std::collections::HashMap<u32, GraphNodeData>,
}

impl GraphProjectionState {
    fn apply(&mut self, clear: bool, delta: &GraphDelta) {
        if clear {
            self.nodes.clear();
        }
        for render_handle in &delta.nodes_removed {
            self.nodes.remove(render_handle);
        }
        for node in delta.nodes_added.iter().chain(delta.nodes_updated.iter()) {
            self.nodes.insert(node.render_handle, node.clone());
        }
    }

    fn nodes(&self) -> Vec<GraphNodeData> {
        self.nodes.values().cloned().collect()
    }
}

fn assert_projection_state_has_valid_layout(state: &GraphProjectionState, label: &str) {
    let nodes = state.nodes();
    let violations = collect_graph_layout_violations(&nodes);
    assert!(
        violations.is_empty(),
        "{label} graph layout violations:\n{}",
        violations
            .into_iter()
            .take(10)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn graph_path_matches(path: &[GraphPathSeg], expected: &[&str]) -> bool {
    path.len() == expected.len()
        && path
            .iter()
            .zip(expected.iter())
            .all(|(segment, key)| segment.tag == 0 && segment.key == *key)
}

#[test]
fn streaming_graph_delta_preserves_collection_value_sem_types() {
    let mut runtime = DocumentRuntime::default();
    let mut metrics = DocumentEngineMetrics::default();
    let handle = start_job(
        &mut runtime,
        &mut metrics,
        DocumentJobSpec {
            kind: DocumentJobKind::AnalyzeSource,
            document_key: "sem-type-regression".into(),
            language: "json".into(),
            input: DocumentInputPlan::SourceText,
            settings: DocumentJobSettings::default(),
            output: OutputPlan {
                analysis: true,
                graph: true,
            },
            base_snapshot_id: None,
            edits: vec![],
        },
    );
    let batch = advance_job(
        &mut runtime,
        &mut metrics,
        handle,
        AdvanceInput::TextChunk(
            r#"{"profile":{"name":"Alice","tags":["owner"]},"count":1}"#.into(),
        ),
    );
    let delta = batch
        .events
        .iter()
        .find_map(|event| match event {
            DocumentEvent::ProjectionDelta {
                graph_data: Some(delta),
                ..
            } => Some(delta),
            _ => None,
        })
        .expect("first JSON chunk should emit graph data");

    assert_eq!(
        graph_delta_row_value_sem_type(delta, &[], "profile"),
        Some(SemType::Map as u32),
    );
    assert_eq!(
        graph_delta_row_value_sem_type(delta, &["profile"], "tags"),
        Some(SemType::Seq as u32),
    );
    assert_eq!(
        graph_delta_row_value_sem_type(delta, &["profile"], "name"),
        Some(SemType::Str as u32),
    );
    assert_eq!(
        graph_delta_row_value_sem_type(delta, &[], "count"),
        Some(SemType::Int as u32),
    );
}
#[test]
fn streaming_graph_delta_exposes_display_text_and_bezier_args_for_graph_ui() {
    let mut runtime = DocumentRuntime::default();
    let mut metrics = DocumentEngineMetrics::default();
    let handle = start_job(
        &mut runtime,
        &mut metrics,
        DocumentJobSpec {
            kind: DocumentJobKind::AnalyzeSource,
            document_key: "graph-ui-streaming".into(),
            language: "json".into(),
            input: DocumentInputPlan::SourceText,
            settings: DocumentJobSettings::default(),
            output: OutputPlan {
                analysis: true,
                graph: true,
            },
            base_snapshot_id: None,
            edits: vec![],
        },
    );
    let batch = advance_job(
        &mut runtime,
        &mut metrics,
        handle,
        AdvanceInput::TextChunk(GRAPH_UI_FIXTURE.into()),
    );
    let delta = batch
        .events
        .iter()
        .find_map(|event| match event {
            DocumentEvent::ProjectionDelta {
                graph_data: Some(delta),
                ..
            } => Some(delta),
            _ => None,
        })
        .expect("streaming JSON chunk should emit graph data");

    assert_graph_ui_contract(delta);
}

#[test]
fn streaming_one_mb_json_fixture_keeps_graph_layout_valid() {
    let mut runtime = DocumentRuntime::default();
    let mut metrics = DocumentEngineMetrics::default();
    let handle = start_job(
        &mut runtime,
        &mut metrics,
        DocumentJobSpec {
            kind: DocumentJobKind::AnalyzeSource,
            document_key: "one-mb-streaming-layout".into(),
            language: "json".into(),
            input: DocumentInputPlan::SourceText,
            settings: DocumentJobSettings::default(),
            output: OutputPlan {
                analysis: true,
                graph: true,
            },
            base_snapshot_id: None,
            edits: vec![],
        },
    );

    let mut state = GraphProjectionState::default();
    for (chunk_index, chunk) in split_utf8_chunks(ONE_MB_MIN_JSON_FIXTURE, STREAM_CHUNK_SIZE)
        .into_iter()
        .enumerate()
    {
        let batch = advance_job(
            &mut runtime,
            &mut metrics,
            handle,
            AdvanceInput::TextChunk(chunk.into()),
        );
        for event in &batch.events {
            if let DocumentEvent::ProjectionDelta {
                clear,
                graph_data: Some(delta),
                ..
            } = event
            {
                state.apply(*clear, delta);
                assert_projection_state_has_valid_layout(
                    &state,
                    &format!("stream chunk {chunk_index}"),
                );
            }
        }
    }

    let close_batch = advance_job(&mut runtime, &mut metrics, handle, AdvanceInput::Close);
    for event in &close_batch.events {
        match event {
            DocumentEvent::ProjectionDelta {
                clear,
                graph_data: Some(delta),
                ..
            } => {
                state.apply(*clear, delta);
                assert_projection_state_has_valid_layout(&state, "close projection delta");
            }
            DocumentEvent::SnapshotReady {
                main_graph:
                    Some(ProjectionDelta {
                        clear,
                        graph_data: Some(delta),
                        ..
                    }),
                ..
            } => {
                state.apply(*clear, delta);
                assert_projection_state_has_valid_layout(&state, "snapshot ready main graph");
            }
            _ => {}
        }
    }

    // Verify the final state has correct cell paths for the first two rows
    let nodes = state.nodes();
    let table_node = nodes
        .iter()
        .find(|n| n.kind == 2 && n.table.is_some() && n.path.is_empty())
        .expect("root-level table node not found");
    let table = table_node.table.as_ref().unwrap();
    assert!(table.rows.len() > 2, "should have at least 2 rows");

    let row0 = &table.rows[0];
    assert!(
        row0.cells.len() >= 3,
        "row 0 should have at least 3 cells (index + name + language)"
    );
    let name_cell = &row0.cells[1];
    assert_eq!(
        name_cell.path.len(),
        2,
        "cell [0].name path should have 2 segments"
    );
    assert_eq!(name_cell.path[0].key, "");
    assert_eq!(name_cell.path[0].tag, 1, "first segment should be Index");
    assert_eq!(name_cell.path[0].index, 0);
    assert_eq!(name_cell.path[1].key, "name");
    assert_eq!(name_cell.path[1].tag, 0);

    let row1 = &table.rows[1];
    let id_cell = row1
        .cells
        .iter()
        .find(|c| c.path.len() >= 2 && c.path.last().map_or(false, |s| s.key == "id"));
    assert!(id_cell.is_some(), "row 1 should have id cell");
    let id_path = &id_cell.unwrap().path;
    assert_eq!(id_path[0].index, 1, "row index should be 1");
    assert_eq!(id_path[1].key, "id");
}
#[test]
fn materialize_graph_projection_exposes_display_text_and_bezier_args_for_graph_ui() {
    let result = materialize(
        &DocumentInputPlan::SourceText,
        "graph-ui-materialize",
        "json",
        GRAPH_UI_FIXTURE,
        false,
        &OutputPlan {
            analysis: true,
            graph: true,
        },
        &[],
        None,
    );

    let delta = result
        .graph
        .as_ref()
        .and_then(|graph| graph.graph_data.as_ref())
        .expect("materialize should produce graph data");

    assert_graph_ui_contract(delta);
}

#[test]
fn materialize_graph_projection_preserves_collection_value_sem_types() {
    let result = materialize(
        &DocumentInputPlan::SourceText,
        "materialize-sem-type",
        "json",
        r#"{"profile":{"name":"Alice","tags":["owner"]},"count":1}"#,
        false,
        &OutputPlan {
            analysis: true,
            graph: true,
        },
        &[],
        None,
    );

    let delta = result
        .graph
        .as_ref()
        .and_then(|graph| graph.graph_data.as_ref())
        .expect("materialize should produce graph data");

    assert_eq!(
        graph_delta_row_value_sem_type(delta, &[], "profile"),
        Some(SemType::Map as u32),
    );
    assert_eq!(
        graph_delta_row_value_sem_type(delta, &["profile"], "tags"),
        Some(SemType::Seq as u32),
    );
    assert_eq!(
        graph_delta_row_value_sem_type(delta, &["profile"], "name"),
        Some(SemType::Str as u32),
    );
    assert_eq!(
        graph_delta_row_value_sem_type(delta, &[], "count"),
        Some(SemType::Int as u32),
    );
}

#[test]
fn projection_delta_with_graph_data_serializes() {
    let delta = ProjectionDelta {
        clear: false,
        patch_seq: 0,
        base_graph_version: 0,
        graph_version: 0,
        graph_data: Some(GraphDelta {
            nodes_added: vec![GraphNodeData {
                render_handle: 42,
                kind: 1, // Object
                path: vec![],
                depth: 0,
                box_args: GraphBoxArgs::default(),
                meta: None,
                rows: vec![],
                table: None,
            }],
            nodes_updated: vec![],
            nodes_removed: vec![],
            edges_added: vec![],
            edges_removed: vec![],
            table_patches: vec![],
            layout_patches: vec![],
        }),
    };

    let json = serde_json::to_string(&delta).expect("serialize ProjectionDelta");
    assert!(json.contains("graphData"));
    assert!(json.contains("\"renderHandle\":42"));
    assert!(json.contains("\"clear\":false"));
}

#[test]
fn graph_delta_with_table_round_trips() {
    let cell = GraphCellData {
        sem_type: 2, // Str
        is_missing: false,
        path: vec![],
        text: "Alice".into(),
        value: "Alice".into(),
        format_text: "Alice".into(),
        box_args: GraphBoxArgs::default(),
        text_args: GraphTextArgs::default(),
    };

    let delta = GraphDelta {
        nodes_added: vec![GraphNodeData {
            render_handle: 10,
            kind: 2, // Table
            path: vec![GraphPathSeg {
                tag: 0,
                key: "users".into(),
                index: 0,
            }],
            depth: 1,
            box_args: GraphBoxArgs::default(),
            meta: None,
            rows: vec![GraphRowData {
                index: 0,
                box_args: GraphBoxArgs::default(),
                cell_box_args: GraphBoxArgs::default(),
                cells: vec![cell.clone()],
            }],
            table: Some(GraphTableData {
                columns: vec![cell],
                rows: vec![],
                header_height: 22,
                total_height: 100,
                view_height: 100,
                row_height: 22,
            }),
        }],
        nodes_updated: vec![],
        nodes_removed: vec![],
        edges_added: vec![],
        edges_removed: vec![],
        table_patches: vec![],
        layout_patches: vec![],
    };

    let json = serde_json::to_string(&delta).expect("serialize");
    let parsed: GraphDelta = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.nodes_added[0].rows[0].cells[0].value, "Alice");
    assert_eq!(
        parsed.nodes_added[0].table.as_ref().unwrap().columns[0].value,
        "Alice"
    );
}

#[test]
fn graph_edge_serializes_beziers() {
    let edge = GraphEdgeData {
        from_render_handle: 1,
        from_kind: 0,
        from_path: vec![],
        from_row: 0,
        to_render_handle: 2,
        to_kind: 0,
        to_path: vec![],
        to_row: 0,
        bezier_args: GraphBezierArgsData {
            from_x: 10,
            from_y: 20,
            c1x: 30,
            c1y: 40,
            c2x: 50,
            c2y: 60,
            to_x: 70,
            to_y: 80,
        },
        bezier_from_x: 10,
        bezier_from_y: 20,
        bezier_c1x: 30,
        bezier_c1y: 40,
        bezier_c2x: 50,
        bezier_c2y: 60,
        bezier_to_x: 70,
        bezier_to_y: 80,
    };

    let json = serde_json::to_string(&edge).expect("serialize edge");
    assert!(json.contains("\"fromRenderHandle\":1"));
    assert!(json.contains("\"toRenderHandle\":2"));
    assert!(json.contains("\"bezierFromX\":10"));
    assert!(json.contains("\"bezierToY\":80"));

    let parsed: GraphEdgeData = serde_json::from_str(&json).expect("deserialize edge");
    assert_eq!(parsed.bezier_from_x, 10);
    assert_eq!(parsed.bezier_to_y, 80);
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

#[test]
fn snapshot_ready_main_graph_serializes_string_path_keys() {
    let mut runtime = DocumentRuntime::default();
    let mut metrics = DocumentEngineMetrics::default();
    let handle = start_job(
        &mut runtime,
        &mut metrics,
        DocumentJobSpec {
            kind: DocumentJobKind::AnalyzeSource,
            document_key: "main-graph-keys".into(),
            language: "json".into(),
            input: DocumentInputPlan::SourceText,
            settings: DocumentJobSettings::default(),
            output: OutputPlan {
                analysis: true,
                graph: true,
            },
            base_snapshot_id: None,
            edits: vec![],
        },
    );

    let _ = advance_job(
        &mut runtime,
        &mut metrics,
        handle,
        AdvanceInput::TextChunk(r#"{"users":[{"name":"Ada"}]}"#.into()),
    );
    let close_batch = advance_job(&mut runtime, &mut metrics, handle, AdvanceInput::Close);

    let snapshot_ready = close_batch
        .events
        .iter()
        .find_map(|event| match event {
            DocumentEvent::SnapshotReady {
                main_graph: Some(main_graph),
                ..
            } => Some(main_graph),
            _ => None,
        })
        .expect("close batch should include SnapshotReady.mainGraph");
    let json =
        serde_json::to_value(snapshot_ready).expect("SnapshotReady.mainGraph should serialize");
    assert_graph_json_path_keys_are_strings(&json);
}

#[test]
fn table_cell_geometry_patch_keeps_incremental_index_and_delta_patch() {
    let source = "name,age\nAda,37\n";
    let replacement = "Bob";
    let edit_start = source.find("Ada").expect("fixture contains cell") as u32;

    let base = materialize(
        &DocumentInputPlan::SourceText,
        "csv-table-cell",
        "csv",
        source,
        false,
        &OutputPlan {
            analysis: true,
            graph: true,
        },
        &[],
        None,
    );
    let result = treease_core::document::materialize_with_base(
        &DocumentInputPlan::BaseTextWithEdits,
        "csv-table-cell",
        "csv",
        source,
        false,
        &OutputPlan {
            analysis: true,
            graph: true,
        },
        &[treease_core::core::DocumentTextEdit {
            start_byte: edit_start,
            old_end_byte: edit_start + "Ada".len() as u32,
            new_end_byte: edit_start + replacement.len() as u32,
            replacement: replacement.to_owned(),
        }],
        None,
        base.analysis.document.as_ref(),
        base.incremental.as_ref(),
    );

    let graph = result.graph.expect("graph projection expected");
    assert_eq!(graph.clear, false);
    let incremental = result.incremental.expect("incremental state expected");
    assert!(
        incremental.graph_model_index.is_some(),
        "incremental should carry graph model index"
    );
}

#[test]
fn structural_subtree_delta_does_not_clear_or_replace_unaffected_root() {
    let source = r#"{"root":{"profile":{"name":"Alice"},"count":1},"tail":{"keep":true}}"#;
    let old = r#"{"name":"Alice"}"#;
    let replacement = r#"{"name":"Bob","role":"owner"}"#;
    let edit_start = source.find(old).expect("fixture contains subtree") as u32;

    let base = materialize(
        &DocumentInputPlan::SourceText,
        "json-subtree-delta",
        "json",
        source,
        false,
        &OutputPlan {
            analysis: true,
            graph: true,
        },
        &[],
        None,
    );
    let result = treease_core::document::materialize_with_base(
        &DocumentInputPlan::BaseTextWithEdits,
        "json-subtree-delta",
        "json",
        source,
        false,
        &OutputPlan {
            analysis: true,
            graph: true,
        },
        &[treease_core::core::DocumentTextEdit {
            start_byte: edit_start,
            old_end_byte: edit_start + old.len() as u32,
            new_end_byte: edit_start + replacement.len() as u32,
            replacement: replacement.to_owned(),
        }],
        None,
        base.analysis.document.as_ref(),
        base.incremental.as_ref(),
    );

    let delta = result
        .graph
        .and_then(|graph| graph.graph_data)
        .expect("incremental graph delta expected");
    assert!(
        delta.nodes_removed.is_empty(),
        "structural subtree delta should not remove unaffected nodes"
    );
    assert!(
        delta
            .nodes_added
            .iter()
            .chain(delta.nodes_updated.iter())
            .any(|node| { node.path.iter().any(|segment| segment.key == "profile") }),
        "delta should mention the impacted profile fragment"
    );
}

#[test]
fn table_cell_value_edit_uses_table_patch_without_clearing_graph() {
    let source = r#"{"rows":[{"name":"Ada","score":1}]}"#;
    let replacement = "2";
    let edit_start = source.find('1').expect("fixture contains score") as u32;

    let base = materialize(
        &DocumentInputPlan::SourceText,
        "doc-table-patch",
        "json",
        source,
        false,
        &OutputPlan {
            analysis: true,
            graph: true,
        },
        &[],
        None,
    );
    let result = treease_core::document::materialize_with_base(
        &DocumentInputPlan::BaseTextWithEdits,
        "doc-table-patch",
        "json",
        source,
        false,
        &OutputPlan {
            analysis: true,
            graph: true,
        },
        &[treease_core::core::DocumentTextEdit {
            start_byte: edit_start,
            old_end_byte: edit_start + 1,
            new_end_byte: edit_start + replacement.len() as u32,
            replacement: replacement.to_owned(),
        }],
        None,
        base.analysis.document.as_ref(),
        base.incremental.as_ref(),
    );

    let graph = result.graph.expect("graph projection expected");
    assert!(!graph.clear);
    let data = graph.graph_data.as_ref().expect("delta data present");
    let has_table_patch = data.table_patches.iter().any(|patch| match patch {
        treease_core::document::protocol::TablePatch::CellsUpdated { cells, .. } => {
            cells.iter().any(|cell| cell.cell.text == "2")
        }
        _ => false,
    });
    assert!(
        has_table_patch,
        "score edit should be emitted as table cell patch"
    );
}
