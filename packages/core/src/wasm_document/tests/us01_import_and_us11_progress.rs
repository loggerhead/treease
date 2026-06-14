use super::*;

#[test]
fn wasm_document_json_streaming_matches_web_graph_render_flow() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = r#"{"user":{"name":"Alice"},"count":42}"#;
    let split = source
        .find(r#""Alice""#)
        .expect("fixture should contain string")
        + 3;
    let started = start_source_job("json-stream", "json");

    let first = text_chunk(started.job_handle, &source[..split]);
    assert!(
        first
            .events
            .iter()
            .any(|event| matches!(event, DocumentEvent::ProjectionDelta { clear: true, .. })),
        "first JSON chunk should emit clear ProjectionDelta",
    );
    assert!(
        first
            .events
            .iter()
            .any(|event| matches!(event, DocumentEvent::Progress { .. })),
        "first JSON chunk should emit progress",
    );

    let second = text_chunk(started.job_handle, &source[split..]);
    assert!(
        second
            .events
            .iter()
            .any(|event| matches!(event, DocumentEvent::ProjectionDelta { clear: false, .. })),
        "second JSON chunk should emit incremental ProjectionDelta",
    );

    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    let snapshot_id = snapshot_id_from_batch(&close_batch);
    let snapshot = stored_snapshot_for_document("json-stream").expect("snapshot should be stored");
    let analysis = snapshot
        .analysis
        .as_ref()
        .expect("analysis should be stored");
    let incremental = snapshot
        .incremental
        .as_ref()
        .expect("incremental state should be stored");
    assert_eq!(snapshot.snapshot_id, snapshot_id);
    assert_eq!(analysis.source, source);
    assert_eq!(analysis.value_json, source);
    assert!(analysis.document.is_some());
    assert!(analysis.ts_tree.is_none());
    assert!(incremental.can_resume);
    assert!(incremental.graph_model_snapshot.is_some());
    assert_eq!(document_runtime_job_count_for_tests(), 0);
}

#[test]
fn wasm_document_non_streaming_languages_only_materialize_on_close_like_web_worker() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let cases = [
        (
            "yaml",
            "yaml-non-stream",
            "root:\n",
            "  name: Ada\n",
            "root:\n  name: Ada\n",
        ),
        (
            "toml",
            "toml-non-stream",
            "name = ",
            "\"Ada\"\n",
            "name = \"Ada\"\n",
        ),
        (
            "csv",
            "csv-non-stream",
            "name,age\n",
            "Ada,37\n",
            "name,age\nAda,37\n",
        ),
        (
            "python",
            "python-non-stream",
            "{\"name\": ",
            "\"Ada\"}",
            "{\"name\": \"Ada\"}",
        ),
        (
            "javascript",
            "javascript-non-stream",
            "({name: ",
            "\"Ada\"})",
            "({name: \"Ada\"})",
        ),
    ];

    for (language, document_key, first_chunk, second_chunk, expected_source) in cases {
        reset_test_state();
        let started = start_source_job(document_key, language);

        let first = text_chunk(started.job_handle, first_chunk);
        assert!(
            first.events.is_empty(),
            "{language} should buffer first chunk"
        );
        assert!(
            first.terminal.is_none(),
            "{language} first chunk should keep job open"
        );

        let second = text_chunk(started.job_handle, second_chunk);
        assert!(
            second.events.is_empty(),
            "{language} should buffer second chunk"
        );
        assert!(
            second.terminal.is_none(),
            "{language} second chunk should keep job open"
        );

        let close_batch = close(started.job_handle);
        assert!(
            matches!(close_batch.terminal, Some(JobTerminal::Completed)),
            "{language} close should complete",
        );
        assert!(
            close_batch
                .events
                .iter()
                .any(|event| matches!(event, DocumentEvent::SnapshotReady { .. })),
            "{language} close should emit snapshot",
        );
        let snapshot =
            stored_snapshot_for_document(document_key).expect("snapshot should be stored");
        assert_eq!(
            snapshot
                .analysis
                .as_ref()
                .map(|analysis| analysis.source.as_str()),
            Some(expected_source),
            "{language} should materialize full source on close",
        );
        assert_eq!(
            document_runtime_job_count_for_tests(),
            0,
            "{language} should not leak jobs"
        );
    }
}

#[test]
fn wasm_document_json_streaming_parse_failed_matches_web_diagnostics_flow() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let started = start_source_job("json-parse-failed", "json");
    let chunk = text_chunk(started.job_handle, r#"{"broken":"#);
    assert!(
        chunk.terminal.is_none(),
        "streaming chunk should keep job open"
    );

    let close_batch = close(started.job_handle);
    assert!(matches!(
        close_batch.terminal,
        Some(JobTerminal::ParseFailed)
    ));
    let snapshot = stored_snapshot_for_document("json-parse-failed")
        .expect("diagnostics snapshot should be stored");
    let analysis = snapshot.analysis.as_ref().expect("analysis should exist");
    assert!(
        close_batch
            .events
            .iter()
            .any(|event| matches!(event, DocumentEvent::ParseFailed { .. })),
        "close should emit ParseFailed event",
    );
    assert!(
        !analysis.diagnostics.is_empty(),
        "parse failure should keep diagnostics"
    );
    assert!(
        analysis.document.is_none(),
        "diagnostics snapshot should not keep decoded document"
    );
    assert!(
        snapshot.graph.is_none(),
        "diagnostics snapshot should clear graph"
    );
}

#[test]
fn wasm_document_json_streaming_boundary_splits_cover_escape_and_utf8_cases() {
    let _guard = lock_test_mutex();

    reset_test_state();
    let escaped = read_repo_fixture("test/fixtures/json/pass01.1.json");
    assert_streaming_fixture_round_trip(
        "json",
        "json-boundary-escape-sequence",
        &escaped,
        split_after_needle_fragment(&escaped, "\\u0123", 3),
    );

    reset_test_state();
    let utf8 = read_repo_fixture("test/fixtures/json/minefield__y_string_utf8.1.json");
    assert_streaming_fixture_round_trip(
        "json",
        "json-boundary-utf8-string",
        &utf8,
        split_before_needle(&utf8, "𝄞"),
    );
}

#[test]
fn wasm_document_json_streaming_boundary_splits_expand_across_valid_fixtures() {
    let _guard = lock_test_mutex();

    let cases = [
        ("json-boundary-pass01", "test/fixtures/json/pass01.1.json"),
        (
            "json-boundary-utf8-fixture",
            "test/fixtures/json/minefield__y_string_utf8.1.json",
        ),
        (
            "json-boundary-big-object",
            "test/fixtures/json/big-object-node.1.json",
        ),
        (
            "json-boundary-large-table",
            "test/fixtures/json/large_headerless_table.1.json",
        ),
        ("json-boundary-1mb-fixture", "test/fixtures/json/1mb.1.json"),
    ];

    for (document_key, path) in cases {
        reset_test_state();
        let source = read_repo_fixture(path);
        assert_streaming_fixture_round_trip(
            "json",
            document_key,
            &source,
            split_inside_first_json_string(&source),
        );
    }
}

#[test]
fn wasm_document_json_streaming_featureful_boundary_splits_expand_across_valid_fixtures() {
    let _guard = lock_test_mutex();

    let cases = [
        ("json-feature-pass01", "test/fixtures/json/pass01.1.json"),
        (
            "json-feature-utf8-fixture",
            "test/fixtures/json/minefield__y_string_utf8.1.json",
        ),
        (
            "json-feature-big-object",
            "test/fixtures/json/big-object-node.1.json",
        ),
        (
            "json-feature-large-table",
            "test/fixtures/json/large_headerless_table.1.json",
        ),
        ("json-feature-1mb-fixture", "test/fixtures/json/1mb.1.json"),
    ];

    for (document_key, path) in cases {
        reset_test_state();
        let source = read_repo_fixture(path);
        assert_streaming_fixture_round_trip(
            "json",
            document_key,
            &source,
            split_inside_featureful_json_boundary(&source),
        );
    }
}
#[test]
fn wasm_document_json_streaming_escape_or_string_boundary_splits_expand_across_valid_fixtures() {
    let _guard = lock_test_mutex();

    let cases = [
        ("json-escape-pass01", "test/fixtures/json/pass01.1.json"),
        (
            "json-escape-utf8-fixture",
            "test/fixtures/json/minefield__y_string_utf8.1.json",
        ),
        (
            "json-escape-big-object",
            "test/fixtures/json/big-object-node.1.json",
        ),
        (
            "json-escape-large-table",
            "test/fixtures/json/large_headerless_table.1.json",
        ),
        ("json-escape-1mb-fixture", "test/fixtures/json/1mb.1.json"),
    ];

    for (document_key, path) in cases {
        reset_test_state();
        let source = read_repo_fixture(path);
        assert_streaming_fixture_round_trip(
            "json",
            document_key,
            &source,
            split_inside_escape_or_json_string(&source),
        );
    }
}

#[test]
fn wasm_document_json_streaming_unicode_or_string_boundary_splits_expand_across_valid_fixtures() {
    let _guard = lock_test_mutex();

    let cases = [
        ("json-unicode-pass01", "test/fixtures/json/pass01.1.json"),
        (
            "json-unicode-utf8-fixture",
            "test/fixtures/json/minefield__y_string_utf8.1.json",
        ),
        (
            "json-unicode-big-object",
            "test/fixtures/json/big-object-node.1.json",
        ),
        (
            "json-unicode-large-table",
            "test/fixtures/json/large_headerless_table.1.json",
        ),
        ("json-unicode-1mb-fixture", "test/fixtures/json/1mb.1.json"),
    ];

    for (document_key, path) in cases {
        reset_test_state();
        let source = read_repo_fixture(path);
        assert_streaming_fixture_round_trip(
            "json",
            document_key,
            &source,
            split_inside_unicode_or_json_string(&source),
        );
    }
}

#[test]
fn wasm_document_json_invalid_fixture_keeps_diagnostics_only_snapshot() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source =
        read_repo_fixture("test/fixtures/json/minefield__n_structure_unclosed_object.0.json");
    let started = start_source_job("json-invalid-fixture", "json");
    let chunk = text_chunk(started.job_handle, &source);
    assert!(
        chunk.terminal.is_none(),
        "invalid fixture chunk should keep job open"
    );
    let close_batch = close(started.job_handle);
    assert_parse_failed_snapshot("json-invalid-fixture", &close_batch);
}

#[test]
fn wasm_document_non_streaming_fixture_materialization_covers_varied_inputs() {
    let _guard = lock_test_mutex();

    let cases = [
        (
            "yaml",
            "yaml-fixture-mapping",
            "test/fixtures/yaml/spec-example-2-6-mapping-of-mappings.1.yaml",
        ),
        (
            "yaml",
            "yaml-fixture-sequence",
            "test/fixtures/yaml/spec-example-2-5-sequence-of-sequences.1.yaml",
        ),
        (
            "yaml",
            "yaml-fixture-unicode",
            "test/fixtures/yaml/literal-unicode.1.yaml",
        ),
        (
            "yaml",
            "yaml-fixture-anchor",
            "test/fixtures/yaml/anchors-in-mapping.1.yaml",
        ),
        (
            "yaml",
            "yaml-fixture-invoice",
            "test/fixtures/yaml/spec-example-2-27-invoice.1.yaml",
        ),
        (
            "toml",
            "toml-fixture-spec",
            "test/fixtures/toml/spec-example-1.1.toml",
        ),
        (
            "toml",
            "toml-fixture-quoted-unicode",
            "test/fixtures/toml/key__quoted-unicode.1.toml",
        ),
        (
            "toml",
            "toml-fixture-unicode-escape",
            "test/fixtures/toml/string__unicode-escape.1.toml",
        ),
        (
            "toml",
            "toml-fixture-array-table",
            "test/fixtures/toml/table__array-table-array.1.toml",
        ),
        (
            "toml",
            "toml-fixture-inline-table",
            "test/fixtures/toml/inline-table__nest.1.toml",
        ),
        (
            "csv",
            "csv-fixture-region-and-currency",
            "test/fixtures/csv/region_and_currency.csv",
        ),
        ("python", "python-fixture-simple", "example/simple.py"),
        (
            "javascript",
            "javascript-fixture-simple",
            "example/simple.js",
        ),
    ];

    for (language, document_key, path) in cases {
        reset_test_state();
        let source = read_repo_fixture(path);
        assert_non_streaming_materialization(
            language,
            document_key,
            &source,
            midpoint_char_boundary(&source),
        );
    }
}

#[test]
fn wasm_document_non_streaming_yaml_rare_fixture_materialization_covers_directives_and_tags() {
    let _guard = lock_test_mutex();

    let cases = [
        (
            "yaml-rare-tag-directive",
            "test/fixtures/yaml-rare/spec-example-6-16-tag-directive.1.yaml",
        ),
        (
            "yaml-rare-flow-nodes",
            "test/fixtures/yaml-rare/spec-example-7-24-flow-nodes.1.yaml",
        ),
        (
            "yaml-rare-anchors-and-tags",
            "test/fixtures/yaml-rare/anchors-and-tags.1.yaml",
        ),
        (
            "yaml-rare-empty-keys",
            "test/fixtures/yaml-rare/empty-keys-in-block-and-flow-mapping.1.yaml",
        ),
        (
            "yaml-rare-directives-documents",
            "test/fixtures/yaml-rare/spec-example-9-5-directives-documents.1.yaml",
        ),
    ];

    for (document_key, path) in cases {
        reset_test_state();
        let source = read_repo_fixture(path);
        assert_non_streaming_materialization(
            "yaml",
            document_key,
            &source,
            midpoint_char_boundary(&source),
        );
    }
}

#[test]
fn wasm_document_non_streaming_toml_rare_fixture_materialization_covers_multiline_and_quoted_keys()
{
    let _guard = lock_test_mutex();

    let cases = [
        (
            "toml-rare-key-escapes",
            "test/fixtures/toml/key__escapes.1.toml",
        ),
        (
            "toml-rare-quoted-dots",
            "test/fixtures/toml/key__quoted-dots.1.toml",
        ),
        (
            "toml-rare-raw-multiline",
            "test/fixtures/toml/string__raw-multiline.1.toml",
        ),
        (
            "toml-rare-multiline-quotes",
            "test/fixtures/toml/string__multiline-quotes.1.toml",
        ),
        (
            "toml-rare-array-within-dotted",
            "test/fixtures/toml/table__array-within-dotted.1.toml",
        ),
    ];

    for (document_key, path) in cases {
        reset_test_state();
        let source = read_repo_fixture(path);
        assert_non_streaming_materialization(
            "toml",
            document_key,
            &source,
            midpoint_char_boundary(&source),
        );
    }
}

#[test]
fn wasm_document_non_streaming_invalid_fixtures_keep_diagnostics_only_snapshots() {
    let _guard = lock_test_mutex();

    let fixture_cases = [
        (
            "yaml",
            "yaml-invalid-fixture",
            "test/fixtures/yaml/invalid-nested-mapping.0.yaml",
        ),
        (
            "toml",
            "toml-invalid-fixture",
            "test/fixtures/toml/string__no-close-01.0.toml",
        ),
    ];

    for (language, document_key, path) in fixture_cases {
        reset_test_state();
        let source = read_repo_fixture(path);
        let started = start_source_job(document_key, language);
        let split_at = midpoint_char_boundary(&source);
        let first = text_chunk(started.job_handle, &source[..split_at]);
        assert!(
            first.events.is_empty(),
            "{document_key} should buffer invalid first chunk"
        );
        let second = text_chunk(started.job_handle, &source[split_at..]);
        assert!(
            second.events.is_empty(),
            "{document_key} should buffer invalid second chunk"
        );
        let close_batch = close(started.job_handle);
        assert_parse_failed_snapshot(document_key, &close_batch);
    }
}

#[test]
fn wasm_document_non_streaming_yaml_rare_invalid_fixtures_keep_diagnostics_only_snapshots() {
    let _guard = lock_test_mutex();

    let cases = [
        (
            "yaml-rare-invalid-duplicate-directive",
            "test/fixtures/yaml-rare/duplicate-yaml-directive.0.yaml",
        ),
        (
            "yaml-rare-invalid-extra-words",
            "test/fixtures/yaml-rare/extra-words-on-yaml-directive.0.yaml",
        ),
        (
            "yaml-rare-invalid-anchor-doc-start",
            "test/fixtures/yaml-rare/mapping-with-anchor-on-document-start-line.0.yaml",
        ),
    ];

    for (document_key, path) in cases {
        reset_test_state();
        let source = read_repo_fixture(path);
        let started = start_source_job(document_key, "yaml");
        let split_at = midpoint_char_boundary(&source);
        let first = text_chunk(started.job_handle, &source[..split_at]);
        assert!(
            first.events.is_empty(),
            "{document_key} should buffer invalid first chunk"
        );
        let second = text_chunk(started.job_handle, &source[split_at..]);
        assert!(
            second.events.is_empty(),
            "{document_key} should buffer invalid second chunk"
        );
        let close_batch = close(started.job_handle);
        assert_parse_failed_snapshot(document_key, &close_batch);
    }
}

#[test]
fn wasm_document_non_streaming_manual_invalid_inputs_keep_diagnostics_only_snapshots() {
    let _guard = lock_test_mutex();

    let cases = [
        (
            "csv",
            "csv-invalid-manual",
            "\"Region\",\"Code\"\n\"Afghanistan\",\"AF",
        ),
        (
            "python",
            "python-invalid-manual",
            "{'object': {'name': 'Ada'",
        ),
        (
            "javascript",
            "javascript-invalid-manual",
            "({object: {name: \"Ada\"}",
        ),
    ];

    for (language, document_key, source) in cases {
        reset_test_state();
        let started = start_source_job(document_key, language);
        let split_at = midpoint_char_boundary(source);
        let first = text_chunk(started.job_handle, &source[..split_at]);
        assert!(
            first.events.is_empty(),
            "{document_key} should buffer invalid first chunk"
        );
        let second = text_chunk(started.job_handle, &source[split_at..]);
        assert!(
            second.events.is_empty(),
            "{document_key} should buffer invalid second chunk"
        );
        let close_batch = close(started.job_handle);
        assert_parse_failed_snapshot(document_key, &close_batch);
    }
}

#[test]
fn wasm_document_json_streaming_large_fixture_emits_progress_before_close() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = read_repo_fixture("test/fixtures/json/1mb.1.json");
    let split_at = midpoint_char_boundary(&source);
    let started = start_source_job("json-progress-large", "json");
    let first = text_chunk(started.job_handle, &source[..split_at]);
    assert!(
        first
            .events
            .iter()
            .any(|event| matches!(event, DocumentEvent::Progress { .. })),
        "large json first chunk should emit progress"
    );
    assert_projection_delta(&first, true, "json-progress-large");
    assert!(first.terminal.is_none());
    let second = text_chunk(started.job_handle, &source[split_at..]);
    assert_projection_delta(&second, false, "json-progress-large");
    let close_batch = close(started.job_handle);
    assert!(matches!(close_batch.terminal, Some(JobTerminal::Completed)));
    assert_snapshot_source("json-progress-large", &source);
}

#[test]
fn wasm_document_json_streaming_boundary_splits_cover_string_midpoint_case() {
    let _guard = lock_test_mutex();
    reset_test_state();

    let source = r#"{"message":"alpha beta","count":1}"#;
    assert_streaming_fixture_round_trip(
        "json",
        "json-boundary-string-midpoint",
        source,
        split_after_needle_fragment(source, "alpha beta", 5),
    );
}
