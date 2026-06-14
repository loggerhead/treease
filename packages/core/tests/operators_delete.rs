use treease_core::expression_pipeline;
use treease_core::operators::delete::delete_child_operator;
use treease_core::operators::{
    Context, DELETE_OP_TYPE, ExpressionNode, NodeId, NodeKind, Operation, SemType,
    TraversePreferences, TreeEngine, TreeNode, create_traversal_tree,
};

fn delete_expression(path_segments: Vec<TreeNode>) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &DELETE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(
            create_traversal_tree(&path_segments, TraversePreferences::default(), false).unwrap(),
        ),
    }
}

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

fn empty_map() -> TreeNode {
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.tag().to_owned(),
        ..TreeNode::default()
    }
}

fn empty_seq() -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        ..TreeNode::default()
    }
}

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

// ── Document-building helpers for expression-pipeline tests ─────

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

fn null_scalar() -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Nil),
        tag: SemType::Nil.tag().to_owned(),
        value: "null".to_owned(),
        ..TreeNode::default()
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[test]
fn delete_operator_removes_entry_in_map() {
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(empty_map());
    push_map_entry(&mut engine, root_id, "a", string_scalar("cat"));
    push_map_entry(&mut engine, root_id, "b", string_scalar("dog"));
    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };
    let mut expr = delete_expression(vec![string_scalar("b")]);

    delete_child_operator(ctx, &mut engine, &mut expr).unwrap();

    let root = engine.store.get(root_id);
    assert_eq!(root.content.len(), 2);
    assert_eq!(root.content[0].value, "a");
    assert_eq!(root.content[1].value, "cat");
}

#[test]
fn delete_operator_removes_entry_in_array_and_reindexes() {
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(empty_seq());
    {
        let root = engine.store.get_mut(root_id);
        for (index, value) in [1_i64, 2, 3].into_iter().enumerate() {
            let mut child = int_scalar(value);
            child.parent = Some(root_id);
            child.sequence_index = Some(index as i64);
            root.content.push(child);
        }
    }
    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };
    let mut expr = delete_expression(vec![int_scalar(1)]);

    delete_child_operator(ctx, &mut engine, &mut expr).unwrap();

    let root = engine.store.get(root_id);
    assert_eq!(root.content.len(), 2);
    assert_eq!(root.content[0].value, "1");
    assert_eq!(root.content[1].value, "3");
    assert_eq!(root.content[0].sequence_index, Some(0));
    assert_eq!(root.content[1].sequence_index, Some(1));
}

#[test]
fn delete_operator_removes_nested_entry_in_array() {
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(empty_seq());
    let nested_id = engine.store.add(empty_map());

    push_map_entry(&mut engine, nested_id, "a", string_scalar("cat"));
    push_map_entry(&mut engine, nested_id, "b", string_scalar("dog"));

    {
        let nested = engine.store.get_mut(nested_id);
        nested.parent = Some(root_id);
        nested.sequence_index = Some(0);
    }
    {
        let nested = engine.store.get(nested_id).clone();
        let root = engine.store.get_mut(root_id);
        root.content.push(nested);
    }

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };
    let mut expr = delete_expression(vec![int_scalar(0), string_scalar("a")]);

    delete_child_operator(ctx, &mut engine, &mut expr).unwrap();

    let nested = engine.store.get(nested_id);
    assert_eq!(nested.content.len(), 2);
    assert_eq!(nested.content[0].value, "b");
    assert_eq!(nested.content[1].value, "dog");
}

#[test]
fn delete_operator_keeps_document_when_path_has_no_matches() {
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(empty_map());
    push_map_entry(&mut engine, root_id, "a", string_scalar("cat"));
    push_map_entry(&mut engine, root_id, "b", string_scalar("dog"));
    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };
    let mut expr = delete_expression(vec![string_scalar("c")]);

    delete_child_operator(ctx, &mut engine, &mut expr).unwrap();

    let root = engine.store.get(root_id);
    assert_eq!(root.content.len(), 4);
    assert_eq!(root.content[0].value, "a");
    assert_eq!(root.content[1].value, "cat");
    assert_eq!(root.content[2].value, "b");
    assert_eq!(root.content[3].value, "dog");
}

// ── Delete nested entry in map ───────────────────────────────────

#[test]
fn delete_nested_entry_in_map() {
    let mut engine = TreeEngine::default();
    let input = mapping(vec![(
        "a",
        mapping(vec![
            ("a1", string_scalar("fred")),
            ("a2", string_scalar("frood")),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut expr = delete_expression(vec![string_scalar("a"), string_scalar("a1")]);

    let out = delete_child_operator(ctx, &mut engine, &mut expr).unwrap();

    let root = &out.matching_nodes[0];
    assert_eq!(root.content.len(), 2);
    assert_eq!(root.content[0].value, "a");
    let inner = &root.content[1];
    assert_eq!(inner.kind, NodeKind::Mapping);
    assert_eq!(inner.content.len(), 2);
    assert_eq!(inner.content[0].value, "a2");
    assert_eq!(inner.content[1].value, "frood");
}

// ── Delete via pipe ──────────────────────────────────────────────

#[test]
fn delete_via_pipe() {
    let input = mapping(vec![(
        "a",
        mapping(vec![
            ("a1", string_scalar("fred")),
            ("a2", string_scalar("frood")),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a | del(.a1)")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a2");
    assert_eq!(out.matching_nodes[0].content[1].value, "frood");
}

// ── Delete whole document ────────────────────────────────────────

#[test]
fn delete_whole_document() {
    let doc1 = mapping(vec![("a", string_scalar("slow"))]);
    let doc2 = mapping(vec![("a", string_scalar("fast"))]);
    let ctx = Context {
        matching_nodes: vec![doc1, doc2],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "del(select(.a == \"fast\"))")
            .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content[1].value, "slow");
}

// ── Delete array element via pipe ────────────────────────────────

#[test]
fn delete_array_element_via_pipe() {
    let input = mapping(vec![(
        "a",
        sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a | del(.[1])")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "1");
    assert_eq!(out.matching_nodes[0].content[1].value, "3");
}

// ── Delete from nested array ─────────────────────────────────────

#[test]
fn delete_from_nested_array_index_1() {
    let input = sequence(vec![
        int_scalar(0),
        mapping(vec![
            ("a", string_scalar("cat")),
            ("b", string_scalar("dog")),
        ]),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[1] | del(.a)")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "b");
    assert_eq!(out.matching_nodes[0].content[1].value, "dog");
}

#[test]
fn delete_from_nested_array_index_0() {
    let input = sequence(vec![mapping(vec![
        ("a", string_scalar("cat")),
        ("b", string_scalar("dog")),
    ])]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[0] | del(.a)")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "b");
    assert_eq!(out.matching_nodes[0].content[1].value, "dog");
}

// ── Delete nested in array ───────────────────────────────────────

#[test]
fn delete_nested_in_array_via_traversal() {
    let input = sequence(vec![mapping(vec![(
        "a",
        mapping(vec![
            ("b", string_scalar("thing")),
            ("c", string_scalar("frog")),
        ]),
    )])]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[0].a | del(.b)")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "c");
    assert_eq!(out.matching_nodes[0].content[1].value, "frog");
}

#[test]
fn delete_nested_in_array_via_pipe() {
    let input = sequence(vec![mapping(vec![(
        "a",
        mapping(vec![
            ("b", string_scalar("thing")),
            ("c", string_scalar("frog")),
        ]),
    )])]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[0] | del(.a.b)")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    // The result is the map at index 0: {a: {c: frog}}
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a");
    let inner = &out.matching_nodes[0].content[1];
    assert_eq!(inner.kind, NodeKind::Mapping);
    assert_eq!(inner.content.len(), 2);
    assert_eq!(inner.content[0].value, "c");
    assert_eq!(inner.content[1].value, "frog");
}

// ── Delete from array-in-map ─────────────────────────────────────

#[test]
fn delete_from_array_in_map_via_index() {
    let input = mapping(vec![(
        "a",
        sequence(vec![
            int_scalar(0),
            mapping(vec![
                ("b", string_scalar("thing")),
                ("c", string_scalar("frog")),
            ]),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a[1] | del(.b)")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "c");
    assert_eq!(out.matching_nodes[0].content[1].value, "frog");
}

#[test]
fn delete_from_array_in_map_via_pipe() {
    let input = mapping(vec![(
        "a",
        sequence(vec![
            int_scalar(0),
            mapping(vec![
                ("b", string_scalar("thing")),
                ("c", string_scalar("frog")),
            ]),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a | del(.[1].b)")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "0");
    let inner = &out.matching_nodes[0].content[1];
    assert_eq!(inner.kind, NodeKind::Mapping);
    assert_eq!(inner.content.len(), 2);
    assert_eq!(inner.content[0].value, "c");
    assert_eq!(inner.content[1].value, "frog");
}

// ── Recursive delete ─────────────────────────────────────────────

#[test]
fn recursive_delete_matching_value() {
    let input = mapping(vec![(
        "a",
        mapping(vec![
            ("a1", string_scalar("fred")),
            ("a2", string_scalar("frood")),
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
        "del(.. | select(.==\"frood\"))",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a");
    let inner = &out.matching_nodes[0].content[1];
    assert_eq!(inner.kind, NodeKind::Mapping);
    assert_eq!(inner.content.len(), 2);
    assert_eq!(inner.content[0].value, "a1");
    assert_eq!(inner.content[1].value, "fred");
}

// ── Delete all array elements ────────────────────────────────────

#[test]
fn delete_all_array_elements() {
    let input = mapping(vec![(
        "a",
        sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "del(.a[])")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a");
    assert_eq!(out.matching_nodes[0].content[1].kind, NodeKind::Sequence);
    assert!(out.matching_nodes[0].content[1].content.is_empty());
}

// ── Delete after append ──────────────────────────────────────────

#[test]
fn delete_after_append() {
    let input = sequence(vec![int_scalar(1), int_scalar(2)]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ". += [3] | del(.[2])")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "1");
    assert_eq!(out.matching_nodes[0].content[1].value, "2");
}

// ── Delete after sort ────────────────────────────────────────────

#[test]
fn delete_after_sort() {
    let input = sequence(vec![int_scalar(3), int_scalar(2), int_scalar(1)]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "sort | del(.[2])")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "1");
    assert_eq!(out.matching_nodes[0].content[1].value, "2");
}

// ── Delete after reverse ─────────────────────────────────────────

#[test]
fn delete_after_reverse() {
    let input = sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "reverse | del(.[2])")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "3");
    assert_eq!(out.matching_nodes[0].content[1].value, "2");
}

// ── Delete after shuffle ─────────────────────────────────────────

#[test]
fn delete_after_shuffle() {
    let input = sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "shuffle | del(.[2])")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    // After shuffle, the order is deterministic (seeded), so we check
    // that the result has 2 elements and the values are a subset of {1,2,3}
    let values: Vec<&str> = out.matching_nodes[0]
        .content
        .iter()
        .map(|n| n.value.as_str())
        .collect();
    assert!(values.contains(&"1"));
    assert!(values.contains(&"3"));
}

// ── Delete from keys ─────────────────────────────────────────────

#[test]
fn delete_from_keys() {
    let input = mapping(vec![
        ("a", int_scalar(1)),
        ("b", int_scalar(2)),
        ("c", int_scalar(3)),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "keys | del(.[] | select(.==\"b\"))",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a");
    assert_eq!(out.matching_nodes[0].content[1].value, "c");
}

// ── Delete after flatten ─────────────────────────────────────────

#[test]
fn delete_after_flatten() {
    let input = sequence(vec![
        int_scalar(1),
        sequence(vec![int_scalar(2)]),
        sequence(vec![sequence(vec![int_scalar(3)])]),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "flatten | del(.[2])")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "1");
    assert_eq!(out.matching_nodes[0].content[1].value, "2");
}

// ── Delete matching values ───────────────────────────────────────

#[test]
fn delete_matching_values() {
    let input = mapping(vec![(
        "a",
        sequence(vec![
            int_scalar(10),
            string_scalar("x"),
            int_scalar(10),
            int_scalar(10),
            string_scalar("x"),
            int_scalar(10),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "del(.a[] | select(. == 10))")
            .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a");
    let arr = &out.matching_nodes[0].content[1];
    assert_eq!(arr.kind, NodeKind::Sequence);
    assert_eq!(arr.content.len(), 2);
    assert_eq!(arr.content[0].value, "x");
    assert_eq!(arr.content[1].value, "x");
}

// ── Delete everything ────────────────────────────────────────────

#[test]
fn delete_everything_on_null() {
    let input = mapping(vec![("a", null_scalar())]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "del(..)")
        .expect("evaluation should succeed");

    assert!(out.matching_nodes.is_empty());
}

#[test]
fn delete_everything_on_nested_maps() {
    let input = mapping(vec![(
        "a",
        mapping(vec![
            ("thing1", string_scalar("yep")),
            ("thing2", string_scalar("cool")),
            ("thing3", string_scalar("hi")),
            (
                "b",
                mapping(vec![
                    ("thing1", string_scalar("cool")),
                    ("great", string_scalar("huh")),
                ]),
            ),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "del(..)")
        .expect("evaluation should succeed");

    assert!(out.matching_nodes.is_empty());
}

// ── Delete with complex select ───────────────────────────────────

#[test]
fn delete_with_complex_select() {
    let input = mapping(vec![(
        "a",
        mapping(vec![
            ("thing1", string_scalar("yep")),
            ("thing2", string_scalar("cool")),
            ("thing3", string_scalar("hi")),
            (
                "b",
                mapping(vec![
                    ("thing1", string_scalar("cool")),
                    ("great", string_scalar("huh")),
                ]),
            ),
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
        "del(.. | select(tag == \"!!map\") | (.b.thing1,.thing2))",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a");
    let inner = &out.matching_nodes[0].content[1];
    assert_eq!(inner.kind, NodeKind::Mapping);
    // Should have: thing1: yep, thing3: hi, b: {great: huh}
    // thing2 was deleted, b.thing1 was deleted
    assert_eq!(inner.content.len(), 6);
    assert_eq!(inner.content[0].value, "thing1");
    assert_eq!(inner.content[1].value, "yep");
    assert_eq!(inner.content[2].value, "thing3");
    assert_eq!(inner.content[3].value, "hi");
    assert_eq!(inner.content[4].value, "b");
    let b_inner = &inner.content[5];
    assert_eq!(b_inner.kind, NodeKind::Mapping);
    assert_eq!(b_inner.content.len(), 2);
    assert_eq!(b_inner.content[0].value, "great");
    assert_eq!(b_inner.content[1].value, "huh");
}

// ── Delete matching entries ──────────────────────────────────────

#[test]
fn delete_matching_entries() {
    let input = mapping(vec![
        ("a", string_scalar("cat")),
        ("b", string_scalar("dog")),
        ("c", string_scalar("bat")),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "del( .[] | select(. == \"*at\") )",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "b");
    assert_eq!(out.matching_nodes[0].content[1].value, "dog");
}

// ── Recursively delete matching keys ─────────────────────────────

#[test]
fn recursively_delete_matching_keys() {
    let input = mapping(vec![(
        "a",
        mapping(vec![
            ("name", string_scalar("frog")),
            (
                "b",
                mapping(vec![
                    ("name", string_scalar("blog")),
                    ("age", int_scalar(12)),
                ]),
            ),
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
        "del(.. | select(has(\"name\")).name)",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a");
    let inner = &out.matching_nodes[0].content[1];
    assert_eq!(inner.kind, NodeKind::Mapping);
    // Should have: b: {age: 12} (name keys were deleted)
    assert_eq!(inner.content.len(), 2);
    assert_eq!(inner.content[0].value, "b");
    let b_inner = &inner.content[1];
    assert_eq!(b_inner.kind, NodeKind::Mapping);
    assert_eq!(b_inner.content.len(), 2);
    assert_eq!(b_inner.content[0].value, "age");
    assert_eq!(b_inner.content[1].value, "12");
}

// ── Repeated delete ──────────────────────────────────────────────

#[test]
fn repeated_delete_first_element() {
    let input = mapping(vec![(
        "a",
        sequence(vec![
            int_scalar(0),
            int_scalar(1),
            int_scalar(2),
            int_scalar(3),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "del(.a[0]) | del(.a[0])")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a");
    let arr = &out.matching_nodes[0].content[1];
    assert_eq!(arr.kind, NodeKind::Sequence);
    assert_eq!(arr.content.len(), 2);
    assert_eq!(arr.content[0].value, "2");
    assert_eq!(arr.content[1].value, "3");
}
