use treease_core::core::CodecService;
use treease_core::evaluator::{AllAtOnceEvaluator, Value};

const SIMPLE_JSON: &str = include_str!("../../../example/simple.json");
const SIMPLE_YAML: &str = include_str!("../../../example/simple.yaml");
const SIMPLE_TOML: &str = include_str!("../../../example/simple.toml");
const SIMPLE_JS: &str = include_str!("../../../example/simple.js");
const SIMPLE_PY: &str = include_str!("../../../example/simple.py");
const EMPTY_YAML_FIXTURE: &str = include_str!("../../../test/fixtures/yaml/empty.1.yaml");
const MULTI_DOC_YAML_FIXTURE: &str =
    include_str!("../../../test/fixtures/yaml/multiple-docs.1.yaml");

fn decoded_doc_count(service: &CodecService, format: &str, input: &str) -> usize {
    service.decode_all(format, input).unwrap().len()
}

#[test]
fn examples_decode_encode_reencodes_json_and_yaml_examples() {
    let service = CodecService::new();

    let json = service
        .convert_string("json", "json", r#"{"name":"Ada","skills":["rust","zig"]}"#)
        .unwrap();
    let yaml = service
        .convert_string("yaml", "yaml", "name: Ada\nskills:\n  - rust\n  - zig\n")
        .unwrap();

    assert!(json.contains("\"name\""));
    assert!(json.contains("\"skills\""));
    assert!(yaml.contains("name: Ada"));
    assert!(yaml.contains("- rust"));
}

#[test]
fn examples_decode_encode_transcodes_between_common_formats() {
    let service = CodecService::new();

    let json_to_yaml = service
        .convert_string(
            "json",
            "yaml",
            r#"{"meta":{"id":"item-001"},"flags":[true,false]}"#,
        )
        .unwrap();
    let toml_to_json = service
        .convert_string("toml", "json", "name = \"Ada\"\n[meta]\nactive = true\n")
        .unwrap();
    assert!(json_to_yaml.contains("meta:"));
    assert!(json_to_yaml.contains("- true"));
    assert!(toml_to_json.contains("\"name\""));
    assert!(toml_to_json.contains("\"active\": true"));
}

#[test]
fn examples_simple_file_corpus_transcodes_shared_repo_examples_to_json() {
    let service = CodecService::new();

    for (format, input) in [
        ("json", SIMPLE_JSON),
        ("yaml", SIMPLE_YAML),
        ("toml", SIMPLE_TOML),
        ("javascript", SIMPLE_JS),
        ("python", SIMPLE_PY),
    ] {
        let out = service.convert_string(format, "json", input).unwrap();

        assert!(
            out.contains("\"table_without_header\""),
            "missing table_without_header for {format}"
        );
        assert!(
            out.contains("\"table_with_header\""),
            "missing table_with_header for {format}"
        );
        assert!(out.contains("\"preview\""), "missing preview for {format}");
        assert!(
            out.contains("\"unicode\""),
            "missing unicode field for {format}"
        );
    }
}

// ── examples empty yaml decode and encode ────────────────────────

#[test]
fn examples_empty_yaml_decode_and_encode() {
    let service = CodecService::new();

    let out = service
        .convert_string("yaml", "yaml", EMPTY_YAML_FIXTURE)
        .unwrap();

    assert_eq!(decoded_doc_count(&service, "yaml", &out), 1);

    let out2 = service.convert_string("yaml", "yaml", &out).unwrap();
    assert_eq!(out, out2);
}

// ── examples empty yaml decode only ───────────────────────────────

#[test]
fn examples_empty_yaml_decode_only() {
    let service = CodecService::new();

    assert_eq!(decoded_doc_count(&service, "yaml", EMPTY_YAML_FIXTURE), 1);
}

// ── examples multi-doc yaml roundtrip ─────────────────────────────

#[test]
fn examples_multi_doc_yaml_roundtrip_preserves_document_count_and_idempotence() {
    let service = CodecService::new();

    let out = service
        .convert_string("yaml", "yaml", MULTI_DOC_YAML_FIXTURE)
        .unwrap();

    let input_docs = decoded_doc_count(&service, "yaml", MULTI_DOC_YAML_FIXTURE);
    let output_docs = decoded_doc_count(&service, "yaml", &out);

    assert!(input_docs > 1);
    assert_eq!(input_docs, output_docs);

    let out2 = service.convert_string("yaml", "yaml", &out).unwrap();
    assert_eq!(out, out2);
}

// ── examples json roundtrip preserves key payload markers ─────────

#[test]
fn examples_json_roundtrip_preserves_key_payload_markers() {
    let service = CodecService::new();

    let input = r#"{"title":"Example","count":42,"tags":["a","b"]}"#;
    let out = service.convert_string("json", "json", input).unwrap();

    assert!(out.contains("\"title\""));
    assert!(out.contains("\"count\""));
    assert!(out.contains("\"tags\""));

    // Re-encoding should be idempotent.
    let out2 = service.convert_string("json", "json", &out).unwrap();
    assert_eq!(out, out2);
}

// ── examples csv roundtrip keeps delimiter and idempotence ────────

#[test]
fn examples_csv_roundtrip_keeps_delimiter_and_idempotence() {
    let service = CodecService::new();

    let input = "name,score\nalice,100\nbob,98\n";
    let out = service.convert_string("csv", "csv", input).unwrap();

    assert!(out.contains(","));
    assert!(out.contains("name"));
    assert!(out.contains("score"));

    // Re-encoding should be idempotent.
    let out2 = service.convert_string("csv", "csv", &out).unwrap();
    assert_eq!(out, out2);
}

// ── examples toml roundtrip keeps key markers and idempotence ─────

#[test]
fn examples_toml_roundtrip_keeps_key_markers_and_idempotence() {
    let service = CodecService::new();

    let input = "title = \"demo\"\ncount = 7\n";
    let out = service.convert_string("toml", "toml", input).unwrap();

    assert!(out.contains("title"));
    assert!(out.contains("count"));
    assert!(out.contains("="));

    // Re-encoding should be idempotent.
    let out2 = service.convert_string("toml", "toml", &out).unwrap();
    assert_eq!(out, out2);
}

// ── examples equivalent semantics across json/yaml/toml ───────────

#[test]
fn examples_equivalent_semantics_across_json_yaml_toml_keep_one_doc_contract() {
    let service = CodecService::new();

    let json_input = r#"{"title":"demo","count":7}"#;
    let yaml_input = "title: demo\ncount: 7\n";
    let toml_input = "title = \"demo\"\ncount = 7\n";

    let json_out = service.convert_string("json", "json", json_input).unwrap();
    let yaml_out = service.convert_string("yaml", "yaml", yaml_input).unwrap();
    let toml_out = service.convert_string("toml", "toml", toml_input).unwrap();

    assert_eq!(decoded_doc_count(&service, "json", &json_out), 1);
    assert_eq!(decoded_doc_count(&service, "yaml", &yaml_out), 1);
    assert_eq!(decoded_doc_count(&service, "toml", &toml_out), 1);

    // All should contain the key "title".
    assert!(json_out.contains("\"title\""));
    assert!(yaml_out.contains("title"));
    assert!(toml_out.contains("title"));

    // All should contain the key "count".
    assert!(json_out.contains("count"));
    assert!(yaml_out.contains("count"));
    assert!(toml_out.contains("count"));
}

// ── examples json formatting variants normalize ───────────────────

#[test]
fn examples_json_formatting_variants_normalize_to_same_canonical_output() {
    let service = CodecService::new();

    let json_compact = r#"{"a":1,"b":2}"#;
    let json_spaced = "{ \"a\" : 1, \"b\" : 2 }";

    let out_compact = service
        .convert_string("json", "json", json_compact)
        .unwrap();
    let out_spaced = service.convert_string("json", "json", json_spaced).unwrap();

    assert_eq!(out_compact, out_spaced);
    assert_eq!(decoded_doc_count(&service, "json", &out_compact), 1);
    assert_eq!(decoded_doc_count(&service, "json", &out_spaced), 1);
}

// ── examples roundtrip preserves semantic key values ──────────────

#[test]
fn examples_roundtrip_preserves_semantic_key_values() {
    let service = CodecService::new();
    let evaluator = AllAtOnceEvaluator::new();

    let json_input = r#"{"title":"demo","count":7,"enabled":true,"tags":["a","b","c"]}"#;
    let json_rt = service.convert_string("json", "json", json_input).unwrap();

    let decoded = service.decode("json", &json_rt).unwrap();

    let count_result = evaluator
        .evaluate_nodes(&decoded.store, ".count", &[decoded.root])
        .unwrap();
    assert_eq!(count_result.len(), 1);
    assert_eq!(count_result[0], Value::Number(7.0));

    let enabled_result = evaluator
        .evaluate_nodes(&decoded.store, ".enabled", &[decoded.root])
        .unwrap();
    assert_eq!(enabled_result.len(), 1);
    assert_eq!(enabled_result[0], Value::Bool(true));

    let tag_result = evaluator
        .evaluate_nodes(&decoded.store, ".tags[2]", &[decoded.root])
        .unwrap();
    assert_eq!(tag_result.len(), 1);
    assert_eq!(tag_result[0], Value::String("c".to_string()));
}

// ── examples cross-format semantics align ─────────────────────────

#[test]
fn examples_cross_format_semantics_align_for_null_bool_float_nested() {
    let service = CodecService::new();
    let evaluator = AllAtOnceEvaluator::new();

    let json_input =
        r#"{"name":"demo","enabled":true,"ratio":1.5,"meta":null,"list":[1,{"k":false}]}"#;
    let yaml_input =
        "name: demo\nenabled: true\nratio: 1.5\nmeta: null\nlist:\n  - 1\n  - k: false\n";
    let toml_input = "name = \"demo\"\nenabled = true\nratio = 1.5\nlist = [1, { k = false }]\n";

    let json_decoded = service.decode("json", json_input).unwrap();
    let yaml_decoded = service.decode("yaml", yaml_input).unwrap();
    let toml_decoded = service.decode("toml", toml_input).unwrap();

    let exprs = [".name", ".enabled", ".ratio", ".list[1].k", ".list[0]"];

    for expr in &exprs {
        let json_val = evaluator
            .evaluate_nodes(&json_decoded.store, expr, &[json_decoded.root])
            .unwrap();
        let yaml_val = evaluator
            .evaluate_nodes(&yaml_decoded.store, expr, &[yaml_decoded.root])
            .unwrap();
        let toml_val = evaluator
            .evaluate_nodes(&toml_decoded.store, expr, &[toml_decoded.root])
            .unwrap();

        assert_eq!(json_val, yaml_val, "mismatch for expr: {}", expr);
        assert_eq!(json_val, toml_val, "mismatch for expr: {}", expr);
    }

    // Check null specifically.
    let json_null = evaluator
        .evaluate_nodes(&json_decoded.store, ".meta", &[json_decoded.root])
        .unwrap();
    let yaml_null = evaluator
        .evaluate_nodes(&yaml_decoded.store, ".meta", &[yaml_decoded.root])
        .unwrap();
    assert_eq!(json_null[0], Value::Null);
    assert_eq!(json_null, yaml_null);
}

// ── examples json to toml omits null object fields ────────────────

#[test]
fn examples_json_to_toml_omits_null_object_fields() {
    let service = CodecService::new();

    let input = r##"{"meta":{"id":"item-001","drop":null},"items":[{"name":"item-a"},{"name":"item-b"}],"preview":{"color":"#4f46e5","drop":null}}"##;
    let out = service.convert_string("json", "toml", input).unwrap();

    // Should contain the non-null fields.
    assert!(out.contains("[meta]"));
    assert!(out.contains("id = \"item-001\""));
    assert!(out.contains("[[items]]"));
    assert!(out.contains("[preview]"));
    assert!(out.contains("color = \"#4f46e5\""));

    // Should NOT contain the dropped null fields.
    assert!(!out.contains("drop"));
    assert_eq!(decoded_doc_count(&service, "toml", &out), 1);
}

// ── examples json to toml preserves non-null graph semantics ──────

#[test]
fn examples_json_to_toml_preserves_non_null_graph_semantics() {
    let service = CodecService::new();
    let evaluator = AllAtOnceEvaluator::new();

    let input = r##"{"meta":{"id":"item-001","drop":null},"items":[{"name":"item-a"},{"name":"item-b"}],"preview":{"color":"#4f46e5","drop":null}}"##;
    let out = service.convert_string("json", "toml", input).unwrap();

    let json_decoded = service.decode("json", input).unwrap();
    let toml_decoded = service.decode("toml", &out).unwrap();

    let exprs = [
        ".meta.id",
        ".items | length",
        ".items[0].name",
        ".items[1].name",
        ".preview.color",
    ];

    for expr in &exprs {
        let json_val = evaluator
            .evaluate_nodes(&json_decoded.store, expr, &[json_decoded.root])
            .unwrap();
        let toml_val = evaluator
            .evaluate_nodes(&toml_decoded.store, expr, &[toml_decoded.root])
            .unwrap();
        assert_eq!(json_val, toml_val, "mismatch for expr: {}", expr);
    }

    // Verify drop fields are not present in TOML output.
    assert!(!out.contains("drop"));
}

// ── examples json to toml rejects unsupported null placements ─────

#[test]
fn examples_json_to_toml_rejects_unsupported_null_placements() {
    let service = CodecService::new();

    // Top-level null should fail.
    assert!(matches!(
        service.convert_string("json", "toml", "null"),
        Err(treease_core::core::errors::CoreError::Parse(
            treease_core::core::errors::ParseError::UnsupportedTomlValue
        ))
    ));

    // Null in array should fail.
    assert!(matches!(
        service.convert_string("json", "toml", r#"{"list":[1,null]}"#),
        Err(treease_core::core::errors::CoreError::Parse(
            treease_core::core::errors::ParseError::UnsupportedTomlValue
        ))
    ));
}

// ── examples decode contracts expose stable error kinds ───────────

#[test]
fn examples_decode_contracts_expose_stable_error_kinds() {
    let service = CodecService::new();

    // Invalid JSON.
    let err = service.decode("json", r#"{"x":"#).unwrap_err();
    assert!(matches!(
        err,
        treease_core::core::errors::CoreError::Parse(
            treease_core::core::errors::ParseError::InvalidJson
        )
    ));

    // Bad TOML document.
    let err = service.decode("toml", "title = \"x\n").unwrap_err();
    assert!(matches!(
        err,
        treease_core::core::errors::CoreError::Parse(
            treease_core::core::errors::ParseError::BadTomlDocument
        )
    ));

    // Unknown format.
    let err = service
        .decode("definitely-not-a-format", "x: 1\n")
        .unwrap_err();
    assert!(matches!(
        err,
        treease_core::core::errors::CoreError::Format(
            treease_core::core::errors::FormatError::UnknownFormat
        )
    ));
}

// ── examples medium input smoke preserves semantic invariants ─────

#[test]
fn examples_medium_input_smoke_preserves_semantic_invariants() {
    let service = CodecService::new();
    let evaluator = AllAtOnceEvaluator::new();

    // Build a medium-sized JSON input (256 items).
    let mut input = String::from(r#"{"meta":{"total":256,"enabled":true},"items":["#);
    for i in 0..256 {
        if i != 0 {
            input.push(',');
        }
        input.push_str(&format!(
            r#"{{"id":{},"ok":{},"score":{}}}"#,
            i,
            i % 2 == 0,
            i * 3
        ));
    }
    input.push_str("]}");

    let output = service.convert_string("json", "json", &input).unwrap();

    let input_decoded = service.decode("json", &input).unwrap();
    let output_decoded = service.decode("json", &output).unwrap();

    // Check .meta.total.
    let total_before = evaluator
        .evaluate_nodes(&input_decoded.store, ".meta.total", &[input_decoded.root])
        .unwrap();
    let total_after = evaluator
        .evaluate_nodes(&output_decoded.store, ".meta.total", &[output_decoded.root])
        .unwrap();
    assert_eq!(total_before, total_after);

    // Check .items | length.
    let len_before = evaluator
        .evaluate_nodes(
            &input_decoded.store,
            ".items | length",
            &[input_decoded.root],
        )
        .unwrap();
    let len_after = evaluator
        .evaluate_nodes(
            &output_decoded.store,
            ".items | length",
            &[output_decoded.root],
        )
        .unwrap();
    assert_eq!(len_before, len_after);

    // Check .items[0].ok.
    let first_ok_before = evaluator
        .evaluate_nodes(&input_decoded.store, ".items[0].ok", &[input_decoded.root])
        .unwrap();
    let first_ok_after = evaluator
        .evaluate_nodes(
            &output_decoded.store,
            ".items[0].ok",
            &[output_decoded.root],
        )
        .unwrap();
    assert_eq!(first_ok_before, first_ok_after);
}
