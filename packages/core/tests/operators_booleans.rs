use treease_core::expression_pipeline;
use treease_core::operators::booleans::{all_operator, any_operator, not_operator};
use treease_core::operators::{
    ALL_OP_TYPE, ANY_OP_TYPE, Context, ExpressionNode, NOT_OP_TYPE, NodeKind, Operation, SemType,
    TreeEngine, TreeNode,
};

fn expression(operation_type: &'static treease_core::operators::OperationType) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type,
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

fn bool_scalar(value: bool) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Boolean),
        tag: SemType::Boolean.tag().to_owned(),
        value: if value { "true" } else { "false" }.to_string(),
        ..TreeNode::default()
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

fn null_scalar() -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Nil),
        tag: SemType::Nil.tag().to_owned(),
        value: String::new(),
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

#[test]
fn any_operator_returns_true_when_any_element_is_truthy() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            bool_scalar(false),
            bool_scalar(true),
            bool_scalar(false),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&ANY_OP_TYPE);

    let out = any_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes[0].value, "true");
}

#[test]
fn all_operator_returns_false_when_any_element_is_falsy() {
    let ctx = Context {
        matching_nodes: vec![
            sequence(vec![bool_scalar(true), bool_scalar(true)]),
            sequence(vec![bool_scalar(true), bool_scalar(false)]),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&ALL_OP_TYPE);

    let out = all_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes[0].value, "true");
    assert_eq!(out.matching_nodes[1].value, "false");
}

#[test]
fn not_operator_inverts_boolean_truthiness() {
    let ctx = Context {
        matching_nodes: vec![bool_scalar(true), bool_scalar(false), null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&NOT_OP_TYPE);

    let out = not_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes[0].value, "false");
    assert_eq!(out.matching_nodes[1].value, "true");
    assert_eq!(out.matching_nodes[2].value, "true");
}

// ── Lexer test ────────────────────────────────────────────────────

#[test]
fn lexer_parses_any_and_all_keywords() {
    use treease_core::core::expression::OperationId;
    use treease_core::parser::{TokenKind, lex_participle};

    let tokens_any = lex_participle("any").unwrap();
    assert_eq!(tokens_any.len(), 1);
    match &tokens_any[0].kind {
        TokenKind::Operation(op) => assert_eq!(op.operation_type.id, OperationId::Any),
        _ => panic!("expected operation token for 'any'"),
    }

    let tokens_all = lex_participle("all").unwrap();
    assert_eq!(tokens_all.len(), 1);
    match &tokens_all[0].kind {
        TokenKind::Operation(op) => assert_eq!(op.operation_type.id, OperationId::All),
        _ => panic!("expected operation token for 'all'"),
    }
}

// ── Basic boolean operator scenarios ──────────────────────────────

#[test]
fn or_operator_true_or_false_returns_true() {
    // `or` example: true or false → true
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "true or false").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "true");
    assert_eq!(out.matching_nodes[0].tag, SemType::Boolean.to_string());
}

#[test]
fn and_operator_true_and_false_returns_false() {
    // `and` example: true and false → false
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "true and false").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
    assert_eq!(out.matching_nodes[0].tag, SemType::Boolean.to_string());
}

#[test]
fn or_operator_false_or_false_returns_false() {
    // false or false → false
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "false or false").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
}

#[test]
fn and_operator_short_circuit_false_and_test() {
    // And should not run 2nd arg if first is false
    // false and test(3) → false (test(3) never evaluated)
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "false and test(3)").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
}

#[test]
fn or_operator_short_circuit_true_or_test() {
    // Or should not run 2nd arg if first is true
    // true or test(3) → true (test(3) never evaluated)
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "true or test(3)").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "true");
}

// ── Not operator truthiness scenarios ─────────────────────────────

#[test]
fn not_operator_true_becomes_false() {
    // Not true is false
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "true | not").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
}

#[test]
fn not_operator_false_becomes_true() {
    // Not false is true
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "false | not").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "true");
}

#[test]
fn not_operator_string_is_truthy() {
    // String values considered to be true
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "\"cat\" | not").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
}

#[test]
fn not_operator_empty_string_is_truthy() {
    // Empty string value considered to be true
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "\"\" | not").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
}

#[test]
fn not_operator_number_is_truthy() {
    // Numbers are considered to be true
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "1 | not").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
}

#[test]
fn not_operator_zero_is_truthy() {
    // Zero is considered to be true
    let ctx = Context {
        matching_nodes: vec![null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "0 | not").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
}

// ── Traversal-based or/and scenarios ──────────────────────────────

#[test]
fn or_operator_missing_key_returns_false() {
    // .a or .c on {b: "hi"} → false (both .a and .c are missing)
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("b", string_scalar("hi"))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a or .c").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
    assert_eq!(out.matching_nodes[0].tag, SemType::Boolean.to_string());
}

#[test]
fn or_operator_false_key_returns_false() {
    // .b or .c on {b: false} → false (.b is false, .c is missing)
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("b", bool_scalar(false))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".b or .c").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
}

#[test]
fn or_operator_multi_rhs_on_map_values() {
    // .[] or (false, true) on {a: true, b: false}
    // For each value: if truthy keep it, otherwise try each RHS
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("a", bool_scalar(true)),
            ("b", bool_scalar(false)),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, ".[] or (false, true)").unwrap();

    // Expected: a=true (truthy, kept), b=false (falsy, replaced by false), b=true (falsy, replaced by true)
    assert_eq!(out.matching_nodes.len(), 3);
    // First: a → true (kept as-is)
    assert_eq!(out.matching_nodes[0].value, "true");
    // Second: b → false (first RHS)
    assert_eq!(out.matching_nodes[1].value, "false");
    // Third: b → true (second RHS)
    assert_eq!(out.matching_nodes[2].value, "true");
}

#[test]
fn and_operator_multi_rhs_on_map_values() {
    // .[] and (false, true) on {a: true, b: false}
    // For each value: if falsy keep it, otherwise try each RHS
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("a", bool_scalar(true)),
            ("b", bool_scalar(false)),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[] and (false, true)")
        .unwrap();

    // Expected: a=false (truthy, replaced by false), a=true (truthy, replaced by true), b=false (falsy, kept)
    assert_eq!(out.matching_nodes.len(), 3);
    // First: a → false (first RHS)
    assert_eq!(out.matching_nodes[0].value, "false");
    // Second: a → true (second RHS)
    assert_eq!(out.matching_nodes[1].value, "true");
    // Third: b → false (kept as-is)
    assert_eq!(out.matching_nodes[2].value, "false");
}

// ── Select-based scenarios ────────────────────────────────────────

#[test]
fn select_with_or_keeps_node_when_condition_true() {
    // select(.a or .b) on {b: "hi"} → keeps node because .b is truthy
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("b", string_scalar("hi"))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "select(.a or .b)").unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn select_with_and_not_keeps_node_when_condition_true() {
    // select((.a and .b) | not) on {b: "hi"}
    // .a and .b → false (because .a is missing), not → true, so keep
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("b", string_scalar("hi"))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "select((.a and .b) | not)")
            .unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn select_with_or_in_array_of_objects() {
    // [.[] | select(.a == "cat" or .b == "dog")]
    // Document: [{a: bird, b: dog}, {a: frog, b: bird}, {a: cat, b: fly}]
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("a", string_scalar("bird")),
                ("b", string_scalar("dog")),
            ]),
            mapping(vec![
                ("a", string_scalar("frog")),
                ("b", string_scalar("bird")),
            ]),
            mapping(vec![
                ("a", string_scalar("cat")),
                ("b", string_scalar("fly")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "[.[] | select(.a == \"cat\" or .b == \"dog\")]",
    )
    .unwrap();

    // Should keep first (b=dog) and third (a=cat)
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    // First kept: {a: bird, b: dog}
    assert_eq!(out.matching_nodes[0].content[0].content[1].value, "bird");
    assert_eq!(out.matching_nodes[0].content[0].content[3].value, "dog");
    // Second kept: {a: cat, b: fly}
    assert_eq!(out.matching_nodes[0].content[1].content[1].value, "cat");
    assert_eq!(out.matching_nodes[0].content[1].content[3].value, "fly");
}

// ── any_c / all_c condition scenarios ─────────────────────────────

#[test]
fn any_c_returns_true_if_any_element_matches_condition() {
    // .[] |= any_c(. == "awesome")
    // Document: {a: [rad, awesome], b: [meh, whatever]}
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            (
                "a",
                sequence(vec![string_scalar("rad"), string_scalar("awesome")]),
            ),
            (
                "b",
                sequence(vec![string_scalar("meh"), string_scalar("whatever")]),
            ),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".[] |= any_c(. == \"awesome\")",
    )
    .unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    // a → true (contains "awesome"), b → false (no "awesome")
    assert_eq!(out.matching_nodes[0].content[1].value, "true");
    assert_eq!(out.matching_nodes[0].content[3].value, "false");
}

#[test]
fn all_c_returns_true_if_all_elements_match_condition() {
    // .[] |= all_c(tag == "!!str")
    // Document: {a: [rad, awesome], b: [meh, 12]}
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            (
                "a",
                sequence(vec![string_scalar("rad"), string_scalar("awesome")]),
            ),
            (
                "b",
                sequence(vec![
                    string_scalar("meh"),
                    TreeNode {
                        kind: NodeKind::Scalar,
                        sem_type: Some(SemType::Float),
                        tag: SemType::Float.tag().to_owned(),
                        value: "12".to_string(),
                        ..TreeNode::default()
                    },
                ]),
            ),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".[] |= all_c(tag == \"!!str\")",
    )
    .unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    // a → true (all strings), b → false (contains a number)
    assert_eq!(out.matching_nodes[0].content[1].value, "true");
    assert_eq!(out.matching_nodes[0].content[3].value, "false");
}

#[test]
fn any_c_with_as_variable_pipe_self() {
    // any_c(.name == "harry") as $c | .
    // Document: [{pet: cat}]
    let ctx = Context {
        matching_nodes: vec![sequence(vec![mapping(vec![("pet", string_scalar("cat"))])])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "any_c(.name == \"harry\") as $c | .",
    )
    .unwrap();

    // The original document is preserved
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
}

#[test]
fn any_c_with_as_variable_pipe_variable() {
    // any_c(.name == "harry") as $c | $c → false
    // Document: [{pet: cat}]
    let ctx = Context {
        matching_nodes: vec![sequence(vec![mapping(vec![("pet", string_scalar("cat"))])])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "any_c(.name == \"harry\") as $c | $c",
    )
    .unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
}

#[test]
fn all_c_with_as_variable_pipe_variable() {
    // all_c(.name == "harry") as $c | $c → false
    // Document: [{pet: cat}]
    let ctx = Context {
        matching_nodes: vec![sequence(vec![mapping(vec![("pet", string_scalar("cat"))])])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "all_c(.name == \"harry\") as $c | $c",
    )
    .unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "false");
}

// ── Variable binding with or/and ──────────────────────────────────

#[test]
fn or_with_as_variable_pipe_self() {
    // (.a.b or .c) as $x | .
    // Document: {} — both .a.b and .c are missing, result is false, but . preserves input
    let ctx = Context {
        matching_nodes: vec![mapping(vec![])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "(.a.b or .c) as $x | .")
        .unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn and_with_as_variable_pipe_self() {
    // (.a.b and .c) as $x | .
    // Document: {} — both .a.b and .c are missing, result is false, but . preserves input
    let ctx = Context {
        matching_nodes: vec![mapping(vec![])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "(.a.b and .c) as $x | .")
        .unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

// ── YAML 1.2 boolean semantics ────────────────────────────────────

#[test]
fn yes_and_no_are_strings_in_yaml_1_2() {
    // In the yaml 1.2 standard, support for yes/no as booleans was dropped -
    // they are now considered strings.
    // .[] | tag on [yes, no] → both should be !!str
    let ctx = Context {
        matching_nodes: vec![sequence(vec![string_scalar("yes"), string_scalar("no")])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[] | tag").unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "!!str");
    assert_eq!(out.matching_nodes[1].value, "!!str");
}
