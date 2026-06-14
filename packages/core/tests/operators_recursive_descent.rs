use treease_core::expression_pipeline;
use treease_core::operators::recursive_descent::recursive_descent_operator;
use treease_core::operators::{
    Context, ExpressionNode, NodeKind, Operation, OperationPreference, RECURSIVE_DESCENT_OP_TYPE,
    RecursiveDescentPreferences, SemType, TraversePreferences, TreeEngine, TreeNode,
};

fn expression() -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &RECURSIVE_DESCENT_OP_TYPE,
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

fn expression_with_preferences(preferences: RecursiveDescentPreferences) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &RECURSIVE_DESCENT_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::RecursiveDescent(preferences))),
            update_assign: false,
        }),
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

fn mapping(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        content.push(scalar(SemType::Str, key));
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
fn recursive_descent_operator_walks_map_values_only() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "a",
            mapping(vec![("b", scalar(SemType::Str, "apple"))]),
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 3);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[2].value, "apple");
}

#[test]
fn recursive_descent_operator_walks_sequence_elements() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            scalar(SemType::Int, "1"),
            scalar(SemType::Int, "2"),
            scalar(SemType::Int, "3"),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 4);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[1].value, "1");
    assert_eq!(out.matching_nodes[2].value, "2");
    assert_eq!(out.matching_nodes[3].value, "3");
}

// ── Additional helpers ────────────────────────────────────────────

fn bool_scalar(value: bool) -> TreeNode {
    scalar(SemType::Boolean, if value { "true" } else { "false" })
}

fn string_scalar(value: &str) -> TreeNode {
    scalar(SemType::Str, value)
}

// ── Recursive descent on empty collections ────────────────────────

#[test]
fn recursive_descent_on_empty_map() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    // Empty map: returns just the map itself (no children to recurse into)
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn recursive_descent_on_empty_array() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    // Empty array: returns just the array itself
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
}

// ── Recursive descent on scalar ───────────────────────────────────

#[test]
fn recursive_descent_on_scalar_returns_scalar_itself() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("hello")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    // Scalar: returns the scalar itself (no children)
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "hello");
}

// ── Recursive descent on nested map ───────────────────────────────

#[test]
fn recursive_descent_on_nested_map() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "a",
            mapping(vec![("b", string_scalar("apple"))]),
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    // Should return: outer map, inner map, "apple"
    assert_eq!(out.matching_nodes.len(), 3);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping); // {a: {b: apple}}
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Mapping); // {b: apple}
    assert_eq!(out.matching_nodes[2].value, "apple");
}

// ── Recursive descent on mixed-type array ─────────────────────────

#[test]
fn recursive_descent_on_mixed_type_array() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", string_scalar("cat"))]),
            scalar(SemType::Int, "2"),
            bool_scalar(true),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    // Should return: the array itself, the map {a: cat}, "cat", 2, true
    assert_eq!(out.matching_nodes.len(), 5);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence); // the array
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Mapping); // {a: cat}
    assert_eq!(out.matching_nodes[2].value, "cat"); // value of a
    assert_eq!(out.matching_nodes[3].value, "2"); // int scalar
    assert_eq!(out.matching_nodes[4].value, "true"); // bool scalar
}

// ── Recursive descent with keys (...) on empty/small values ───────

#[test]
fn recursive_descent_keys_on_empty_map() {
    // ... on {} — keys variant on empty map returns just the map itself
    let ctx = Context {
        matching_nodes: vec![mapping(vec![])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn recursive_descent_keys_on_empty_array() {
    // ... on [] — keys variant on empty array returns just the array itself
    let ctx = Context {
        matching_nodes: vec![sequence(vec![])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
}

#[test]
fn recursive_descent_keys_on_scalar_returns_scalar_itself() {
    // ... on "cat" — keys variant on a scalar returns just the scalar (no keys to emit)
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cat");
}

#[test]
fn recursive_descent_can_exclude_map_keys_like_double_dot() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "a",
            mapping(vec![("b", string_scalar("apple"))]),
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_preferences(RecursiveDescentPreferences {
        traverse_preferences: TraversePreferences {
            include_map_keys: false,
            ..TraversePreferences::default()
        },
        recurse_array: true,
    });

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 3);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[2].value, "apple");
}

#[test]
fn recursive_descent_can_include_map_keys_like_triple_dot() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "a",
            mapping(vec![("b", string_scalar("apple"))]),
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_preferences(RecursiveDescentPreferences {
        traverse_preferences: TraversePreferences {
            include_map_keys: true,
            ..TraversePreferences::default()
        },
        recurse_array: true,
    });

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 5);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[1].value, "a");
    assert_eq!(out.matching_nodes[2].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[3].value, "b");
    assert_eq!(out.matching_nodes[4].value, "apple");
}

// ── Recursive descent with keys (...) on maps ─────────────────────

#[test]
fn recursive_descent_keys_on_single_entry_map() {
    // ... on {a: frog} — emits the map, the key "a", and the value "frog"
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", string_scalar("frog"))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_preferences(RecursiveDescentPreferences {
        traverse_preferences: TraversePreferences {
            include_map_keys: true,
            ..TraversePreferences::default()
        },
        recurse_array: true,
    });

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    // Expected: map, key "a", value "frog"
    assert_eq!(out.matching_nodes.len(), 3);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping); // {a: frog}
    assert_eq!(out.matching_nodes[1].value, "a"); // key
    assert_eq!(out.matching_nodes[2].value, "frog"); // value
}

#[test]
fn recursive_descent_keys_on_nested_map() {
    // ... on {a: {b: apple}} — emits all keys and values at all levels
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "a",
            mapping(vec![("b", string_scalar("apple"))]),
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_preferences(RecursiveDescentPreferences {
        traverse_preferences: TraversePreferences {
            include_map_keys: true,
            ..TraversePreferences::default()
        },
        recurse_array: true,
    });

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    // Expected: outer map, key "a", inner map, key "b", value "apple"
    assert_eq!(out.matching_nodes.len(), 5);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping); // {a: {b: apple}}
    assert_eq!(out.matching_nodes[1].value, "a"); // key
    assert_eq!(out.matching_nodes[2].kind, NodeKind::Mapping); // {b: apple}
    assert_eq!(out.matching_nodes[3].value, "b"); // key
    assert_eq!(out.matching_nodes[4].value, "apple"); // value
}

// ── Recursive descent with keys (...) on arrays ───────────────────

#[test]
fn recursive_descent_keys_on_array() {
    // ... on [1,2,3] — same as .. since arrays have no keys
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            scalar(SemType::Int, "1"),
            scalar(SemType::Int, "2"),
            scalar(SemType::Int, "3"),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_preferences(RecursiveDescentPreferences {
        traverse_preferences: TraversePreferences {
            include_map_keys: true,
            ..TraversePreferences::default()
        },
        recurse_array: true,
    });

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 4);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence); // [1, 2, 3]
    assert_eq!(out.matching_nodes[1].value, "1");
    assert_eq!(out.matching_nodes[2].value, "2");
    assert_eq!(out.matching_nodes[3].value, "3");
}

#[test]
fn recursive_descent_keys_on_mixed_array() {
    // ... on [{a: cat}, 2, true] — emits keys ("a") in addition to values
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", string_scalar("cat"))]),
            scalar(SemType::Int, "2"),
            bool_scalar(true),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_preferences(RecursiveDescentPreferences {
        traverse_preferences: TraversePreferences {
            include_map_keys: true,
            ..TraversePreferences::default()
        },
        recurse_array: true,
    });

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    // Expected: array, map {a: cat}, key "a", "cat", 2, true
    assert_eq!(out.matching_nodes.len(), 6);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence); // the array
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Mapping); // {a: cat}
    assert_eq!(out.matching_nodes[2].value, "a"); // key
    assert_eq!(out.matching_nodes[3].value, "cat"); // value
    assert_eq!(out.matching_nodes[4].value, "2"); // int scalar
    assert_eq!(out.matching_nodes[5].value, "true"); // bool scalar
}

// ── Recursive descent with aliases ────────────────────────────────

fn alias_node(anchor_name: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Alias,
        value: format!("*{}", anchor_name),
        anchor: anchor_name.to_owned(),
        ..TreeNode::default()
    }
}

fn anchored_mapping(anchor_name: &str, entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut node = mapping(entries);
    node.anchor = anchor_name.to_owned();
    node
}

#[test]
fn recursive_descent_does_not_traverse_aliases() {
    // [..] on {a: &cat {c: frog}, b: *cat}
    // .. does not follow YAML aliases; the alias target is emitted once,
    // the alias reference is emitted as-is.
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            (
                "a",
                anchored_mapping("cat", vec![("c", string_scalar("frog"))]),
            ),
            ("b", alias_node("cat")),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    // Expected: outer map, anchor target &cat {c: frog}, frog, alias *cat
    assert_eq!(out.matching_nodes.len(), 4);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping); // {a: &cat {c: frog}, b: *cat}
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Mapping); // &cat {c: frog}
    assert_eq!(out.matching_nodes[1].anchor, "cat");
    assert_eq!(out.matching_nodes[2].value, "frog"); // value of c
    assert_eq!(out.matching_nodes[3].kind, NodeKind::Alias); // *cat
    assert_eq!(out.matching_nodes[3].value, "*cat");
}

#[test]
fn recursive_descent_keys_on_aliased_document() {
    // ... on {a: &cat {c: frog}, b: *cat}
    // ... emits keys alongside values, aliases still not followed
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            (
                "a",
                anchored_mapping("cat", vec![("c", string_scalar("frog"))]),
            ),
            ("b", alias_node("cat")),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_preferences(RecursiveDescentPreferences {
        traverse_preferences: TraversePreferences {
            include_map_keys: true,
            ..TraversePreferences::default()
        },
        recurse_array: true,
    });

    let out = recursive_descent_operator(ctx, &mut engine, &mut expr).unwrap();

    // Expected: map, key "a", anchor map, key "c", frog, key "b", alias *cat
    assert_eq!(out.matching_nodes.len(), 7);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping); // {a: &cat {c: frog}, b: *cat}
    assert_eq!(out.matching_nodes[1].value, "a"); // key
    assert_eq!(out.matching_nodes[2].kind, NodeKind::Mapping); // &cat {c: frog}
    assert_eq!(out.matching_nodes[2].anchor, "cat");
    assert_eq!(out.matching_nodes[3].value, "c"); // key
    assert_eq!(out.matching_nodes[4].value, "frog"); // value
    assert_eq!(out.matching_nodes[5].value, "b"); // key
    assert_eq!(out.matching_nodes[6].kind, NodeKind::Alias); // *cat
    assert_eq!(out.matching_nodes[6].value, "*cat");
}

// ── Recursive descent combined with select via pipeline ───────────

#[test]
fn recursive_descent_find_nodes_with_keys_using_select_has() {
    // [.. | select(has("name"))]
    // Using .. with select(has("name")) to find all nodes that have a "name" key.
    // Document: {a: {name: frog, b: {name: blog, age: 12}}}
    let input = mapping(vec![(
        "a",
        mapping(vec![
            ("name", string_scalar("frog")),
            (
                "b",
                mapping(vec![
                    ("name", string_scalar("blog")),
                    ("age", scalar(SemType::Int, "12")),
                ]),
            ),
        ]),
    )]);

    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "[.. | select(has(\"name\"))]")
            .expect("evaluation should succeed");

    // Expected: one collected sequence containing the two matching maps.
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].kind, NodeKind::Mapping); // {name: frog, b: {name: blog, age: 12}}
    assert_eq!(out.matching_nodes[0].content[1].kind, NodeKind::Mapping); // {name: blog, age: 12}
}

#[test]
fn recursive_descent_find_nodes_with_values_using_select() {
    // .. | select(. == "frog")
    // Using .. with select to find all nodes with value "frog".
    // Document: {a: {nameA: frog, b: {nameB: frog, age: 12}}}
    let input = mapping(vec![(
        "a",
        mapping(vec![
            ("nameA", string_scalar("frog")),
            (
                "b",
                mapping(vec![
                    ("nameB", string_scalar("frog")),
                    ("age", scalar(SemType::Int, "12")),
                ]),
            ),
        ]),
    )]);

    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, ".. | select(. == \"frog\")")
            .expect("evaluation should succeed");

    // Expected: two "frog" nodes
    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "frog");
    assert_eq!(out.matching_nodes[1].value, "frog");
}
