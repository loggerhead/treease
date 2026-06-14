use treease_core::core::{
    CodecService, CoreError, FormatDefinition, FormatError, FormatLanguage, FormatRegistry,
    ParseError, RegistryFormatPreferences, SemType, TreeNode, TreeNodeKind, TreeStore,
    format_from_string, format_string_from_filename, get_map_entry,
};
use treease_core::formats::{
    CsvDecoder, CsvEncoder, CsvObjectDecoder, Decode, Encode, FormatPreferences, JavascriptEncoder,
    JavascriptObjectDecoder, JsonDecoder, JsonEncoder, PythonEncoder, PythonObjectDecoder,
    TomlDecoder, TomlEncoder, YamlDecoder, YamlEncoder, default_language_preferences,
    formats_helpers::ArenaDecoderState,
};
use treease_core::stream::{DecodeOptions, StreamingEvent, decode_with_options};

fn compact_json_encoder() -> JsonEncoder {
    let mut prefs = default_language_preferences().effective(FormatLanguage::Json);
    prefs.indent = 0;
    prefs.smart = false;
    prefs.unwrap_scalar = false;
    JsonEncoder::new(prefs)
}

fn registry_json_encoder_factory(
    prefs: &RegistryFormatPreferences,
) -> Result<Box<dyn Encode>, CoreError> {
    let mut json_prefs = default_language_preferences().effective(FormatLanguage::Json);
    json_prefs.indent = prefs.indent;
    json_prefs.unwrap_scalar = false;
    Ok(Box::new(JsonEncoder::new(json_prefs)))
}

fn registry_json_decoder_factory(
    _prefs: &RegistryFormatPreferences,
) -> Result<Box<dyn Decode>, CoreError> {
    Ok(Box::new(JsonDecoder))
}

fn registry_yaml_encoder_factory(
    prefs: &RegistryFormatPreferences,
) -> Result<Box<dyn Encode>, CoreError> {
    let mut yaml_prefs = default_language_preferences().effective(FormatLanguage::Yaml);
    yaml_prefs.indent = prefs.indent;
    Ok(Box::new(YamlEncoder::new(yaml_prefs)))
}

fn registry_yaml_decoder_factory(
    _prefs: &RegistryFormatPreferences,
) -> Result<Box<dyn Decode>, CoreError> {
    Ok(Box::new(YamlDecoder))
}

#[test]
fn json_decoder_builds_node_id_tree_and_encoder_writes_json() {
    let decoded = JsonDecoder
        .decode_str(r#"{"name":"Ada","values":[1,true,null]}"#)
        .unwrap();

    let root = decoded.store.get(decoded.root).unwrap();
    assert_eq!(root.kind, TreeNodeKind::Mapping);
    let name = get_map_entry(&decoded.store, decoded.root, "name")
        .unwrap()
        .unwrap()
        .value;
    assert_eq!(
        decoded.store.get(name).unwrap().sem_type,
        Some(SemType::Str)
    );

    let encoded = JsonEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert!(encoded.contains(r#""name": "Ada""#));
    assert!(encoded.contains(r#""values": ["#));
}

#[test]
fn format_registry_creates_codecs_from_registered_factories_like_zig() {
    let mut registry = FormatRegistry::init();
    registry.register_format(FormatDefinition {
        name: "json".to_owned(),
        encoder_symbol: Some("legacy_encoder_symbol".to_owned()),
        decoder_symbol: Some("legacy_decoder_symbol".to_owned()),
        encoder_prefs_symbol: None,
        decoder_prefs_symbol: None,
    });
    registry.register_encoder_factory("json", registry_json_encoder_factory);
    registry.register_decoder_factory("json", registry_json_decoder_factory);

    let prefs = RegistryFormatPreferences {
        indent: 0,
        pretty: false,
    };
    let decoded = registry
        .create_decoder_by_prefs("json", &prefs)
        .expect("decoder factory should be used")
        .decode_str(r#"{"name":"Ada"}"#)
        .unwrap();
    let encoded = registry
        .create_encoder_by_prefs("json", &prefs)
        .expect("encoder factory should be used")
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, "{\"name\":\"Ada\"}\n");
}

#[test]
fn codec_service_with_registry_uses_registered_factories_like_zig() {
    let mut registry = FormatRegistry::init();
    registry.register_format(FormatDefinition {
        name: "yaml".to_owned(),
        encoder_symbol: Some("legacy_yaml_encoder".to_owned()),
        decoder_symbol: Some("legacy_yaml_decoder".to_owned()),
        encoder_prefs_symbol: None,
        decoder_prefs_symbol: None,
    });
    registry.register_encoder_factory("yaml", registry_yaml_encoder_factory);
    registry.register_decoder_factory("yaml", registry_yaml_decoder_factory);

    let service = CodecService::with_registry(registry);

    assert!(matches!(
        service.get_encoder("json", 2),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));
    assert!(matches!(
        service.get_decoder("json"),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));

    let decoded = service.decode("yaml", "name: Ada\n").unwrap();
    let encoded = service
        .encode_to_string("yaml", &decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, "name: Ada\n");
}

#[test]
fn json_ported_compact_roundtrip_scenarios_run() {
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

        assert_eq!(encoded, expected);
    }
}

#[test]
fn json_ported_decode_to_yaml_keeps_nested_values_and_key_order() {
    let decoded = JsonDecoder
        .decode_str(r#"{"a":"first","b":{"c":2,"d":[3,4]},"ab":"last"}"#)
        .unwrap();

    let root = decoded.store.get(decoded.root).unwrap();
    let keys: Vec<_> = root
        .content
        .chunks_exact(2)
        .map(|pair| decoded.store.get(pair[0]).unwrap().value.as_str())
        .collect();
    assert_eq!(keys, ["a", "b", "ab"]);

    let encoded = YamlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert_eq!(
        encoded,
        "a: first\nb:\n  c: 2\n  d:\n    - 3\n    - 4\nab: last\n"
    );
}

#[test]
fn json_ported_decodes_numbers_booleans_and_null_sem_types() {
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
fn json_ported_rejects_invalid_and_adjacent_documents() {
    assert_eq!(
        JsonDecoder.decode_str(r#"{"a": 1 b": 2}"#).unwrap_err(),
        CoreError::Parse(ParseError::InvalidJson)
    );
    assert_eq!(
        JsonDecoder.decode_str("{\"a\":1}\n{\"b\":2}").unwrap_err(),
        CoreError::Parse(ParseError::InvalidJson)
    );
}

#[test]
fn json_ported_rejects_empty_input_like_zig() {
    assert_eq!(
        JsonDecoder.decode_str("").unwrap_err(),
        CoreError::Parse(ParseError::InvalidJson)
    );
}

#[test]
fn decoder_helper_arena_state_resets_and_releases_like_zig() {
    let mut state = ArenaDecoderState::new();

    assert!(!state.is_inited());

    state.init();
    assert!(state.is_inited());
    let first = state.alloc_bytes(4, 1);
    first.copy_from_slice(b"rust");
    assert_eq!(first, b"rust");

    state.reset();
    let second = state.alloc_bytes(2, 1);
    second.copy_from_slice(b"rs");
    assert_eq!(second, b"rs");
    assert!(state.is_inited());

    state.release();
    assert!(!state.is_inited());
}

#[test]
fn decoder_json_multidoc_ported_rejects_consecutive_multiline_documents() {
    let input = "{\n  \"this\": \"is a multidoc json file\"\n}\n{\n  \"it\": [\n    \"has\",\n    \"consecutive\",\n    \"json documents\"\n  ]\n}\n{\n  \"a number\": 4\n}";

    assert_eq!(
        JsonDecoder.decode_str(input).unwrap_err(),
        CoreError::Parse(ParseError::InvalidJson)
    );
}

#[test]
fn json_nested_option_ported_controls_nested_string_expansion() {
    let input = r#"{"a":"{\"b\":1}"}"#;
    let disabled = decode_with_options(
        "json",
        input,
        DecodeOptions {
            emit_path: true,
            ..DecodeOptions::default()
        },
    )
    .unwrap();
    let enabled = decode_with_options(
        "json",
        input,
        DecodeOptions {
            nest_json: true,
            emit_path: true,
        },
    )
    .unwrap();

    assert!(disabled.iter().any(|event| matches!(
        event,
        StreamingEvent::Scalar { value, meta }
            if value == r#"{"b":1}"# && meta.path == "$.a"
    )));
    assert!(!disabled.iter().any(|event| matches!(
        event,
        StreamingEvent::MapStart(meta) if meta.path == "$.a"
    )));
    assert!(enabled.iter().any(|event| matches!(
        event,
        StreamingEvent::MapStart(meta) if meta.path == "$.a"
    )));
    assert!(enabled.iter().any(|event| matches!(
        event,
        StreamingEvent::Scalar { value, meta }
            if value == "1" && meta.path == "$.a.b"
    )));
}

#[test]
fn yaml_decoder_and_encoder_handle_nested_mapping() {
    let decoded = YamlDecoder
        .decode_str("name: Ada\nskills:\n  - rust\n")
        .unwrap();

    let skills = get_map_entry(&decoded.store, decoded.root, "skills")
        .unwrap()
        .unwrap()
        .value;
    assert_eq!(
        decoded.store.get(skills).unwrap().kind,
        TreeNodeKind::Sequence
    );

    let encoded = YamlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert!(encoded.contains("name: Ada"));
    assert!(encoded.contains("- rust"));
}

#[test]
fn yaml_ported_decode_scalars_sequence_and_mapping() {
    let decoded = YamlDecoder
        .decode_str("a: Easy! as one two three\nb:\n  c: 2\n  d:\n    - 3\n    - 4\n")
        .unwrap();

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
    assert_eq!(
        encoded,
        "a: Easy! as one two three\nb:\n  c: 2\n  d:\n    - 3\n    - 4\n"
    );
}

#[test]
fn yaml_ported_rejects_invalid_document() {
    assert_eq!(
        YamlDecoder.decode_str("a: [1, 2\n").unwrap_err(),
        CoreError::Parse(ParseError::InvalidYaml)
    );
}

#[test]
fn yaml_ported_empty_input_decodes_to_nil_root() {
    let decoded = YamlDecoder.decode_str("").unwrap();

    assert_eq!(
        decoded.store.get(decoded.root).unwrap().sem_type,
        Some(SemType::Nil)
    );
    assert_eq!(
        YamlEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap(),
        "null\n"
    );
}

#[test]
fn yaml_ported_decodes_missing_mapping_value_as_empty_string_like_zig() {
    let decoded = YamlDecoder.decode_str("empty:\n").unwrap();
    let empty = get_map_entry(&decoded.store, decoded.root, "empty")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(
        decoded.store.get(empty).unwrap().sem_type,
        Some(SemType::Str)
    );
    assert_eq!(decoded.store.get(empty).unwrap().value, "");
}

#[test]
fn yaml_ported_keeps_quoted_scalar_escape_text_like_zig() {
    let decoded = YamlDecoder
        .decode_str("message: \"line\\nnext\"\n")
        .unwrap();
    let message = get_map_entry(&decoded.store, decoded.root, "message")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(decoded.store.get(message).unwrap().value, "line\nnext");
}

#[test]
fn yaml_ported_keeps_single_quote_escape_text_like_zig() {
    let decoded = YamlDecoder.decode_str("message: 'it''s'\n").unwrap();
    let message = get_map_entry(&decoded.store, decoded.root, "message")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(decoded.store.get(message).unwrap().value, "it's");
}

#[test]
fn yaml_decoder_ported_decode_all_multiple_documents() {
    let docs = CodecService::new()
        .decode_all("yaml", "a: 1\n---\n- 2\n")
        .unwrap();

    assert_eq!(docs.len(), 2);
    assert_eq!(
        docs[0].store.get(docs[0].root).unwrap().kind,
        TreeNodeKind::Mapping
    );
    assert_eq!(
        docs[1].store.get(docs[1].root).unwrap().kind,
        TreeNodeKind::Sequence
    );
}

#[test]
fn toml_decoder_and_encoder_handle_tables_and_scalars() {
    let decoded = TomlDecoder
        .decode_str("name = \"Ada\"\ncount = 2\n[meta]\nactive = true\n")
        .unwrap();

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

#[test]
fn toml_ported_roundtrips_nested_table() {
    let input = "a = \"b\"\n\n[c]\ne = \"f\"\n";
    let decoded = TomlDecoder.decode_str(input).unwrap();

    let encoded = TomlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, input);
}

#[test]
fn toml_ported_decodes_arrays_and_booleans_to_yaml() {
    let decoded = TomlDecoder
        .decode_str("A = \"B\"\nD = true\nM = [\"1\", 2, true]\n[N]\na = \"b\"\n")
        .unwrap();

    let encoded = YamlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(
        encoded,
        "A: B\nD: true\nM:\n  - '1'\n  - 2\n  - true\nN:\n  a: b\n"
    );
}

#[test]
fn toml_ported_rejects_bad_document_and_unsupported_null() {
    assert_eq!(
        TomlDecoder.decode_str("[a]\nb = \"boogie").unwrap_err(),
        CoreError::Parse(ParseError::BadTomlDocument)
    );

    let decoded = JsonDecoder.decode_str("null").unwrap();
    assert_eq!(
        TomlEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap_err(),
        CoreError::Parse(ParseError::UnsupportedTomlValue)
    );
}

#[test]
fn toml_ported_rejects_null_array_items_from_json_input() {
    let decoded = JsonDecoder.decode_str(r#"{"list":[1,null]}"#).unwrap();
    assert_eq!(
        TomlEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap_err(),
        CoreError::Parse(ParseError::UnsupportedTomlValue)
    );
}

#[test]
fn toml_ported_skips_null_map_values_like_zig() {
    let decoded = JsonDecoder
        .decode_str(r#"{"name":"Ada","missing":null}"#)
        .unwrap();

    let encoded = TomlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, "name = \"Ada\"\n");
}

#[test]
fn toml_ported_skips_null_fields_inside_inline_tables_like_zig() {
    let decoded = JsonDecoder
        .decode_str(r#"{"list":[{"a":1,"missing":null},2]}"#)
        .unwrap();

    let encoded = TomlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, "list = [{ a = 1 }, 2]\n");
}

#[test]
fn toml_ported_decodes_inline_tables_and_nested_arrays() {
    let decoded = TomlDecoder
        .decode_str("profile = { name = \"Ada\", nested = { score = 1 }, flags = [true, false] }\n")
        .unwrap();
    let profile = get_map_entry(&decoded.store, decoded.root, "profile")
        .unwrap()
        .unwrap()
        .value;
    let nested = get_map_entry(&decoded.store, profile, "nested")
        .unwrap()
        .unwrap()
        .value;
    let score = get_map_entry(&decoded.store, nested, "score")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(decoded.store.get(score).unwrap().value, "1");
}

#[test]
fn toml_ported_decodes_string_escapes_like_zig() {
    let decoded = TomlDecoder
        .decode_str("text = \"a\\bb\\fc\\u0041\"\n")
        .unwrap();
    let text = get_map_entry(&decoded.store, decoded.root, "text")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(decoded.store.get(text).unwrap().value, "a\u{08}b\u{0c}cA");
}

#[test]
fn toml_ported_decodes_timestamp_as_tagged_string_like_zig() {
    let decoded = TomlDecoder.decode_str("date = 2024-01-02\n").unwrap();
    let date = get_map_entry(&decoded.store, decoded.root, "date")
        .unwrap()
        .unwrap()
        .value;
    let node = decoded.store.get(date).unwrap();

    assert_eq!(node.value, "2024-01-02");
    assert_eq!(node.tag, "!!timestamp");
}

#[test]
fn toml_ported_rejects_invalid_timestamp_dates_like_zig() {
    assert!(TomlDecoder.decode_str("date = 2024-99-99\n").is_err());
    assert!(TomlDecoder.decode_str("date = 2024-02-31\n").is_err());
    assert!(TomlDecoder.decode_str("date = 2024-01-02abc\n").is_err());
}

#[test]
fn toml_ported_decodes_dotted_assignment_keys_as_nested_tables() {
    let decoded = TomlDecoder
        .decode_str("meta.profile.name = \"Alice\"\n")
        .unwrap();
    let meta = get_map_entry(&decoded.store, decoded.root, "meta")
        .unwrap()
        .unwrap()
        .value;
    let profile = get_map_entry(&decoded.store, meta, "profile")
        .unwrap()
        .unwrap()
        .value;
    let name = get_map_entry(&decoded.store, profile, "name")
        .unwrap()
        .unwrap()
        .value;

    assert_eq!(decoded.store.get(name).unwrap().value, "Alice");
}

#[test]
fn toml_ported_roundtrips_literal_dotted_key() {
    let input = "[\"meta.profile\"]\nname = \"Alice\"\n";
    let decoded = TomlDecoder.decode_str(input).unwrap();
    let encoded = TomlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert_eq!(encoded, input);
}

#[test]
fn toml_ported_roundtrips_parent_attrs_before_array_tables() {
    let input = "[meta]\nid = \"item-001\"\nflags = [true, false, true]\n\n[[meta.items]]\nname = \"item-a\"\nvalue = 1\n";
    let decoded = TomlDecoder.decode_str(input).unwrap();
    let encoded = TomlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert_eq!(encoded, input);
}

#[test]
fn format_ported_resolves_aliases_and_filename_extensions() {
    assert_eq!(format_from_string("yaml").unwrap().formal_name, "yaml");
    assert_eq!(format_from_string("yml").unwrap().formal_name, "yaml");
    assert_eq!(format_from_string("py").unwrap().formal_name, "python");
    assert_eq!(format_from_string("js").unwrap().formal_name, "javascript");
    assert_eq!(
        format_from_string("doc").unwrap_err(),
        CoreError::Format(FormatError::UnknownFormat)
    );
    assert_eq!(
        format_from_string("").unwrap_err(),
        CoreError::Format(FormatError::UnknownFormat)
    );

    // format_from_string aliases (matching Zig format.zig)
    assert_eq!(format_from_string("y").unwrap().formal_name, "yaml");
    assert_eq!(format_from_string("j").unwrap().formal_name, "json");

    // format_string_from_filename known extensions
    assert_eq!(format_string_from_filename("a.yml"), "yaml");
    assert_eq!(format_string_from_filename("a.yaml"), "yaml");
    assert_eq!(format_string_from_filename("a.json"), "json");
    assert_eq!(format_string_from_filename("a.py"), "python");
    assert_eq!(format_string_from_filename("a.js"), "javascript");
    assert_eq!(format_string_from_filename("a.mjs"), "javascript");
    assert_eq!(format_string_from_filename("a.cjs"), "javascript");

    // format_string_from_filename case handling
    assert_eq!(format_string_from_filename("test.Yaml"), "yaml");
    assert_eq!(format_string_from_filename("test.index.Yaml"), "yaml");
    assert_eq!(format_string_from_filename("test.json"), "json");
    assert_eq!(format_string_from_filename("TEST.JSON"), "json");
    assert_eq!(format_string_from_filename("test.json/foo"), "json");
    assert_eq!(format_string_from_filename(""), "json");
    assert_eq!(format_string_from_filename("test"), "json");

    // format_string_from_filename unknown extensions
    assert_eq!(format_string_from_filename("a.abc"), "abc");
    assert_eq!(format_string_from_filename("a.AbC"), "AbC");
}

#[test]
fn python_object_decoder_accepts_python_literals_and_unquoted_keys() {
    let decoded = PythonObjectDecoder
        .decode_str("{'name': 'Ada', 'active': True, 'none': None, 'nums': [1, 2,],}")
        .unwrap();

    let encoded = compact_json_encoder()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(
        encoded,
        "{\"name\":\"Ada\",\"active\":true,\"none\":null,\"nums\":[1,2]}\n"
    );
}

#[test]
fn javascript_object_decoder_accepts_js_literals_comments_and_unquoted_keys() {
    let decoded = JavascriptObjectDecoder
        .decode_str("({name: 'Ada', active: true, none: null, nums: [1, 2,]})")
        .unwrap();

    let encoded = compact_json_encoder()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(
        encoded,
        "{\"name\":\"Ada\",\"active\":true,\"none\":null,\"nums\":[1,2]}\n"
    );
}

#[test]
fn javascript_object_decoder_rejects_empty_braced_unicode_escape_like_zig() {
    assert_eq!(
        JavascriptObjectDecoder
            .decode_str(r#"({name: "\u{}"})"#)
            .unwrap_err(),
        CoreError::Parse(ParseError::InvalidJavaScript)
    );
}

#[test]
fn yaml_ported_missing_tag_payload_decodes_to_nil_empty_value_like_zig() {
    let decoded = YamlDecoder.decode_str("key: !tag\n").unwrap();
    let value = get_map_entry(&decoded.store, decoded.root, "key")
        .unwrap()
        .unwrap()
        .value;
    let node = decoded.store.get(value).unwrap();

    assert_eq!(node.sem_type, Some(SemType::Nil));
    assert_eq!(node.value, "");
}

#[test]
fn toml_ported_quotes_root_string_scalar_like_zig() {
    let decoded = JsonDecoder.decode_str(r#""Ada""#).unwrap();
    let encoded = TomlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, "\"Ada\"\n");
}

#[test]
fn csv_decoder_builds_rows_as_nested_sequences() {
    let decoded = CsvDecoder.decode_str("a,b\n1,\"two,three\"\n").unwrap();

    let encoded = compact_json_encoder()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, "[[\"a\",\"b\"],[\"1\",\"two,three\"]]\n");
}

#[test]
fn csv_ported_empty_input_creates_empty_sequence_like_zig() {
    let decoded = CsvDecoder.decode_str("").unwrap();

    let encoded = compact_json_encoder()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();

    assert_eq!(encoded, "[]\n");
}

#[test]
fn csv_ported_rejects_unterminated_quoted_field() {
    assert_eq!(
        CsvDecoder.decode_str("\"unterminated").unwrap_err(),
        CoreError::Parse(ParseError::BadCsv)
    );
}

#[test]
fn language_preferences_match_ported_defaults() {
    let prefs = default_language_preferences();

    assert!(!prefs.effective(FormatLanguage::Json).unwrap_scalar);
    assert!(prefs.effective(FormatLanguage::Yaml).print_doc_separators);
    assert_eq!(prefs.effective(FormatLanguage::Csv).separator, ',');
}

// ============================================================================
// Gap Group 1: Encoder Tests (ported from encoder.zig)
// ============================================================================

/// Helper: create a JSON encoder with given indent, unwrap_scalar=false, smart=false.
fn json_encoder(indent: i32) -> JsonEncoder {
    let mut prefs = default_language_preferences().effective(FormatLanguage::Json);
    prefs.indent = indent;
    prefs.unwrap_scalar = false;
    prefs.smart = false;
    JsonEncoder::new(prefs)
}

/// Helper: create a JSON encoder from full FormatPreferences.
fn json_encoder_with(prefs: FormatPreferences) -> JsonEncoder {
    JsonEncoder::new(prefs)
}

#[test]
fn json_encoder_preserves_object_order_like_zig() {
    let mut store = TreeStore::new();
    let encoder = json_encoder(2);

    let inner_map = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: "!!map".to_string(),
        ..TreeNode::default()
    });
    store
        .add_key_value_child(
            inner_map,
            TreeNode::scalar(SemType::Str, "cobra"),
            TreeNode::scalar(SemType::Str, "kai"),
        )
        .unwrap();
    store
        .add_key_value_child(
            inner_map,
            TreeNode::scalar(SemType::Str, "angus"),
            TreeNode::scalar(SemType::Str, "bob"),
        )
        .unwrap();

    let seq = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        tag: "!!seq".to_string(),
        ..TreeNode::default()
    });
    store
        .add_child(seq, store.get(inner_map).unwrap().clone())
        .unwrap();

    let root = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: "!!map".to_string(),
        ..TreeNode::default()
    });
    store
        .add_key_value_child(
            root,
            TreeNode::scalar(SemType::Str, "zabbix"),
            TreeNode::scalar(SemType::Str, "winner"),
        )
        .unwrap();
    store
        .add_key_value_child(
            root,
            TreeNode::scalar(SemType::Str, "apple"),
            TreeNode::scalar(SemType::Str, "great"),
        )
        .unwrap();
    store
        .add_key_value_child(
            root,
            TreeNode::scalar(SemType::Str, "banana"),
            store.get(seq).unwrap().clone(),
        )
        .unwrap();

    let actual = encoder.encode_to_string(&store, root).unwrap();
    let expected = concat!(
        "{\n",
        "  \"zabbix\": \"winner\",\n",
        "  \"apple\": \"great\",\n",
        "  \"banana\": [\n",
        "    {\n",
        "      \"cobra\": \"kai\",\n",
        "      \"angus\": \"bob\"\n",
        "    }\n",
        "  ]\n",
        "}\n",
    );
    assert_eq!(actual, expected);
}

#[test]
fn json_null_in_array_like_zig() {
    let mut store = TreeStore::new();
    let encoder = json_encoder(0);

    let seq = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        tag: "!!seq".to_string(),
        ..TreeNode::default()
    });
    store
        .add_child(seq, TreeNode::scalar(SemType::Nil, "null"))
        .unwrap();

    let actual = encoder.encode_to_string(&store, seq).unwrap();
    assert_eq!(actual, "[null]\n");
}

#[test]
fn json_null_scalar_like_zig() {
    let mut store = TreeStore::new();
    let encoder = json_encoder(0);

    let n = store.add(TreeNode::scalar(SemType::Nil, "null"));
    let actual = encoder.encode_to_string(&store, n).unwrap();
    assert_eq!(actual, "null\n");
}

#[test]
fn json_null_in_object_like_zig() {
    let mut store = TreeStore::new();
    let encoder = json_encoder(0);

    let root = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: "!!map".to_string(),
        ..TreeNode::default()
    });
    store
        .add_key_value_child(
            root,
            TreeNode::scalar(SemType::Str, "x"),
            TreeNode::scalar(SemType::Nil, "null"),
        )
        .unwrap();

    let actual = encoder.encode_to_string(&store, root).unwrap();
    assert_eq!(actual, "{\"x\":null}\n");
}

#[test]
fn json_encoder_does_not_escape_html_chars_like_zig() {
    let mut store = TreeStore::new();
    let encoder = json_encoder(0);

    let root = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: "!!map".to_string(),
        ..TreeNode::default()
    });
    store
        .add_key_value_child(
            root,
            TreeNode::scalar(SemType::Str, "build"),
            TreeNode::scalar(
                SemType::Str,
                "( ./lint && ./format && ./compile ) < src.code",
            ),
        )
        .unwrap();

    let actual = encoder.encode_to_string(&store, root).unwrap();
    assert_eq!(
        actual,
        "{\"build\":\"( ./lint && ./format && ./compile ) < src.code\"}\n"
    );
}

#[test]
fn json_smart_align_object_array_like_zig() {
    let mut store = TreeStore::new();

    let obj1 = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: "!!map".to_string(),
        ..TreeNode::default()
    });
    store
        .add_key_value_child(
            obj1,
            TreeNode::scalar(SemType::Str, "a"),
            TreeNode::scalar(SemType::Int, "1"),
        )
        .unwrap();
    store
        .add_key_value_child(
            obj1,
            TreeNode::scalar(SemType::Str, "bb"),
            TreeNode::scalar(SemType::Int, "2"),
        )
        .unwrap();

    let obj2 = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: "!!map".to_string(),
        ..TreeNode::default()
    });
    store
        .add_key_value_child(
            obj2,
            TreeNode::scalar(SemType::Str, "a"),
            TreeNode::scalar(SemType::Int, "10"),
        )
        .unwrap();
    store
        .add_key_value_child(
            obj2,
            TreeNode::scalar(SemType::Str, "bb"),
            TreeNode::scalar(SemType::Int, "3"),
        )
        .unwrap();

    let arr = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        tag: "!!seq".to_string(),
        ..TreeNode::default()
    });
    store
        .add_child(arr, store.get(obj1).unwrap().clone())
        .unwrap();
    store
        .add_child(arr, store.get(obj2).unwrap().clone())
        .unwrap();

    let mut prefs = default_language_preferences().effective(FormatLanguage::Json);
    prefs.indent = 2;
    prefs.unwrap_scalar = false;
    prefs.smart = true;
    prefs.max_line_length = 80;
    prefs.max_inline_complexity = 2;
    prefs.max_array_inline_items = 6;
    prefs.align_object_arrays = true;
    let encoder = json_encoder_with(prefs);

    let actual = encoder.encode_to_string(&store, arr).unwrap();
    let expected = concat!(
        "[\n",
        "  {\"a\":  1, \"bb\": 2},\n",
        "  {\"a\": 10, \"bb\": 3}\n",
        "]\n",
    );
    assert_eq!(actual, expected);
}

#[test]
fn javascript_smart_format_roundtrip_like_zig() {
    let js_content = include_str!("../../../example/simple.js");
    let decoded = JavascriptObjectDecoder.decode_str(js_content).unwrap();

    let mut prefs = default_language_preferences().effective(FormatLanguage::Javascript);
    prefs.indent = 2;
    prefs.unwrap_scalar = false;
    prefs.smart = true;
    prefs.max_line_length = 100;
    prefs.max_inline_complexity = 1;
    prefs.max_array_inline_items = 6;
    prefs.align_object_arrays = true;
    let encoder = JavascriptEncoder::new(prefs);

    let encoded = encoder
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    // The smart formatter should reproduce the original (minus trailing newline)
    assert_eq!(encoded.trim_end(), js_content.trim_end());
}

#[test]
fn python_smart_format_roundtrip_like_zig() {
    let py_content = include_str!("../../../example/simple.py");
    let decoded = PythonObjectDecoder.decode_str(py_content).unwrap();

    let mut prefs = default_language_preferences().effective(FormatLanguage::Python);
    prefs.indent = 2;
    prefs.unwrap_scalar = false;
    prefs.smart = true;
    prefs.max_line_length = 100;
    prefs.max_inline_complexity = 1;
    prefs.max_array_inline_items = 6;
    prefs.align_object_arrays = true;
    let encoder = PythonEncoder::new(prefs);

    let encoded = encoder
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    // The smart formatter should reproduce the original (minus trailing newline)
    assert_eq!(encoded.trim_end(), py_content.trim_end());
}

#[test]
fn json_smart_array_multi_line_like_zig() {
    let mut store = TreeStore::new();

    let arr = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        tag: "!!seq".to_string(),
        ..TreeNode::default()
    });
    for val in ["1", "2", "3", "4", "5", "6", "7"] {
        store
            .add_child(arr, TreeNode::scalar(SemType::Int, val))
            .unwrap();
    }

    let mut prefs = default_language_preferences().effective(FormatLanguage::Json);
    prefs.indent = 2;
    prefs.unwrap_scalar = false;
    prefs.smart = true;
    prefs.max_line_length = 80;
    prefs.max_inline_complexity = 1;
    prefs.max_array_inline_items = 4;
    prefs.align_object_arrays = true;
    let encoder = json_encoder_with(prefs);

    let actual = encoder.encode_to_string(&store, arr).unwrap();
    let expected = concat!("[\n", "  1, 2, 3, 4,\n", "  5, 6, 7\n", "]\n",);
    assert_eq!(actual, expected);
}

#[test]
fn json_smart_array_inline_respects_max_items_like_zig() {
    let mut store = TreeStore::new();

    let arr = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        tag: "!!seq".to_string(),
        ..TreeNode::default()
    });
    for val in ["1", "2", "3"] {
        store
            .add_child(arr, TreeNode::scalar(SemType::Int, val))
            .unwrap();
    }

    let mut prefs = default_language_preferences().effective(FormatLanguage::Json);
    prefs.indent = 2;
    prefs.unwrap_scalar = false;
    prefs.smart = true;
    prefs.max_line_length = 80;
    prefs.max_inline_complexity = 1;
    prefs.max_array_inline_items = 2;
    prefs.align_object_arrays = true;
    let encoder = json_encoder_with(prefs);

    let actual = encoder.encode_to_string(&store, arr).unwrap();
    let expected = concat!("[\n", "  1, 2,\n", "  3\n", "]\n",);
    assert_eq!(actual, expected);
}

#[test]
fn json_smart_array_inline_disabled_when_limit_exceeded_like_zig() {
    let mut store = TreeStore::new();

    let arr = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        tag: "!!seq".to_string(),
        ..TreeNode::default()
    });
    for val in ["1", "2", "3", "4", "5"] {
        store
            .add_child(arr, TreeNode::scalar(SemType::Int, val))
            .unwrap();
    }

    let mut prefs = default_language_preferences().effective(FormatLanguage::Json);
    prefs.indent = 2;
    prefs.unwrap_scalar = false;
    prefs.smart = true;
    prefs.max_line_length = 80;
    prefs.max_inline_complexity = 1;
    prefs.max_array_inline_items = 4;
    prefs.align_object_arrays = true;
    let encoder = json_encoder_with(prefs);

    let actual = encoder.encode_to_string(&store, arr).unwrap();
    let expected = concat!("[\n", "  1, 2, 3, 4,\n", "  5\n", "]\n",);
    assert_eq!(actual, expected);
}

#[test]
fn json_smart_object_array_no_align_different_keys_like_zig() {
    let mut store = TreeStore::new();

    let obj1 = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: "!!map".to_string(),
        ..TreeNode::default()
    });
    store
        .add_key_value_child(
            obj1,
            TreeNode::scalar(SemType::Str, "a"),
            TreeNode::scalar(SemType::Int, "1"),
        )
        .unwrap();
    store
        .add_key_value_child(
            obj1,
            TreeNode::scalar(SemType::Str, "bb"),
            TreeNode::scalar(SemType::Int, "2"),
        )
        .unwrap();

    let obj2 = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: "!!map".to_string(),
        ..TreeNode::default()
    });
    store
        .add_key_value_child(
            obj2,
            TreeNode::scalar(SemType::Str, "bb"),
            TreeNode::scalar(SemType::Int, "3"),
        )
        .unwrap();
    store
        .add_key_value_child(
            obj2,
            TreeNode::scalar(SemType::Str, "a"),
            TreeNode::scalar(SemType::Int, "10"),
        )
        .unwrap();

    let arr = store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        tag: "!!seq".to_string(),
        ..TreeNode::default()
    });
    store
        .add_child(arr, store.get(obj1).unwrap().clone())
        .unwrap();
    store
        .add_child(arr, store.get(obj2).unwrap().clone())
        .unwrap();

    let mut prefs = default_language_preferences().effective(FormatLanguage::Json);
    prefs.indent = 2;
    prefs.unwrap_scalar = false;
    prefs.smart = true;
    prefs.max_line_length = 80;
    prefs.max_inline_complexity = 2;
    prefs.max_array_inline_items = 6;
    prefs.align_object_arrays = true;
    let encoder = json_encoder_with(prefs);

    let actual = encoder.encode_to_string(&store, arr).unwrap();
    let expected = concat!(
        "[\n",
        "  {\"a\": 1, \"bb\": 2},\n",
        "  {\"bb\": 3, \"a\": 10}\n",
        "]\n",
    );
    assert_eq!(actual, expected);
}

#[test]
fn json_smart_inline_complexity_limit_like_zig() {
    let mut store = TreeStore::new();

    let inner = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: "!!map".to_string(),
        ..TreeNode::default()
    });
    store
        .add_key_value_child(
            inner,
            TreeNode::scalar(SemType::Str, "x"),
            TreeNode::scalar(SemType::Int, "1"),
        )
        .unwrap();
    store
        .add_key_value_child(
            inner,
            TreeNode::scalar(SemType::Str, "y"),
            TreeNode::scalar(SemType::Int, "2"),
        )
        .unwrap();

    let root = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        tag: "!!map".to_string(),
        ..TreeNode::default()
    });
    store
        .add_key_value_child(
            root,
            TreeNode::scalar(SemType::Str, "outer"),
            store.get(inner).unwrap().clone(),
        )
        .unwrap();
    store
        .add_key_value_child(
            root,
            TreeNode::scalar(SemType::Str, "n"),
            TreeNode::scalar(SemType::Int, "3"),
        )
        .unwrap();

    let mut prefs = default_language_preferences().effective(FormatLanguage::Json);
    prefs.indent = 2;
    prefs.unwrap_scalar = false;
    prefs.smart = true;
    prefs.max_line_length = 80;
    prefs.max_inline_complexity = 1;
    prefs.max_array_inline_items = 6;
    prefs.align_object_arrays = true;
    let encoder = json_encoder_with(prefs);

    let actual = encoder.encode_to_string(&store, root).unwrap();
    let expected = concat!(
        "{\n",
        "  \"outer\": {\"x\": 1, \"y\": 2},\n",
        "  \"n\": 3\n",
        "}\n",
    );
    assert_eq!(actual, expected);
}

// ============================================================================
// Gap Group 2: JSON Nest Feature Tests (from json.zig)
// NOTE: The Rust JSON decoder does not have setNestEnabled — the nest
// feature is only available in the streaming JSON parser. These tests
// cannot be directly ported. Skipped.
// ============================================================================

// ============================================================================
// Gap Group 3: Literal Tests (ported from literals.zig)
// ============================================================================

#[test]
fn python_literals_decode_encode_roundtrip_like_zig() {
    let input = concat!(
        "{ \"a\": 1, \"b\": [True, None, \"x\\n\"], \"c\": {\"k\": \"v\"} }\n",
        "{ \"z\": False }\n",
    );

    let mut prefs = default_language_preferences().effective(FormatLanguage::Python);
    prefs.indent = 2;
    prefs.unwrap_scalar = false;
    let encoder = PythonEncoder::new(prefs);

    let decoded = PythonObjectDecoder.decode_str(input).unwrap();
    let out = encoder
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    // Verify the output contains expected Python literal forms
    assert!(out.contains("'a': 1"));
    assert!(out.contains("True"));
    assert!(out.contains("None"));
    assert!(out.contains("'b'"));
    assert!(out.contains("'c'"));
    assert!(out.contains("'k'"));
    assert!(out.contains("'v'"));
    // The newline character within the string should appear somewhere
    assert!(out.contains("\\n"));
}

#[test]
fn javascript_literals_decode_encode_roundtrip_like_zig() {
    let input = "{a: 1, \"b\": [true, null]}\n";

    let mut prefs = default_language_preferences().effective(FormatLanguage::Javascript);
    prefs.indent = 2;
    prefs.unwrap_scalar = false;
    let encoder = JavascriptEncoder::new(prefs);

    let decoded = JavascriptObjectDecoder.decode_str(input).unwrap();
    let encoded = encoder
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert!(encoded.contains("a: 1"));
    assert!(encoded.contains("true"));
    assert!(encoded.contains("null"));
}

#[test]
fn python_decoder_maps_bool_and_none_like_zig() {
    let decoded = PythonObjectDecoder
        .decode_str("{'a': True, 'b': None}")
        .unwrap();
    let root = decoded.store.get(decoded.root).unwrap();
    assert_eq!(root.kind, TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 4);

    let key_a = decoded.store.get(root.content[0]).unwrap();
    assert_eq!(key_a.tag, "!!str");
    assert_eq!(key_a.value, "a");

    let val_a = decoded.store.get(root.content[1]).unwrap();
    assert_eq!(val_a.tag, "!!bool");
    assert_eq!(val_a.value, "true");

    let key_b = decoded.store.get(root.content[2]).unwrap();
    assert_eq!(key_b.tag, "!!str");
    assert_eq!(key_b.value, "b");

    let val_b = decoded.store.get(root.content[3]).unwrap();
    assert_eq!(val_b.tag, "!!null");
}

#[test]
fn javascript_decoder_maps_null_and_boolean_like_zig() {
    let decoded = JavascriptObjectDecoder
        .decode_str("({a: null, b: false})")
        .unwrap();
    let root = decoded.store.get(decoded.root).unwrap();
    assert_eq!(root.kind, TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 4);

    let key_a = decoded.store.get(root.content[0]).unwrap();
    assert_eq!(key_a.tag, "!!str");
    assert_eq!(key_a.value, "a");

    let val_a = decoded.store.get(root.content[1]).unwrap();
    assert_eq!(val_a.tag, "!!null");

    let key_b = decoded.store.get(root.content[2]).unwrap();
    assert_eq!(key_b.tag, "!!str");
    assert_eq!(key_b.value, "b");

    let val_b = decoded.store.get(root.content[3]).unwrap();
    assert_eq!(val_b.tag, "!!bool");
    assert_eq!(val_b.value, "false");
}

#[test]
fn javascript_decoder_accepts_parenthesized_escaped_string_keys() {
    let decoded = JavascriptObjectDecoder
        .decode_str(r#"({"new\\key": "value"})"#)
        .unwrap();
    let root = decoded.store.get(decoded.root).unwrap();
    assert_eq!(root.kind, TreeNodeKind::Mapping);

    let key = decoded.store.get(root.content[0]).unwrap();
    assert_eq!(key.value, r#"new\key"#);
}

#[test]
fn python_decoder_rejects_invalid_input_like_zig() {
    let err = PythonObjectDecoder.decode_str("{'a': }").unwrap_err();
    // Rust's tree-sitter Python decoder may return InvalidSyntax for
    // malformed input, while Zig returns InvalidPython. Both are parse errors.
    assert!(matches!(
        err,
        CoreError::Parse(ParseError::InvalidPython | ParseError::InvalidSyntax)
    ));
}

#[test]
fn javascript_decoder_rejects_invalid_input_like_zig() {
    assert_eq!(
        JavascriptObjectDecoder.decode_str("({a:})").unwrap_err(),
        CoreError::Parse(ParseError::InvalidJavaScript)
    );
}

#[test]
fn json_literals_reject_multiple_top_level_values_like_zig() {
    let input = concat!(
        "{ \"a\": 1 }\n",
        "[true, false, null]\n",
        "\"x\\u0041\"\n",
        "-12.5\n",
    );

    assert_eq!(
        JsonDecoder.decode_str(input).unwrap_err(),
        CoreError::Parse(ParseError::InvalidJson)
    );
}

#[test]
fn python_literals_comment_and_semicolon_splitting_like_zig() {
    let input = concat!("{'a': 1} # trailing comment\n", "{'b': 2}; {'c': 3}\n",);

    let decoded1 = PythonObjectDecoder.decode_str(input).unwrap();
    let root1 = decoded1.store.get(decoded1.root).unwrap();
    assert_eq!(root1.kind, TreeNodeKind::Mapping);
    // After Rust's decoder handles multi-document Python, the first parsed
    // mapping should be {'a': 1}. The decoder consumes as many top-level docs
    // as it can.
    let key_a = get_map_entry(&decoded1.store, decoded1.root, "a")
        .unwrap()
        .unwrap();
    assert_eq!(decoded1.store.get(key_a.value).unwrap().value, "1");
}

#[test]
fn javascript_literals_identifier_keys_like_zig() {
    let decoded = JavascriptObjectDecoder
        .decode_str("{a: 1, \"b\": [true, null]}\n")
        .unwrap();
    let root = decoded.store.get(decoded.root).unwrap();
    assert_eq!(root.kind, TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 4);

    let key_a = decoded.store.get(root.content[0]).unwrap();
    assert_eq!(key_a.value, "a");

    let val_a = decoded.store.get(root.content[1]).unwrap();
    assert_eq!(val_a.value, "1");

    let key_b = decoded.store.get(root.content[2]).unwrap();
    assert_eq!(key_b.value, "b");

    let val_b = decoded.store.get(root.content[3]).unwrap();
    assert_eq!(val_b.kind, TreeNodeKind::Sequence);
}

#[test]
fn decoders_invalid_input_returns_invalid_error_like_zig() {
    // Python decoder rejects lone '{'
    assert_eq!(
        PythonObjectDecoder.decode_str("{").unwrap_err(),
        CoreError::Parse(ParseError::InvalidPython)
    );

    // JavaScript decoder rejects lone '{'
    assert_eq!(
        JavascriptObjectDecoder.decode_str("{").unwrap_err(),
        CoreError::Parse(ParseError::InvalidJavaScript)
    );

    // JSON decoder rejects lone '{'
    assert_eq!(
        JsonDecoder.decode_str("{").unwrap_err(),
        CoreError::Parse(ParseError::InvalidJson)
    );
}

#[test]
fn json_encoder_unwrap_scalar_outputs_raw_value_like_zig() {
    let mut store = TreeStore::new();
    let node = store.add(TreeNode {
        kind: TreeNodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: SemType::Str.tag().to_owned(),
        value: "hello".to_owned(),
        ..TreeNode::default()
    });

    let mut prefs = default_language_preferences().effective(FormatLanguage::Json);
    prefs.indent = 2;
    prefs.unwrap_scalar = true;
    let encoder = JsonEncoder::new(prefs);

    let encoded = encoder.encode_to_string(&store, node).unwrap();
    assert_eq!(encoded, "hello\n");
}

// ============================================================================
// Gap Group 4: CSV Tests (ported from csv.zig)
// ============================================================================

#[test]
fn csv_defaults_like_zig() {
    let prefs = default_language_preferences().effective(FormatLanguage::Csv);
    assert_eq!(prefs.separator, ',');
    assert!(prefs.auto_parse);
}

#[test]
fn csv_encode_simple_like_zig() {
    // Decode YAML "[a, b]" and encode as CSV
    let decoded = YamlDecoder.decode_str("- a\n- b\n").unwrap();
    let encoder = CsvEncoder::default();
    let encoded = encoder
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert_eq!(encoded, "a,b\n");
}

#[test]
fn csv_decode_object_mode_like_zig() {
    let decoded = CsvObjectDecoder::default()
        .decode_str("a,b\n1,2\n")
        .unwrap();
    let encoded = YamlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert_eq!(encoded, "- a: 1\n  b: 2\n");
}

#[test]
fn csv_decode_with_bom_like_zig() {
    let input = "\u{feff}a,b\n1,2\n";
    let decoded = CsvObjectDecoder::default().decode_str(input).unwrap();
    let encoded = YamlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert_eq!(encoded, "- a: 1\n  b: 2\n");
}

#[test]
fn csv_decode_quoted_comma_like_zig() {
    let decoded = CsvObjectDecoder::default()
        .decode_str("a,b\n\"x,y\",z\n")
        .unwrap();
    let encoded = YamlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert_eq!(encoded, "- a: \"x,y\"\n  b: z\n");
}

#[test]
fn csv_decode_no_auto_parse_like_zig() {
    let mut prefs = default_language_preferences().effective(FormatLanguage::Csv);
    prefs.auto_parse = false;
    let decoded = CsvObjectDecoder::new(prefs)
        .decode_str("a,b\nnull,null\n")
        .unwrap();
    let encoded = YamlEncoder::default()
        .encode_to_string(&decoded.store, decoded.root)
        .unwrap();
    assert_eq!(encoded, "- a: null\n  b: null\n");
}
