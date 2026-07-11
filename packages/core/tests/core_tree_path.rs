use treease_core::core::{
    Decoder, NodeId, OwnedPathSeg, SemType, TreeNode, TreeNodeKind, TreeStore,
    build_tree_path_parts, compute_path_span, compute_tree_path_segments, format_tree_path,
    format_tree_path_segment, is_simple_key, parse_tree_path, path_seg_index, path_seg_key,
    path_seg_key_slice,
};

#[test]
fn tree_path_helpers_identify_simple_keys() {
    assert!(is_simple_key("a"));
    assert!(is_simple_key("_a1"));
    assert!(is_simple_key("$name"));
    assert!(!is_simple_key(""));
    assert!(!is_simple_key("1a"));
    assert!(!is_simple_key("x.y"));
}

#[test]
fn tree_path_helpers_format_segments_and_full_paths() {
    let key = path_seg_key("preview");
    let quoted = path_seg_key("x.y");
    let index = path_seg_index(2);

    assert_eq!(path_seg_key_slice(key), "preview");
    assert_eq!(format_tree_path_segment(key), "preview");
    assert_eq!(format_tree_path_segment(quoted), "[\"x.y\"]");
    assert_eq!(format_tree_path_segment(index), "[2]");

    let parts = build_tree_path_parts(&[path_seg_key("a"), path_seg_index(1), path_seg_key("x.y")]);
    assert_eq!(parts, vec!["$", "a", "[1]", "[\"x.y\"]"]);

    let full = format_tree_path(&[path_seg_key("a"), path_seg_index(1), path_seg_key("x.y")]);
    assert_eq!(full, "$.a[1][\"x.y\"]");
    assert_eq!(format_tree_path(&[]), "$");
}

#[test]
fn snapshot_tree_path_parser_owns_root_keys_indices_and_rejects_invalid_syntax() {
    assert_eq!(parse_tree_path(""), Some(Vec::new()));
    assert_eq!(parse_tree_path("$"), Some(Vec::new()));
    assert_eq!(
        parse_tree_path(r#"$.rows[2]["quoted.key"]["right]bracket"]["escaped\"quote"]"#),
        Some(vec![
            OwnedPathSeg::Key("rows".to_owned()),
            OwnedPathSeg::Index(2),
            OwnedPathSeg::Key("quoted.key".to_owned()),
            OwnedPathSeg::Key("right]bracket".to_owned()),
            OwnedPathSeg::Key("escaped\"quote".to_owned()),
        ])
    );

    for invalid in ["$.", "$[]", "$[x]", "$[\"unterminated]", "$rows"] {
        assert_eq!(parse_tree_path(invalid), None, "{invalid}");
    }
}

// ── Helper: add a tree node with byte spans ──────────────────────

fn node_with_span(
    store: &mut TreeStore,
    kind: TreeNodeKind,
    sem_type: Option<SemType>,
    value: &str,
    start_byte: u32,
    end_byte: u32,
) -> NodeId {
    store.add(TreeNode {
        kind,
        sem_type,
        tag: sem_type
            .map(treease_core::core::CompactTag::from_sem_type)
            .unwrap_or_default(),
        value: value.to_string().into(),
        start_byte,
        end_byte,
        ..TreeNode::default()
    })
}

// ── Tree path ignores stale stored language source or tree-sitter errors ──

#[test]
fn tree_path_ignores_stale_stored_language_source_or_tree_sitter_errors() {
    let mut store = TreeStore::new();
    let cache_key = "test-stale-tree-path";
    let source = "{\"a\":1}";

    // Store a JSON document analysis
    let root = node_with_span(
        &mut store,
        TreeNodeKind::Mapping,
        Some(SemType::Map),
        "",
        0,
        source.len() as u32,
    );
    store.set_document_analysis(
        cache_key,
        "json",
        root,
        None,
        source,
        vec![],
        vec![],
        vec![],
        String::new(),
    );

    // Language mismatch → empty
    let result = compute_tree_path_segments(Some(&store), cache_key, "toml", source, 0, 0);
    assert!(result.is_empty());

    // Source mismatch → empty
    let result = compute_tree_path_segments(Some(&store), cache_key, "json", "{\"a\":2}", 0, 0);
    assert!(result.is_empty());

    // Store a document with diagnostics → empty
    store.set_document_analysis(
        cache_key,
        "json",
        root,
        None,
        source,
        vec![],
        vec![0, 0, 0, 1, 1], // non-empty diagnostics
        vec![],
        String::new(),
    );
    let result = compute_tree_path_segments(Some(&store), cache_key, "json", source, 0, 0);
    assert!(result.is_empty());

    // JSON array source queried as TOML → empty (mirrors Zig's json_array_as_toml check)
    let json_array_source = "[{\"a\":1}]";
    let result =
        compute_tree_path_segments(Some(&store), cache_key, "toml", json_array_source, 0, 0);
    assert!(result.is_empty());

    // computePathSpan with stale (mismatched) source returns all -1 sentinel values
    let path = &[path_seg_key("a")];
    let stale_span = compute_path_span(Some(&store), cache_key, "json", "{\"a\":2}", path, false);
    assert_eq!(stale_span.start_byte, -1);
    assert_eq!(stale_span.end_byte, -1);
    assert_eq!(stale_span.row, -1);
    assert_eq!(stale_span.column, -1);

    // The Zig test also covers:
    //  - Valid TOML tree path (requires Tree-Sitter for TOML parsing; skip)
    //  - Tree-sitter error tree with mismatched language (requires Tree-Sitter; skip)
    // Both are skipped because the Rust test environment does not have Tree-Sitter
    // available for dynamic document analysis.
}

// ── Python tree path root boundary returns empty path ────────────

#[test]
fn python_tree_path_root_boundary_returns_empty_path_from_stored_analysis() {
    let mut store = TreeStore::new();
    let cache_key = "test-python-tree-path-root-boundary";
    let source = "x = 1\n";

    let root = node_with_span(
        &mut store,
        TreeNodeKind::Mapping,
        Some(SemType::Map),
        "",
        0,
        source.len() as u32,
    );
    store.set_document_analysis(
        cache_key,
        "python",
        root,
        None,
        source,
        vec![],
        vec![],
        vec![],
        String::new(),
    );

    let result = compute_tree_path_segments(Some(&store), cache_key, "python", source, 0, 0);
    assert!(result.is_empty());
}

// ── Headerless JSON array computes index-based tree paths ─────────

#[test]
fn json_headerless_array_computes_index_path_and_span() {
    use treease_core::formats::JsonDecoder;
    let cache_key = "test-json-headerless-array";
    let source = "[10, 20, 30]";
    let mut decoded = JsonDecoder
        .decode_str(source)
        .expect("JSON array should decode");
    decoded
        .store
        .set_tree(cache_key, "json", decoded.root, None, source, vec![]);

    // Byte offset of "30" in "[10, 20, 30]" is at position 9
    let result = compute_tree_path_segments(Some(&decoded.store), cache_key, "json", source, 0, 9);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], OwnedPathSeg::Index(2));

    // compute_path_span for index path
    let path = &[path_seg_index(2)];
    let span = compute_path_span(Some(&decoded.store), cache_key, "json", source, path, false);
    assert!(
        span.start_byte >= 0,
        "headerless index span start should be valid"
    );
    assert!(
        span.end_byte > span.start_byte,
        "headerless index span should cover content"
    );
}

// ── TOML inline table resolves tree path for inner value ─────────

#[test]
fn toml_inline_table_resolves_tree_path_for_inner_value() {
    use treease_core::formats::TomlDecoder;

    let cache_key = "test-toml-inline-table";
    let source = "profile = { name = \"Ada\" }\n";
    let mut decoded = TomlDecoder
        .decode_str(source)
        .expect("TOML inline table should decode");

    let ts_tree = treease_core::core::parse_tree("toml", source.as_bytes());
    assert!(
        ts_tree.is_some(),
        "tree-sitter-toml should parse the inline table"
    );

    decoded
        .store
        .set_tree(cache_key, "toml", decoded.root, ts_tree, source, vec![]);

    // "Ada" starts at byte 20 in `profile = { name = "Ada" }\n`
    let result = compute_tree_path_segments(Some(&decoded.store), cache_key, "toml", source, 0, 20);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], OwnedPathSeg::Key("profile".to_owned()));
    assert_eq!(result[1], OwnedPathSeg::Key("name".to_owned()));
}

// ── Owned TreePathIndex (shared entity Task 1) ──────────────────

#[test]
fn tree_path_index_resolves_value_and_key_nodes_without_string_leaks() {
    use treease_core::core::{CodecService, PathLookup, TreePathIndex};
    use treease_core::wasm_types::PathSegTag;

    let source = r#"{"wide":{"first":1,"target":"old","last":3},"rows":[{"name":"Ada"}]}"#;
    let decoded = CodecService::new()
        .decode("json", source)
        .expect("json fixture should decode");

    let index = TreePathIndex::build(&decoded.store, decoded.root);

    let target_path = index.path(&[PathLookup::key("wide"), PathLookup::key("target")]);
    let value_id = index
        .value_node(&target_path)
        .expect("target value node should be indexed");
    let key_id = index
        .key_node(&target_path)
        .expect("target key node should be indexed");

    let key = decoded.store.get(key_id).expect("key node should exist");
    assert_eq!(decoded.store.value_for(value_id).unwrap(), "old");
    assert_eq!(decoded.store.value_for(key_id).unwrap(), "target");
    assert!(key.is_map_key);

    let indexed_path = index
        .path_for_node(value_id)
        .expect("value node should have reverse path");
    assert_eq!(indexed_path.len(), 2);
    assert_eq!(indexed_path[0].tag, PathSegTag::Key);
    assert_eq!(indexed_path[0].key, "wide");
    assert_eq!(indexed_path[1].key, "target");
}

// ── Reuse TreePathIndex in PathSpanResolver (Task 2 safety net) ─

#[test]
fn compute_path_span_for_document_keeps_existing_snapshot_span_behavior() {
    use treease_core::core::{CodecService, compute_path_span_for_document, path_seg_key};

    let source = r#"{"wide":{"a":1,"b":2,"target":"old"}}"#;
    let decoded = CodecService::new()
        .decode("json", source)
        .expect("json fixture should decode");
    let path = [path_seg_key("wide"), path_seg_key("target")];

    let span = compute_path_span_for_document(
        &decoded.store,
        decoded.root,
        None,
        &[],
        "json",
        source,
        &path,
        false,
    );

    let old_start = source.find("\"old\"").expect("fixture contains old value") as i32;
    assert_eq!(span.start_byte, old_start);
    assert_eq!(span.end_byte, old_start + "\"old\"".len() as i32);
}

// ── Indexed tree path lookup (P2 Task 1) ──────────────────────

#[test]
fn find_node_by_path_with_index_resolves_value_and_key() {
    use treease_core::core::{
        CodecService, TreePathIndex, find_node_by_path_with_index, path_seg_key,
    };

    let source = r#"{"wide":{"first":1,"target":"old","last":3}}"#;
    let decoded = CodecService::new()
        .decode("json", source)
        .expect("json fixture should decode");
    let index = TreePathIndex::build(&decoded.store, decoded.root);
    let path = [path_seg_key("wide"), path_seg_key("target")];

    let value_id =
        find_node_by_path_with_index(decoded.root, &path, false, &decoded.store, Some(&index))
            .expect("indexed value lookup should resolve");
    let key_id =
        find_node_by_path_with_index(decoded.root, &path, true, &decoded.store, Some(&index))
            .expect("indexed key lookup should resolve");

    assert_eq!(decoded.store.value_for(value_id).unwrap(), "old");
    assert_eq!(decoded.store.value_for(key_id).unwrap(), "target");
    assert!(decoded.store.get(key_id).unwrap().is_map_key);
}

// ── Summary path span with index (P2 Task 2) ────────────────────

#[test]
fn compute_path_span_for_document_with_index_targets_key_span() {
    use treease_core::core::{
        CodecService, TreePathIndex, compute_path_span_for_document_with_index, path_seg_key,
    };

    let source = r#"{"wide":{"target":"old"}}"#;
    let decoded = CodecService::new()
        .decode("json", source)
        .expect("json fixture should decode");
    let index = TreePathIndex::build(&decoded.store, decoded.root);
    let path = [path_seg_key("wide"), path_seg_key("target")];

    let span = compute_path_span_for_document_with_index(
        &decoded.store,
        decoded.root,
        None,
        &[],
        "json",
        source,
        &path,
        true,
        Some(&index),
    );

    let key_start = source.find("\"target\"").expect("fixture contains key") as i32;
    assert_eq!(span.start_byte, key_start);
    assert_eq!(span.end_byte, key_start + "\"target\"".len() as i32);
}
