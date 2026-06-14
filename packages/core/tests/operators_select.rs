use treease_core::expression_pipeline;
use treease_core::operators::select::select_operator;
use treease_core::operators::{
    Context, EQUALS_OP_TYPE, ExpressionNode, NodeKind, OR_OP_TYPE, Operation, SELECT_OP_TYPE,
    SELF_REFERENCE_OP_TYPE, SemType, TEST_OP_TYPE, TraversePreferences, TreeEngine, TreeNode,
    UNION_OP_TYPE, create_traversal_tree, create_value_operation,
};

fn select_expression(rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &SELECT_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(rhs)),
    }
}

fn self_expression() -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &SELF_REFERENCE_OP_TYPE,
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

fn value_expression(node: TreeNode) -> ExpressionNode {
    ExpressionNode {
        operation: create_value_operation(Box::new(node)).unwrap(),
        lhs: None,
        rhs: None,
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

fn bool_scalar(value: bool) -> TreeNode {
    scalar(SemType::Boolean, if value { "true" } else { "false" })
}

fn string_scalar(value: &str) -> TreeNode {
    scalar(SemType::Str, value)
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
fn select_operator_keeps_node_for_truthy_rhs_and_drops_falsy_rhs() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut truthy = select_expression(value_expression(bool_scalar(true)));
    let mut falsy = select_expression(value_expression(bool_scalar(false)));

    let kept = select_operator(ctx.clone(), &mut engine, &mut truthy).unwrap();
    let dropped = select_operator(ctx, &mut engine, &mut falsy).unwrap();

    assert_eq!(kept.matching_nodes.len(), 1);
    assert_eq!(kept.matching_nodes[0].value, "cat");
    assert!(dropped.matching_nodes.is_empty());
}

#[test]
fn select_operator_filters_nodes_by_truthy_nested_field() {
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![
                ("include", bool_scalar(true)),
                ("name", string_scalar("frog")),
            ]),
            mapping(vec![
                ("include", bool_scalar(false)),
                ("name", string_scalar("toad")),
            ]),
            mapping(vec![
                ("include", string_scalar("fold")),
                ("name", string_scalar("newt")),
            ]),
        ],
        ..Context::default()
    };
    let rhs = create_traversal_tree(
        &[string_scalar("include")],
        TraversePreferences::default(),
        false,
    )
    .unwrap();
    let mut expr = select_expression(*rhs);
    let mut engine = TreeEngine::default();

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].content[3].value, "frog");
    assert_eq!(out.matching_nodes[1].content[3].value, "newt");
}

#[test]
fn select_operator_can_keep_current_node_via_self_rhs() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut expr = select_expression(self_expression());
    let mut engine = TreeEngine::default();

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cat");
}

// ── Additional helpers ────────────────────────────────────────────

fn null_scalar() -> TreeNode {
    scalar(SemType::Nil, "null")
}

fn equals_expression(lhs: ExpressionNode, rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &EQUALS_OP_TYPE,
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

fn test_expression(pattern: &str) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &TEST_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(value_expression(string_scalar(pattern)))),
    }
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

fn or_expression(lhs: ExpressionNode, rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &OR_OP_TYPE,
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

// ── Wildcard matching tests ───────────────────────────────────────

#[test]
fn select_with_wildcard_suffix_matching() {
    let ctx = Context {
        matching_nodes: vec![
            string_scalar("cat"),
            string_scalar("bat"),
            string_scalar("dog"),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // select(. == "*at") — keeps nodes ending with "at"
    let eq = equals_expression(self_expression(), value_expression(string_scalar("*at")));
    let mut expr = select_expression(eq);

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "cat");
    assert_eq!(out.matching_nodes[1].value, "bat");
}

#[test]
fn select_with_wildcard_prefix_matching() {
    let ctx = Context {
        matching_nodes: vec![
            string_scalar("goat"),
            string_scalar("gopher"),
            string_scalar("dog"),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // select(. == "go*") — keeps nodes starting with "go"
    let eq = equals_expression(self_expression(), value_expression(string_scalar("go*")));
    let mut expr = select_expression(eq);

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "goat");
    assert_eq!(out.matching_nodes[1].value, "gopher");
}

#[test]
fn select_with_wildcard_infix_matching() {
    let ctx = Context {
        matching_nodes: vec![
            string_scalar("dragon"),
            string_scalar("ago"),
            string_scalar("cat"),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // select(. == "*go*") — keeps nodes containing "go"
    let eq = equals_expression(self_expression(), value_expression(string_scalar("*go*")));
    let mut expr = select_expression(eq);

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "dragon");
    assert_eq!(out.matching_nodes[1].value, "ago");
}

#[test]
fn select_with_wildcard_returns_empty_when_no_matches() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat"), string_scalar("dog")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // select(. == "*xyz*") — no node contains "xyz"
    let eq = equals_expression(self_expression(), value_expression(string_scalar("*xyz*")));
    let mut expr = select_expression(eq);

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert!(out.matching_nodes.is_empty());
}

// ── Regex test ────────────────────────────────────────────────────

#[test]
fn select_with_regex_test() {
    let ctx = Context {
        matching_nodes: vec![
            string_scalar("hello_1"),
            string_scalar("world_2"),
            string_scalar("no_match"),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // select(test("[a-zA-Z]+_[0-9]$"))
    let test_expr = test_expression("[a-zA-Z]+_[0-9]$");
    let mut expr = select_expression(test_expr);

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "hello_1");
    assert_eq!(out.matching_nodes[1].value, "world_2");
}

// ── Multiple boolean args ─────────────────────────────────────────

#[test]
fn select_with_multiple_boolean_args_false_true() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // select(false, true) — at least one arg is truthy, so keep
    let block = union_expression(
        value_expression(bool_scalar(false)),
        value_expression(bool_scalar(true)),
    );
    let mut expr = select_expression(block);

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cat");
}

#[test]
fn select_with_multiple_boolean_args_true_false() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // select(true, false) — at least one arg is truthy, so keep
    let block = union_expression(
        value_expression(bool_scalar(true)),
        value_expression(bool_scalar(false)),
    );
    let mut expr = select_expression(block);

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cat");
}

#[test]
fn select_with_single_false_arg_drops_node() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // select(false) — only arg is falsy, so drop
    let mut expr = select_expression(value_expression(bool_scalar(false)));

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert!(out.matching_nodes.is_empty());
}

// ── OR conditions ─────────────────────────────────────────────────

#[test]
fn select_with_or_conditions() {
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![
                ("a", string_scalar("hello")),
                ("b", string_scalar("x")),
            ]),
            mapping(vec![
                ("a", string_scalar("x")),
                ("b", string_scalar("world")),
            ]),
            mapping(vec![("a", string_scalar("x")), ("b", string_scalar("y"))]),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // select(.a == "hello" or .b == "world")
    let eq_a = equals_expression(
        *create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
            .unwrap(),
        value_expression(string_scalar("hello")),
    );
    let eq_b = equals_expression(
        *create_traversal_tree(&[string_scalar("b")], TraversePreferences::default(), false)
            .unwrap(),
        value_expression(string_scalar("world")),
    );
    let or_expr = or_expression(eq_a, eq_b);
    let mut expr = select_expression(or_expr);

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].content[1].value, "hello");
    assert_eq!(out.matching_nodes[1].content[3].value, "world");
}

// ── Null values ───────────────────────────────────────────────────

#[test]
fn select_on_null_values() {
    let ctx = Context {
        matching_nodes: vec![null_scalar(), string_scalar("cat"), null_scalar()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // select(. == null) — keeps null nodes
    let eq = equals_expression(self_expression(), value_expression(null_scalar()));
    let mut expr = select_expression(eq);

    let out = select_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].tag, SemType::Nil.to_string());
    assert_eq!(out.matching_nodes[1].tag, SemType::Nil.to_string());
}

// ── Full expression pipeline tests ────────────────────────────────
//
// These scenarios involve select combined with other operators
// (pipe, splat, update-assign, traversal) and are tested via
// expression_pipeline::execute_on_context which dispatches through
// the full operator registry.

#[test]
fn select_no_match_piped_into_expression() {
    // select(.nope) | key + " why though?"
    // Document: cat: pants
    // When select finds nothing, the piped expression produces nothing.
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("cat", string_scalar("pants"))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "select(.nope) | key + \" why though?\"",
    )
    .expect("evaluation should succeed");

    assert!(out.matching_nodes.is_empty());
}

#[test]
fn select_splat_with_or_condition() {
    // select(.a == "hello" or .b == "world")[]
    // Two documents: {a: "hello"} and {b: "world"}
    // Select keeps both, then [] splats the mappings into values.
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![("a", string_scalar("hello"))]),
            mapping(vec![("b", string_scalar("world"))]),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "select(.a == \"hello\" or .b == \"world\")[]",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "hello");
    assert_eq!(out.matching_nodes[1].value, "world");
}

#[test]
fn select_does_not_update_the_map() {
    // (.[] | select(.legs.cool == true).canWalk) = true | (.[] | .alive.things) = "yes"
    // Document: [{animal: cat, legs: {cool: true}}, {animal: fish}]
    // Verifies select doesn't mutate the original structure during
    // update-assign chain: only the first element gets canWalk,
    // but both elements get alive.things.
    let input = sequence(vec![
        mapping(vec![
            ("animal", string_scalar("cat")),
            ("legs", mapping(vec![("cool", bool_scalar(true))])),
        ]),
        mapping(vec![("animal", string_scalar("fish"))]),
    ]);

    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "(.[] | select(.legs.cool == true).canWalk) = true | (.[] | .alive.things) = \"yes\"",
    )
    .expect("evaluation should succeed");

    // Should have one result: the modified array
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);

    // First element should have canWalk = true
    let first = &out.matching_nodes[0].content[0];
    assert_eq!(first.kind, NodeKind::Mapping);
    let has_can_walk = first
        .content
        .chunks(2)
        .any(|chunk| chunk.len() == 2 && chunk[0].value == "canWalk" && chunk[1].value == "true");
    assert!(has_can_walk, "first element should have canWalk = true");

    // Second element should NOT have canWalk
    let second = &out.matching_nodes[0].content[1];
    assert_eq!(second.kind, NodeKind::Mapping);
    let second_has_can_walk = second
        .content
        .chunks(2)
        .any(|chunk| chunk.len() == 2 && chunk[0].value == "canWalk");
    assert!(
        !second_has_can_walk,
        "second element should not have canWalk"
    );
}

#[test]
fn nested_wildcard_select() {
    // .a.[] | select(. == "*at")
    // Document: {a: [cat, goat, dog]}
    // Traverses to .a, iterates array, keeps elements matching "*at".
    let input = mapping(vec![(
        "a",
        sequence(vec![
            string_scalar("cat"),
            string_scalar("goat"),
            string_scalar("dog"),
        ]),
    )]);

    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.[] | select(. == \"*at\")")
            .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "cat");
    assert_eq!(out.matching_nodes[1].value, "goat");
}

#[test]
fn select_items_from_map_with_compound_condition() {
    // .[] | select(. == "cat" or test("og$"))
    // Document: {things: cat, bob: goat, horse: dog}
    // Keeps "cat" (exact match) and "dog" (regex "og$").
    let input = mapping(vec![
        ("things", string_scalar("cat")),
        ("bob", string_scalar("goat")),
        ("horse", string_scalar("dog")),
    ]);

    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".[] | select(. == \"cat\" or test(\"og$\"))",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "cat");
    assert_eq!(out.matching_nodes[1].value, "dog");
}

#[test]
fn select_multiple_items_in_map_and_update() {
    // (.a.[] | select(. == "cat" or . == "goat")) |= "rabbit"
    // Document: {a: {things: cat, bob: goat, horse: dog}}
    // Update-assign: replaces "cat" and "goat" with "rabbit".
    let inner = mapping(vec![
        ("things", string_scalar("cat")),
        ("bob", string_scalar("goat")),
        ("horse", string_scalar("dog")),
    ]);
    let input = mapping(vec![("a", inner)]);

    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "(.a.[] | select(. == \"cat\" or . == \"goat\")) |= \"rabbit\"",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);

    // The result is the root mapping with key "a"
    let a_val = &out.matching_nodes[0].content[1];
    assert_eq!(a_val.kind, NodeKind::Mapping);

    // "things" -> "rabbit", "bob" -> "rabbit", "horse" -> "dog"
    let things_val = &a_val.content[1];
    assert_eq!(things_val.value, "rabbit");
    let bob_val = &a_val.content[3];
    assert_eq!(bob_val.value, "rabbit");
    let horse_val = &a_val.content[5];
    assert_eq!(horse_val.value, "dog");
}
