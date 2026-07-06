// core_root.rs — root-level test aligned with tests/lib/root.zig
//
// The Zig test verifies:
//   1. refAllDecls root module
//   2. wasm language token capture mapping (token_type_ids, tokenTypeIdFromCapture)
//   3. wasm query sources (queryFromLanguage for yaml/toml/python/javascript)
//   4. wasm capture aliases consistency
//
// In Rust, the wasm_types module does not expose a tokenTypeIdFromCapture
// equivalent. Instead we verify:
//   - The crate compiles and basic types are accessible
//   - Language specs are available via lang_from_name
//   - Query sources are available via query_from_language
//   - Semantic token types are consistent

use std::collections::BTreeSet;

use treease_core::core::semantic_tokens::TOKEN_TYPES as SEMANTIC_TOKEN_TYPES;
use treease_core::core::{
    CompactTag, LANG_SPECS, SemType, TreeNode, TreeNodeKind, TreeStore,
    collect_token_spans_with_tree, encode_semantic_tokens, lang_from_name, parse_with_tree,
    query_capture_name_for_id, query_from_language, query_new, tree_sitter_language,
};

fn token_type_ids_for(language: &str, source: &str) -> BTreeSet<u32> {
    encode_semantic_tokens(language, source)
        .chunks_exact(5)
        .map(|chunk| chunk[3])
        .collect()
}

fn token_type_id(name: &str) -> u32 {
    SEMANTIC_TOKEN_TYPES
        .iter()
        .position(|candidate| *candidate == name)
        .expect("token type should exist") as u32
}

fn capture_names(query: &str) -> Vec<&str> {
    let bytes = query.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'@' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut end = start;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'.')
        {
            end += 1;
        }
        if end > start {
            out.push(&query[start..end]);
        }
        i = end.max(i + 1);
    }
    out
}

// ── Test 1: Language specs are available ─────────────────────────

#[test]
fn language_specs_are_available() {
    // Verify known languages resolve
    assert!(lang_from_name("json").is_some());
    assert!(lang_from_name("yaml").is_some());
    assert!(lang_from_name("yml").is_some());
    assert!(lang_from_name("toml").is_some());
    assert!(lang_from_name("python").is_some());
    assert!(lang_from_name("py").is_some());
    assert!(lang_from_name("javascript").is_some());
    assert!(lang_from_name("js").is_some());
    assert!(lang_from_name("csv").is_some());

    // Verify unknown languages return None
    assert!(lang_from_name("definitely-not-a-language").is_none());
    assert!(lang_from_name("").is_none());

    // Verify JSON spec has expected properties
    let json_spec = lang_from_name("json").unwrap();
    assert_eq!(json_spec.name, "json");
    assert!(json_spec.enabled);
    assert!(json_spec.is_format);

    // Verify YAML spec has expected properties
    let yaml_spec = lang_from_name("yaml").unwrap();
    assert_eq!(yaml_spec.name, "yaml");
    assert!(yaml_spec.enabled);
    assert!(yaml_spec.is_format);

    // Verify LANG_SPECS contains all expected entries
    let names: Vec<&str> = LANG_SPECS.iter().map(|s| s.name).collect();
    assert!(names.contains(&"json"));
    assert!(names.contains(&"yaml"));
    assert!(names.contains(&"toml"));
    assert!(names.contains(&"python"));
    assert!(names.contains(&"javascript"));
    assert!(names.contains(&"csv"));
}

// ── Test 3: Query sources are available for enabled languages ────

#[test]
fn query_sources_are_available_for_enabled_languages() {
    // Unknown language returns None
    assert!(query_from_language("definitely-not-a-language").is_none());
    assert!(query_from_language("json-disabled").is_none());

    let json_query = query_from_language("json").expect("json query should be registered");
    assert!(!json_query.is_empty(), "json query should not be empty");
    assert!(json_query.contains('@'));

    // Languages with tree-sitter queries should return non-empty query strings
    for lang in ["yaml", "toml", "python", "javascript"] {
        if let Some(query) = query_from_language(lang) {
            assert!(!query.is_empty(), "query for {lang} should not be empty");
            assert!(
                query.contains('@'),
                "query for {lang} should contain capture references"
            );
        }
    }
}

#[test]
fn semantic_token_capture_mapping_matches_rust_query_boundaries() {
    let yaml_ids = token_type_ids_for("yaml", "name: \"Ada\"\ncount: 3\nactive: true\n");
    assert!(yaml_ids.contains(&token_type_id("key")));
    assert!(yaml_ids.contains(&token_type_id("str")));
    assert!(yaml_ids.contains(&token_type_id("int")));
    assert!(yaml_ids.contains(&token_type_id("boolean")));

    let toml_ids = token_type_ids_for("toml", "name = \"Ada\"\nratio = 3.5\n");
    assert!(toml_ids.contains(&token_type_id("key")));
    assert!(toml_ids.contains(&token_type_id("str")));
    assert!(toml_ids.contains(&token_type_id("float")) || toml_ids.contains(&token_type_id("int")));

    let python_ids = token_type_ids_for("python", "def f(x):\n    return None\n");
    assert!(
        python_ids.contains(&token_type_id("function"))
            || python_ids.contains(&token_type_id("variable"))
    );
    assert!(
        python_ids.contains(&token_type_id("operator"))
            || python_ids.contains(&token_type_id("nil"))
    );

    let js_ids = token_type_ids_for("javascript", "function f(x) { return null; }\n");
    assert!(js_ids.contains(&token_type_id("function")));
    assert!(js_ids.contains(&token_type_id("operator")));
    assert!(js_ids.contains(&token_type_id("nil")));

    assert!(token_type_ids_for("definitely-not-a-language", "name: value").is_empty());
}

#[test]
fn query_capture_families_are_present_for_registered_rust_queries() {
    for language in ["yaml", "toml", "python", "javascript"] {
        let query = query_from_language(language).expect("registered query expected");
        let captures = capture_names(query);
        assert!(!captures.is_empty(), "{language} should declare captures");

        let families: BTreeSet<&str> = captures
            .iter()
            .map(|capture| capture.split('.').next().unwrap_or(capture))
            .collect();
        assert!(
            families.len() >= 2,
            "{language} should expose multiple capture families, got {families:?}"
        );
    }
}

#[test]
fn query_helpers_round_trip_capture_names_for_custom_queries() {
    let language = tree_sitter_language("json").expect("json tree-sitter should be available");
    let query = query_new(
        &language,
        "(pair key: (string) @property value: (number) @number.float)",
    )
    .expect("query should compile");

    assert_eq!(query_capture_name_for_id(&query, 0), Some("property"));
    assert_eq!(query_capture_name_for_id(&query, 1), Some("number.float"));
    assert_eq!(query_capture_name_for_id(&query, 2), None);
}

#[test]
fn custom_query_capture_aliases_map_to_expected_semantic_token_types() {
    let json_language = tree_sitter_language("json").expect("json tree-sitter should be available");
    let json_source = r#"{"name": 1}"#;
    let json_tree = parse_with_tree(&json_language, json_source.as_bytes(), None)
        .expect("json source should parse");
    let json_spans = collect_token_spans_with_tree(
        &json_tree,
        &json_language,
        r#"
        (pair key: (string) @property value: (number) @number.integer)
        (pair key: (string) @label value: (number) @number..float)
        (pair key: (string) @string value: (number) @number.float)
        (pair key: (string) @keyword value: (number) @constant.builtin)
        (pair key: (string) @operator value: (number) @boolean)
        (pair key: (string) @type.builtin value: (number) @attribute.name)
        (pair key: (string) @tag value: (number) @comment.line)
        (pair key: (string) @function.call value: (number) @variable.parameter)
        (pair key: (string) @comment.block value: (number) @number.hex)
        (object) @punctuation.delimiter
        "#,
        json_source,
    );
    let json_token_types: Vec<u32> = json_spans.iter().map(|span| span.token_type).collect();
    let json_ids: BTreeSet<u32> = json_token_types.iter().copied().collect();
    assert!(json_ids.contains(&token_type_id("key")));
    assert!(json_ids.contains(&token_type_id("str")));
    assert!(json_ids.contains(&token_type_id("int")));
    assert!(json_ids.contains(&token_type_id("float")));
    assert!(json_ids.contains(&token_type_id("operator")));
    assert!(json_ids.contains(&token_type_id("nil")));
    assert!(json_ids.contains(&token_type_id("boolean")));
    assert!(json_ids.contains(&token_type_id("tag")));
    assert!(json_ids.contains(&token_type_id("attribute")));
    assert!(json_ids.contains(&token_type_id("punctuation")));
    assert!(json_ids.contains(&token_type_id("comment")));
    assert!(json_ids.contains(&token_type_id("function")));
    assert!(json_ids.contains(&token_type_id("variable")));
    assert!(
        json_token_types
            .iter()
            .filter(|&&id| id == token_type_id("key"))
            .count()
            >= 2
    );
    assert!(
        json_token_types
            .iter()
            .filter(|&&id| id == token_type_id("int"))
            .count()
            >= 3
    );
    assert!(
        json_token_types
            .iter()
            .filter(|&&id| id == token_type_id("operator"))
            .count()
            >= 2
    );
    assert!(
        json_token_types
            .iter()
            .filter(|&&id| id == token_type_id("tag"))
            .count()
            >= 2
    );
    assert!(
        json_token_types
            .iter()
            .filter(|&&id| id == token_type_id("comment"))
            .count()
            >= 2
    );

    let js_language =
        tree_sitter_language("javascript").expect("javascript tree-sitter should be available");
    let js_source = "function f(arg) { return true ?? null; } // note\n";
    let js_tree = parse_with_tree(&js_language, js_source.as_bytes(), None)
        .expect("javascript source should parse");
    let js_spans = collect_token_spans_with_tree(
        &js_tree,
        &js_language,
        r#"
        (function_declaration) @keyword.operator
        (identifier) @function.call
        (identifier) @variable.parameter
        (true) @boolean
        (null) @constant.builtin
        (comment) @comment.line
        "#,
        js_source,
    );
    let js_ids: BTreeSet<u32> = js_spans.into_iter().map(|span| span.token_type).collect();
    assert!(js_ids.contains(&token_type_id("operator")));
    assert!(js_ids.contains(&token_type_id("function")));
    assert!(js_ids.contains(&token_type_id("variable")));
    assert!(js_ids.contains(&token_type_id("boolean")));
    assert!(js_ids.contains(&token_type_id("nil")));
    assert!(js_ids.contains(&token_type_id("comment")));
}

#[test]
fn capture_alias_equivalence_matches_zig_cases_where_rust_exposes_results() {
    let json_language = tree_sitter_language("json").expect("json tree-sitter should be available");
    let json_source = r#"{"name": 1}"#;
    let json_tree = parse_with_tree(&json_language, json_source.as_bytes(), None)
        .expect("json source should parse");
    let json_capture_type = |query: &str| {
        let spans = collect_token_spans_with_tree(&json_tree, &json_language, query, json_source);
        assert_eq!(spans.len(), 1, "expected exactly one span for {query}");
        spans[0].token_type
    };

    assert_eq!(
        json_capture_type("(pair key: (string) @property)"),
        json_capture_type("(pair key: (string) @label)")
    );
    assert_eq!(
        json_capture_type("(pair key: (string) @property)"),
        token_type_id("key")
    );

    assert_eq!(
        json_capture_type("(pair value: (number) @number.integer)"),
        json_capture_type("(pair value: (number) @number.hex)")
    );
    assert_eq!(
        json_capture_type("(pair value: (number) @number.integer)"),
        json_capture_type("(pair value: (number) @number)")
    );
    assert_eq!(
        json_capture_type("(pair value: (number) @number.integer)"),
        json_capture_type("(pair value: (number) @number..float)")
    );
    assert_eq!(
        json_capture_type("(pair value: (number) @number.integer)"),
        token_type_id("int")
    );
    assert_eq!(
        json_capture_type("(pair value: (number) @number.float)"),
        token_type_id("float")
    );
    assert_ne!(
        json_capture_type("(pair value: (number) @number.float)"),
        json_capture_type("(pair value: (number) @number.integer)")
    );

    let js_language =
        tree_sitter_language("javascript").expect("javascript tree-sitter should be available");
    let js_source = "/* note */\nfunction f(arg) { return true ?? null; }\n";
    let js_tree = parse_with_tree(&js_language, js_source.as_bytes(), None)
        .expect("javascript source should parse");
    let js_capture_type = |query: &str| {
        let spans = collect_token_spans_with_tree(&js_tree, &js_language, query, js_source);
        assert_eq!(spans.len(), 1, "expected exactly one span for {query}");
        spans[0].token_type
    };

    assert_eq!(
        js_capture_type("(function_declaration) @keyword.operator"),
        js_capture_type("(function_declaration) @operator")
    );
    assert_eq!(
        js_capture_type("(function_declaration) @operator"),
        token_type_id("operator")
    );

    assert_eq!(
        js_capture_type("(null) @constant.builtin"),
        token_type_id("nil")
    );
    assert_eq!(js_capture_type("(true) @boolean"), token_type_id("boolean"));
    assert_ne!(
        js_capture_type("(null) @constant.builtin"),
        js_capture_type("(true) @boolean")
    );

    assert_eq!(
        js_capture_type("(comment) @comment.line"),
        js_capture_type("(comment) @comment.block")
    );
    assert_eq!(
        js_capture_type("(comment) @comment.line"),
        token_type_id("comment")
    );

    assert_eq!(
        js_capture_type("(function_declaration name: (identifier) @type.builtin)"),
        token_type_id("tag")
    );
    assert_eq!(
        js_capture_type("(function_declaration name: (identifier) @tag)"),
        token_type_id("tag")
    );
    assert_eq!(
        js_capture_type("(function_declaration name: (identifier) @function.call)"),
        token_type_id("function")
    );
    assert_eq!(
        js_capture_type("(formal_parameters (identifier) @variable.parameter)"),
        token_type_id("variable")
    );
}

#[test]
fn unsupported_capture_names_are_ignored_by_semantic_token_collection() {
    let json_language = tree_sitter_language("json").expect("json tree-sitter should be available");
    let json_source = r#"{"name": 1}"#;
    let json_tree = parse_with_tree(&json_language, json_source.as_bytes(), None)
        .expect("json source should parse");
    let spans = collect_token_spans_with_tree(
        &json_tree,
        &json_language,
        r#"
        (pair key: (string) @PROPERTY value: (number) @unknown.capture)
        (object) @unknown
        "#,
        json_source,
    );

    assert!(spans.is_empty());
}

#[test]
fn language_and_query_registration_rust_boundaries_are_stable() {
    assert!(lang_from_name("definitely-not-a-language").is_none());
    assert!(lang_from_name("json-disabled").is_none());
    assert!(query_from_language("definitely-not-a-language").is_none());
    assert!(lang_from_name(" json").is_some());
    assert!(lang_from_name("json ").is_some());
    assert!(lang_from_name("JSON").is_some());

    assert!(query_from_language("json").is_some());
    assert!(query_from_language("JSON").is_some());
    for language in ["yaml", "toml", "python", "javascript"] {
        assert!(lang_from_name(language).is_some());
        assert!(query_from_language(language).is_some());
    }
}

// ── Test 4: Semantic token types are consistent ──────────────────

#[test]
fn semantic_token_types_are_consistent() {
    // Verify the token type array has the expected entries in the expected order
    assert_eq!(SEMANTIC_TOKEN_TYPES.len(), 15);
    assert_eq!(SEMANTIC_TOKEN_TYPES[0], "map");
    assert_eq!(SEMANTIC_TOKEN_TYPES[1], "key");
    assert_eq!(SEMANTIC_TOKEN_TYPES[2], "seq");
    assert_eq!(SEMANTIC_TOKEN_TYPES[3], "str");
    assert_eq!(SEMANTIC_TOKEN_TYPES[4], "int");
    assert_eq!(SEMANTIC_TOKEN_TYPES[5], "float");
    assert_eq!(SEMANTIC_TOKEN_TYPES[6], "boolean");
    assert_eq!(SEMANTIC_TOKEN_TYPES[7], "nil");
    assert_eq!(SEMANTIC_TOKEN_TYPES[8], "punctuation");
    assert_eq!(SEMANTIC_TOKEN_TYPES[9], "comment");
    assert_eq!(SEMANTIC_TOKEN_TYPES[10], "operator");
    assert_eq!(SEMANTIC_TOKEN_TYPES[11], "function");
    assert_eq!(SEMANTIC_TOKEN_TYPES[12], "variable");
    assert_eq!(SEMANTIC_TOKEN_TYPES[13], "tag");
    assert_eq!(SEMANTIC_TOKEN_TYPES[14], "attribute");

    // Verify no duplicates
    let mut sorted: Vec<&str> = SEMANTIC_TOKEN_TYPES.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), SEMANTIC_TOKEN_TYPES.len());
}

// ── Test 5: Language spec aliases are consistent ─────────────────

#[test]
fn language_spec_aliases_are_consistent() {
    // YAML aliases
    let yaml = lang_from_name("yaml").unwrap();
    assert!(yaml.matches_name("yml"));
    assert!(yaml.matches_name("y"));
    // JSON aliases
    let json = lang_from_name("json").unwrap();
    assert!(json.matches_name("j"));

    // Python aliases
    let python = lang_from_name("python").unwrap();
    assert!(python.matches_name("py"));

    // JavaScript aliases
    let js = lang_from_name("javascript").unwrap();
    assert!(js.matches_name("js"));

    // CSV aliases
    let csv = lang_from_name("csv").unwrap();
    assert!(csv.matches_name("c"));

    // Case insensitivity
    assert!(lang_from_name("JSON").is_some());
    assert!(lang_from_name("YAML").is_some());
}

#[test]
fn semantic_token_alias_families_are_distinct_where_rust_exposes_them() {
    assert_ne!(token_type_id("float"), token_type_id("int"));
    assert_ne!(token_type_id("nil"), token_type_id("boolean"));
    assert_ne!(token_type_id("operator"), token_type_id("nil"));
    assert_eq!(SEMANTIC_TOKEN_TYPES[token_type_id("key") as usize], "key");
    assert_eq!(SEMANTIC_TOKEN_TYPES[token_type_id("tag") as usize], "tag");
    assert_eq!(
        SEMANTIC_TOKEN_TYPES[token_type_id("comment") as usize],
        "comment"
    );
}

// ── Test 6: NodeId and TreeStore basic operations ─────────────────

#[test]
fn node_id_and_tree_store_basic_operations() {
    let mut store = TreeStore::new();

    // Add a mapping root
    let root = store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: CompactTag::from_sem_type(SemType::Map),
        ..TreeNode::default()
    });

    // Add a key-value child
    let (key_id, value_id) = store
        .add_key_value_child(
            root,
            TreeNode::scalar(SemType::Str, "name"),
            TreeNode::scalar(SemType::Str, "Ada"),
        )
        .unwrap();

    assert!(store.get(key_id).unwrap().is_map_key);
    assert_eq!(store.get(value_id).unwrap().key(), Some(key_id));
    assert_eq!(store.get(value_id).unwrap().parent, Some(root));

    // Verify path
    let path = store.path_for(value_id).unwrap();
    assert_eq!(path.len(), 1);
    assert_eq!(
        path[0],
        treease_core::core::ParsedKey::Str("name".to_string())
    );
}
