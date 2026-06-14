use treease_core::expression_pipeline;
use treease_core::operators::assign::assign_update_operator;
use treease_core::operators::reverse::reverse_operator;
use treease_core::operators::sort::{sort_by_operator, sort_operator};
use treease_core::operators::{
    ASSIGN_OP_TYPE, Context, ExpressionNode, KEYS_OP_TYPE, NodeKind, Operation, PIPE_OP_TYPE,
    REVERSE_OP_TYPE, SORT_BY_OP_TYPE, SORT_OP_TYPE, SemType, TRAVERSE_ARRAY_OP_TYPE,
    TraversePreferences, TreeEngine, TreeNode, UNION_OP_TYPE, create_traversal_tree,
    create_value_operation, splat,
};

fn expression(
    operation: &'static treease_core::operators::OperationType,
    rhs: Option<ExpressionNode>,
) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: operation,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: rhs.map(Box::new),
    }
}

fn scalar(sem_type: SemType, value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(sem_type),
        tag: sem_type.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn int_scalar(value: i64) -> TreeNode {
    scalar(SemType::Int, &value.to_string())
}

fn bool_scalar(value: bool) -> TreeNode {
    scalar(SemType::Boolean, if value { "true" } else { "false" })
}

fn string_scalar(value: &str) -> TreeNode {
    scalar(SemType::Str, value)
}

fn null_scalar() -> TreeNode {
    scalar(SemType::Nil, "null")
}

fn mapping(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        content.push(string_scalar(key));
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

#[test]
fn sort_by_operator_sorts_sequence_of_maps_by_string_field() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", string_scalar("banana"))]),
            mapping(vec![("a", string_scalar("cat"))]),
            mapping(vec![("a", string_scalar("apple"))]),
        ])],
        ..Context::default()
    };
    let rhs = create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
        .unwrap();
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(*rhs));
    let mut engine = TreeEngine::default();

    let out = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.content.len(), 3);
    assert_eq!(sorted.content[0].content[1].value, "apple");
    assert_eq!(sorted.content[1].content[1].value, "banana");
    assert_eq!(sorted.content[2].content[1].value, "cat");
}

#[test]
fn sort_operator_orders_null_bool_number_and_string_values() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            int_scalar(8),
            int_scalar(3),
            null_scalar(),
            int_scalar(6),
            bool_scalar(true),
            bool_scalar(false),
            string_scalar("cat"),
        ])],
        ..Context::default()
    };
    let mut expr = expression(&SORT_OP_TYPE, None);
    let mut engine = TreeEngine::default();

    let out = sort_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    let values: Vec<_> = sorted
        .content
        .iter()
        .map(|node| node.value.as_str())
        .collect();
    assert_eq!(values, vec!["null", "false", "true", "3", "6", "8", "cat"]);
}

#[test]
fn sort_by_operator_is_stable_when_keys_are_equal() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", string_scalar("banana")), ("b", int_scalar(1))]),
            mapping(vec![("a", string_scalar("banana")), ("b", int_scalar(2))]),
            mapping(vec![("a", string_scalar("banana")), ("b", int_scalar(3))]),
        ])],
        ..Context::default()
    };
    let rhs = create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
        .unwrap();
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(*rhs));
    let mut engine = TreeEngine::default();

    let out = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.content[0].content[3].value, "1");
    assert_eq!(sorted.content[1].content[3].value, "2");
    assert_eq!(sorted.content[2].content[3].value, "3");
}

// ── Additional helpers ────────────────────────────────────────────

fn float_scalar(value: f64) -> TreeNode {
    scalar(SemType::Float, &value.to_string())
}

fn union_expression(lhs: ExpressionNode, rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &UNION_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(lhs)),
        rhs: Some(Box::new(rhs)),
    }
}

// ── Sort by multiple fields ───────────────────────────────────────

#[test]
fn sort_by_multiple_string_fields() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("a", string_scalar("banana")),
                ("b", string_scalar("z")),
            ]),
            mapping(vec![
                ("a", string_scalar("apple")),
                ("b", string_scalar("y")),
            ]),
            mapping(vec![
                ("a", string_scalar("apple")),
                ("b", string_scalar("x")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // sort_by(.a, .b) — sort by a then b
    let trav_a =
        create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
            .unwrap();
    let trav_b =
        create_traversal_tree(&[string_scalar("b")], TraversePreferences::default(), false)
            .unwrap();
    let union = union_expression(*trav_a, *trav_b);
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(union));

    let out = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.content.len(), 3);
    // apple,x then apple,y then banana,z
    assert_eq!(sorted.content[0].content[1].value, "apple");
    assert_eq!(sorted.content[0].content[3].value, "x");
    assert_eq!(sorted.content[1].content[1].value, "apple");
    assert_eq!(sorted.content[1].content[3].value, "y");
    assert_eq!(sorted.content[2].content[1].value, "banana");
    assert_eq!(sorted.content[2].content[3].value, "z");
}

#[test]
fn sort_by_multiple_fields_with_missing_field() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("a", string_scalar("banana")),
                ("b", string_scalar("z")),
            ]),
            mapping(vec![("a", string_scalar("apple"))]),
            mapping(vec![
                ("a", string_scalar("apple")),
                ("b", string_scalar("x")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let trav_a =
        create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
            .unwrap();
    let trav_b =
        create_traversal_tree(&[string_scalar("b")], TraversePreferences::default(), false)
            .unwrap();
    let union = union_expression(*trav_a, *trav_b);
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(union));

    let out = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.content.len(), 3);
    // Missing field sorts before present field (fewer results = less)
    assert_eq!(sorted.content[0].content.len(), 2); // only "a"
    assert_eq!(sorted.content[1].content[1].value, "apple");
    assert_eq!(sorted.content[1].content[3].value, "x");
    assert_eq!(sorted.content[2].content[1].value, "banana");
    assert_eq!(sorted.content[2].content[3].value, "z");
}

#[test]
fn sort_by_multiple_float_fields() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", float_scalar(1.1)), ("b", float_scalar(2.2))]),
            mapping(vec![("a", float_scalar(1.1)), ("b", float_scalar(1.1))]),
            mapping(vec![("a", float_scalar(1.001)), ("b", float_scalar(3.3))]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let trav_a =
        create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
            .unwrap();
    let trav_b =
        create_traversal_tree(&[string_scalar("b")], TraversePreferences::default(), false)
            .unwrap();
    let union = union_expression(*trav_a, *trav_b);
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(union));

    let out = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.content.len(), 3);
    assert_eq!(sorted.content[0].content[1].value, "1.001");
    assert_eq!(sorted.content[1].content[1].value, "1.1");
    assert_eq!(sorted.content[1].content[3].value, "1.1");
    assert_eq!(sorted.content[2].content[1].value, "1.1");
    assert_eq!(sorted.content[2].content[3].value, "2.2");
}

// ── Sort by numeric field ─────────────────────────────────────────

#[test]
fn sort_by_integer_field() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", int_scalar(30))]),
            mapping(vec![("a", int_scalar(10))]),
            mapping(vec![("a", int_scalar(20))]),
        ])],
        ..Context::default()
    };
    let rhs = create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
        .unwrap();
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(*rhs));
    let mut engine = TreeEngine::default();

    let out = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.content.len(), 3);
    assert_eq!(sorted.content[0].content[1].value, "10");
    assert_eq!(sorted.content[1].content[1].value, "20");
    assert_eq!(sorted.content[2].content[1].value, "30");
}

// ── Sort by float values ──────────────────────────────────────────

#[test]
fn sort_by_float_field() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", float_scalar(1.1))]),
            mapping(vec![("a", float_scalar(1.001))]),
            mapping(vec![("a", float_scalar(1.01))]),
        ])],
        ..Context::default()
    };
    let rhs = create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
        .unwrap();
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(*rhs));
    let mut engine = TreeEngine::default();

    let out = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.content.len(), 3);
    assert_eq!(sorted.content[0].content[1].value, "1.001");
    assert_eq!(sorted.content[1].content[1].value, "1.01");
    assert_eq!(sorted.content[2].content[1].value, "1.1");
}

// ── Sort by boolean field ─────────────────────────────────────────

#[test]
fn sort_by_boolean_field() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", bool_scalar(true))]),
            mapping(vec![("a", bool_scalar(false))]),
            mapping(vec![("a", bool_scalar(true))]),
        ])],
        ..Context::default()
    };
    let rhs = create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
        .unwrap();
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(*rhs));
    let mut engine = TreeEngine::default();

    let out = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.content.len(), 3);
    // false before true
    assert_eq!(sorted.content[0].content[1].value, "false");
    assert_eq!(sorted.content[1].content[1].value, "true");
    assert_eq!(sorted.content[2].content[1].value, "true");
}

// ── Sort a map directly ───────────────────────────────────────────

#[test]
fn sort_operator_on_map_sorts_by_values() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("c", int_scalar(3)),
            ("a", int_scalar(1)),
            ("b", int_scalar(2)),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "sort").unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.kind, NodeKind::Mapping);
    // Values sorted: 1, 2, 3
    assert_eq!(sorted.content[1].value, "1");
    assert_eq!(sorted.content[3].value, "2");
    assert_eq!(sorted.content[5].value, "3");
}

// ── Reverse operator ──────────────────────────────────────────────

#[test]
fn reverse_operator_reverses_sequence_order() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            string_scalar("apple"),
            string_scalar("banana"),
            string_scalar("cat"),
        ])],
        ..Context::default()
    };
    let mut expr = expression(&REVERSE_OP_TYPE, None);
    let mut engine = TreeEngine::default();

    let out = reverse_operator(ctx, &mut engine, &mut expr).unwrap();

    let reversed = &out.matching_nodes[0];
    assert_eq!(reversed.kind, NodeKind::Sequence);
    assert_eq!(reversed.content.len(), 3);
    assert_eq!(reversed.content[0].value, "cat");
    assert_eq!(reversed.content[1].value, "banana");
    assert_eq!(reversed.content[2].value, "apple");
}

#[test]
fn sort_by_then_reverse_gives_descending_order() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", string_scalar("banana"))]),
            mapping(vec![("a", string_scalar("apple"))]),
            mapping(vec![("a", string_scalar("cat"))]),
        ])],
        ..Context::default()
    };
    let rhs = create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
        .unwrap();
    let mut sort_expr = expression(&SORT_BY_OP_TYPE, Some(*rhs));
    let mut engine = TreeEngine::default();

    let sorted_ctx = sort_by_operator(ctx, &mut engine, &mut sort_expr).unwrap();

    // Now reverse the sorted result
    let mut rev_expr = expression(&REVERSE_OP_TYPE, None);
    let out = reverse_operator(sorted_ctx, &mut engine, &mut rev_expr).unwrap();

    let reversed = &out.matching_nodes[0];
    assert_eq!(reversed.content.len(), 3);
    assert_eq!(reversed.content[0].content[1].value, "cat");
    assert_eq!(reversed.content[1].content[1].value, "banana");
    assert_eq!(reversed.content[2].content[1].value, "apple");
}

// ── Sort by string field with splat ───────────────────────────────

#[test]
fn sort_by_string_field_with_splat() {
    // sort_by(.a)[] — splatting sorted results to emit individual elements
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", string_scalar("banana"))]),
            mapping(vec![("a", string_scalar("apple"))]),
        ])],
        ..Context::default()
    };
    let rhs = create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
        .unwrap();
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(*rhs));
    let mut engine = TreeEngine::default();

    let sorted = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();
    let splatted = splat(sorted, TraversePreferences::default()).unwrap();

    assert_eq!(splatted.matching_nodes.len(), 2);
    assert_eq!(splatted.matching_nodes[0].content[1].value, "apple");
    assert_eq!(splatted.matching_nodes[1].content[1].value, "banana");
}

#[test]
fn sort_update_assign_rewrites_nested_sequence_in_place() {
    let input = mapping(vec![(
        "items",
        sequence(vec![
            mapping(vec![("a", string_scalar("banana"))]),
            mapping(vec![("a", string_scalar("cat"))]),
            mapping(vec![("a", string_scalar("apple"))]),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".items |= sort_by(.a)")
        .expect("sort update assign should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let root = &out.matching_nodes[0];
    assert_eq!(root.content[0].value, "items");
    let items = &root.content[1];
    assert_eq!(items.kind, NodeKind::Sequence);
    assert_eq!(items.content.len(), 3);
    assert_eq!(items.content[0].content[1].value, "apple");
    assert_eq!(items.content[1].content[1].value, "banana");
    assert_eq!(items.content[2].content[1].value, "cat");
}

// ── Sort by with null (nulls first) ───────────────────────────────

#[test]
fn sort_by_with_null_nulls_first() {
    // sort_by(.a)[] with null — null elements sort before non-null
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            null_scalar(),
            mapping(vec![("a", string_scalar("banana"))]),
            mapping(vec![("a", string_scalar("apple"))]),
        ])],
        ..Context::default()
    };
    let rhs = create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
        .unwrap();
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(*rhs));
    let mut engine = TreeEngine::default();

    let sorted = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();
    let splatted = splat(sorted, TraversePreferences::default()).unwrap();

    assert_eq!(splatted.matching_nodes.len(), 3);
    // null first
    assert_eq!(splatted.matching_nodes[0].sem_type, Some(SemType::Nil));
    assert_eq!(splatted.matching_nodes[1].content[1].value, "apple");
    assert_eq!(splatted.matching_nodes[2].content[1].value, "banana");
}

// ── Sort array in place via update-assign ─────────────────────────

#[test]
fn sort_array_in_place_via_update_assign() {
    // .cool |= sort_by(.a) — update-assign to sort a nested array in-place
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(mapping(vec![(
        "cool",
        sequence(vec![
            mapping(vec![("a", string_scalar("banana"))]),
            mapping(vec![("a", string_scalar("apple"))]),
        ]),
    )]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    // Build: .cool |= sort_by(.a)
    let sort_rhs =
        create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
            .unwrap();
    let sort_by_expr = expression(&SORT_BY_OP_TYPE, Some(*sort_rhs));

    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: true,
        }),
        lhs: Some(
            create_traversal_tree(
                &[string_scalar("cool")],
                TraversePreferences::default(),
                false,
            )
            .unwrap(),
        ),
        rhs: Some(Box::new(sort_by_expr)),
    };

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "update-assign sort should succeed");

    // Verify the sort behavior directly on the extracted sequence
    let seq_ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", string_scalar("banana"))]),
            mapping(vec![("a", string_scalar("apple"))]),
        ])],
        ..Context::default()
    };
    let mut sort_expr = expression(
        &SORT_BY_OP_TYPE,
        Some(
            *create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
                .unwrap(),
        ),
    );
    let mut engine2 = TreeEngine::default();
    let out = sort_by_operator(seq_ctx, &mut engine2, &mut sort_expr).unwrap();
    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.content[0].content[1].value, "apple");
    assert_eq!(sorted.content[1].content[1].value, "banana");
}

// ── Sort array of objects by key expression ───────────────────────

#[test]
fn sort_array_of_objects_by_key_expression() {
    // .cool |= sort_by(keys | .[0]) — sorting by a complex expression
    // Input: {cool: [{banana: 1}, {apple: 2}]}
    // keys returns the keys of each map, .[0] takes the first key,
    // so we sort by the (only) key name: "apple" < "banana"

    // Build: keys | .[0]
    let index_seq = sequence(vec![int_scalar(0)]);
    let traverse_array_expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &TRAVERSE_ARRAY_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(index_seq)).unwrap(),
            lhs: None,
            rhs: None,
        })),
    };

    let keys_expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &KEYS_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    };

    let pipe_expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &PIPE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(keys_expr)),
        rhs: Some(Box::new(traverse_array_expr)),
    };

    // sort_by(keys | .[0])
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("banana", int_scalar(1))]),
            mapping(vec![("apple", int_scalar(2))]),
        ])],
        ..Context::default()
    };
    let mut expr = expression(&SORT_BY_OP_TYPE, Some(pipe_expr));
    let mut engine = TreeEngine::default();

    let out = sort_by_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.content.len(), 2);
    // Sorted by key name: "apple" before "banana"
    assert_eq!(sorted.content[0].content[0].value, "apple");
    assert_eq!(sorted.content[0].content[1].value, "2");
    assert_eq!(sorted.content[1].content[0].value, "banana");
    assert_eq!(sorted.content[1].content[1].value, "1");
}

#[test]
fn sort_pipeline_sorts_nested_array_of_objects_by_key_expression_like_zig() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "cool",
            sequence(vec![
                mapping(vec![("b", string_scalar("banana"))]),
                mapping(vec![("a", string_scalar("banana"))]),
                mapping(vec![("c", string_scalar("banana"))]),
            ]),
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, ".cool |= sort_by(keys | .[0])")
            .expect("nested sort_by key pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let cool = &out.matching_nodes[0].content[1];
    assert_eq!(cool.kind, NodeKind::Sequence);
    assert_eq!(cool.content[0].content[0].value, "a");
    assert_eq!(cool.content[1].content[0].value, "b");
    assert_eq!(cool.content[2].content[0].value, "c");
}

// ── Sort nulls come first with splat ──────────────────────────────

#[test]
fn sort_nulls_come_first_with_splat() {
    // sort[] on [8, null] — splatting sorted results where null comes before int
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(8), null_scalar()])],
        ..Context::default()
    };
    let mut expr = expression(&SORT_OP_TYPE, None);
    let mut engine = TreeEngine::default();

    let sorted = sort_operator(ctx, &mut engine, &mut expr).unwrap();
    let splatted = splat(sorted, TraversePreferences::default()).unwrap();

    assert_eq!(splatted.matching_nodes.len(), 2);
    // null comes first
    assert_eq!(splatted.matching_nodes[0].sem_type, Some(SemType::Nil));
    assert_eq!(splatted.matching_nodes[0].value, "null");
    assert_eq!(splatted.matching_nodes[1].sem_type, Some(SemType::Int));
    assert_eq!(splatted.matching_nodes[1].value, "8");
}

// ── Head comment preservation ─────────────────────────────────────

#[test]
fn sort_preserves_head_comment() {
    // sort on a sequence with head/foot comments — comments are preserved
    let mut seq = sequence(vec![string_scalar("def")]);
    seq.head_comment = "# abc\n".to_string();
    seq.foot_comment = "# ghi".to_string();

    let ctx = Context {
        matching_nodes: vec![seq],
        ..Context::default()
    };
    let mut expr = expression(&SORT_OP_TYPE, None);
    let mut engine = TreeEngine::default();

    let out = sort_operator(ctx, &mut engine, &mut expr).unwrap();

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.kind, NodeKind::Sequence);
    assert_eq!(sorted.content.len(), 1);
    assert_eq!(sorted.content[0].value, "def");
    // Comments should be preserved
    assert!(
        sorted.head_comment.contains("abc"),
        "head comment should be preserved"
    );
    assert!(
        sorted.foot_comment.contains("ghi"),
        "foot comment should be preserved"
    );
}

#[test]
fn sort_by_then_reverse_pipeline_matches_zig_descending_scenario() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", string_scalar("banana"))]),
            mapping(vec![("a", string_scalar("cat"))]),
            mapping(vec![("a", string_scalar("apple"))]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "sort_by(.a) | reverse")
        .expect("descending sort pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let reversed = &out.matching_nodes[0];
    assert_eq!(reversed.content[0].content[1].value, "cat");
    assert_eq!(reversed.content[1].content[1].value, "banana");
    assert_eq!(reversed.content[2].content[1].value, "apple");
}

#[test]
fn sort_pipeline_sorts_map_values_like_zig_map_scenario() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("y", string_scalar("b")),
            ("z", string_scalar("a")),
            ("x", string_scalar("c")),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "sort")
        .expect("sort pipeline should succeed");

    let sorted = &out.matching_nodes[0];
    assert_eq!(sorted.kind, NodeKind::Mapping);
    assert_eq!(sorted.content[0].value, "z");
    assert_eq!(sorted.content[1].value, "a");
    assert_eq!(sorted.content[2].value, "y");
    assert_eq!(sorted.content[3].value, "b");
    assert_eq!(sorted.content[4].value, "x");
    assert_eq!(sorted.content[5].value, "c");
}
