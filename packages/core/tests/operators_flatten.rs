use treease_core::expression_pipeline;
use treease_core::operators::flatten::flatten_op;
use treease_core::operators::{
    Context, CoreError, ExpressionNode, FLATTEN_OP_TYPE, FlattenPreferences, NodeKind, Operation,
    OperationPreference, SemType, TreeEngine, TreeNode,
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

fn sequence(items: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: items,
        ..TreeNode::default()
    }
}

fn mapping(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        let mut k = string_scalar(key);
        k.is_map_key = true;
        content.push(k);
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

fn flatten_expression() -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &FLATTEN_OP_TYPE,
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

fn flatten_expression_with_depth(_depth: i32) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &FLATTEN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Flatten(FlattenPreferences {
                depth: _depth,
            }))),
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[test]
fn flatten_op_flattens_nested_sequences_unlimited_depth() {
    // [[1, 2], [3, [4, 5]]] -> [1, 2, 3, 4, 5] with depth -1 (unlimited)
    let nested = sequence(vec![
        sequence(vec![int_scalar(1), int_scalar(2)]),
        sequence(vec![
            int_scalar(3),
            sequence(vec![int_scalar(4), int_scalar(5)]),
        ]),
    ]);

    let ctx = Context {
        matching_nodes: vec![nested],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expr = flatten_expression();

    let result = flatten_op(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "flatten should succeed on nested arrays");

    let out = result.unwrap();
    assert_eq!(
        out.matching_nodes.len(),
        1,
        "should still have one matching node"
    );
    assert_eq!(out.matching_nodes[0].content.len(), 5);
    assert_eq!(out.matching_nodes[0].content[0].value, "1");
    assert_eq!(out.matching_nodes[0].content[1].value, "2");
    assert_eq!(out.matching_nodes[0].content[2].value, "3");
    assert_eq!(out.matching_nodes[0].content[3].value, "4");
    assert_eq!(out.matching_nodes[0].content[4].value, "5");
}

#[test]
fn flatten_op_on_single_level_array_is_noop() {
    // [1, 2, 3] stays [1, 2, 3]
    let flat = sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)]);

    let ctx = Context {
        matching_nodes: vec![flat.clone()],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expr = flatten_expression();

    let result = flatten_op(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "flatten on flat array should succeed");
}

#[test]
fn flatten_op_on_empty_array() {
    let empty = sequence(vec![]);

    let ctx = Context {
        matching_nodes: vec![empty],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expr = flatten_expression();

    let result = flatten_op(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "flatten on empty array should succeed");

    let out = result.unwrap();
    assert_eq!(out.matching_nodes.len(), 1);
    assert!(out.matching_nodes[0].content.is_empty());
}

#[test]
fn flatten_op_on_array_of_objects() {
    // [{a: 1}, {b: 2}] — objects are not sequences, so they pass through unchanged
    let arr = sequence(vec![
        mapping(vec![("a", int_scalar(1))]),
        mapping(vec![("b", int_scalar(2))]),
    ]);

    let ctx = Context {
        matching_nodes: vec![arr],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expr = flatten_expression();

    let result = flatten_op(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "flatten on array of objects should succeed");

    let out = result.unwrap();
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].content[0].value, "a");
    assert_eq!(out.matching_nodes[0].content[1].content[0].value, "b");
}

#[test]
fn flatten_op_errors_on_non_array_input() {
    // Passing a map should error
    let map = mapping(vec![("key", string_scalar("value"))]);

    let ctx = Context {
        matching_nodes: vec![map],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expr = flatten_expression();

    let result = flatten_op(ctx, &mut engine, &mut expr);
    assert!(result.is_err(), "flatten on non-array should error");

    match result.unwrap_err() {
        CoreError::OperatorMessage { op, message } => {
            assert_eq!(op, "flatten");
            assert!(message.contains("only arrays are supported"));
        }
        other => panic!("expected OperatorMessage error, got {:?}", other),
    }
}

#[test]
fn flatten_op_errors_on_scalar_input() {
    let scalar = string_scalar("not_an_array");

    let ctx = Context {
        matching_nodes: vec![scalar],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expr = flatten_expression();

    let result = flatten_op(ctx, &mut engine, &mut expr);
    assert!(result.is_err(), "flatten on scalar should error");
}

#[test]
fn flatten_op_deeply_nested_arrays() {
    // [[[1]]] -> [1] with unlimited depth
    let deep = sequence(vec![sequence(vec![sequence(vec![int_scalar(1)])])]);

    let ctx = Context {
        matching_nodes: vec![deep],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expr = flatten_expression();

    let result = flatten_op(ctx, &mut engine, &mut expr);
    assert!(
        result.is_ok(),
        "flatten on deeply nested arrays should succeed"
    );
}

#[test]
fn flatten_op_mixed_array_with_scalars_and_sequences() {
    // [1, [2, 3], 4] -> [1, 2, 3, 4]
    let mixed = sequence(vec![
        int_scalar(1),
        sequence(vec![int_scalar(2), int_scalar(3)]),
        int_scalar(4),
    ]);

    let ctx = Context {
        matching_nodes: vec![mixed],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expr = flatten_expression();

    let result = flatten_op(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "flatten on mixed array should succeed");

    let out = result.unwrap();
    assert_eq!(out.matching_nodes[0].content.len(), 4);
    assert_eq!(out.matching_nodes[0].content[0].value, "1");
    assert_eq!(out.matching_nodes[0].content[1].value, "2");
    assert_eq!(out.matching_nodes[0].content[2].value, "3");
    assert_eq!(out.matching_nodes[0].content[3].value, "4");
}

#[test]
fn flatten_op_respects_depth_limit() {
    // [1, [2, [3]]] with depth=1 → [1, 2, [3]]
    let nested = sequence(vec![
        int_scalar(1),
        sequence(vec![int_scalar(2), sequence(vec![int_scalar(3)])]),
    ]);

    let ctx = Context {
        matching_nodes: vec![nested],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expr = flatten_expression_with_depth(1);

    let result = flatten_op(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "flatten with depth=1 should succeed");

    let out = result.unwrap();
    assert_eq!(out.matching_nodes[0].content.len(), 3);
    assert_eq!(out.matching_nodes[0].content[0].value, "1");
    assert_eq!(out.matching_nodes[0].content[1].value, "2");
    assert_eq!(out.matching_nodes[0].content[2].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content[2].content[0].value, "3");
}

#[test]
fn flatten_op_with_depth_zero_is_noop() {
    // [1, [2]] with depth=0 stays [1, [2]]
    let nested = sequence(vec![int_scalar(1), sequence(vec![int_scalar(2)])]);

    let ctx = Context {
        matching_nodes: vec![nested],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expr = flatten_expression_with_depth(0);

    let result = flatten_op(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "flatten with depth=0 should succeed");

    let out = result.unwrap();
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "1");
    assert_eq!(out.matching_nodes[0].content[1].kind, NodeKind::Sequence);
}

#[test]
fn flatten_expression_supports_splat_output() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            int_scalar(1),
            sequence(vec![int_scalar(2)]),
            sequence(vec![sequence(vec![int_scalar(3)])]),
        ])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "flatten[]")
        .expect("flatten splat pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 3);
    assert_eq!(out.matching_nodes[0].value, "1");
    assert_eq!(out.matching_nodes[1].value, "2");
    assert_eq!(out.matching_nodes[2].value, "3");
}

#[test]
fn flatten_expression_with_depth_one_supports_splat_output() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            int_scalar(1),
            sequence(vec![int_scalar(2)]),
            sequence(vec![sequence(vec![int_scalar(3)])]),
        ])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "flatten(1)[]")
        .expect("flatten depth-one splat pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 3);
    assert_eq!(out.matching_nodes[0].value, "1");
    assert_eq!(out.matching_nodes[1].value, "2");
    assert_eq!(out.matching_nodes[2].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[2].content.len(), 1);
    assert_eq!(out.matching_nodes[2].content[0].value, "3");
}

#[test]
fn flatten_expression_flattens_nested_array_of_objects() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("foo", string_scalar("bar"))]),
            sequence(vec![mapping(vec![("foo", string_scalar("baz"))])]),
        ])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "flatten")
        .expect("flatten nested object array pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].content[0].value, "foo");
    assert_eq!(out.matching_nodes[0].content[0].content[1].value, "bar");
    assert_eq!(out.matching_nodes[0].content[1].content[0].value, "foo");
    assert_eq!(out.matching_nodes[0].content[1].content[1].value, "baz");
}
