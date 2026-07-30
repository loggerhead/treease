use super::*;

#[test]
fn wasm_document_apply_edits_matches_web_editor_incremental_flow_across_languages() {
    let _guard = lock_test_mutex();

    struct ApplyCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        ApplyCase {
            language: "json",
            document_key: "apply-json",
            source: r#"{"root":{"k":1}}"#,
            old: "1",
            replacement: "2",
            expected_source: r#"{"root":{"k":2}}"#,
        },
        ApplyCase {
            language: "yaml",
            document_key: "apply-yaml",
            source: "root:\n  k: old\n",
            old: "old",
            replacement: "new",
            expected_source: "root:\n  k: new\n",
        },
        ApplyCase {
            language: "toml",
            document_key: "apply-toml",
            source: "root = { k = \"old\" }\n",
            old: "\"old\"",
            replacement: "\"new\"",
            expected_source: "root = { k = \"new\" }\n",
        },
        ApplyCase {
            language: "csv",
            document_key: "apply-csv",
            source: "name,age\nAda,37\n",
            old: "Ada",
            replacement: "Bob",
            expected_source: "name,age\nBob,37\n",
        },
        ApplyCase {
            language: "python",
            document_key: "apply-python",
            source: "{\"root\": {\"name\": \"old\"}, \"n\": 1}",
            old: "\"old\"",
            replacement: "\"new\"",
            expected_source: "{\"root\": {\"name\": \"new\"}, \"n\": 1}",
        },
        ApplyCase {
            language: "javascript",
            document_key: "apply-javascript",
            source: "({root: {name: \"old\"}, n: 1})",
            old: "\"old\"",
            replacement: "\"new\"",
            expected_source: "({root: {name: \"new\"}, n: 1})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);

        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} apply-edits close should complete",
            case.language,
        );
        let spec = document_runtime_latest_job_spec_for_document_for_tests(case.document_key)
            .expect("latest job spec should be stored");
        assert_eq!(spec.kind, DocumentJobKind::ApplyEdits, "{}", case.language);
        assert_eq!(
            spec.input,
            DocumentInputPlan::BaseTextWithEdits,
            "{} should keep base+edits input plan",
            case.language,
        );
        let main_graph_clear = close_batch.events.iter().find_map(|event| match event {
            DocumentEvent::SnapshotReady { main_graph, .. } => {
                main_graph.as_ref().map(|projection| projection.clear)
            }
            _ => None,
        });
        assert_eq!(
            main_graph_clear,
            Some(false),
            "{} should emit incremental main graph delta",
            case.language,
        );

        let snapshot = stored_snapshot_for_document(case.document_key)
            .expect("applied snapshot should be stored");
        let analysis = snapshot.analysis.as_ref().expect("analysis should exist");
        let incremental = snapshot
            .incremental
            .as_ref()
            .expect("incremental state should exist");
        assert_eq!(analysis.source, case.expected_source, "{}", case.language);
        assert!(
            incremental.can_resume,
            "{} should remain resumable",
            case.language
        );
        assert!(
            incremental.graph_model_snapshot.is_some(),
            "{} should retain graph model",
            case.language
        );
        assert_eq!(
            document_runtime_job_count_for_tests(),
            0,
            "{} should not leak jobs",
            case.language
        );
    }
}

#[test]
fn wasm_document_plan_then_apply_value_edit_matches_web_graph_edit_round_trip() {
    let _guard = lock_test_mutex();

    struct PlanCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_value: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        PlanCase {
            language: "json",
            document_key: "plan-json",
            source: r#"{"name":"old"}"#,
            path: vec![key_seg("name")],
            next_value: "new",
            expected_source: r#"{"name":"new"}"#,
        },
        PlanCase {
            language: "yaml",
            document_key: "plan-yaml",
            source: "name: old\n",
            path: vec![key_seg("name")],
            next_value: "new",
            expected_source: "name: 'new'\n",
        },
        PlanCase {
            language: "toml",
            document_key: "plan-toml",
            source: "name = \"old\"\n",
            path: vec![key_seg("name")],
            next_value: "new",
            expected_source: "name = \"new\"\n",
        },
        PlanCase {
            language: "csv",
            document_key: "plan-csv",
            source: "name,age\nold,1\n",
            path: vec![index_seg(0), key_seg("name")],
            next_value: "new",
            expected_source: "name,age\nnew,1\n",
        },
        PlanCase {
            language: "python",
            document_key: "plan-python",
            source: "{\"name\": \"old\"}",
            path: vec![key_seg("name")],
            next_value: "new",
            expected_source: "{\"name\": 'new'}",
        },
        PlanCase {
            language: "javascript",
            document_key: "plan-javascript",
            source: "({name: \"old\"})",
            path: vec![key_seg("name")],
            next_value: "new",
            expected_source: "({name: \"new\"})",
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
            "{} should return concrete document edits",
            case.language,
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
            "{} apply-edits close should complete",
            case.language,
        );

        let snapshot = stored_snapshot_for_document(case.document_key)
            .expect("round-trip snapshot should be stored");
        let analysis = snapshot.analysis.as_ref().expect("analysis should exist");
        assert_eq!(analysis.source, case.expected_source, "{}", case.language);
    }
}

#[test]
fn wasm_document_plan_then_apply_key_edit_matches_supported_web_round_trip() {
    let _guard = lock_test_mutex();

    struct KeyCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old_key: &'static str,
        next_key: &'static str,
        path: Vec<GraphPathSeg>,
        expected_source: &'static str,
    }

    let cases = [
        KeyCase {
            language: "json",
            document_key: "key-json",
            source: r#"{"oldKey":"value"}"#,
            old_key: "oldKey",
            next_key: "renamedKey",
            path: vec![key_seg("oldKey")],
            expected_source: r#"{"renamedKey":"value"}"#,
        },
        KeyCase {
            language: "yaml",
            document_key: "key-yaml",
            source: "oldKey: value\n",
            old_key: "oldKey",
            next_key: "renamedKey",
            path: vec![key_seg("oldKey")],
            expected_source: "'renamedKey': value\n",
        },
        KeyCase {
            language: "toml",
            document_key: "key-toml",
            source: "old_key = \"value\"\n",
            old_key: "old_key",
            next_key: "renamed_key",
            path: vec![key_seg("old_key")],
            expected_source: "renamed_key = \"value\"\n",
        },
        KeyCase {
            language: "python",
            document_key: "key-python",
            source: "{\"oldKey\": \"value\"}",
            old_key: "oldKey",
            next_key: "renamedKey",
            path: vec![key_seg("oldKey")],
            expected_source: "{'renamedKey': \"value\"}",
        },
        KeyCase {
            language: "javascript",
            document_key: "key-javascript",
            source: "({oldKey: \"value\"})",
            old_key: "oldKey",
            next_key: "renamedKey",
            path: vec![key_seg("oldKey")],
            expected_source: "({renamedKey: \"value\"})",
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
            prefer_key: true,
            raw_replacement: None,
            value: scalar_edit_value(case.next_key),
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
            "{} key rename should return concrete document edits",
            case.language,
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
            "{} key apply close should complete",
            case.language,
        );

        let snapshot = stored_snapshot_for_document(case.document_key)
            .expect("renamed snapshot should be stored");
        let analysis = snapshot.analysis.as_ref().expect("analysis should exist");
        assert_eq!(analysis.source, case.expected_source, "{}", case.language);
        assert!(
            !analysis.source.contains(case.old_key),
            "{} should remove the original key spelling",
            case.language,
        );
    }
}

#[test]
fn wasm_document_plan_then_apply_mixed_escape_key_edits_match_supported_round_trip() {
    let _guard = lock_test_mutex();

    struct MixedKeyCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old_key: &'static str,
        next_key: &'static str,
        path: Vec<GraphPathSeg>,
        expected_source: &'static str,
    }

    let cases = [
        MixedKeyCase {
            language: "json",
            document_key: "mixed-key-json",
            source: r#"{"oldKey":"value"}"#,
            old_key: "oldKey",
            next_key: "mix\\\"'key",
            path: vec![key_seg("oldKey")],
            expected_source: r#"{"mix\\\"'key":"value"}"#,
        },
        MixedKeyCase {
            language: "yaml",
            document_key: "mixed-key-yaml",
            source: "oldKey: value\n",
            old_key: "oldKey",
            next_key: "mix\\\"'key",
            path: vec![key_seg("oldKey")],
            expected_source: "'mix\\\"''key': value\n",
        },
        MixedKeyCase {
            language: "toml",
            document_key: "mixed-key-toml",
            source: "old_key = \"value\"\n",
            old_key: "old_key",
            next_key: "mix\\\"'key",
            path: vec![key_seg("old_key")],
            expected_source: "\"mix\\\\\\\"'key\" = \"value\"\n",
        },
        MixedKeyCase {
            language: "python",
            document_key: "mixed-key-python",
            source: "{\"oldKey\": \"value\"}",
            old_key: "oldKey",
            next_key: "mix\\\"'key",
            path: vec![key_seg("oldKey")],
            expected_source: "{\"mix\\\\\\\"'key\": \"value\"}",
        },
        MixedKeyCase {
            language: "javascript",
            document_key: "mixed-key-javascript",
            source: "({oldKey: \"value\"})",
            old_key: "oldKey",
            next_key: "mix\\\"'key",
            path: vec![key_seg("oldKey")],
            expected_source: "({\"mix\\\\\\\"'key\": \"value\"})",
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
            prefer_key: true,
            raw_replacement: None,
            value: scalar_edit_value(case.next_key),
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
            "{} mixed key rename should emit edits",
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
            "{} mixed key planner round-trip should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
        let snapshot = stored_snapshot_for_document(case.document_key)
            .expect("mixed key snapshot should be stored");
        let analysis = snapshot.analysis.as_ref().expect("analysis should exist");
        assert!(
            !analysis.source.contains(case.old_key),
            "{} should remove the original key spelling",
            case.language
        );
    }
}

#[test]
fn wasm_document_key_edit_supports_csv() {
    let _guard = lock_test_mutex();

    let cases = [(
        "csv",
        "key-csv",
        "name,age\nAda,37\n",
        vec![index_seg(0), key_seg("name")],
    )];

    for (language, document_key, source, path) in cases {
        reset_test_state();
        let (base_snapshot_id, _) = analyze_document_via_job(document_key, language, &[source]);
        let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
            document_key: document_key.to_owned(),
            snapshot_id: base_snapshot_id,
            language: language.to_owned(),
            path,
            prefer_key: true,
            raw_replacement: None,
            value: scalar_edit_value("renamedKey"),
        })
        .expect("planner should execute");

        let plan = match planned {
            SnapshotReadResult::Ready { data } => data,
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };
        assert_eq!(plan.mode, GraphValueEditPlanMode::Edits, "{language}");
        assert_eq!(plan.edits.len(), 1, "{language} should produce 1 edit");
        assert!(
            plan.reason.is_none(),
            "{language} should not report a fallback reason",
        );
    }
}

#[test]
fn wasm_document_apply_edits_full_document_replace_matches_web_set_value_flow() {
    let _guard = lock_test_mutex();

    struct ReplaceCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        replacement: &'static str,
    }

    let cases = [
        ReplaceCase {
            language: "json",
            document_key: "replace-json",
            source: r#"{"profile":{"name":"Alice"},"items":[1]}"#,
            replacement: r#"{"profile":{"name":"Bob","role":"admin"},"items":[1,2]}"#,
        },
        ReplaceCase {
            language: "yaml",
            document_key: "replace-yaml",
            source: "profile:\n  name: Alice\nitems:\n  - 1\n",
            replacement: "profile:\n  name: Bob\n  role: admin\nitems:\n  - 1\n  - 2\n",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![full_replace_edit(case.source, case.replacement)],
        );
        let close_batch = close(started.job_handle);

        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} full-replace ApplyEdits should complete",
            case.language,
        );
        let snapshot = stored_snapshot_for_document(case.document_key)
            .expect("replaced snapshot should be stored");
        assert_eq!(
            snapshot
                .analysis
                .as_ref()
                .map(|analysis| analysis.source.as_str()),
            Some(case.replacement),
            "{} should store replaced source",
            case.language,
        );
    }
}

#[test]
fn wasm_document_plan_graph_value_edit_reports_snapshot_not_ready_without_snapshot() {
    let _guard = lock_test_mutex();

    let cases = [
        ("json", "missing-snapshot-json", vec![key_seg("name")]),
        ("yaml", "missing-snapshot-yaml", vec![key_seg("name")]),
        ("toml", "missing-snapshot-toml", vec![key_seg("name")]),
        (
            "csv",
            "missing-snapshot-csv",
            vec![index_seg(0), key_seg("name")],
        ),
        ("python", "missing-snapshot-python", vec![key_seg("name")]),
        (
            "javascript",
            "missing-snapshot-javascript",
            vec![key_seg("name")],
        ),
    ];

    for (language, document_key, path) in cases {
        reset_test_state();
        let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
            document_key: document_key.to_owned(),
            snapshot_id: SnapshotId(999),
            language: language.to_owned(),
            path,
            prefer_key: false,
            raw_replacement: None,
            value: scalar_edit_value("next"),
        })
        .expect("planner should return status");

        assert!(
            matches!(planned, SnapshotReadResult::SnapshotNotReady),
            "{language} missing snapshot should report snapshotNotReady",
        );
    }
}

#[test]
fn wasm_document_plan_graph_value_edit_rejects_snapshot_from_another_document() {
    let _guard = lock_test_mutex();
    reset_test_state();
    let (snapshot_id, _) =
        analyze_document_via_job("planner-identity-source", "json", &[r#"{"name":"old"}"#]);

    let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "planner-identity-other".to_owned(),
        snapshot_id,
        language: "json".to_owned(),
        path: vec![key_seg("name")],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("next"),
    })
    .expect("planner should return a snapshot read status");

    assert_eq!(planned, SnapshotReadResult::SnapshotNotReady);
}

#[test]
fn wasm_document_plan_graph_value_edit_reports_invalid_path_for_missing_node() {
    let _guard = lock_test_mutex();

    struct InvalidValueCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
    }

    let cases = [
        InvalidValueCase {
            language: "json",
            document_key: "invalid-value-path-json",
            source: r#"{"name":"old"}"#,
            path: vec![key_seg("missing")],
        },
        InvalidValueCase {
            language: "yaml",
            document_key: "invalid-value-path-yaml",
            source: "name: old\n",
            path: vec![key_seg("missing")],
        },
        InvalidValueCase {
            language: "toml",
            document_key: "invalid-value-path-toml",
            source: "name = \"old\"\n",
            path: vec![key_seg("missing")],
        },
        InvalidValueCase {
            language: "csv",
            document_key: "invalid-value-path-csv",
            source: "name,age\nold,1\n",
            path: vec![index_seg(0), key_seg("missing")],
        },
        InvalidValueCase {
            language: "python",
            document_key: "invalid-value-path-python",
            source: "{\"name\": \"old\"}",
            path: vec![key_seg("missing")],
        },
        InvalidValueCase {
            language: "javascript",
            document_key: "invalid-value-path-javascript",
            source: "({name: \"old\"})",
            path: vec![key_seg("missing")],
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
            value: scalar_edit_value("new"),
        })
        .expect("planner should execute");

        let plan = match planned {
            SnapshotReadResult::Ready { data } => data,
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };
        assert_eq!(
            plan.mode,
            GraphValueEditPlanMode::Replace,
            "{}",
            case.language
        );
        assert_eq!(
            plan.reason,
            Some(GraphValueEditFallbackReason::InvalidPath),
            "{} invalid value path should report invalidPath",
            case.language,
        );
        assert!(
            plan.edits.is_empty(),
            "{} invalid value path should not emit direct edits",
            case.language,
        );
    }
}

#[test]
fn wasm_document_apply_edits_table_cell_value_matches_web_editor_table_cases() {
    let _guard = lock_test_mutex();

    struct ApplyCase {
        language: &'static str,
        document_key: &'static str,
        source: String,
        old: String,
        replacement: String,
    }

    let mut cases = vec![
        ApplyCase {
            language: "json",
            document_key: "apply-table-json",
            source: build_json_table_document(2),
            old: "\"row-0\"".to_owned(),
            replacement: "\"row-0-updated\"".to_owned(),
        },
        ApplyCase {
            language: "yaml",
            document_key: "apply-table-yaml",
            source: build_yaml_table_document(2),
            old: "row-0".to_owned(),
            replacement: "row-0-updated".to_owned(),
        },
        ApplyCase {
            language: "toml",
            document_key: "apply-table-toml",
            source: "[[table_with_header]]\nid = 0\nname = \"row-0\"\nstatus = \"ready\"\n\n[[table_with_header]]\nid = 1\nname = \"row-1\"\nstatus = \"hold\"\n".to_owned(),
            old: "\"row-0\"".to_owned(),
            replacement: "\"row-0-updated\"".to_owned(),
        },
        ApplyCase {
            language: "python",
            document_key: "apply-table-python",
            source: "{'table_with_header': [{'id': 0, 'name': 'row-0', 'status': 'ready'}, {'id': 1, 'name': 'row-1', 'status': 'hold'}], 'meta': {'owner': 'Ada'}}".to_owned(),
            old: "'row-0'".to_owned(),
            replacement: "'row-0-updated'".to_owned(),
        },
        ApplyCase {
            language: "javascript",
            document_key: "apply-table-javascript",
            source: "({table_with_header: [{id: 0, name: \"row-0\", status: \"ready\"}, {id: 1, name: \"row-1\", status: \"hold\"}], meta: {owner: \"Ada\"}})".to_owned(),
            old: "\"row-0\"".to_owned(),
            replacement: "\"row-0-updated\"".to_owned(),
        },
    ];
    cases.push(ApplyCase {
        language: "csv",
        document_key: "apply-table-csv",
        source: read_repo_fixture("test/fixtures/csv/region_and_currency.csv"),
        old: "\"USD\"".to_owned(),
        replacement: "\"USN\"".to_owned(),
    });

    for case in cases {
        reset_test_state();
        let expected_source = case.source.replacen(&case.old, &case.replacement, 1);
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[&case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(&case.source, &case.old, &case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} table ApplyEdits should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, &expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_row_hundred_name_matches_web_editor_large_table_cases() {
    let _guard = lock_test_mutex();

    let cases = [(
        "json",
        "apply-row100-json",
        build_json_table_document(140),
        "\"row-100\"",
        "\"row-100-updated\"",
    )];

    for (language, document_key, source, old, replacement) in cases {
        reset_test_state();
        let expected_source = source.replacen(old, replacement, 1);
        let (base_snapshot_id, _) = analyze_document_via_job(document_key, language, &[&source]);
        let started = start_apply_job(
            document_key,
            language,
            base_snapshot_id,
            vec![replace_edit(&source, old, replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{language} row-100 editor edit should complete",
        );
        assert_snapshot_source(document_key, &expected_source);
    }
}

#[test]
#[ignore = "manual patch_post perf breakdown for same-width large-table cell edit"]
fn perf_patch_post_same_width_large_table_cell_edit() {
    let _guard = lock_test_mutex();

    let document_key = "perf-patch-post-same-width-cell";
    let language = "json";
    let source = build_json_table_document(1_000);
    let old = "\"row-800\"";
    let replacement = "\"row-900\"";

    reset_test_state();
    let expected_source = source.replacen(old, replacement, 1);
    let (base_snapshot_id, _) = analyze_document_via_job(document_key, language, &[&source]);
    let started = start_apply_job(
        document_key,
        language,
        base_snapshot_id,
        vec![replace_edit(&source, old, replacement)],
    );
    let close_batch = close(started.job_handle);
    assert!(
        matches!(close_batch.terminal, Some(JobTerminal::Completed)),
        "{language} same-width large-table cell edit should complete",
    );
    assert_snapshot_source(document_key, &expected_source);
}

#[test]
fn wasm_document_apply_edits_key_rename_matches_web_editor_text_mutations() {
    let _guard = lock_test_mutex();

    struct ApplyCase {
        language: &'static str,
        document_key: &'static str,
        source: String,
        old: &'static str,
        replacement: &'static str,
    }

    let mut cases = vec![
        ApplyCase {
            language: "json",
            document_key: "apply-key-json",
            source: r#"{"profile":{"oldKey":"value"},"count":1}"#.to_owned(),
            old: "oldKey",
            replacement: "renamedKey",
        },
        ApplyCase {
            language: "yaml",
            document_key: "apply-key-yaml",
            source: "profile:\n  oldKey: value\ncount: 1\n".to_owned(),
            old: "oldKey",
            replacement: "renamed_key",
        },
        ApplyCase {
            language: "toml",
            document_key: "apply-key-toml",
            source: "[profile]\nold_key = \"value\"\ncount = 1\n".to_owned(),
            old: "old_key",
            replacement: "renamed_key",
        },
        ApplyCase {
            language: "python",
            document_key: "apply-key-python",
            source: "{'profile': {'oldKey': \"value\"}, 'count': 1}".to_owned(),
            old: "oldKey",
            replacement: "renamedKey",
        },
        ApplyCase {
            language: "javascript",
            document_key: "apply-key-javascript",
            source: "({profile: {oldKey: \"value\"}, count: 1})".to_owned(),
            old: "oldKey",
            replacement: "renamedKey",
        },
    ];
    cases.push(ApplyCase {
        language: "csv",
        document_key: "apply-key-csv",
        source: read_repo_fixture("test/fixtures/csv/region_and_currency.csv"),
        old: "Currency Code",
        replacement: "Currency Id",
    });

    for case in cases {
        reset_test_state();
        let expected_source = case.source.replacen(case.old, case.replacement, 1);
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[&case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(&case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} key ApplyEdits should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, &expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_subtree_replace_matches_web_editor_object_node_cases() {
    let _guard = lock_test_mutex();

    struct ReplaceCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
    }

    let cases = [
        ReplaceCase {
            language: "json",
            document_key: "apply-subtree-json",
            source: r#"{"profile":{"name":"Alice","role":"admin"},"count":1}"#,
            old: r#"{"name":"Alice","role":"admin"}"#,
            replacement: r#"{"name":"Bob","role":"owner","team":"ops"}"#,
        },
        ReplaceCase {
            language: "yaml",
            document_key: "apply-subtree-yaml",
            source: "profile:\n  name: Alice\n  role: admin\ncount: 1\n",
            old: "profile:\n  name: Alice\n  role: admin\n",
            replacement: "profile:\n  name: Bob\n  role: owner\n  team: ops\n",
        },
        ReplaceCase {
            language: "toml",
            document_key: "apply-subtree-toml",
            source: "profile = { name = \"Alice\", role = \"admin\" }\ncount = 1\n",
            old: "profile = { name = \"Alice\", role = \"admin\" }\n",
            replacement: "profile = { name = \"Bob\", role = \"owner\", team = \"ops\" }\n",
        },
        ReplaceCase {
            language: "python",
            document_key: "apply-subtree-python",
            source: "{'profile': {'name': 'Alice', 'role': 'admin'}, 'count': 1}",
            old: "{'name': 'Alice', 'role': 'admin'}",
            replacement: "{'name': 'Bob', 'role': 'owner', 'team': 'ops'}",
        },
        ReplaceCase {
            language: "javascript",
            document_key: "apply-subtree-javascript",
            source: "({profile: {name: \"Alice\", role: \"admin\"}, count: 1})",
            old: "{name: \"Alice\", role: \"admin\"}",
            replacement: "{name: \"Bob\", role: \"owner\", team: \"ops\"}",
        },
    ];

    for case in cases {
        reset_test_state();
        let expected_source = case.source.replacen(case.old, case.replacement, 1);
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} subtree ApplyEdits should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, &expected_source);
    }
}

#[test]
fn wasm_document_plan_then_apply_nested_value_edit_extends_non_streaming_languages() {
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
            language: "toml",
            document_key: "nested-toml",
            source: "[profile]\nname = \"Alice\"\nrole = \"admin\"\ncount = 42\n",
            path: vec![key_seg("profile"), key_seg("name")],
            next_value: "Carol",
            expected_source: "[profile]\nname = \"Carol\"\nrole = \"admin\"\ncount = 42\n",
        },
        NestedCase {
            language: "python",
            document_key: "nested-python",
            source: "{\"user\": {\"name\": \"Alice\", \"role\": \"admin\"}, \"count\": 42}",
            path: vec![key_seg("user"), key_seg("name")],
            next_value: "Carol",
            expected_source: "{\"user\": {\"name\": 'Carol', \"role\": \"admin\"}, \"count\": 42}",
        },
        NestedCase {
            language: "javascript",
            document_key: "nested-javascript",
            source: "({user: {name: \"Alice\", role: \"admin\"}, count: 42})",
            path: vec![key_seg("user"), key_seg("name")],
            next_value: "Carol",
            expected_source: "({user: {name: \"Carol\", role: \"admin\"}, count: 42})",
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
            "{} nested value edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_plan_then_apply_key_edit_covers_nested_and_quoted_supported_cases() {
    let _guard = lock_test_mutex();

    struct KeyCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_key: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        KeyCase {
            language: "json",
            document_key: "key-nested-json",
            source: r#"{"profile":{"oldKey":"value","role":"owner"},"count":1}"#,
            path: vec![key_seg("profile"), key_seg("oldKey")],
            next_key: "new key",
            expected_source: r#"{"profile":{"new key":"value","role":"owner"},"count":1}"#,
        },
        KeyCase {
            language: "yaml",
            document_key: "key-nested-yaml",
            source: "profile:\n  oldKey: value\n  role: owner\ncount: 1\n",
            path: vec![key_seg("profile"), key_seg("oldKey")],
            next_key: "new key",
            expected_source: "profile:\n  'new key': value\n  role: owner\ncount: 1\n",
        },
        KeyCase {
            language: "toml",
            document_key: "key-nested-toml",
            source: "[profile]\nold_key = \"value\"\nrole = \"owner\"\n",
            path: vec![key_seg("profile"), key_seg("old_key")],
            next_key: "new key",
            expected_source: "[profile]\n\"new key\" = \"value\"\nrole = \"owner\"\n",
        },
        KeyCase {
            language: "python",
            document_key: "key-nested-python",
            source: "{\"profile\": {\"oldKey\": \"value\", \"role\": \"owner\"}, \"count\": 1}",
            path: vec![key_seg("profile"), key_seg("oldKey")],
            next_key: "new key",
            expected_source: "{\"profile\": {'new key': \"value\", \"role\": \"owner\"}, \"count\": 1}",
        },
        KeyCase {
            language: "javascript",
            document_key: "key-nested-javascript",
            source: "({profile: {oldKey: \"value\", role: \"owner\"}, count: 1})",
            path: vec![key_seg("profile"), key_seg("oldKey")],
            next_key: "new key",
            expected_source: "({profile: {\"new key\": \"value\", role: \"owner\"}, count: 1})",
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
            prefer_key: true,
            raw_replacement: None,
            value: scalar_edit_value(case.next_key),
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
            "{} nested key edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_plan_graph_value_edit_falls_back_for_subtree_targets() {
    let _guard = lock_test_mutex();

    struct FallbackCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_value: serde_json::Value,
    }

    let cases = [
        FallbackCase {
            language: "yaml",
            document_key: "fallback-yaml-object",
            source: "profile:\n  name: Alice\ncount: 1\n",
            path: vec![key_seg("profile")],
            next_value: json!({"name": "Bob", "team": "ops"}),
        },
        FallbackCase {
            language: "toml",
            document_key: "fallback-toml-object",
            source: "profile = { name = \"Alice\" }\ncount = 1\n",
            path: vec![key_seg("profile")],
            next_value: json!({"name": "Bob", "team": "ops"}),
        },
        FallbackCase {
            language: "python",
            document_key: "fallback-python-object",
            source: "{'profile': {'name': 'Alice'}, 'count': 1}",
            path: vec![key_seg("profile")],
            next_value: json!({"name": "Bob", "team": "ops"}),
        },
        FallbackCase {
            language: "javascript",
            document_key: "fallback-javascript-object",
            source: "({profile: {name: \"Alice\"}, count: 1})",
            path: vec![key_seg("profile")],
            next_value: json!({"name": "Bob", "team": "ops"}),
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
            value: edit_tree_from_plain(case.next_value),
        })
        .expect("planner should execute");

        let plan = match planned {
            SnapshotReadResult::Ready { data } => data,
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };
        assert_eq!(
            plan.mode,
            GraphValueEditPlanMode::Replace,
            "{}",
            case.language
        );
        assert!(
            plan.reason.is_some(),
            "{} subtree fallback should report a reason",
            case.language
        );
        assert!(
            plan.edits.is_empty(),
            "{} subtree fallback should not emit direct edits",
            case.language
        );
    }
}

#[test]
fn wasm_document_plan_then_apply_json_subtree_edits_match_web_replace_flow() {
    let _guard = lock_test_mutex();

    struct JsonSubtreeCase {
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_value: serde_json::Value,
        expected_source: &'static str,
    }

    let cases = [
        JsonSubtreeCase {
            document_key: "json-subtree-object",
            source: r#"{"profile":{"name":"Alice"},"count":1}"#,
            path: vec![key_seg("profile")],
            next_value: json!({"name": "Bob", "team": "ops"}),
            expected_source: r#"{"profile":{"name":"Bob","team":"ops"},"count":1}"#,
        },
        JsonSubtreeCase {
            document_key: "json-subtree-array",
            source: r#"{"items":[1,2],"count":1}"#,
            path: vec![key_seg("items")],
            next_value: json!([1, 2, 3]),
            expected_source: r#"{"items":[1,2,3],"count":1}"#,
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, "json", &[case.source]);
        let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
            document_key: case.document_key.to_owned(),
            snapshot_id: base_snapshot_id,
            language: "json".to_owned(),
            path: case.path,
            prefer_key: false,
            raw_replacement: None,
            value: edit_tree_from_plain(case.next_value),
        })
        .expect("planner should execute");

        let plan = match planned {
            SnapshotReadResult::Ready { data } => data,
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };
        assert_eq!(plan.mode, GraphValueEditPlanMode::Edits);
        assert!(
            !plan.edits.is_empty(),
            "json subtree replace should emit concrete edits"
        );
        let started = start_apply_job(case.document_key, "json", base_snapshot_id, plan.edits);
        let close_batch = close(started.job_handle);
        assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_subtree_projection_edit_preserves_raw_formatting() {
    let _guard = lock_test_mutex();
    reset_test_state();
    let document_key = "json-subtree-source-projection";
    let source = "{\n  \"object\": {\n    \"int\": 42,\n    \"bool\": true\n  }\n}";
    let replacement = "{\n    \"int\": 43,\n    \"bool\": true\n  }";
    let expected = source.replace("{\n    \"int\": 42,\n    \"bool\": true\n  }", replacement);
    let (base_snapshot_id, _) = analyze_document_via_job(document_key, "json", &[source]);

    let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: document_key.to_owned(),
        snapshot_id: base_snapshot_id,
        language: "json".to_owned(),
        path: vec![key_seg("object")],
        prefer_key: false,
        raw_replacement: Some(replacement.to_owned()),
        value: edit_tree_from_plain(json!({"int": 43, "bool": true})),
    })
    .expect("planner should execute");
    let plan = match planned {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(plan.mode, GraphValueEditPlanMode::Edits);

    let started = start_apply_job(document_key, "json", base_snapshot_id, plan.edits);
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source(document_key, &expected);
}

#[test]
fn wasm_document_plan_then_apply_unicode_value_edits_match_web_round_trip() {
    let _guard = lock_test_mutex();

    struct UnicodeCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_value: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        UnicodeCase {
            language: "json",
            document_key: "unicode-json",
            source: r#"{"message":"hello"}"#,
            path: vec![key_seg("message")],
            next_value: "你好 & ok",
            expected_source: r#"{"message":"你好 & ok"}"#,
        },
        UnicodeCase {
            language: "yaml",
            document_key: "unicode-yaml",
            source: "message: hello\n",
            path: vec![key_seg("message")],
            next_value: "你好 & ok",
            expected_source: "message: '你好 & ok'\n",
        },
        UnicodeCase {
            language: "toml",
            document_key: "unicode-toml",
            source: "message = \"hello\"\n",
            path: vec![key_seg("message")],
            next_value: "你好 & ok",
            expected_source: "message = \"你好 & ok\"\n",
        },
        UnicodeCase {
            language: "python",
            document_key: "unicode-python",
            source: "{\"message\": \"hello\"}",
            path: vec![key_seg("message")],
            next_value: "你好 & ok",
            expected_source: "{\"message\": '你好 & ok'}",
        },
        UnicodeCase {
            language: "javascript",
            document_key: "unicode-javascript",
            source: "({message: \"hello\"})",
            path: vec![key_seg("message")],
            next_value: "你好 & ok",
            expected_source: "({message: \"你好 & ok\"})",
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
            "{} unicode value edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_plan_then_apply_unicode_key_edits_match_supported_round_trip() {
    let _guard = lock_test_mutex();

    struct UnicodeKeyCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_key: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        UnicodeKeyCase {
            language: "json",
            document_key: "unicode-key-json",
            source: r#"{"oldKey":"value"}"#,
            path: vec![key_seg("oldKey")],
            next_key: "你好 key",
            expected_source: r#"{"你好 key":"value"}"#,
        },
        UnicodeKeyCase {
            language: "yaml",
            document_key: "unicode-key-yaml",
            source: "oldKey: value\n",
            path: vec![key_seg("oldKey")],
            next_key: "你好 key",
            expected_source: "'你好 key': value\n",
        },
        UnicodeKeyCase {
            language: "toml",
            document_key: "unicode-key-toml",
            source: "old_key = \"value\"\n",
            path: vec![key_seg("old_key")],
            next_key: "你好 key",
            expected_source: "\"你好 key\" = \"value\"\n",
        },
        UnicodeKeyCase {
            language: "python",
            document_key: "unicode-key-python",
            source: "{\"oldKey\": \"value\"}",
            path: vec![key_seg("oldKey")],
            next_key: "你好 key",
            expected_source: "{'你好 key': \"value\"}",
        },
        UnicodeKeyCase {
            language: "javascript",
            document_key: "unicode-key-javascript",
            source: "({oldKey: \"value\"})",
            path: vec![key_seg("oldKey")],
            next_key: "你好 key",
            expected_source: "({\"你好 key\": \"value\"})",
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
            prefer_key: true,
            raw_replacement: None,
            value: scalar_edit_value(case.next_key),
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
            "{} unicode key edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_cross_subtree_mutation_matches_web_editor_freeform_cases() {
    let _guard = lock_test_mutex();

    struct EditCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        EditCase {
            language: "json",
            document_key: "cross-subtree-json",
            source: r#"{"name":"Alice","role":"admin","status":"ready"}"#,
            old: r#""Alice","role":"admin""#,
            replacement: r#""Bob","role":"owner""#,
            expected_source: r#"{"name":"Bob","role":"owner","status":"ready"}"#,
        },
        EditCase {
            language: "yaml",
            document_key: "cross-subtree-yaml",
            source: "name: Alice\nrole: admin\nstatus: ready\n",
            old: "Alice\nrole: admin",
            replacement: "Bob\nrole: owner",
            expected_source: "name: Bob\nrole: owner\nstatus: ready\n",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} cross-subtree ApplyEdits should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_root_boundary_matches_web_editor_full_object_mutation() {
    let _guard = lock_test_mutex();

    let cases = [(
        "json",
        "root-boundary-json",
        r#"{"name":"Alice","count":1}"#,
        r#"{"name":"Alice""#,
        r#"{"name":"Bob","role":"admin""#,
        r#"{"name":"Bob","role":"admin","count":1}"#,
    )];

    for (language, document_key, source, old, replacement, expected_source) in cases {
        reset_test_state();
        let (base_snapshot_id, _) = analyze_document_via_job(document_key, language, &[source]);
        let started = start_apply_job(
            document_key,
            language,
            base_snapshot_id,
            vec![replace_edit(source, old, replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{language} root-boundary ApplyEdits should complete",
        );
        assert_snapshot_source(document_key, expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_nested_leaf_value_matches_web_editor_probe_cases() {
    let _guard = lock_test_mutex();

    struct NestedCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        NestedCase {
            language: "json",
            document_key: "apply-nested-json",
            source: r#"{"user":{"name":"Alice","role":"admin"},"count":42}"#,
            old: "\"Alice\"",
            replacement: "\"Carol\"",
            expected_source: r#"{"user":{"name":"Carol","role":"admin"},"count":42}"#,
        },
        NestedCase {
            language: "yaml",
            document_key: "apply-nested-yaml",
            source: "table_with_header:\n  - id: 0\n    meta:\n      name: Alice\n      role: owner\n    status: ready\n",
            old: "Alice",
            replacement: "Carol",
            expected_source: "table_with_header:\n  - id: 0\n    meta:\n      name: Carol\n      role: owner\n    status: ready\n",
        },
        NestedCase {
            language: "toml",
            document_key: "apply-nested-toml",
            source: "[profile]\nname = \"Alice\"\nrole = \"admin\"\ncount = 42\n",
            old: "\"Alice\"",
            replacement: "\"Carol\"",
            expected_source: "[profile]\nname = \"Carol\"\nrole = \"admin\"\ncount = 42\n",
        },
        NestedCase {
            language: "python",
            document_key: "apply-nested-python",
            source: "{\"user\": {\"name\": \"Alice\", \"role\": \"admin\"}, \"count\": 42}",
            old: "\"Alice\"",
            replacement: "\"Carol\"",
            expected_source: "{\"user\": {\"name\": \"Carol\", \"role\": \"admin\"}, \"count\": 42}",
        },
        NestedCase {
            language: "javascript",
            document_key: "apply-nested-javascript",
            source: "({user: {name: \"Alice\", role: \"admin\"}, count: 42})",
            old: "\"Alice\"",
            replacement: "\"Carol\"",
            expected_source: "({user: {name: \"Carol\", role: \"admin\"}, count: 42})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} nested editor edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_root_key_rename_matches_supported_editor_text_mutations() {
    let _guard = lock_test_mutex();

    struct RootKeyCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        RootKeyCase {
            language: "json",
            document_key: "apply-root-key-json",
            source: r#"{"oldKey":"value","count":1}"#,
            old: "oldKey",
            replacement: "renamedKey",
            expected_source: r#"{"renamedKey":"value","count":1}"#,
        },
        RootKeyCase {
            language: "yaml",
            document_key: "apply-root-key-yaml",
            source: "oldKey: value\ncount: 1\n",
            old: "oldKey",
            replacement: "renamedKey",
            expected_source: "renamedKey: value\ncount: 1\n",
        },
        RootKeyCase {
            language: "toml",
            document_key: "apply-root-key-toml",
            source: "old_key = \"value\"\ncount = 1\n",
            old: "old_key",
            replacement: "renamed_key",
            expected_source: "renamed_key = \"value\"\ncount = 1\n",
        },
        RootKeyCase {
            language: "python",
            document_key: "apply-root-key-python",
            source: "{\"oldKey\": \"value\", \"count\": 1}",
            old: "oldKey",
            replacement: "renamedKey",
            expected_source: "{\"renamedKey\": \"value\", \"count\": 1}",
        },
        RootKeyCase {
            language: "javascript",
            document_key: "apply-root-key-javascript",
            source: "({oldKey: \"value\", count: 1})",
            old: "oldKey",
            replacement: "renamedKey",
            expected_source: "({renamedKey: \"value\", count: 1})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} root key editor edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_escaped_key_rename_matches_editor_text_mutations() {
    let _guard = lock_test_mutex();

    struct EscapedKeyCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        EscapedKeyCase {
            language: "json",
            document_key: "apply-escaped-key-json",
            source: r#"{"old key":"value"}"#,
            old: "old key",
            replacement: r#"new\\key"#,
            expected_source: r#"{"new\\key":"value"}"#,
        },
        EscapedKeyCase {
            language: "yaml",
            document_key: "apply-escaped-key-yaml",
            source: "'old key': value\n",
            old: "old key",
            replacement: r#"new\key"#,
            expected_source: "'new\\key': value\n",
        },
        EscapedKeyCase {
            language: "toml",
            document_key: "apply-escaped-key-toml",
            source: "\"old key\" = \"value\"\n",
            old: "old key",
            replacement: r#"new\\key"#,
            expected_source: "\"new\\\\key\" = \"value\"\n",
        },
        EscapedKeyCase {
            language: "python",
            document_key: "apply-escaped-key-python",
            source: "{\"old key\": \"value\"}",
            old: "old key",
            replacement: r#"new\\key"#,
            expected_source: "{\"new\\\\key\": \"value\"}",
        },
        EscapedKeyCase {
            language: "javascript",
            document_key: "apply-escaped-key-javascript",
            source: "({\"old key\": \"value\"})",
            old: "old key",
            replacement: r#"new\\key"#,
            expected_source: "({\"new\\\\key\": \"value\"})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} escaped key editor edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_table_without_header_value_matches_editor_array_cases() {
    let _guard = lock_test_mutex();

    struct HeaderlessCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        HeaderlessCase {
            language: "json",
            document_key: "apply-headerless-json",
            source: r#"{"table_without_header":["a","b","c"]}"#,
            old: "\"b\"",
            replacement: "\"beta\"",
            expected_source: r#"{"table_without_header":["a","beta","c"]}"#,
        },
        HeaderlessCase {
            language: "yaml",
            document_key: "apply-headerless-yaml",
            source: "table_without_header:\n  - a\n  - b\n  - c\n",
            old: "  - b\n",
            replacement: "  - beta\n",
            expected_source: "table_without_header:\n  - a\n  - beta\n  - c\n",
        },
        HeaderlessCase {
            language: "toml",
            document_key: "apply-headerless-toml",
            source: "table_without_header = [ \"a\", \"b\", \"c\" ]\n",
            old: "\"b\"",
            replacement: "\"beta\"",
            expected_source: "table_without_header = [ \"a\", \"beta\", \"c\" ]\n",
        },
        HeaderlessCase {
            language: "python",
            document_key: "apply-headerless-python",
            source: "{'table_without_header': ['a', 'b', 'c']}",
            old: "'b'",
            replacement: "'beta'",
            expected_source: "{'table_without_header': ['a', 'beta', 'c']}",
        },
        HeaderlessCase {
            language: "javascript",
            document_key: "apply-headerless-javascript",
            source: "({table_without_header: ['a', 'b', 'c']})",
            old: "'b'",
            replacement: "\"beta\"",
            expected_source: "({table_without_header: ['a', \"beta\", 'c']})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} headerless array editor edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_sequence_subtree_replace_matches_editor_array_node_cases() {
    let _guard = lock_test_mutex();

    struct SequenceCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        SequenceCase {
            language: "json",
            document_key: "apply-seq-json",
            source: r#"{"items":[1,2],"count":1}"#,
            old: "[1,2]",
            replacement: "[1,2,3]",
            expected_source: r#"{"items":[1,2,3],"count":1}"#,
        },
        SequenceCase {
            language: "yaml",
            document_key: "apply-seq-yaml",
            source: "items:\n  - 1\n  - 2\ncount: 1\n",
            old: "items:\n  - 1\n  - 2\n",
            replacement: "items:\n  - 1\n  - 2\n  - 3\n",
            expected_source: "items:\n  - 1\n  - 2\n  - 3\ncount: 1\n",
        },
        SequenceCase {
            language: "toml",
            document_key: "apply-seq-toml",
            source: "items = [1, 2]\ncount = 1\n",
            old: "[1, 2]",
            replacement: "[1, 2, 3]",
            expected_source: "items = [1, 2, 3]\ncount = 1\n",
        },
        SequenceCase {
            language: "python",
            document_key: "apply-seq-python",
            source: "{'items': [1, 2], 'count': 1}",
            old: "[1, 2]",
            replacement: "[1, 2, 3]",
            expected_source: "{'items': [1, 2, 3], 'count': 1}",
        },
        SequenceCase {
            language: "javascript",
            document_key: "apply-seq-javascript",
            source: "({items: [1, 2], count: 1})",
            old: "[1, 2]",
            replacement: "[1, 2, 3]",
            expected_source: "({items: [1, 2, 3], count: 1})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} sequence subtree editor edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_table_row_key_rename_matches_editor_table_object_cases() {
    let _guard = lock_test_mutex();

    struct RowKeyCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        RowKeyCase {
            language: "json",
            document_key: "apply-row-key-json",
            source: r#"{"table_with_header":[{"h1":11,"h2":12},{"h1":21,"h2":22}]}"#,
            old: "\"h1\"",
            replacement: "\"primary\"",
            expected_source: r#"{"table_with_header":[{"primary":11,"h2":12},{"h1":21,"h2":22}]}"#,
        },
        RowKeyCase {
            language: "yaml",
            document_key: "apply-row-key-yaml",
            source: "table_with_header:\n  - h1: 11\n    h2: 12\n  - h1: 21\n    h2: 22\n",
            old: "h1",
            replacement: "primary",
            expected_source: "table_with_header:\n  - primary: 11\n    h2: 12\n  - h1: 21\n    h2: 22\n",
        },
        RowKeyCase {
            language: "toml",
            document_key: "apply-row-key-toml",
            source: "[[table_with_header]]\nh1 = 11\nh2 = 12\n\n[[table_with_header]]\nh1 = 21\nh2 = 22\n",
            old: "h1",
            replacement: "primary",
            expected_source: "[[table_with_header]]\nprimary = 11\nh2 = 12\n\n[[table_with_header]]\nh1 = 21\nh2 = 22\n",
        },
        RowKeyCase {
            language: "python",
            document_key: "apply-row-key-python",
            source: "{'table_with_header': [{'h1': 11, 'h2': 12}, {'h1': 21, 'h2': 22}]}",
            old: "h1",
            replacement: "primary",
            expected_source: "{'table_with_header': [{'primary': 11, 'h2': 12}, {'h1': 21, 'h2': 22}]}",
        },
        RowKeyCase {
            language: "javascript",
            document_key: "apply-row-key-javascript",
            source: "({table_with_header: [{h1: 11, h2: 12}, {h1: 21, h2: 22}]})",
            old: "h1",
            replacement: "primary",
            expected_source: "({table_with_header: [{primary: 11, h2: 12}, {h1: 21, h2: 22}]})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} row key editor edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_apply_edits_imprecise_scalar_span_matches_editor_rebuild_cases() {
    let _guard = lock_test_mutex();

    let cases = [
        (
            "json",
            "apply-imprecise-json",
            r#"{"name":"Alice","role":"admin","count":1}"#,
            r#""name":"Alice""#,
            r#""name":"Bob""#,
            r#"{"name":"Bob","role":"admin","count":1}"#,
        ),
        (
            "yaml",
            "apply-imprecise-yaml",
            "name: Alice\nrole: admin\ncount: 1\n",
            "name: Alice",
            "name: Bob",
            "name: Bob\nrole: admin\ncount: 1\n",
        ),
    ];

    for (language, document_key, source, old, replacement, expected_source) in cases {
        reset_test_state();
        let (base_snapshot_id, _) = analyze_document_via_job(document_key, language, &[source]);
        let started = start_apply_job(
            document_key,
            language,
            base_snapshot_id,
            vec![replace_edit(source, old, replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{language} imprecise span editor edit should complete",
        );
        assert_snapshot_source(document_key, expected_source);
    }
}

#[test]
fn wasm_document_plan_graph_key_edit_reports_snapshot_not_ready_without_snapshot() {
    let _guard = lock_test_mutex();

    let cases = [
        ("json", "missing-key-snapshot-json", vec![key_seg("oldKey")]),
        ("yaml", "missing-key-snapshot-yaml", vec![key_seg("oldKey")]),
        (
            "toml",
            "missing-key-snapshot-toml",
            vec![key_seg("old_key")],
        ),
        (
            "csv",
            "missing-key-snapshot-csv",
            vec![index_seg(0), key_seg("name")],
        ),
        (
            "python",
            "missing-key-snapshot-python",
            vec![key_seg("oldKey")],
        ),
        (
            "javascript",
            "missing-key-snapshot-javascript",
            vec![key_seg("oldKey")],
        ),
    ];

    for (language, document_key, path) in cases {
        reset_test_state();
        let planned = plan_graph_value_edit_impl(GraphValueEditRequest {
            document_key: document_key.to_owned(),
            snapshot_id: SnapshotId(777),
            language: language.to_owned(),
            path,
            prefer_key: true,
            raw_replacement: None,
            value: scalar_edit_value("renamedKey"),
        })
        .expect("planner should return status");

        assert!(
            matches!(planned, SnapshotReadResult::SnapshotNotReady),
            "{language} missing key snapshot should report snapshotNotReady",
        );
    }
}

#[test]
fn wasm_document_plan_graph_key_edit_reports_invalid_path_for_missing_node() {
    let _guard = lock_test_mutex();

    struct InvalidKeyCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
    }

    let cases = [
        InvalidKeyCase {
            language: "json",
            document_key: "invalid-key-path-json",
            source: r#"{"oldKey":"value"}"#,
            path: vec![key_seg("missingKey")],
        },
        InvalidKeyCase {
            language: "yaml",
            document_key: "invalid-key-path-yaml",
            source: "oldKey: value\n",
            path: vec![key_seg("missingKey")],
        },
        InvalidKeyCase {
            language: "csv",
            document_key: "invalid-key-path-csv",
            source: "name,age\nold,1\n",
            path: vec![index_seg(0), key_seg("missing")],
        },
        InvalidKeyCase {
            language: "toml",
            document_key: "invalid-key-path-toml",
            source: "old_key = \"value\"\n",
            path: vec![key_seg("missing_key")],
        },
        InvalidKeyCase {
            language: "python",
            document_key: "invalid-key-path-python",
            source: "{\"oldKey\": \"value\"}",
            path: vec![key_seg("missingKey")],
        },
        InvalidKeyCase {
            language: "javascript",
            document_key: "invalid-key-path-javascript",
            source: "({oldKey: \"value\"})",
            path: vec![key_seg("missingKey")],
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
            prefer_key: true,
            raw_replacement: None,
            value: scalar_edit_value("renamedKey"),
        })
        .expect("planner should execute");

        let plan = match planned {
            SnapshotReadResult::Ready { data } => data,
            SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
        };
        assert_eq!(
            plan.mode,
            GraphValueEditPlanMode::Replace,
            "{}",
            case.language
        );
        assert_eq!(
            plan.reason,
            Some(GraphValueEditFallbackReason::InvalidPath),
            "{} invalid key path should report invalidPath",
            case.language,
        );
        assert!(
            plan.edits.is_empty(),
            "{} invalid key path should not emit direct edits",
            case.language,
        );
    }
}

#[test]
fn wasm_document_plan_then_apply_table_row_key_edit_matches_supported_round_trip() {
    let _guard = lock_test_mutex();

    struct RowKeyPlanCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_key: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        RowKeyPlanCase {
            language: "json",
            document_key: "plan-row-key-json",
            source: r#"{"table_with_header":[{"h1":11,"h2":12},{"h1":21,"h2":22}]}"#,
            path: vec![key_seg("table_with_header"), index_seg(0), key_seg("h1")],
            next_key: "primary",
            expected_source: r#"{"table_with_header":[{"primary":11,"h2":12},{"h1":21,"h2":22}]}"#,
        },
        RowKeyPlanCase {
            language: "yaml",
            document_key: "plan-row-key-yaml",
            source: "table_with_header:\n  - h1: 11\n    h2: 12\n  - h1: 21\n    h2: 22\n",
            path: vec![key_seg("table_with_header"), index_seg(0), key_seg("h1")],
            next_key: "primary",
            expected_source: "table_with_header:\n  - 'primary': 11\n    h2: 12\n  - h1: 21\n    h2: 22\n",
        },
        RowKeyPlanCase {
            language: "toml",
            document_key: "plan-row-key-toml",
            source: "[[table_with_header]]\nh1 = 11\nh2 = 12\n\n[[table_with_header]]\nh1 = 21\nh2 = 22\n",
            path: vec![key_seg("table_with_header"), index_seg(0), key_seg("h1")],
            next_key: "primary",
            expected_source: "[[table_with_header]]\nprimary = 11\nh2 = 12\n\n[[table_with_header]]\nh1 = 21\nh2 = 22\n",
        },
        RowKeyPlanCase {
            language: "python",
            document_key: "plan-row-key-python",
            source: "{'table_with_header': [{'h1': 11, 'h2': 12}, {'h1': 21, 'h2': 22}]}",
            path: vec![key_seg("table_with_header"), index_seg(0), key_seg("h1")],
            next_key: "primary",
            expected_source: "{'table_with_header': [{'primary': 11, 'h2': 12}, {'h1': 21, 'h2': 22}]}",
        },
        RowKeyPlanCase {
            language: "javascript",
            document_key: "plan-row-key-javascript",
            source: "({table_with_header: [{h1: 11, h2: 12}, {h1: 21, h2: 22}]})",
            path: vec![key_seg("table_with_header"), index_seg(0), key_seg("h1")],
            next_key: "primary",
            expected_source: "({table_with_header: [{primary: 11, h2: 12}, {h1: 21, h2: 22}]})",
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
            prefer_key: true,
            raw_replacement: None,
            value: scalar_edit_value(case.next_key),
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
            "{} row key planner round-trip should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_plan_then_apply_escaped_value_edits_match_round_trip() {
    let _guard = lock_test_mutex();

    struct EscapedValueCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_value: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        EscapedValueCase {
            language: "json",
            document_key: "escaped-value-json",
            source: r#"{"message":"hello"}"#,
            path: vec![key_seg("message")],
            next_value: "quote \"slash\\path\"",
            expected_source: r#"{"message":"quote \"slash\\path\""}"#,
        },
        EscapedValueCase {
            language: "yaml",
            document_key: "escaped-value-yaml",
            source: "message: hello\n",
            path: vec![key_seg("message")],
            next_value: "quote \"slash\\path\"",
            expected_source: "message: 'quote \"slash\\path\"'\n",
        },
        EscapedValueCase {
            language: "toml",
            document_key: "escaped-value-toml",
            source: "message = \"hello\"\n",
            path: vec![key_seg("message")],
            next_value: "quote \"slash\\path\"",
            expected_source: "message = \"quote \\\"slash\\\\path\\\"\"\n",
        },
        EscapedValueCase {
            language: "python",
            document_key: "escaped-value-python",
            source: "{\"message\": \"hello\"}",
            path: vec![key_seg("message")],
            next_value: "quote \"slash\\path\"",
            expected_source: "{\"message\": 'quote \"slash\\\\path\"'}",
        },
        EscapedValueCase {
            language: "javascript",
            document_key: "escaped-value-javascript",
            source: "({message: \"hello\"})",
            path: vec![key_seg("message")],
            next_value: "quote \"slash\\path\"",
            expected_source: "({message: \"quote \\\"slash\\\\path\\\"\"})",
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
            "{} escaped value planner round-trip should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_plan_then_apply_multiline_value_edits_match_round_trip() {
    let _guard = lock_test_mutex();

    struct MultilineCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        path: Vec<GraphPathSeg>,
        next_value: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        MultilineCase {
            language: "json",
            document_key: "multiline-json",
            source: r#"{"message":"hello"}"#,
            path: vec![key_seg("message")],
            next_value: "line1\nline2",
            expected_source: "{\"message\":\"line1\\nline2\"}",
        },
        MultilineCase {
            language: "python",
            document_key: "multiline-python",
            source: "{\"message\": \"hello\"}",
            path: vec![key_seg("message")],
            next_value: "line1\nline2",
            expected_source: "{\"message\": 'line1\\nline2'}",
        },
        MultilineCase {
            language: "javascript",
            document_key: "multiline-javascript",
            source: "({message: \"hello\"})",
            path: vec![key_seg("message")],
            next_value: "line1\nline2",
            expected_source: "({message: \"line1\\nline2\"})",
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
            "{} multiline value planner round-trip should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}

#[test]
fn wasm_document_plan_yaml_anchor_and_alias_cases_cover_round_trip_and_fallback() {
    let _guard = lock_test_mutex();

    reset_test_state();
    let anchor_source = read_repo_fixture("test/fixtures/yaml/spec-example-7-1-alias-nodes.1.yaml");
    let (anchor_snapshot_id, _) =
        analyze_document_via_job("yaml-anchor-round-trip", "yaml", &[&anchor_source]);
    let anchor_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "yaml-anchor-round-trip".to_owned(),
        snapshot_id: anchor_snapshot_id,
        language: "yaml".to_owned(),
        path: vec![key_seg("First occurrence")],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("Bar"),
    })
    .expect("planner should execute");
    let anchor_plan = match anchor_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(anchor_plan.mode, GraphValueEditPlanMode::Edits);
    let started = start_apply_job(
        "yaml-anchor-round-trip",
        "yaml",
        anchor_snapshot_id,
        anchor_plan.edits,
    );
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source(
        "yaml-anchor-round-trip",
        "First occurrence: &anchor 'Bar'\nSecond occurrence: *anchor\nOverride anchor: &anchor Bar\nReuse anchor: *anchor\n",
    );

    reset_test_state();
    let alias_source = read_repo_fixture("test/fixtures/yaml/spec-example-7-1-alias-nodes.1.yaml");
    let (alias_snapshot_id, _) =
        analyze_document_via_job("yaml-alias-fallback", "yaml", &[&alias_source]);
    let alias_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "yaml-alias-fallback".to_owned(),
        snapshot_id: alias_snapshot_id,
        language: "yaml".to_owned(),
        path: vec![key_seg("Second occurrence")],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("Baz"),
    })
    .expect("planner should execute");
    let alias_plan = match alias_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(alias_plan.mode, GraphValueEditPlanMode::Replace);
    assert!(alias_plan.reason.is_some());
    assert!(alias_plan.edits.is_empty());

    reset_test_state();
    let keyed_anchor_source = read_repo_fixture("test/fixtures/yaml/anchors-in-mapping.1.yaml");
    let (keyed_anchor_snapshot_id, _) = analyze_document_via_job(
        "yaml-keyed-anchor-round-trip",
        "yaml",
        &[&keyed_anchor_source],
    );
    let keyed_anchor_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "yaml-keyed-anchor-round-trip".to_owned(),
        snapshot_id: keyed_anchor_snapshot_id,
        language: "yaml".to_owned(),
        path: vec![key_seg("c")],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("done"),
    })
    .expect("planner should execute");
    let keyed_anchor_plan = match keyed_anchor_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(keyed_anchor_plan.mode, GraphValueEditPlanMode::Edits);
    let started = start_apply_job(
        "yaml-keyed-anchor-round-trip",
        "yaml",
        keyed_anchor_snapshot_id,
        keyed_anchor_plan.edits,
    );
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source("yaml-keyed-anchor-round-trip", "&a a: b\nc: &d 'done'\n");

    reset_test_state();
    let flow_nodes_source =
        read_repo_fixture("test/fixtures/yaml-rare/spec-example-7-24-flow-nodes.1.yaml");
    let (flow_nodes_snapshot_id, _) =
        analyze_document_via_job("yaml-flow-nodes-round-trip", "yaml", &[&flow_nodes_source]);
    let flow_nodes_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "yaml-flow-nodes-round-trip".to_owned(),
        snapshot_id: flow_nodes_snapshot_id,
        language: "yaml".to_owned(),
        path: vec![index_seg(1)],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("beta"),
    })
    .expect("planner should execute");
    let flow_nodes_plan = match flow_nodes_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(flow_nodes_plan.mode, GraphValueEditPlanMode::Edits);
    let started = start_apply_job(
        "yaml-flow-nodes-round-trip",
        "yaml",
        flow_nodes_snapshot_id,
        flow_nodes_plan.edits,
    );
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source(
        "yaml-flow-nodes-round-trip",
        "- !!str \"a\"\n- 'beta'\n- &anchor \"c\"\n- *anchor\n- !!str\n",
    );

    reset_test_state();
    let anchors_tags_source = read_repo_fixture("test/fixtures/yaml-rare/anchors-and-tags.1.yaml");
    let (anchors_tags_snapshot_id, _) = analyze_document_via_job(
        "yaml-anchors-tags-round-trip",
        "yaml",
        &[&anchors_tags_source],
    );
    let anchors_tags_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "yaml-anchors-tags-round-trip".to_owned(),
        snapshot_id: anchors_tags_snapshot_id,
        language: "yaml".to_owned(),
        path: vec![index_seg(3)],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("echo"),
    })
    .expect("planner should execute");
    let anchors_tags_plan = match anchors_tags_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(anchors_tags_plan.mode, GraphValueEditPlanMode::Edits);
    let started = start_apply_job(
        "yaml-anchors-tags-round-trip",
        "yaml",
        anchors_tags_snapshot_id,
        anchors_tags_plan.edits,
    );
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source(
        "yaml-anchors-tags-round-trip",
        " - &a !!str a\n - !!int 2\n - !!int &c 4\n - &d 'echo'\n",
    );
}

#[test]
fn wasm_document_plan_toml_rare_structure_cases_cover_array_table_and_inline_table() {
    let _guard = lock_test_mutex();

    reset_test_state();
    let array_table_source =
        read_repo_fixture("test/fixtures/toml/table__array-table-array.1.toml");
    let (array_table_snapshot_id, _) =
        analyze_document_via_job("toml-array-table", "toml", &[&array_table_source]);
    let array_table_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "toml-array-table".to_owned(),
        snapshot_id: array_table_snapshot_id,
        language: "toml".to_owned(),
        path: vec![
            key_seg("a"),
            index_seg(0),
            key_seg("b"),
            index_seg(1),
            key_seg("c"),
            key_seg("d"),
        ],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("done"),
    })
    .expect("planner should execute");
    let array_table_plan = match array_table_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(array_table_plan.mode, GraphValueEditPlanMode::Edits);
    let started = start_apply_job(
        "toml-array-table",
        "toml",
        array_table_snapshot_id,
        array_table_plan.edits,
    );
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source(
        "toml-array-table",
        "[[a]]\n    [[a.b]]\n        [a.b.c]\n            d = \"val0\"\n    [[a.b]]\n        [a.b.c]\n            d = \"done\"\n",
    );

    reset_test_state();
    let inline_source = read_repo_fixture("test/fixtures/toml/inline-table__nest.1.toml");

    reset_test_state();
    let quoted_unicode_source = read_repo_fixture("test/fixtures/toml/key__quoted-unicode.1.toml");
    let (quoted_unicode_snapshot_id, _) =
        analyze_document_via_job("toml-quoted-unicode-key", "toml", &[&quoted_unicode_source]);
    let quoted_unicode_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "toml-quoted-unicode-key".to_owned(),
        snapshot_id: quoted_unicode_snapshot_id,
        language: "toml".to_owned(),
        path: vec![key_seg("~  ÿ ퟿ \u{e000} \u{ffff} 𐀀 􏿿")],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("updated basic key"),
    })
    .expect("planner should execute");
    let quoted_unicode_plan = match quoted_unicode_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(quoted_unicode_plan.mode, GraphValueEditPlanMode::Edits);
    let started = start_apply_job(
        "toml-quoted-unicode-key",
        "toml",
        quoted_unicode_snapshot_id,
        quoted_unicode_plan.edits,
    );
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source(
        "toml-quoted-unicode-key",
        "\n\"\\u0000\" = \"null\"\n'\\u0000' = \"different key\"\n\"\\u0008 \\u000c \\U00000041 \\u007f \\u0080 \\u00ff \\ud7ff \\ue000 \\uffff \\U00010000 \\U0010ffff\" = \"escaped key\"\n\n\"~  ÿ ퟿  ￿ 𐀀 􏿿\" = \"updated basic key\"\n'l ~  ÿ ퟿  ￿ 𐀀 􏿿' = \"literal key\"\n",
    );
    let (inline_snapshot_id, _) =
        analyze_document_via_job("toml-inline-table", "toml", &[&inline_source]);
    let inline_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "toml-inline-table".to_owned(),
        snapshot_id: inline_snapshot_id,
        language: "toml".to_owned(),
        path: vec![key_seg("tbl_tbl_val"), key_seg("tbl_1"), key_seg("one")],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("2"),
    })
    .expect("planner should execute");
    let inline_plan = match inline_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(inline_plan.mode, GraphValueEditPlanMode::Edits);
    let started = start_apply_job(
        "toml-inline-table",
        "toml",
        inline_snapshot_id,
        inline_plan.edits,
    );
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source(
        "toml-inline-table",
        "tbl_tbl_empty = { tbl_0 = {} }\ntbl_tbl_val   = { tbl_1 = { one = 2 } }\ntbl_arr_tbl   = { arr_tbl = [ { one = 1 } ] }\narr_tbl_tbl   = [ { tbl = { one = 1 } } ]\n\n# Array-of-array-of-table is interesting because it can only\n# be represented in inline form.\narr_arr_tbl_empty = [ [ {} ] ]\narr_arr_tbl_val = [ [ { one = 1 } ] ]\narr_arr_tbls  = [ [ { one = 1 }, { two = 2 } ] ]\n",
    );

    reset_test_state();
    let quoted_dots_source = read_repo_fixture("test/fixtures/toml/key__quoted-dots.1.toml");
    let (quoted_dots_snapshot_id, _) =
        analyze_document_via_job("toml-quoted-dots", "toml", &[&quoted_dots_source]);
    let quoted_dots_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "toml-quoted-dots".to_owned(),
        snapshot_id: quoted_dots_snapshot_id,
        language: "toml".to_owned(),
        path: vec![key_seg("plain_table"), key_seg("with.dot")],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("40"),
    })
    .expect("planner should execute");
    let quoted_dots_plan = match quoted_dots_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(quoted_dots_plan.mode, GraphValueEditPlanMode::Edits);
    let started = start_apply_job(
        "toml-quoted-dots",
        "toml",
        quoted_dots_snapshot_id,
        quoted_dots_plan.edits,
    );
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source(
        "toml-quoted-dots",
        "plain = 1\n\"with.dot\" = 2\n\n[plain_table]\nplain = 3\n\"with.dot\" = 40\n\n[table.withdot]\nplain = 5\n\"key.with.dots\" = 6\n\"escaped\\u002edot\" = 7\n",
    );

    reset_test_state();
    let array_within_dotted_source =
        read_repo_fixture("test/fixtures/toml/table__array-within-dotted.1.toml");
    let (array_within_dotted_snapshot_id, _) = analyze_document_via_job(
        "toml-array-within-dotted",
        "toml",
        &[&array_within_dotted_source],
    );
    let array_within_dotted_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "toml-array-within-dotted".to_owned(),
        snapshot_id: array_within_dotted_snapshot_id,
        language: "toml".to_owned(),
        path: vec![
            key_seg("fruit"),
            key_seg("apple"),
            key_seg("seeds"),
            index_seg(0),
            key_seg("size"),
        ],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("3"),
    })
    .expect("planner should execute");
    let array_within_dotted_plan = match array_within_dotted_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(array_within_dotted_plan.mode, GraphValueEditPlanMode::Edits);
    let started = start_apply_job(
        "toml-array-within-dotted",
        "toml",
        array_within_dotted_snapshot_id,
        array_within_dotted_plan.edits,
    );
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source(
        "toml-array-within-dotted",
        "[fruit]\napple.color = \"red\"\n\n[[fruit.apple.seeds]]\nsize = 3\n",
    );

    reset_test_state();
    let key_escapes_source = read_repo_fixture("test/fixtures/toml/key__escapes.1.toml");
    let (key_escapes_snapshot_id, _) =
        analyze_document_via_job("toml-key-escapes", "toml", &[&key_escapes_source]);
    let key_escapes_plan = plan_graph_value_edit_impl(GraphValueEditRequest {
        document_key: "toml-key-escapes".to_owned(),
        snapshot_id: key_escapes_snapshot_id,
        language: "toml".to_owned(),
        path: vec![key_seg("\"quoted\""), key_seg("quote")],
        prefer_key: false,
        raw_replacement: None,
        value: scalar_edit_value("false"),
    })
    .expect("planner should execute");
    let key_escapes_plan = match key_escapes_plan {
        SnapshotReadResult::Ready { data } => data,
        SnapshotReadResult::SnapshotNotReady => panic!("snapshot should be ready"),
    };
    assert_eq!(key_escapes_plan.mode, GraphValueEditPlanMode::Edits);
    let started = start_apply_job(
        "toml-key-escapes",
        "toml",
        key_escapes_snapshot_id,
        key_escapes_plan.edits,
    );
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source(
        "toml-key-escapes",
        "\"\\n\" = \"newline\"\n\"\\b\" = \"bell\"\n\"\\u00c0\" = \"latin capital letter A with grave\"\n\"\\\"\" = \"just a quote\"\n\n[\"backsp\\b\\b\"]\n\n[\"\\\"quoted\\\"\"]\nquote = false\n\n[\"a.b\".\"\\u00c0\"]\n",
    );
}

#[test]
fn wasm_document_apply_edits_table_row_subtree_replace_matches_editor_row_node_cases() {
    let _guard = lock_test_mutex();

    struct RowReplaceCase {
        language: &'static str,
        document_key: &'static str,
        source: &'static str,
        old: &'static str,
        replacement: &'static str,
        expected_source: &'static str,
    }

    let cases = [
        RowReplaceCase {
            language: "json",
            document_key: "apply-row-subtree-json",
            source: r#"{"table_with_header":[{"h1":11,"h2":12},{"h1":21,"h2":22}]}"#,
            old: r#"{"h1":11,"h2":12}"#,
            replacement: r#"{"h1":111,"h2":112,"status":"ready"}"#,
            expected_source: r#"{"table_with_header":[{"h1":111,"h2":112,"status":"ready"},{"h1":21,"h2":22}]}"#,
        },
        RowReplaceCase {
            language: "yaml",
            document_key: "apply-row-subtree-yaml",
            source: "table_with_header:\n  - h1: 11\n    h2: 12\n  - h1: 21\n    h2: 22\n",
            old: "  - h1: 11\n    h2: 12\n",
            replacement: "  - h1: 111\n    h2: 112\n    status: ready\n",
            expected_source: "table_with_header:\n  - h1: 111\n    h2: 112\n    status: ready\n  - h1: 21\n    h2: 22\n",
        },
        RowReplaceCase {
            language: "toml",
            document_key: "apply-row-subtree-toml",
            source: "[[table_with_header]]\nh1 = 11\nh2 = 12\n\n[[table_with_header]]\nh1 = 21\nh2 = 22\n",
            old: "[[table_with_header]]\nh1 = 11\nh2 = 12\n",
            replacement: "[[table_with_header]]\nh1 = 111\nh2 = 112\nstatus = \"ready\"\n",
            expected_source: "[[table_with_header]]\nh1 = 111\nh2 = 112\nstatus = \"ready\"\n\n[[table_with_header]]\nh1 = 21\nh2 = 22\n",
        },
        RowReplaceCase {
            language: "python",
            document_key: "apply-row-subtree-python",
            source: "{'table_with_header': [{'h1': 11, 'h2': 12}, {'h1': 21, 'h2': 22}]}",
            old: "{'h1': 11, 'h2': 12}",
            replacement: "{'h1': 111, 'h2': 112, 'status': 'ready'}",
            expected_source: "{'table_with_header': [{'h1': 111, 'h2': 112, 'status': 'ready'}, {'h1': 21, 'h2': 22}]}",
        },
        RowReplaceCase {
            language: "javascript",
            document_key: "apply-row-subtree-javascript",
            source: "({table_with_header: [{h1: 11, h2: 12}, {h1: 21, h2: 22}]})",
            old: "{h1: 11, h2: 12}",
            replacement: "{h1: 111, h2: 112, status: \"ready\"}",
            expected_source: "({table_with_header: [{h1: 111, h2: 112, status: \"ready\"}, {h1: 21, h2: 22}]})",
        },
    ];

    for case in cases {
        reset_test_state();
        let (base_snapshot_id, _) =
            analyze_document_via_job(case.document_key, case.language, &[case.source]);
        let started = start_apply_job(
            case.document_key,
            case.language,
            base_snapshot_id,
            vec![replace_edit(case.source, case.old, case.replacement)],
        );
        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{} row subtree editor edit should complete",
            case.language
        );
        assert_snapshot_source(case.document_key, case.expected_source);
    }
}
