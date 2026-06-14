// core_corpus.rs — corpus smoke test aligned with tests/lib/corpus.zig
//
// The Zig test is a complex corpus test runner that:
//   1. Collects fixture files from test/fixtures/{json,yaml,toml}/
//   2. Decodes each fixture
//   3. Optionally compares against yq reference output
//   4. Builds graph models and compares them
//
// For Rust, we write a simplified smoke test that:
//   1. Verifies basic JSON/YAML/TOML parsing works
//   2. Verifies basic evaluation works
//   3. Verifies format conversion (encode/decode round-trip) works

use treease_core::core::{
    CoreError, FormatLanguage, GraphBuilder, GraphLanguage, ParseError, SemType, TreeNodeKind,
    default_config, get_map_entry,
};
use treease_core::formats::{
    Decode, DecodedDocument, Encode, JsonDecoder, JsonEncoder, TomlDecoder, TomlEncoder,
    YamlDecoder, YamlEncoder, default_language_preferences,
};
use treease_core::operators::{
    NodeId as CompatNodeId, NodeKind as CompatNodeKind, SemType as CompatSemType,
    TreeNode as CompatTreeNode,
};

const TOML_DOTTED_KEY_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/key__dotted-01.1.toml");
const TOML_INVALID_TABLE_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/spec-1.0.0__inline-table__trailing-comma.0.toml");
const TOML_IMPLICIT_AND_EXPLICIT_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/implicit-and-explicit-after.1.toml");
const TOML_MULTILINE_EMPTY_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/string__multiline-empty.1.toml");
const TOML_MULTILINE_QUOTES_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/string__multiline-quotes.1.toml");
const TOML_INLINE_TABLE_NEWLINE_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/inline-table__newline.1.toml");
const TOML_HEX_ESCAPE_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/string__hex-escape.1.toml");
const TOML_INVALID_ARRAY_TABLE_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/array__tables-02.0.toml");
const TOML_INVALID_DATETIME_NO_SECONDS_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/spec-1.0.0__datetime__no-secs.0.toml");
const TOML_DATETIME_NO_SECONDS_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/datetime__no-seconds.1.toml");
const TOML_INVALID_TABLE_REDEFINITION_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/spec-1.0.0__table-9-0.0.toml");
const TOML_INLINE_TABLE_COMMON_47_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/spec-1.1.0__common-47.1.toml");
const TOML_LITERAL_MULTILINE_QUOTES_INVALID_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/string__literal-multiline-quotes-01.0.toml");
const TOML_INTEGER_LITERALS_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/integer__literals.1.toml");
const TOML_INTEGER_LONG_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/integer__long.1.toml");
const TOML_COMMON_INTEGER_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/spec-1.1.0__common-22.1.toml");
const TOML_INTEGER_ZERO_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/integer__zero.1.toml");
const TOML_INTEGER_SPEC_FIXTURE: &str =
    include_str!("../../../test/fixtures/toml/spec-1.0.0__integer-2.1.toml");
const YAML_VERBATIM_TAG_FIXTURE: &str = include_str!("../../../test/fixtures/yaml/7FWL.1.yaml");
const YAML_VERBATIM_TAG_EXAMPLE_FIXTURE: &str =
    include_str!("../../../test/fixtures/yaml/spec-example-6-24-verbatim-tags.1.yaml");
const YAML_DIRECTIVE_INVALID_FIXTURE: &str =
    include_str!("../../../test/fixtures/yaml/yaml-directive-without-document-end-marker.0.yaml");
const YAML_BARE_DOCUMENTS_FIXTURE: &str =
    include_str!("../../../test/fixtures/yaml/spec-example-9-3-bare-documents.1.yaml");
const YAML_DIRECTIVE_BOUNDARY_VALID_FIXTURE: &str =
    include_str!("../../../test/fixtures/yaml/7Z25.1.yaml");
const YAML_DIRECTIVE_BOUNDARY_INVALID_FIXTURE: &str =
    include_str!("../../../test/fixtures/yaml/RHX7.0.yaml");
const YAML_INVALID_AFTER_DOCUMENT_END_FIXTURE: &str =
    include_str!("../../../test/fixtures/yaml/3HFZ.0.yaml");
const YAML_UNDECLARED_LATER_DOCUMENT_FIXTURE: &str =
    include_str!("../../../test/fixtures/yaml/QLJ7.0.yaml");

// ── Helper: compact JSON encoder ─────────────────────────────────

fn compact_json_encoder() -> JsonEncoder {
    let mut prefs = default_language_preferences().effective(FormatLanguage::Json);
    prefs.indent = 0;
    prefs.smart = false;
    prefs.unwrap_scalar = false;
    JsonEncoder::new(prefs)
}

fn decode_toml_fixture(input: &str, filename: &str) -> Result<DecodedDocument, CoreError> {
    TomlDecoder.decode_str_with_filename(input, filename)
}

fn compat_kind(kind: TreeNodeKind) -> CompatNodeKind {
    match kind {
        TreeNodeKind::Sequence => CompatNodeKind::Sequence,
        TreeNodeKind::Mapping => CompatNodeKind::Mapping,
        TreeNodeKind::Scalar => CompatNodeKind::Scalar,
        TreeNodeKind::Alias => CompatNodeKind::Alias,
        TreeNodeKind::Unknown => CompatNodeKind::Unknown,
    }
}

fn compat_sem_type(sem_type: Option<SemType>) -> Option<CompatSemType> {
    sem_type.map(|sem_type| match sem_type {
        SemType::Nil => CompatSemType::Nil,
        SemType::Str => CompatSemType::Str,
        SemType::Int => CompatSemType::Int,
        SemType::Float => CompatSemType::Float,
        SemType::Boolean => CompatSemType::Boolean,
        SemType::Map => CompatSemType::Map,
        SemType::Seq => CompatSemType::Seq,
    })
}

fn compat_tree_from_core(
    store: &treease_core::core::TreeStore,
    node_id: treease_core::core::NodeId,
) -> CompatTreeNode {
    let source = store.get(node_id).unwrap();
    let mut out = CompatTreeNode {
        kind: compat_kind(source.kind),
        sequence_closed: source.sequence_closed,
        sem_type: compat_sem_type(source.sem_type),
        tag: source.tag.clone(),
        value: source.value.clone(),
        start_byte: source.start_byte,
        end_byte: source.end_byte,
        anchor: source.anchor.clone(),
        alias: source.alias.map(|id| CompatNodeId(id.0)),
        head_comment: source.head_comment.clone(),
        line_comment: source.line_comment.clone(),
        foot_comment: source.foot_comment.clone(),
        parent: source.parent.map(|id| CompatNodeId(id.0)),
        key: source.key.map(|id| CompatNodeId(id.0)),
        sequence_index: source.sequence_index,
        leading_content: source.leading_content.clone(),
        document: source.document,
        filename: source.filename.clone(),
        line: source.line,
        column: source.column,
        file_index: source.file_index,
        is_map_key: source.is_map_key,
        encode_separate: source.encode_separate,
        evaluate_together: source.evaluate_together,
        ..CompatTreeNode::default()
    };
    out.content = source
        .content
        .iter()
        .map(|child_id| compat_tree_from_core(store, *child_id))
        .collect();
    out
}

fn build_graph(
    store: &treease_core::core::TreeStore,
    node_id: treease_core::core::NodeId,
) -> treease_core::core::graph_builder::GraphModel {
    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::None);
    let compat_root = compat_tree_from_core(store, node_id);
    builder.build(&compat_root)
}

// ── Test 1: JSON decode/encode round-trip ────────────────────────

#[test]
fn json_decode_encode_round_trip() {
    let input = r#"{"name":"Ada","values":[1,true,null]}"#;

    let decoded = JsonDecoder.decode_str(input).unwrap();
    let root = decoded.store.get(decoded.root).unwrap();
    assert_eq!(root.kind, TreeNodeKind::Mapping);

    // Verify map entry access
    let name = get_map_entry(&decoded.store, decoded.root, "name")
        .unwrap()
        .unwrap()
        .value;
    assert_eq!(
        decoded.store.get(name).unwrap().sem_type,
        Some(SemType::Str)
    );
    assert_eq!(decoded.store.get(name).unwrap().value, "Ada");

    // Verify array entry
    let values = get_map_entry(&decoded.store, decoded.root, "values")
        .unwrap()
        .unwrap()
        .value;
    assert_eq!(
        decoded.store.get(values).unwrap().kind,
        TreeNodeKind::Sequence
    );

    // Encode back to JSON
    let encoded = JsonEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert!(encoded.contains(r#""name": "Ada""#));
    assert!(encoded.contains(r#""values": ["#));
}

// ── Test 2: JSON compact round-trip ──────────────────────────────

#[test]
fn json_compact_round_trip() {
    for (input, expected) in [
        ("[]", "[]\n"),
        ("[3]", "[3]\n"),
        (r#"[{"x": 3}]"#, "[{\"x\":3}]\n"),
        ("[null]", "[null]\n"),
        ("[true, false]", "[true,false]\n"),
    ] {
        let decoded = JsonDecoder.decode_str(input).unwrap();
        let encoded = compact_json_encoder()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap();
        assert_eq!(encoded, expected, "round-trip failed for input: {input}");
    }
}

// ── Test 3: JSON decode to YAML ──────────────────────────────────

#[test]
fn json_decode_to_yaml() {
    let decoded = JsonDecoder
        .decode_str(r#"{"a":"first","b":{"c":2,"d":[3,4]},"ab":"last"}"#)
        .unwrap();

    let encoded = YamlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(
        encoded,
        "a: first\nb:\n  c: 2\n  d:\n    - 3\n    - 4\nab: last\n"
    );
}

// ── Test 4: YAML decode/encode round-trip ────────────────────────

#[test]
fn yaml_decode_encode_round_trip() {
    let input = "a: Easy! as one two three\nb:\n  c: 2\n  d:\n    - 3\n    - 4\n";

    let decoded = YamlDecoder.decode_str(input).unwrap();

    let b = get_map_entry(&decoded.store, decoded.root, "b")
        .unwrap()
        .unwrap()
        .value;
    let d = get_map_entry(&decoded.store, b, "d")
        .unwrap()
        .unwrap()
        .value;
    assert_eq!(decoded.store.get(d).unwrap().kind, TreeNodeKind::Sequence);

    let encoded = YamlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, input);
}

// ── Test 5: YAML empty input decodes to nil ──────────────────────

#[test]
fn yaml_empty_input_decodes_to_nil() {
    let decoded = YamlDecoder.decode_str("").unwrap();

    assert_eq!(
        decoded.store.get(decoded.root).unwrap().sem_type,
        Some(SemType::Nil)
    );

    let encoded = YamlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, "null\n");
}

// ── Test 6: TOML decode/encode round-trip ────────────────────────

#[test]
fn toml_decode_encode_round_trip() {
    let input = "name = \"Ada\"\ncount = 2\n[meta]\nactive = true\n";

    let decoded = TomlDecoder.decode_str(input).unwrap();

    let meta = get_map_entry(&decoded.store, decoded.root, "meta")
        .unwrap()
        .unwrap()
        .value;
    assert_eq!(decoded.store.get(meta).unwrap().kind, TreeNodeKind::Mapping);

    let encoded = TomlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert!(encoded.contains("name = \"Ada\""));
    assert!(encoded.contains("[meta]"));
}

// ── Test 7: TOML nested table round-trip ─────────────────────────

#[test]
fn toml_nested_table_round_trip() {
    let input = "a = \"b\"\n\n[c]\ne = \"f\"\n";

    let decoded = TomlDecoder.decode_str(input).unwrap();

    let encoded = TomlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, input);
}

// ── Test 10: JSON rejects invalid input ──────────────────────────

#[test]
fn json_rejects_invalid_input() {
    assert_eq!(
        JsonDecoder.decode_str(r#"{"a": 1 b": 2}"#).unwrap_err(),
        CoreError::Parse(ParseError::InvalidJson)
    );
}

// ── Test 11: YAML rejects invalid input ──────────────────────────

#[test]
fn yaml_rejects_invalid_input() {
    assert_eq!(
        YamlDecoder.decode_str("a: [1, 2\n").unwrap_err(),
        CoreError::Parse(ParseError::InvalidYaml)
    );
}

// ── Test 12: TOML rejects bad document ───────────────────────────

#[test]
fn toml_rejects_bad_document() {
    assert_eq!(
        TomlDecoder.decode_str("[a]\nb = \"boogie").unwrap_err(),
        CoreError::Parse(ParseError::BadTomlDocument)
    );
}

// ── Test 14: Cross-format conversion: JSON to TOML ───────────────

#[test]
fn cross_format_json_to_toml() {
    let decoded = JsonDecoder
        .decode_str(r#"{"name":"Ada","active":true,"count":42}"#)
        .unwrap();

    let encoded = TomlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert!(encoded.contains("name = \"Ada\""));
    assert!(encoded.contains("active = true"));
    assert!(encoded.contains("count = 42"));
}

// ── Test 15: Cross-format conversion: YAML to JSON ───────────────

#[test]
fn cross_format_yaml_to_json() {
    let decoded = YamlDecoder
        .decode_str("name: Ada\nskills:\n  - rust\n  - zig\n")
        .unwrap();

    let encoded = compact_json_encoder()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(
        encoded,
        "{\"name\":\"Ada\",\"skills\":[\"rust\",\"zig\"]}\n"
    );
}

// ── Test 16: JSON number types are preserved ─────────────────────

#[test]
fn json_number_types_are_preserved() {
    let decoded = JsonDecoder
        .decode_str(r#"[3,3.1,-1,true,false,null,"cat"]"#)
        .unwrap();

    let root = decoded.store.get(decoded.root).unwrap();
    let sem_types: Vec<_> = root
        .content
        .iter()
        .map(|id| decoded.store.get(*id).unwrap().sem_type)
        .collect();

    assert_eq!(
        sem_types,
        [
            Some(SemType::Int),
            Some(SemType::Float),
            Some(SemType::Int),
            Some(SemType::Boolean),
            Some(SemType::Boolean),
            Some(SemType::Nil),
            Some(SemType::Str),
        ]
    );
}

#[test]
fn corpus_toml_dotted_key_fixture_matches_zig_regression_subset() {
    let decoded = TomlDecoder.decode_str(TOML_DOTTED_KEY_FIXTURE).unwrap();

    let name = get_map_entry(&decoded.store, decoded.root, "name")
        .unwrap()
        .unwrap()
        .value;
    let first = get_map_entry(&decoded.store, name, "first")
        .unwrap()
        .unwrap()
        .value;
    let many = get_map_entry(&decoded.store, decoded.root, "many")
        .unwrap()
        .unwrap()
        .value;
    let dots = get_map_entry(&decoded.store, many, "dots")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(decoded.store.get(first).unwrap().value, "Arthur");
    assert_eq!(decoded.store.get(dots).unwrap().kind, TreeNodeKind::Mapping);
}

#[test]
fn corpus_toml_invalid_redefinition_fixture_matches_zig_regression_subset() {
    assert_eq!(
        decode_toml_fixture(
            TOML_INVALID_TABLE_FIXTURE,
            "spec-1.0.0__inline-table__trailing-comma.0.toml",
        )
        .unwrap_err(),
        CoreError::Parse(ParseError::BadTomlDocument)
    );
}

#[test]
fn corpus_toml_implicit_and_explicit_after_fixture_matches_zig_regression_subset() {
    let decoded = TomlDecoder
        .decode_str(TOML_IMPLICIT_AND_EXPLICIT_FIXTURE)
        .unwrap();

    let a = get_map_entry(&decoded.store, decoded.root, "a")
        .unwrap()
        .unwrap()
        .value;
    let b = get_map_entry(&decoded.store, a, "b")
        .unwrap()
        .unwrap()
        .value;
    let c = get_map_entry(&decoded.store, b, "c")
        .unwrap()
        .unwrap()
        .value;
    let answer = get_map_entry(&decoded.store, c, "answer")
        .unwrap()
        .unwrap()
        .value;
    let better = get_map_entry(&decoded.store, a, "better")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(decoded.store.get(answer).unwrap().value, "42");
    assert_eq!(decoded.store.get(better).unwrap().value, "43");
}

#[test]
fn corpus_toml_multiline_empty_fixture_matches_zig_regression_subset() {
    let decoded = TomlDecoder
        .decode_str(TOML_MULTILINE_EMPTY_FIXTURE)
        .unwrap();

    for key in ["empty-1", "empty-2", "empty-3", "empty-4"] {
        let value = get_map_entry(&decoded.store, decoded.root, key)
            .unwrap()
            .unwrap()
            .value;
        assert_eq!(decoded.store.get(value).unwrap().value, "", "key={key}");
    }
}

#[test]
fn corpus_toml_multiline_quotes_fixture_matches_zig_regression_subset() {
    let decoded = TomlDecoder
        .decode_str(TOML_MULTILINE_QUOTES_FIXTURE)
        .unwrap();

    for (key, expected) in [
        ("lit_one", "'one quote"),
        ("lit_two", "''two quotes"),
        ("one", "\"one quote"),
        ("two", "\"\"two quotes"),
    ] {
        let value = get_map_entry(&decoded.store, decoded.root, key)
            .unwrap()
            .unwrap()
            .value;
        assert!(
            decoded.store.get(value).unwrap().value.contains(expected),
            "key={key}"
        );
    }
}

#[test]
fn corpus_toml_inline_table_newline_fixture_matches_zig_regression_subset() {
    let decoded = TomlDecoder
        .decode_str(TOML_INLINE_TABLE_NEWLINE_FIXTURE)
        .unwrap();

    let trailing_comma = get_map_entry(&decoded.store, decoded.root, "trailing-comma-1")
        .unwrap()
        .unwrap()
        .value;
    let tbl = get_map_entry(&decoded.store, decoded.root, "tbl-1")
        .unwrap()
        .unwrap()
        .value;
    let nested_tbl = get_map_entry(&decoded.store, tbl, "tbl")
        .unwrap()
        .unwrap()
        .value;
    let nested_key = get_map_entry(&decoded.store, nested_tbl, "k")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(
        decoded.store.get(trailing_comma).unwrap().kind,
        TreeNodeKind::Mapping
    );
    assert_eq!(
        decoded.store.get(nested_tbl).unwrap().kind,
        TreeNodeKind::Mapping
    );
    assert_eq!(decoded.store.get(nested_key).unwrap().value, "1");
}

#[test]
fn corpus_toml_hex_escape_fixture_matches_zig_regression_subset() {
    let decoded = TomlDecoder.decode_str(TOML_HEX_ESCAPE_FIXTURE).unwrap();

    let hello = get_map_entry(&decoded.store, decoded.root, "hello")
        .unwrap()
        .unwrap()
        .value;
    let higher = get_map_entry(&decoded.store, decoded.root, "higher-than-127")
        .unwrap()
        .unwrap()
        .value;
    let multiline = get_map_entry(&decoded.store, decoded.root, "multiline")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(decoded.store.get(hello).unwrap().value, "hello\n");
    assert_eq!(decoded.store.get(higher).unwrap().value, "Sørmirbæren");
    assert!(
        decoded
            .store
            .get(multiline)
            .unwrap()
            .value
            .contains("Sørmirbæren")
    );
}

#[test]
fn corpus_toml_invalid_array_table_fixture_matches_zig_regression_subset() {
    assert!(
        TomlDecoder
            .decode_str(TOML_INVALID_ARRAY_TABLE_FIXTURE)
            .is_err()
    );
}

#[test]
fn corpus_toml_invalid_datetime_no_seconds_fixture_matches_zig_regression_subset() {
    assert_eq!(
        decode_toml_fixture(
            TOML_INVALID_DATETIME_NO_SECONDS_FIXTURE,
            "spec-1.0.0__datetime__no-secs.0.toml",
        )
        .unwrap_err(),
        CoreError::Parse(ParseError::BadTomlDocument)
    );
}

#[test]
fn corpus_toml_datetime_no_seconds_fixture_matches_zig_regression_subset() {
    let decoded = TomlDecoder
        .decode_str(TOML_DATETIME_NO_SECONDS_FIXTURE)
        .unwrap();

    for (key, expected) in [
        ("without-seconds-1", "13:37"),
        ("without-seconds-2", "1979-05-27 07:32Z"),
        ("without-seconds-3", "1979-05-27 07:32-07:00"),
        ("without-seconds-4", "1979-05-27T07:32"),
    ] {
        let value = get_map_entry(&decoded.store, decoded.root, key)
            .unwrap()
            .unwrap()
            .value;
        assert_eq!(decoded.store.get(value).unwrap().value, expected);
    }
}

#[test]
fn corpus_toml_invalid_table_redefinition_fixture_matches_zig_regression_subset() {
    assert!(
        TomlDecoder
            .decode_str(TOML_INVALID_TABLE_REDEFINITION_FIXTURE)
            .is_err()
    );
}

#[test]
fn corpus_toml_inline_table_common_47_fixture_matches_zig_regression_subset() {
    let decoded = TomlDecoder
        .decode_str(TOML_INLINE_TABLE_COMMON_47_FIXTURE)
        .unwrap();

    let name = get_map_entry(&decoded.store, decoded.root, "name")
        .unwrap()
        .unwrap()
        .value;
    let first = get_map_entry(&decoded.store, name, "first")
        .unwrap()
        .unwrap()
        .value;
    let animal = get_map_entry(&decoded.store, decoded.root, "animal")
        .unwrap()
        .unwrap()
        .value;
    let animal_type = get_map_entry(&decoded.store, animal, "type")
        .unwrap()
        .unwrap()
        .value;
    let type_name = get_map_entry(&decoded.store, animal_type, "name")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(decoded.store.get(first).unwrap().value, "Tom");
    assert_eq!(decoded.store.get(type_name).unwrap().value, "pug");
}

#[test]
fn corpus_toml_literal_multiline_quotes_invalid_fixture_matches_zig_regression_subset() {
    assert_eq!(
        TomlDecoder
            .decode_str(TOML_LITERAL_MULTILINE_QUOTES_INVALID_FIXTURE)
            .unwrap_err(),
        CoreError::Parse(ParseError::BadTomlDocument)
    );
}

#[test]
fn corpus_toml_integer_literal_fixtures_match_zig_regression_subset() {
    for input in [
        TOML_INTEGER_LITERALS_FIXTURE,
        TOML_COMMON_INTEGER_FIXTURE,
        TOML_INTEGER_SPEC_FIXTURE,
    ] {
        let decoded = TomlDecoder.decode_str(input).unwrap();
        let hex = get_map_entry(&decoded.store, decoded.root, "hex1")
            .unwrap()
            .unwrap()
            .value;
        let oct = get_map_entry(&decoded.store, decoded.root, "oct2")
            .unwrap()
            .unwrap()
            .value;
        let bin = get_map_entry(&decoded.store, decoded.root, "bin1")
            .unwrap()
            .unwrap()
            .value;

        assert_eq!(decoded.store.get(hex).unwrap().sem_type, Some(SemType::Int));
        assert_eq!(decoded.store.get(oct).unwrap().sem_type, Some(SemType::Int));
        assert_eq!(decoded.store.get(bin).unwrap().sem_type, Some(SemType::Int));
    }
}

#[test]
fn corpus_toml_long_integer_fixture_matches_zig_regression_subset() {
    let decoded = TomlDecoder.decode_str(TOML_INTEGER_LONG_FIXTURE).unwrap();

    let max = get_map_entry(&decoded.store, decoded.root, "int64-max")
        .unwrap()
        .unwrap()
        .value;
    let min = get_map_entry(&decoded.store, decoded.root, "int64-max-neg")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(decoded.store.get(max).unwrap().value, "9223372036854775807");
    assert_eq!(
        decoded.store.get(min).unwrap().value,
        "-9223372036854775808"
    );
}

#[test]
fn corpus_toml_zero_integer_fixture_matches_zig_regression_subset() {
    let decoded = TomlDecoder.decode_str(TOML_INTEGER_ZERO_FIXTURE).unwrap();

    for key in [
        "d1", "d2", "d3", "h1", "h2", "h3", "o1", "a2", "a3", "b1", "b2", "b3",
    ] {
        let value = get_map_entry(&decoded.store, decoded.root, key)
            .unwrap()
            .unwrap()
            .value;
        assert_eq!(
            decoded.store.get(value).unwrap().sem_type,
            Some(SemType::Int)
        );
    }
}

#[test]
fn corpus_yaml_verbatim_tag_fixture_matches_zig_regression_subset() {
    let decoded = YamlDecoder.decode_str(YAML_VERBATIM_TAG_FIXTURE).unwrap();

    let root = decoded.store.get(decoded.root).unwrap();
    assert_eq!(root.kind, TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 2);
}

#[test]
fn corpus_yaml_verbatim_tag_example_fixture_matches_zig_regression_subset() {
    let decoded = YamlDecoder
        .decode_str(YAML_VERBATIM_TAG_EXAMPLE_FIXTURE)
        .unwrap();

    let foo = get_map_entry(&decoded.store, decoded.root, "foo")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(
        decoded.store.get(decoded.root).unwrap().kind,
        TreeNodeKind::Mapping
    );
    assert_eq!(decoded.store.get(foo).unwrap().value, "baz");
}

#[test]
fn corpus_yaml_directive_boundary_fixture_matches_zig_regression_subset() {
    assert!(matches!(
        YamlDecoder
            .decode_str(YAML_DIRECTIVE_INVALID_FIXTURE)
            .unwrap_err(),
        CoreError::Parse(ParseError::InvalidSyntax | ParseError::InvalidYaml)
    ));
}

#[test]
fn corpus_yaml_bare_documents_fixture_matches_zig_regression_subset() {
    let decoded = YamlDecoder.decode_str(YAML_BARE_DOCUMENTS_FIXTURE).unwrap();

    assert_eq!(
        decoded.store.get(decoded.root).unwrap().kind,
        TreeNodeKind::Scalar
    );
    assert_eq!(
        decoded.store.get(decoded.root).unwrap().value,
        "Bare document"
    );
}

#[test]
fn corpus_yaml_directive_boundary_valid_fixture_matches_zig_regression_subset() {
    let decoded = YamlDecoder
        .decode_str(YAML_DIRECTIVE_BOUNDARY_VALID_FIXTURE)
        .unwrap();

    assert_eq!(
        decoded.store.get(decoded.root).unwrap().kind,
        TreeNodeKind::Scalar
    );
    assert_eq!(decoded.store.get(decoded.root).unwrap().value, "scalar1");
}

#[test]
fn corpus_yaml_directive_boundary_invalid_fixture_matches_zig_regression_subset() {
    assert!(
        YamlDecoder
            .decode_str(YAML_DIRECTIVE_BOUNDARY_INVALID_FIXTURE)
            .is_err()
    );
}

#[test]
fn corpus_yaml_undeclared_tag_handle_later_document_matches_zig_regression_subset() {
    assert_eq!(
        YamlDecoder
            .decode_str(YAML_UNDECLARED_LATER_DOCUMENT_FIXTURE)
            .unwrap_err(),
        CoreError::Parse(ParseError::InvalidSyntax)
    );
}

#[test]
fn corpus_yaml_invalid_after_document_end_fixture_matches_zig_regression_subset() {
    assert!(matches!(
        YamlDecoder
            .decode_str(YAML_INVALID_AFTER_DOCUMENT_END_FIXTURE)
            .unwrap_err(),
        CoreError::Parse(ParseError::InvalidSyntax | ParseError::InvalidYaml)
    ));
}

#[test]
fn corpus_yaml_alias_survives_graph_paths_matches_zig_regression_subset() {
    let input = "a: &x {k: v}\nb: *x\n";
    let lhs = YamlDecoder.decode_str(input).unwrap();
    let rhs = YamlDecoder.decode_str(input).unwrap();

    let lhs_root = lhs.store.get(lhs.root).unwrap();
    let rhs_root = rhs.store.get(rhs.root).unwrap();
    assert_eq!(lhs_root.kind, TreeNodeKind::Mapping);
    assert_eq!(rhs_root.kind, TreeNodeKind::Mapping);

    let lhs_graph = build_graph(&lhs.store, lhs.root);
    let rhs_graph = build_graph(&rhs.store, rhs.root);
    assert!(!lhs_graph.nodes.is_empty());
    assert_eq!(lhs_graph.nodes.len(), rhs_graph.nodes.len());
    assert_eq!(lhs_graph.edges.len(), rhs_graph.edges.len());
    assert_eq!(lhs_graph.nodes[0].kind, rhs_graph.nodes[0].kind);
}
