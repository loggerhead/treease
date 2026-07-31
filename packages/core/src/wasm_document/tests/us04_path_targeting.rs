use super::*;
use crate::document::protocol::DocumentDirectChild;

const COMPLEX_JSON_FIXTURE: &str = "test/fixtures/json/complex.1.json";
const COMPLEX_LONG_KEY: &str = "we___are___such___stuff___as___dreams___are___made___on___and___our___little___life___is___rounded___with___sleep";

fn analyze_complex_fixture(document_key: &str) -> SnapshotId {
    let source = read_repo_fixture(COMPLEX_JSON_FIXTURE);
    let (snapshot_id, _) = analyze_document_via_job(document_key, "json", &[&source]);
    snapshot_id
}

fn analyze_complex_fixture_with_nest(document_key: &str) -> SnapshotId {
    let source = read_repo_fixture(COMPLEX_JSON_FIXTURE);
    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: document_key.to_owned(),
        language: "json".to_owned(),
        output_graph: true,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: crate::document::protocol::DocumentFormattingSettings::default(),
        },
    })
    .expect("nested complex fixture job should start");

    let _ = text_chunk(started.job_handle, &source);
    let close_batch = close(started.job_handle);
    snapshot_id_from_batch(&close_batch)
}

fn assert_find_anchors_ready(
    document_key: &str,
    snapshot_id: SnapshotId,
    path: &str,
    target: QueryTargetKind,
) {
    let result = query_snapshot_impl(QuerySnapshotRequest {
        document_key: document_key.to_owned(),
        snapshot_id: snapshot_id.0 as u32,
        query_kind: QueryKind::FindAnchors as u8,
        has_path: true,
        path_pattern: path.to_owned(),
        span_start: 0,
        span_end: 0,
        target,
    })
    .expect("query should execute");

    match result {
        SnapshotReadResult::Ready { data } => {
            assert!(
                !data.anchors.is_empty(),
                "FindAnchors for {path} should return anchors, got empty. document_key={document_key} snapshot_id={}",
                snapshot_id.0,
            );
            for anchor in &data.anchors {
                assert!(
                    anchor.span_end >= anchor.span_start,
                    "FindAnchors for {path} returned invalid span {}..{} for document_key={document_key} snapshot_id={}",
                    anchor.span_start,
                    anchor.span_end,
                    snapshot_id.0,
                );
            }
        }
        SnapshotReadResult::SnapshotNotReady => {
            panic!("snapshot for {document_key} should be ready");
        }
    }
}

#[test]
fn wasm_document_plan_then_apply_nested_value_edit_matches_web_probe_cases() {
    let _guard = lock_test_mutex();

    struct NestedCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_value: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        NestedCase {
            language: "json",
            document_key: "nested-json",
            source: r#"{"user":{"name":"Alice","role":"admin"},"count":42}"#,
            path: vec![key_seg("user"), key_seg("name")],
            next_value: "Carol",
            expected_source: r#"{"user":{"name":"Carol","role":"admin"},"count":42}"#,
        },
        NestedCase {
            language: "yaml",
            document_key: "nested-yaml",
            source: "table_with_header:\n  - id: 0\n    meta:\n      name: Alice\n      role: owner\n    status: ready\n",
            path: vec![
                key_seg("table_with_header"),
                index_seg(0),
                key_seg("meta"),
                key_seg("name"),
            ],
            next_value: "Bob",
            expected_source: "table_with_header:\n  - id: 0\n    meta:\n      name: 'Bob'\n      role: owner\n    status: ready\n",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
            document_key: case.document_key.to_owned(),
            snapshot_id: base_snapshot_id,
            language: case.language.to_owned(),
            path: case.path,
            prefer_key: false,
            raw_replacement: None,
            value: scalar_edit_value(case.next_value),
        })
        .expect("planner should execute");

        let plan = match planned {
            SnapshotReadResult::Ready { data } => data,
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };
        assert_eq!(
            plan.mode,
            GraphValueEditPlanMode::Edits,
            "{}",
            case.language
        );
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            plan.edits,
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} nested edit should complete",
            case.language,
        );
        let snapshot = stored_snapshot_for_document(case.document_key)
            .expect("nested edit snapshot should be stored");
        assert_eq!(
            snapshot
                .analysis
                .as_ref()
                .map(|analysis| analysis.source.as_str()),
            Some(case.expected_source),
            "{} nested edit should update target leaf",
            case.language,
        );
    }
}

#[test]
fn wasm_document_query_snapshot_rejects_stale_snapshot_for_document() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let stale_source = r#"{"table_with_header":[{"h2":1}]}"#;
    let (stale_snapshot_id, _) = analyze_document_via_job("query-stale", "json", &[stale_source]);

    let current_source = r#"{"Result":{"Blocks":[{"Id":"7569035291635154985"}]}}"#;
    let (_current_snapshot_id, _) =
        analyze_document_via_job("query-current", "json", &[current_source]);
    let cursor_offset = current_source
        .find("7569035291635154985")
        .expect("fixture should contain target value") as u32;

    let result = query_snapshot_impl(QuerySnapshotRequest {
        document_key: "query-current".to_owned(),
        snapshot_id: stale_snapshot_id.0 as u32,
        query_kind: QueryKind::ResolvePath as u8,
        has_path: false,
        path_pattern: String::new(),
        span_start: cursor_offset,
        span_end: cursor_offset,
        target: QueryTargetKind::Value,
    })
    .expect("query should execute");

    assert!(matches!(result, SnapshotReadResult::SnapshotNotReady));
}

#[test]
fn wasm_document_query_snapshot_returns_lightweight_projections() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = r#"{"user":{"name":"Ada","age":37},"items":[true,null]}"#;
    let (snapshot_id, _) = analyze_document_via_job("query-projections", "json", &[source]);

    let root = query_snapshot_impl(QuerySnapshotRequest {
        document_key: "query-projections".to_owned(),
        snapshot_id: snapshot_id.0 as u32,
        query_kind: QueryKind::RootValueKind as u8,
        has_path: false,
        path_pattern: String::new(),
        span_start: 0,
        span_end: 0,
        target: QueryTargetKind::Value,
    })
    .expect("root projection should execute");
    let root = match root {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(root.root_value_kind.as_deref(), Some("object"));

    let direct_children = query_snapshot_impl(QuerySnapshotRequest {
        document_key: "query-projections".to_owned(),
        snapshot_id: snapshot_id.0 as u32,
        query_kind: QueryKind::DirectChildren as u8,
        has_path: true,
        path_pattern: "$.items".to_owned(),
        span_start: 0,
        span_end: 0,
        target: QueryTargetKind::Value,
    })
    .expect("direct children query should execute");
    let direct_children = match direct_children {
        SnapshotReadResult::Ready { data } => data.direct_children,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(direct_children.len(), 2);
    assert!(matches!(
        direct_children.first(),
        Some(DocumentDirectChild::Index {
            index: 0,
            preview,
            value_type,
            is_container: false,
            ..
        }) if preview == "true" && value_type == "boolean"
    ));

    let path_value = query_snapshot_impl(QuerySnapshotRequest {
        document_key: "query-projections".to_owned(),
        snapshot_id: snapshot_id.0 as u32,
        query_kind: QueryKind::PathValue as u8,
        has_path: true,
        path_pattern: "$.user.name".to_owned(),
        span_start: 0,
        span_end: 0,
        target: QueryTargetKind::Value,
    })
    .expect("path value projection should execute");
    let path_value = match path_value {
        SnapshotReadResult::Ready { data } => data.path_value.expect("path value present"),
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(path_value.value_type, "string");
    assert_eq!(path_value.value, "Ada");
    assert_eq!(path_value.source_text, r#""Ada""#);

    let preview = query_snapshot_impl(QuerySnapshotRequest {
        document_key: "query-projections".to_owned(),
        snapshot_id: snapshot_id.0 as u32,
        query_kind: QueryKind::NodePreview as u8,
        has_path: true,
        path_pattern: "$.user.age".to_owned(),
        span_start: 0,
        span_end: 0,
        target: QueryTargetKind::Value,
    })
    .expect("node preview projection should execute");
    let preview = match preview {
        SnapshotReadResult::Ready { data } => data.node_preview.expect("node preview present"),
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(preview.value_type, "number");
    assert_eq!(preview.value, "37");

    let labels = query_snapshot_impl(QuerySnapshotRequest {
        document_key: "query-projections".to_owned(),
        snapshot_id: snapshot_id.0 as u32,
        query_kind: QueryKind::FieldLabels as u8,
        has_path: false,
        path_pattern: String::new(),
        span_start: 0,
        span_end: 0,
        target: QueryTargetKind::Value,
    })
    .expect("field labels projection should execute");
    let labels = match labels {
        SnapshotReadResult::Ready { data } => data.field_labels,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert!(labels.contains(&"user".to_owned()));
    assert!(labels.contains(&"name".to_owned()));
    assert!(labels.contains(&"age".to_owned()));
    assert!(labels.contains(&"items".to_owned()));
}

#[test]
fn wasm_document_path_value_for_toml_tables_includes_source_text() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let preview_source = "[preview]\ncolor = \"#4f46e5\"\ntime = \"2026-04-13T10:00:00Z\"\n\n";
    let items_source = "[[items]]\nname = \"Ada\"\n\n[[items]]\nname = \"Grace\"\n";
    let source = format!("{preview_source}{items_source}");
    let (snapshot_id, _) = analyze_document_via_job("query-toml-tables", "toml", &[&source]);

    for (path_pattern, value_type, expected_source) in [
        ("$.preview", "object", preview_source),
        ("$.items", "array", items_source),
    ] {
        let result = query_snapshot_impl(QuerySnapshotRequest {
            document_key: "query-toml-tables".to_owned(),
            snapshot_id: snapshot_id.0 as u32,
            query_kind: QueryKind::PathValue as u8,
            has_path: true,
            path_pattern: path_pattern.to_owned(),
            span_start: 0,
            span_end: 0,
            target: QueryTargetKind::Value,
        })
        .expect("path-value query should execute");
        let path_value = match result {
            SnapshotReadResult::Ready { data } => {
                data.path_value.expect("TOML table path value present")
            }
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };

        assert_eq!(path_value.value_type, value_type, "{path_pattern}");
        assert_eq!(path_value.source_text, expected_source, "{path_pattern}");
    }
}

#[test]
fn wasm_document_query_snapshot_resolves_path_on_2mb_json_fixture() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = read_repo_fixture("test/fixtures/json/2mb.1.json");
    let (snapshot_id, _) = analyze_document_via_job("query-2mb", "json", &[&source]);
    let cursor_offset = source
        .find("3506455323725496609")
        .expect("fixture should contain first block id") as u32;

    let result = query_snapshot_impl(QuerySnapshotRequest {
        document_key: "query-2mb".to_owned(),
        snapshot_id: snapshot_id.0 as u32,
        query_kind: QueryKind::ResolvePath as u8,
        has_path: false,
        path_pattern: String::new(),
        span_start: cursor_offset,
        span_end: cursor_offset,
        target: QueryTargetKind::Value,
    })
    .expect("query should execute");

    match result {
        SnapshotReadResult::Ready { data } => {
            assert_eq!(data.anchors.len(), 1);
            assert_eq!(data.anchors[0].path, "$.Result.Blocks[0].Id");
        }
        SnapshotReadResult::SnapshotNotReady => panic!("2mb fixture snapshot should be ready"),
    }
}

#[test]
fn wasm_document_plan_then_apply_large_table_row_hundred_matches_web_table_probe_cases() {
    let _guard = lock_test_mutex();

    struct LargeTableCase {
        language: &'static str,
        document_key: &'static str,
        source: String,
        path: Vec<GraphPathSeg>,
        next_value: &'static str,
        expected_mode: GraphValueEditPlanMode,
        expected_source: Option<String>,
    }

    let json_source = build_json_table_document(140);
    let yaml_source = build_yaml_table_document(140);

    let cases = vec![
        LargeTableCase {
            language: "json",
            document_key: "table-json-100",
            expected_source: Some(json_source.replacen("row-100", "row-100-updated", 1)),
            source: json_source,
            path: vec![
                key_seg("table_with_header"),
                index_seg(100),
                key_seg("name"),
            ],
            next_value: "row-100-updated",
            expected_mode: GraphValueEditPlanMode::Edits,
        },
        LargeTableCase {
            language: "yaml",
            document_key: "table-yaml-100",
            expected_source: Some(yaml_source.replacen(
                "name: row-100\n",
                "name: 'row-100-updated'\n",
                1,
            )),
            source: yaml_source,
            path: vec![
                key_seg("table_with_header"),
                index_seg(100),
                key_seg("name"),
            ],
            next_value: "row-100-updated",
            expected_mode: GraphValueEditPlanMode::Edits,
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[&case.source]);
        let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
            document_key: case.document_key.to_owned(),
            snapshot_id: base_snapshot_id,
            language: case.language.to_owned(),
            path: case.path,
            prefer_key: false,
            raw_replacement: None,
            value: scalar_edit_value(case.next_value),
        })
        .expect("planner should execute");

        let plan = match planned {
            SnapshotReadResult::Ready { data } => data,
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };
        assert_eq!(plan.mode, case.expected_mode, "{}", case.language);

        match case.expected_source {
            Some(expected_source) => {
                let started = start_apply_job(
                    case.document_key,
                    case.language,
                    base_snapshot_id,
                    plan.edits,
                );
                let close_batch = close(started.job_handle);
                assert!(
                    matches!(close_batch.terminal, Some(JobTerminal::Completed)),
                    "{} row-100 edit should complete",
                    case.language,
                );
                assert_snapshot_source(case.document_key, &expected_source);
            }
            None => {
                assert!(
                    plan.reason.is_some(),
                    "{} row-100 fallback should report a reason",
                    case.language,
                );
                assert!(
                    plan.edits.is_empty(),
                    "{} row-100 fallback should not emit direct edits",
                    case.language,
                );
            }
        }
    }
}

#[test]
fn wasm_document_plan_then_apply_table_cell_value_extends_non_streaming_languages() {
    let _guard = lock_test_mutex();

    struct TableCellPlanCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_value: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        TableCellPlanCase {
            language: "toml",
            document_key: "plan-table-cell-toml",
            source: "[[table_with_header]]\nid = 0\nname = \"row-0\"\nstatus = \"ready\"\n\n[[table_with_header]]\nid = 1\nname = \"row-1\"\nstatus = \"hold\"\n",
            path: vec![key_seg("table_with_header"), index_seg(0), key_seg("name")],
            next_value: "row-0-updated",
            expected_source: "[[table_with_header]]\nid = 0\nname = \"row-0-updated\"\nstatus = \"ready\"\n\n[[table_with_header]]\nid = 1\nname = \"row-1\"\nstatus = \"hold\"\n",
        },
        TableCellPlanCase {
            language: "python",
            document_key: "plan-table-cell-python",
            source: "{'table_with_header': [{'id': 0, 'name': 'row-0', 'status': 'ready'}, {'id': 1, 'name': 'row-1', 'status': 'hold'}], 'meta': {'owner': 'Ada'}}",
            path: vec![key_seg("table_with_header"), index_seg(0), key_seg("name")],
            next_value: "row-0-updated",
            expected_source: "{'table_with_header': [{'id': 0, 'name': 'row-0-updated', 'status': 'ready'}, {'id': 1, 'name': 'row-1', 'status': 'hold'}], 'meta': {'owner': 'Ada'}}",
        },
        TableCellPlanCase {
            language: "javascript",
            document_key: "plan-table-cell-javascript",
            source: "({table_with_header: [{id: 0, name: \"row-0\", status: \"ready\"}, {id: 1, name: \"row-1\", status: \"hold\"}], meta: {owner: \"Ada\"}})",
            path: vec![key_seg("table_with_header"), index_seg(0), key_seg("name")],
            next_value: "row-0-updated",
            expected_source: "({table_with_header: [{id: 0, name: \"row-0-updated\", status: \"ready\"}, {id: 1, name: \"row-1\", status: \"hold\"}], meta: {owner: \"Ada\"}})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
            document_key: case.document_key.to_owned(),
            snapshot_id: base_snapshot_id,
            language: case.language.to_owned(),
            path: case.path,
            prefer_key: false,
            raw_replacement: None,
            value: scalar_edit_value(case.next_value),
        })
        .expect("planner should execute");

        let plan = match planned {
            SnapshotReadResult::Ready { data } => data,
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };
        assert_eq!(
            plan.mode,
            GraphValueEditPlanMode::Edits,
            "{}",
            case.language
        );
        assert!(
            !plan.edits.is_empty(),
            "{} table cell planner should emit edits",
            case.language
        );

        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            plan.edits,
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} table cell planner round-trip should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_plans_csv_header_key_edit() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = read_repo_fixture("test/fixtures/csv/region_and_currency.csv");
    let (base_snapshot_id, _) = analyze_document_via_job("csv-header-key", "csv", &[&source]);
    let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "csv-header-key".to_owned(),
        snapshot_id: base_snapshot_id,
        language: "csv".to_owned(),
        path: vec![index_seg(0), key_seg("Currency Code")],
        prefer_key: true,
        raw_replacement: None,
        value: scalar_edit_value("Currency Id"),
    })
    .expect("planner should execute");

    let plan = match planned {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(plan.mode, GraphValueEditPlanMode::Edits);
    assert!(plan.reason.is_none());
    assert_eq!(plan.edits.len(), 1);
    let edit = &plan.edits[0];
    assert_eq!(
        &source[edit.start_byte as usize..edit.old_end_byte as usize],
        "\"Currency Code\""
    );
    assert_eq!(edit.replacement, "Currency Id");
}

#[test]
fn wasm_document_plan_then_apply_table_without_header_value_matches_round_trip() {
    let _guard = lock_test_mutex();

    struct HeaderlessPlanCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_value: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        HeaderlessPlanCase {
            language: "json",
            document_key: "plan-headerless-json",
            source: r#"{"table_without_header":["a","b","c"]}"#,
            path: vec![key_seg("table_without_header"), index_seg(1)],
            next_value: "beta",
            expected_source: r#"{"table_without_header":["a","beta","c"]}"#,
        },
        HeaderlessPlanCase {
            language: "yaml",
            document_key: "plan-headerless-yaml",
            source: "table_without_header:\n  - a\n  - b\n  - c\n",
            path: vec![key_seg("table_without_header"), index_seg(1)],
            next_value: "beta",
            expected_source: "table_without_header:\n  - a\n  - 'beta'\n  - c\n",
        },
        HeaderlessPlanCase {
            language: "toml",
            document_key: "plan-headerless-toml",
            source: "table_without_header = [ \"a\", \"b\", \"c\" ]\n",
            path: vec![key_seg("table_without_header"), index_seg(1)],
            next_value: "beta",
            expected_source: "table_without_header = [ \"a\", \"beta\", \"c\" ]\n",
        },
        HeaderlessPlanCase {
            language: "python",
            document_key: "plan-headerless-python",
            source: "{'table_without_header': ['a', 'b', 'c']}",
            path: vec![key_seg("table_without_header"), index_seg(1)],
            next_value: "beta",
            expected_source: "{'table_without_header': ['a', 'beta', 'c']}",
        },
        HeaderlessPlanCase {
            language: "javascript",
            document_key: "plan-headerless-javascript",
            source: "({table_without_header: ['a', 'b', 'c']})",
            path: vec![key_seg("table_without_header"), index_seg(1)],
            next_value: "beta",
            expected_source: "({table_without_header: ['a', \"beta\", 'c']})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
            document_key: case.document_key.to_owned(),
            snapshot_id: base_snapshot_id,
            language: case.language.to_owned(),
            path: case.path,
            prefer_key: false,
            raw_replacement: None,
            value: scalar_edit_value(case.next_value),
        })
        .expect("planner should execute");
        let plan = match planned {
            SnapshotReadResult::Ready { data } => data,
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };
        assert_eq!(
            plan.mode,
            GraphValueEditPlanMode::Edits,
            "{}",
            case.language
        );
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            plan.edits,
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} headerless planner round-trip should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_plan_repeated_path_cases_cover_object_and_nested_sequences() {
    let _guard = lock_test_mutex();

    struct RepeatedCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_value: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        RepeatedCase {
            language: "json",
            document_key: "repeated-json-object",
            source: r#"{"items":[{"name":"a"},{"name":"b"}]}"#,
            path: vec![key_seg("items"), index_seg(1), key_seg("name")],
            next_value: "c",
            expected_source: r#"{"items":[{"name":"a"},{"name":"c"}]}"#,
        },
        RepeatedCase {
            language: "yaml",
            document_key: "repeated-yaml-object",
            source: "items:\n  - name: a\n  - name: b\n",
            path: vec![key_seg("items"), index_seg(1), key_seg("name")],
            next_value: "c",
            expected_source: "items:\n  - name: a\n  - name: 'c'\n",
        },
        RepeatedCase {
            language: "json",
            document_key: "repeated-json-nested",
            source: r#"{"groups":[{"items":[{"name":"a"}]},{"items":[{"name":"b"}]}]}"#,
            path: vec![
                key_seg("groups"),
                index_seg(1),
                key_seg("items"),
                index_seg(0),
                key_seg("name"),
            ],
            next_value: "c",
            expected_source: r#"{"groups":[{"items":[{"name":"a"}]},{"items":[{"name":"c"}]}]}"#,
        },
        RepeatedCase {
            language: "yaml",
            document_key: "repeated-yaml-nested",
            source: "groups:\n  - items:\n      - name: a\n  - items:\n      - name: b\n",
            path: vec![
                key_seg("groups"),
                index_seg(1),
                key_seg("items"),
                index_seg(0),
                key_seg("name"),
            ],
            next_value: "c",
            expected_source:
                "groups:\n  - items:\n      - name: a\n  - items:\n      - name: 'c'\n",
        },
        RepeatedCase {
            language: "toml",
            document_key: "repeated-toml-object",
            source: "[[items]]\nname = \"a\"\n\n[[items]]\nname = \"b\"\n",
            path: vec![key_seg("items"), index_seg(1), key_seg("name")],
            next_value: "c",
            expected_source: "[[items]]\nname = \"a\"\n\n[[items]]\nname = \"c\"\n",
        },
        RepeatedCase {
            language: "python",
            document_key: "repeated-python-object",
            source: "{'items': [{'name': 'a'}, {'name': 'b'}]}",
            path: vec![key_seg("items"), index_seg(1), key_seg("name")],
            next_value: "c",
            expected_source: "{'items': [{'name': 'a'}, {'name': 'c'}]}",
        },
        RepeatedCase {
            language: "javascript",
            document_key: "repeated-javascript-object",
            source: "({items: [{name: \"a\"}, {name: \"b\"}]})",
            path: vec![key_seg("items"), index_seg(1), key_seg("name")],
            next_value: "c",
            expected_source: "({items: [{name: \"a\"}, {name: \"c\"}]})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
            document_key: case.document_key.to_owned(),
            snapshot_id: base_snapshot_id,
            language: case.language.to_owned(),
            path: case.path,
            prefer_key: false,
            raw_replacement: None,
            value: scalar_edit_value(case.next_value),
        })
        .expect("planner should execute");
        let plan = match planned {
            SnapshotReadResult::Ready { data } => data,
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };
        assert_eq!(
            plan.mode,
            GraphValueEditPlanMode::Edits,
            "{}",
            case.language
        );
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            plan.edits,
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} repeated path planner round-trip should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

/// The WASM `query_snapshot` receives `queryKind` as the string `"findAnchors"`
/// from TypeScript. Serde must deserialize this correctly — if it doesn't, the
/// query falls through to ResolveHover (the catch-all branch) and returns empty.
#[test]
fn wasm_document_querykind_findanchors_serde() {
    let val = serde_json::json!("findAnchors");
    let kind: crate::document::protocol::QueryKind =
        serde_json::from_value(val).expect("deserialize findAnchors");
    assert_eq!(kind, crate::document::protocol::QueryKind::FindAnchors);

    // Also verify that a case mismatch does not accidentally deserialize.
    let bogus = serde_json::json!("findanchor");
    let result: Result<crate::document::protocol::QueryKind, _> = serde_json::from_value(bogus);
    assert!(result.is_err(), "case mismatch should not deserialize");
}

/// Direct Rust test that mimics the Wasm `query_snapshot` call for the
/// complex.1.json fixture — targeting a deep key cell through a long
/// array- and empty-string-key path.
#[test]
fn wasm_document_find_anchors_complex_fixture_deep_empty_key() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let snapshot_id = analyze_complex_fixture("complex-deep");
    let path = format!("$.{COMPLEX_LONG_KEY}[43][\"\"]");
    assert_find_anchors_ready("complex-deep", snapshot_id, &path, QueryTargetKind::Value);
}

/// Same fixture, path without empty-string key — just index into the array.
/// This is the case originally investigated in the previous session.
#[test]
fn wasm_document_find_anchors_complex_fixture_simple_index() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let snapshot_id = analyze_complex_fixture("complex-index");
    let path = format!("$.{COMPLEX_LONG_KEY}[16]");
    assert_find_anchors_ready("complex-index", snapshot_id, &path, QueryTargetKind::Value);
}

/// Verify that FindAnchors with target Key works for the same deep path.
#[test]
fn wasm_document_find_anchors_complex_fixture_deep_empty_key_target_key() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let snapshot_id = analyze_complex_fixture("complex-deep-key");
    let path = format!("$.{COMPLEX_LONG_KEY}[43][\"\"]");
    assert_find_anchors_ready("complex-deep-key", snapshot_id, &path, QueryTargetKind::Key);
}

#[test]
fn wasm_document_find_anchors_complex_fixture_nested_json_first_level_with_nest() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let snapshot_id = analyze_complex_fixture_with_nest("complex-nested-first-level");
    let path = format!("$.{COMPLEX_LONG_KEY}[0][\"inner object\"][0].object1[\"object in str\"].a");
    assert_find_anchors_ready(
        "complex-nested-first-level",
        snapshot_id,
        &path,
        QueryTargetKind::Value,
    );
}

#[test]
fn wasm_document_find_anchors_complex_fixture_nested_json_recursive_with_nest() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let snapshot_id = analyze_complex_fixture_with_nest("complex-nested-recursive");
    let path =
        format!("$.{COMPLEX_LONG_KEY}[0][\"inner object\"][0].object1[\"object in str\"].b.c");
    assert_find_anchors_ready(
        "complex-nested-recursive",
        snapshot_id,
        &path,
        QueryTargetKind::Value,
    );
}

#[test]
fn wasm_document_find_anchors_complex_fixture_deep_empty_key_with_nest() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let snapshot_id = analyze_complex_fixture_with_nest("complex-deep-with-nest");
    let path = format!("$.{COMPLEX_LONG_KEY}[43][\"\"]");
    assert_find_anchors_ready(
        "complex-deep-with-nest",
        snapshot_id,
        &path,
        QueryTargetKind::Value,
    );
}

/// Verify that FindAnchors works for BOTH index 0 AND index 1+
/// in the complex.1.json fixture through the direct Rust path.
/// This is the same document that the Wasm test loads.
#[test]
fn wasm_document_find_anchors_complex_fixture_index_scan() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let snapshot_id = analyze_complex_fixture("complex-scan");

    for idx in [0u32, 1, 2, 5, 10, 20, 30, 40, 42, 43, 50, 100, 150] {
        let path = format!("$.{COMPLEX_LONG_KEY}[{idx}][\"\"]");
        assert_find_anchors_ready("complex-scan", snapshot_id, &path, QueryTargetKind::Value);
    }
}

/// Verify that FindAnchors works with output_graph=false (like the Wasm
/// analyzer) and enable_nest=false (the default, which is the correct
/// setting for analysis — nest expansion is a display concern).
#[test]
fn wasm_document_find_anchors_complex_no_graph_no_nest() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = read_repo_fixture(COMPLEX_JSON_FIXTURE);

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "complex-nograph-nonest".to_owned(),
        language: "json".to_owned(),
        output_graph: false,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings::default(),
    })
    .expect("job should start");

    let job_handle = started.job_handle;
    let _ = text_chunk(job_handle, &source);
    let close_batch = close(job_handle);
    let snapshot_id = snapshot_id_from_batch(&close_batch);

    for idx in [0u32, 1, 2, 43] {
        let path = format!("$.{COMPLEX_LONG_KEY}[{idx}][\"\"]");
        assert_find_anchors_ready(
            "complex-nograph-nonest",
            snapshot_id,
            &path,
            QueryTargetKind::Value,
        );
    }
}

#[test]
fn wasm_document_find_anchors_complex_no_graph_with_nest() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = read_repo_fixture(COMPLEX_JSON_FIXTURE);

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "complex-nograph-nest".to_owned(),
        language: "json".to_owned(),
        output_graph: false,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: crate::document::protocol::DocumentFormattingSettings::default(),
        },
    })
    .expect("job should start");

    let job_handle = started.job_handle;
    let _ = text_chunk(job_handle, &source);
    let close_batch = close(job_handle);
    let snapshot_id = snapshot_id_from_batch(&close_batch);

    for idx in [0u32, 1, 2, 43] {
        let path = format!("$.{COMPLEX_LONG_KEY}[{idx}][\"\"]");
        assert_find_anchors_ready(
            "complex-nograph-nest",
            snapshot_id,
            &path,
            QueryTargetKind::Value,
        );
    }
}

#[test]
fn wasm_document_find_anchors_complex_no_graph_with_nest_chunked_stream() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = read_repo_fixture(COMPLEX_JSON_FIXTURE);

    let started = start_document_job_impl(StartDocumentJobRequest {
        document_key: "complex-nograph-nest-chunked".to_owned(),
        language: "json".to_owned(),
        output_graph: false,
        output_analysis: true,
        builder_config: None,
        base_snapshot_id: None,
        edits: vec![],
        settings: DocumentJobSettings {
            parser: crate::document::protocol::DocumentParserSettings {
                enable_nest: true,
                nest_max_depth: 8,
            },
            formatting: crate::document::protocol::DocumentFormattingSettings::default(),
        },
    })
    .expect("job should start");

    for chunk in source.as_bytes().chunks(16 * 1024) {
        let chunk = std::str::from_utf8(chunk).expect("fixture chunks should stay utf-8");
        let _ = text_chunk(started.job_handle, chunk);
    }
    let close_batch = close(started.job_handle);
    let snapshot_id = snapshot_id_from_batch(&close_batch);

    assert_find_anchors_ready(
        "complex-nograph-nest-chunked",
        snapshot_id,
        &format!("$.{COMPLEX_LONG_KEY}[43][\"\"]"),
        QueryTargetKind::Value,
    );
    assert_find_anchors_ready(
        "complex-nograph-nest-chunked",
        snapshot_id,
        &format!("$.{COMPLEX_LONG_KEY}[0][\"inner object\"][0].object1[\"object in str\"].b.c"),
        QueryTargetKind::Value,
    );
}
