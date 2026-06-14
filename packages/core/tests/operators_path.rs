use treease_core::core::ParsedKey;
use treease_core::expression_pipeline;
use treease_core::operators::path::{
    get_path, get_path_array_from_node, get_path_operator, set_path_operator,
};
use treease_core::operators::{
    Context, ExpressionNode, GET_PATH_OP_TYPE, NodeId, NodeKind, Operation, SET_PATH_OP_TYPE,
    SemType, TreeEngine, TreeNode,
};

// ── Helpers ───────────────────────────────────────────────────────

fn string_scalar(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: SemType::Str.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn int_scalar(value: i64) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: value.to_string(),
        ..TreeNode::default()
    }
}

fn map_node(parent: Option<NodeId>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.tag().to_owned(),
        parent,
        ..TreeNode::default()
    }
}

fn seq_node(parent: Option<NodeId>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        parent,
        ..TreeNode::default()
    }
}

/// Add a key-value pair to a map node in the store, with proper parent/key linkage.
fn push_map_entry(
    engine: &mut TreeEngine,
    parent_id: NodeId,
    key_name: &str,
    mut value: TreeNode,
) -> NodeId {
    let mut key = string_scalar(key_name);
    key.parent = Some(parent_id);
    key.is_map_key = true;
    let key_id = engine.store.add(key.clone());

    value.parent = Some(parent_id);
    value.key = Some(key_id);
    value.is_map_key = false;

    let parent = engine.store.get_mut(parent_id);
    parent.content.push(key);
    parent.content.push(value);
    key_id
}

/// Add a child to a sequence node in the store, with proper parent/sequence_index.
fn push_seq_child(engine: &mut TreeEngine, parent_id: NodeId, mut child: TreeNode) {
    let parent = engine.store.get(parent_id);
    let index = parent.content.len();
    child.parent = Some(parent_id);
    child.sequence_index = Some(index as i64);
    let parent_mut = engine.store.get_mut(parent_id);
    parent_mut.content.push(child);
}

fn path_expression() -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &GET_PATH_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[test]
fn get_path_returns_map_path_as_sequence() {
    // Build: root -> child "a" -> grandchild "b" -> leaf "cat"
    // Path should be ["a", "b", "cat"]
    let mut engine = TreeEngine::default();

    let root_id = engine.store.add(map_node(None));
    let a_id = push_map_entry(&mut engine, root_id, "a", map_node(Some(root_id)));
    // Get the value node for "a" (it's the map we just added)
    let a_value_id = {
        let root = engine.store.get(root_id);
        // content is [key("a"), value(map)], so value is at index 1
        let a_map = root.content[1].clone();
        engine.store.add(a_map)
    };
    // Fix up the stored node's parent
    {
        let a_map = engine.store.get_mut(a_value_id);
        a_map.parent = Some(root_id);
        a_map.key = Some(a_id);
    }

    let b_id = push_map_entry(&mut engine, a_value_id, "b", map_node(Some(a_value_id)));
    let b_value_id = {
        let a_map = engine.store.get(a_value_id);
        let b_map = a_map.content[1].clone();
        engine.store.add(b_map)
    };
    {
        let b_map = engine.store.get_mut(b_value_id);
        b_map.parent = Some(a_value_id);
        b_map.key = Some(b_id);
    }

    let cat_id = push_map_entry(&mut engine, b_value_id, "cat", string_scalar("meow"));
    let cat_value_id = {
        let b_map = engine.store.get(b_value_id);
        let cat_val = b_map.content[1].clone();
        engine.store.add(cat_val)
    };
    {
        let cat_val = engine.store.get_mut(cat_value_id);
        cat_val.parent = Some(b_value_id);
        cat_val.key = Some(cat_id);
    }

    let path = get_path(engine.store.get(cat_value_id), &engine.store).unwrap();

    assert_eq!(path.len(), 3, "path should have 3 segments");
    assert_eq!(path[0], ParsedKey::Str("a".to_string()));
    assert_eq!(path[1], ParsedKey::Str("b".to_string()));
    assert_eq!(path[2], ParsedKey::Str("cat".to_string()));
}

#[test]
fn get_path_returns_array_index_as_int() {
    // Build: root -> sequence -> [0: "first", 1: "second"]
    // Path to "second" should be [0, 1] (indices)
    let mut engine = TreeEngine::default();

    let root_id = engine.store.add(map_node(None));
    let arr_key_id = push_map_entry(&mut engine, root_id, "items", seq_node(Some(root_id)));

    // Get the sequence value node
    let arr_value_id = {
        let root = engine.store.get(root_id);
        let arr = root.content[1].clone();
        engine.store.add(arr)
    };
    {
        let arr = engine.store.get_mut(arr_value_id);
        arr.parent = Some(root_id);
        arr.key = Some(arr_key_id);
    }

    push_seq_child(&mut engine, arr_value_id, string_scalar("first"));
    push_seq_child(&mut engine, arr_value_id, string_scalar("second"));

    // Get the "second" child (index 1)
    let arr = engine.store.get(arr_value_id);
    let second = &arr.content[1];

    let path = get_path(second, &engine.store).unwrap();

    // Path should be: ["items", 1]
    assert_eq!(path.len(), 2, "path should have 2 segments");
    assert_eq!(path[0], ParsedKey::Str("items".to_string()));
    assert_eq!(path[1], ParsedKey::Int(1));
}

#[test]
fn get_path_operator_returns_path_for_nested_nodes() {
    let mut engine = TreeEngine::default();

    let root_id = engine.store.add(map_node(None));
    let a_id = push_map_entry(&mut engine, root_id, "x", map_node(Some(root_id)));
    let a_value_id = {
        let root = engine.store.get(root_id);
        let a_map = root.content[1].clone();
        engine.store.add(a_map)
    };
    {
        let a_map = engine.store.get_mut(a_value_id);
        a_map.parent = Some(root_id);
        a_map.key = Some(a_id);
    }

    let y_id = push_map_entry(&mut engine, a_value_id, "y", string_scalar("val"));
    let y_value_id = {
        let a_map = engine.store.get(a_value_id);
        let y_val = a_map.content[1].clone();
        engine.store.add(y_val)
    };
    {
        let y_val = engine.store.get_mut(y_value_id);
        y_val.parent = Some(a_value_id);
        y_val.key = Some(y_id);
    }

    let ctx = Context {
        matching_nodes: vec![engine.store.get(y_value_id).clone()],
        ..Context::default()
    };

    let mut expr = path_expression();
    let result = get_path_operator(ctx, &mut engine, &mut expr).unwrap();

    // Result should be a sequence of path segments
    assert_eq!(result.matching_nodes.len(), 1);
    let path_seq = &result.matching_nodes[0];
    assert_eq!(path_seq.kind, NodeKind::Sequence);
    assert_eq!(path_seq.content.len(), 2);
    assert_eq!(path_seq.content[0].value, "x");
    assert_eq!(path_seq.content[1].value, "y");
}

#[test]
fn get_path_array_from_node_parses_sequence_of_strings_and_ints() {
    // A sequence node with ["a", 1, "b"] should parse to [Str("a"), Int(1), Str("b")]
    let seq = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: vec![string_scalar("a"), int_scalar(1), string_scalar("b")],
        ..TreeNode::default()
    };

    let parsed = get_path_array_from_node(&seq).unwrap();
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[0], ParsedKey::Str("a".to_string()));
    assert_eq!(parsed[1], ParsedKey::Int(1));
    assert_eq!(parsed[2], ParsedKey::Str("b".to_string()));
}

#[test]
fn get_path_array_from_node_errors_on_non_sequence() {
    let scalar = string_scalar("not_a_seq");
    let result = get_path_array_from_node(&scalar);
    assert!(result.is_err(), "should error on non-sequence input");
}

#[test]
fn get_path_array_from_node_errors_on_invalid_type_in_sequence() {
    // A sequence with a map child should error
    let seq = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: vec![
            string_scalar("a"),
            TreeNode {
                kind: NodeKind::Mapping,
                sem_type: Some(SemType::Map),
                tag: SemType::Map.tag().to_owned(),
                ..TreeNode::default()
            },
        ],
        ..TreeNode::default()
    };

    let result = get_path_array_from_node(&seq);
    assert!(result.is_err(), "should error on map inside path sequence");
}

#[test]
fn set_path_operator_errors_without_block_rhs() {
    // set_path_operator requires RHS to be a block (;) expression
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(map_node(None));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    // Create a setpath expression without a block RHS — should error
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &SET_PATH_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(ExpressionNode {
            operation: Box::new(Operation {
                operation_type: &GET_PATH_OP_TYPE, // not BLOCK_OP_TYPE
                value: None,
                string_value: String::new(),
                tree_node: None,
                preferences: None,
                update_assign: false,
            }),
            lhs: None,
            rhs: None,
        })),
    };

    let result = set_path_operator(ctx, &mut engine, &mut expr);
    assert!(result.is_err(), "set_path without block RHS should error");
}

#[test]
fn get_path_on_root_node_returns_empty_path() {
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(map_node(None));

    let root = engine.store.get(root_id);
    let path = get_path(root, &engine.store).unwrap();

    // Root node with no parent should have an empty path
    assert!(path.is_empty(), "root node path should be empty");
}

#[test]
fn get_path_operator_on_multiple_nodes() {
    let mut engine = TreeEngine::default();

    let root_id = engine.store.add(map_node(None));
    let a_id = push_map_entry(&mut engine, root_id, "a", string_scalar("val_a"));
    let a_value_id = {
        let root = engine.store.get(root_id);
        let a_val = root.content[1].clone();
        engine.store.add(a_val)
    };
    {
        let a_val = engine.store.get_mut(a_value_id);
        a_val.parent = Some(root_id);
        a_val.key = Some(a_id);
    }

    let b_id = push_map_entry(&mut engine, root_id, "b", string_scalar("val_b"));
    let b_value_id = {
        let root = engine.store.get(root_id);
        let b_val = root.content[1].clone();
        engine.store.add(b_val)
    };
    {
        let b_val = engine.store.get_mut(b_value_id);
        b_val.parent = Some(root_id);
        b_val.key = Some(b_id);
    }

    let ctx = Context {
        matching_nodes: vec![
            engine.store.get(a_value_id).clone(),
            engine.store.get(b_value_id).clone(),
        ],
        ..Context::default()
    };

    let mut expr = path_expression();
    let result = get_path_operator(ctx, &mut engine, &mut expr).unwrap();

    // Should have two path results
    assert_eq!(result.matching_nodes.len(), 2);
    assert_eq!(result.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(result.matching_nodes[1].kind, NodeKind::Sequence);
    // First path: ["a"]
    assert_eq!(result.matching_nodes[0].content[0].value, "a");
    // Second path: ["b"]
    assert_eq!(result.matching_nodes[1].content[0].value, "b");
}

// ── Expression-level helpers ─────────────────────────────────────

fn mapping(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        let mut key_node = string_scalar(key);
        key_node.is_map_key = true;
        content.push(key_node);
        content.push(value);
    }
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.tag().to_owned(),
        content,
        ..TreeNode::default()
    }
}

fn sequence(items: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: items,
        ..TreeNode::default()
    }
}

// ── Expression-level scenarios (aligned with path.zig) ────────────

#[test]
fn expression_map_path() {
    // .a.b | path  ->  ["a", "b"]
    let input = mapping(vec![("a", mapping(vec![("b", string_scalar("cat"))]))]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b | path")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let path_seq = &out.matching_nodes[0];
    assert_eq!(path_seq.kind, NodeKind::Sequence);
    assert_eq!(path_seq.content.len(), 2);
    assert_eq!(path_seq.content[0].value, "a");
    assert_eq!(path_seq.content[1].value, "b");
}

#[test]
fn expression_map_path_splat() {
    // .a.b | path[]  ->  "a", "b" (iterated)
    let input = mapping(vec![("a", mapping(vec![("b", string_scalar("cat"))]))]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b | path[]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "a");
    assert_eq!(out.matching_nodes[1].value, "b");
}

#[test]
fn expression_array_iteration() {
    // .a.b.c.[]  ->  0, 1, 2, 3
    let input = mapping(vec![(
        "a",
        mapping(vec![(
            "b",
            mapping(vec![(
                "c",
                sequence(vec![
                    int_scalar(0),
                    int_scalar(1),
                    int_scalar(2),
                    int_scalar(3),
                ]),
            )]),
        )]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b.c.[]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 4);
    assert_eq!(out.matching_nodes[0].value, "0");
    assert_eq!(out.matching_nodes[1].value, "1");
    assert_eq!(out.matching_nodes[2].value, "2");
    assert_eq!(out.matching_nodes[3].value, "3");
}

#[test]
fn expression_get_map_key() {
    // .a.b | path | .[-1]  ->  "b"
    let input = mapping(vec![("a", mapping(vec![("b", string_scalar("cat"))]))]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b | path | .[-1]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "b");
}

#[test]
fn expression_array_path() {
    // .a.[] | select(. == "dog") | path  ->  ["a", 1]
    let input = mapping(vec![(
        "a",
        sequence(vec![string_scalar("cat"), string_scalar("dog")]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".a.[] | select(. == \"dog\") | path",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let path_seq = &out.matching_nodes[0];
    assert_eq!(path_seq.kind, NodeKind::Sequence);
    assert_eq!(path_seq.content.len(), 2);
    assert_eq!(path_seq.content[0].value, "a");
    assert_eq!(path_seq.content[1].value, "1");
}

#[test]
fn expression_get_array_index() {
    // .a.[] | select(. == "dog") | path | .[-1]  ->  1
    let input = mapping(vec![(
        "a",
        sequence(vec![string_scalar("cat"), string_scalar("dog")]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".a.[] | select(. == \"dog\") | path | .[-1]",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "1");
}

#[test]
fn expression_print_path_and_value() {
    // .a[] | select(. == "*og") | [{"path":path, "value":.}]
    let input = mapping(vec![(
        "a",
        sequence(vec![
            string_scalar("cat"),
            string_scalar("dog"),
            string_scalar("frog"),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".a[] | select(. == \"*og\") | [{\"path\":path, \"value\":.}]",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    let expected = [(1_i64, "dog"), (2_i64, "frog")];
    for (result, (expected_index, expected_value)) in out.matching_nodes.iter().zip(expected) {
        assert_eq!(result.kind, NodeKind::Sequence);
        assert_eq!(result.content.len(), 1);
        let obj = &result.content[0];
        assert_eq!(obj.kind, NodeKind::Mapping);
        assert_eq!(obj.content.len(), 4); // 2 key-value pairs

        assert_eq!(obj.content[0].value, "path");
        let path = &obj.content[1];
        assert_eq!(path.kind, NodeKind::Sequence);
        assert_eq!(path.content.len(), 2);
        assert_eq!(path.content[0].value, "a");
        assert_eq!(path.content[1].value, expected_index.to_string());

        assert_eq!(obj.content[2].value, "value");
        assert_eq!(obj.content[3].value, expected_value);
    }
}

#[test]
fn expression_set_path() {
    // setpath(["a", "b"]; "things")  ->  {a: {b: things}}
    let input = mapping(vec![("a", mapping(vec![("b", string_scalar("cat"))]))]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "setpath([\"a\", \"b\"]; \"things\")",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Mapping);
    assert_eq!(result.content.len(), 2);
    assert_eq!(result.content[0].value, "a");
    let inner = &result.content[1];
    assert_eq!(inner.kind, NodeKind::Mapping);
    assert_eq!(inner.content.len(), 2);
    assert_eq!(inner.content[0].value, "b");
    assert_eq!(inner.content[1].value, "things");
}

#[test]
fn expression_set_path_on_empty() {
    // setpath(["a", "b"]; "things") on empty  ->  {a: {b: things}}
    let input = TreeNode::default();
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "setpath([\"a\", \"b\"]; \"things\")",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Mapping);
    assert_eq!(result.content.len(), 2);
    assert_eq!(result.content[0].value, "a");
    let inner = &result.content[1];
    assert_eq!(inner.kind, NodeKind::Mapping);
    assert_eq!(inner.content[0].value, "b");
    assert_eq!(inner.content[1].value, "things");
}

#[test]
fn expression_set_array_path() {
    // setpath(["a", 0]; "things")  ->  {a: [things, frog]}
    let input = mapping(vec![(
        "a",
        sequence(vec![string_scalar("cat"), string_scalar("frog")]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "setpath([\"a\", 0]; \"things\")",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Mapping);
    assert_eq!(result.content[0].value, "a");
    let arr = &result.content[1];
    assert_eq!(arr.kind, NodeKind::Sequence);
    assert_eq!(arr.content.len(), 2);
    assert_eq!(arr.content[0].value, "things");
    assert_eq!(arr.content[1].value, "frog");
}

#[test]
fn expression_set_array_path_empty() {
    // setpath(["a", 0]; "things") on empty  ->  {a: [things]}
    let input = TreeNode::default();
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "setpath([\"a\", 0]; \"things\")",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Mapping);
    assert_eq!(result.content[0].value, "a");
    let arr = &result.content[1];
    assert_eq!(arr.kind, NodeKind::Sequence);
    assert_eq!(arr.content.len(), 1);
    assert_eq!(arr.content[0].value, "things");
}

#[test]
fn expression_delete_path() {
    // delpaths([["a", "c"], ["a", "d"]])  ->  {a: {b: cat}}
    let input = mapping(vec![(
        "a",
        mapping(vec![
            ("b", string_scalar("cat")),
            ("c", string_scalar("dog")),
            ("d", string_scalar("frog")),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "delpaths([[\"a\", \"c\"], [\"a\", \"d\"]])",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Mapping);
    assert_eq!(result.content[0].value, "a");
    let inner = &result.content[1];
    assert_eq!(inner.kind, NodeKind::Mapping);
    assert_eq!(inner.content.len(), 2);
    assert_eq!(inner.content[0].value, "b");
    assert_eq!(inner.content[1].value, "cat");
}

#[test]
fn expression_delete_array_path() {
    // delpaths([["a", 0]])  ->  {a: [frog]}
    let input = mapping(vec![(
        "a",
        sequence(vec![string_scalar("cat"), string_scalar("frog")]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "delpaths([[\"a\", 0]])")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Mapping);
    assert_eq!(result.content[0].value, "a");
    let arr = &result.content[1];
    assert_eq!(arr.kind, NodeKind::Sequence);
    assert_eq!(arr.content.len(), 1);
    assert_eq!(arr.content[0].value, "frog");
}

#[test]
fn expression_delete_splat() {
    // delpaths([["a", 0]])[]  ->  [frog]
    let input = mapping(vec![(
        "a",
        sequence(vec![string_scalar("cat"), string_scalar("frog")]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "delpaths([[\"a\", 0]])[]")
            .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Sequence);
    assert_eq!(result.content.len(), 1);
    assert_eq!(result.content[0].value, "frog");
}

#[test]
fn expression_delete_wrong_parameter_error() {
    // delpaths(["a", 0])  ->  error (not an array of path arrays)
    let input = mapping(vec![(
        "a",
        sequence(vec![string_scalar("cat"), string_scalar("frog")]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let result = expression_pipeline::execute_on_context(&mut engine, &ctx, "delpaths([\"a\", 0])");
    assert!(
        result.is_err(),
        "delpaths with wrong parameter should error"
    );
}

#[test]
fn expression_array_slicing() {
    // .[1:3]  ->  [dog, frog]
    let input = sequence(vec![
        string_scalar("cat"),
        string_scalar("dog"),
        string_scalar("frog"),
        string_scalar("cow"),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[1:3]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Sequence);
    assert_eq!(result.content.len(), 2);
    assert_eq!(result.content[0].value, "dog");
    assert_eq!(result.content[1].value, "frog");
}

#[test]
fn expression_array_slicing_without_first() {
    // .[:2]  ->  [cat, dog]
    let input = sequence(vec![
        string_scalar("cat"),
        string_scalar("dog"),
        string_scalar("frog"),
        string_scalar("cow"),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[:2]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Sequence);
    assert_eq!(result.content.len(), 2);
    assert_eq!(result.content[0].value, "cat");
    assert_eq!(result.content[1].value, "dog");
}

#[test]
fn expression_array_slicing_without_second() {
    // .[2:]  ->  [frog, cow]
    let input = sequence(vec![
        string_scalar("cat"),
        string_scalar("dog"),
        string_scalar("frog"),
        string_scalar("cow"),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[2:]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Sequence);
    assert_eq!(result.content.len(), 2);
    assert_eq!(result.content[0].value, "frog");
    assert_eq!(result.content[1].value, "cow");
}

#[test]
fn expression_array_slicing_negative() {
    // .[1:-1]  ->  [dog, frog]
    let input = sequence(vec![
        string_scalar("cat"),
        string_scalar("dog"),
        string_scalar("frog"),
        string_scalar("cow"),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[1:-1]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Sequence);
    assert_eq!(result.content.len(), 2);
    assert_eq!(result.content[0].value, "dog");
    assert_eq!(result.content[1].value, "frog");
}

#[test]
fn expression_insert_into_middle_of_array() {
    // (.[] | select(. == "dog") | key + 1) as $pos | .[0:($pos)] + ["rabbit"] + .[$pos:]
    // -> [cat, dog, rabbit, frog, cow]
    let input = sequence(vec![
        string_scalar("cat"),
        string_scalar("dog"),
        string_scalar("frog"),
        string_scalar("cow"),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "(.[] | select(. == \"dog\") | key + 1) as $pos | .[0:($pos)] + [\"rabbit\"] + .[$pos:]",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Sequence);
    assert_eq!(result.content.len(), 5);
    assert_eq!(result.content[0].value, "cat");
    assert_eq!(result.content[1].value, "dog");
    assert_eq!(result.content[2].value, "rabbit");
    assert_eq!(result.content[3].value, "frog");
    assert_eq!(result.content[4].value, "cow");
}

#[test]
fn expression_slice_on_nested_arrays() {
    // .[] | .[1:3]  ->  [dog, frog], [banana, grape]
    let input = sequence(vec![
        sequence(vec![
            string_scalar("cat"),
            string_scalar("dog"),
            string_scalar("frog"),
            string_scalar("cow"),
        ]),
        sequence(vec![
            string_scalar("apple"),
            string_scalar("banana"),
            string_scalar("grape"),
            string_scalar("mango"),
        ]),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[] | .[1:3]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "dog");
    assert_eq!(out.matching_nodes[0].content[1].value, "frog");
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[1].content.len(), 2);
    assert_eq!(out.matching_nodes[1].content[0].value, "banana");
    assert_eq!(out.matching_nodes[1].content[1].value, "grape");
}

#[test]
fn expression_slice_second_index_beyond_clamps() {
    // .[:3] on [cat]  ->  [cat] (clamped)
    let input = sequence(vec![string_scalar("cat")]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[:3]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Sequence);
    assert_eq!(result.content.len(), 1);
    assert_eq!(result.content[0].value, "cat");
}

#[test]
fn expression_slice_first_index_beyond_returns_nothing() {
    // .[3:] on [cat]  ->  [] (empty)
    let input = sequence(vec![string_scalar("cat")]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[3:]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Sequence);
    assert_eq!(result.content.len(), 0);
}

#[test]
fn expression_slice_negative_indices() {
    // .[] | .[-2:-1]  ->  [frog], [grape]
    let input = sequence(vec![
        sequence(vec![
            string_scalar("cat"),
            string_scalar("dog"),
            string_scalar("frog"),
            string_scalar("cow"),
        ]),
        sequence(vec![
            string_scalar("apple"),
            string_scalar("banana"),
            string_scalar("grape"),
            string_scalar("mango"),
        ]),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[] | .[-2:-1]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 1);
    assert_eq!(out.matching_nodes[0].content[0].value, "frog");
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[1].content.len(), 1);
    assert_eq!(out.matching_nodes[1].content[0].value, "grape");
}

#[test]
fn expression_slice_boundary_indices() {
    // .[10:11] on [cat1..cat11]  ->  [cat11]
    let input = sequence(
        (1..=11)
            .map(|i| string_scalar(&format!("cat{}", i)))
            .collect(),
    );
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[10:11]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Sequence);
    assert_eq!(result.content.len(), 1);
    assert_eq!(result.content[0].value, "cat11");
}

#[test]
fn expression_slice_boundary_negative_indices() {
    // .[-11:-10] on [cat1..cat11]  ->  [cat1]
    let input = sequence(
        (1..=11)
            .map(|i| string_scalar(&format!("cat{}", i)))
            .collect(),
    );
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[-11:-10]")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let result = &out.matching_nodes[0];
    assert_eq!(result.kind, NodeKind::Sequence);
    assert_eq!(result.content.len(), 1);
    assert_eq!(result.content[0].value, "cat1");
}
