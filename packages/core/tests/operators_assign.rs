use treease_core::expression_pipeline;
use treease_core::operators::assign::assign_update_operator;
use treease_core::operators::{
    ASSIGN_OP_TYPE, AssignPreferences, Context, ExpressionNode, NodeKind, Operation,
    OperationPreference, SemType, TraversePreferences, TreeEngine, TreeNode, create_traversal_tree,
    create_value_operation,
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

fn bool_scalar(value: bool) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Boolean),
        tag: SemType::Boolean.tag().to_owned(),
        value: if value {
            "true".to_owned()
        } else {
            "false".to_owned()
        },
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

fn sequence(items: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: items,
        ..TreeNode::default()
    }
}

fn map_value<'a>(map: &'a TreeNode, key: &str) -> &'a TreeNode {
    let mut idx = 0;
    while idx + 1 < map.content.len() {
        if map.content[idx].value == key {
            return &map.content[idx + 1];
        }
        idx += 2;
    }
    panic!("missing key {key}");
}

/// Build an assign expression: `path = value`.
fn assign_expression(
    path_segments: Vec<TreeNode>,
    value_node: TreeNode,
    update_assign: bool,
) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign,
        }),
        lhs: Some(
            create_traversal_tree(&path_segments, TraversePreferences::default(), false).unwrap(),
        ),
        rhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(value_node)).unwrap(),
            lhs: None,
            rhs: None,
        })),
    }
}

/// Build an assign expression where RHS is a traversal (for self-reference: .a = [.a]).
fn assign_with_traversal_rhs(
    path_segments: Vec<TreeNode>,
    rhs_traversal_segments: Vec<TreeNode>,
    update_assign: bool,
) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign,
        }),
        lhs: Some(
            create_traversal_tree(&path_segments, TraversePreferences::default(), false).unwrap(),
        ),
        rhs: Some(
            create_traversal_tree(
                &rhs_traversal_segments,
                TraversePreferences::default(),
                false,
            )
            .unwrap(),
        ),
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[test]
fn assign_new_key_value_to_empty_map() {
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(empty_map());

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    let mut expr = assign_expression(vec![string_scalar("name")], string_scalar("Alice"), false);

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "assign should succeed on empty map");

    // After assign, the map in the store should have the auto-created key.
    let root = engine.store.get(root_id);
    assert_eq!(root.content.len(), 2, "map should have one key-value pair");
    assert_eq!(root.content[0].value, "name", "key should be 'name'");
    assert_eq!(root.content[1].value, "Alice");
    assert_eq!(root.content[1].sem_type, Some(SemType::Str));
    assert_eq!(root.content[1].tag, SemType::Str.tag());
}

#[test]
fn assign_overwrites_existing_value() {
    let mut engine = TreeEngine::default();
    let root_id = engine
        .store
        .add(mapping(vec![("color", string_scalar("red"))]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    let mut expr = assign_expression(vec![string_scalar("color")], string_scalar("blue"), false);

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "assign should succeed on existing key");

    // The map should still have the key; the value may or may not be updated
    let root = engine.store.get(root_id);
    assert_eq!(root.content.len(), 2);
    assert_eq!(root.content[0].value, "color");
    assert_eq!(root.content[1].value, "blue");
    assert_eq!(root.content[1].sem_type, Some(SemType::Str));
    assert_eq!(root.content[1].tag, SemType::Str.tag());
}

#[test]
fn assign_with_update_assign_flag() {
    let mut engine = TreeEngine::default();
    let root_id = engine
        .store
        .add(mapping(vec![("x", int_scalar(10)), ("y", int_scalar(20))]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    // update_assign = true: the operator iterates LHS matches in reverse
    // and evaluates RHS in each candidate's context.
    let mut expr = assign_expression(
        vec![string_scalar("x")],
        int_scalar(99),
        true, // update_assign
    );

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "update_assign should succeed");
}

#[test]
fn self_reference_assignment() {
    // .a = [.a]  -- self-reference: assign key "a" to a traversal of itself.
    let mut engine = TreeEngine::default();
    let root_id = engine
        .store
        .add(mapping(vec![("a", string_scalar("original"))]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    // RHS is a traversal to "a" (self-reference)
    let mut expr =
        assign_with_traversal_rhs(vec![string_scalar("a")], vec![string_scalar("a")], false);

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "self-reference assign should succeed");
}

#[test]
fn assign_changes_type_string_to_int() {
    let mut engine = TreeEngine::default();
    let root_id = engine
        .store
        .add(mapping(vec![("val", string_scalar("hello"))]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    let mut expr = assign_expression(vec![string_scalar("val")], int_scalar(42), false);

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(
        result.is_ok(),
        "assign changing string to int should succeed"
    );

    let root = engine.store.get(root_id);
    let value = map_value(root, "val");
    assert_eq!(value.value, "42");
    assert_eq!(value.sem_type, Some(SemType::Int));
    assert_eq!(value.tag, SemType::Int.tag());
}

#[test]
fn assign_changes_type_string_to_bool() {
    let mut engine = TreeEngine::default();
    let root_id = engine
        .store
        .add(mapping(vec![("flag", string_scalar("yes"))]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    let mut expr = assign_expression(vec![string_scalar("flag")], bool_scalar(true), false);

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(
        result.is_ok(),
        "assign changing string to bool should succeed"
    );

    let root = engine.store.get(root_id);
    let value = map_value(root, "flag");
    assert_eq!(value.value, "true");
    assert_eq!(value.sem_type, Some(SemType::Boolean));
    assert_eq!(value.tag, SemType::Boolean.tag());
}

#[test]
fn assign_preserves_custom_tags() {
    let mut engine = TreeEngine::default();

    // Create a map with a custom-tagged value
    let mut tagged_value = string_scalar("data");
    tagged_value.tag = "!custom".to_owned();
    let root_id = engine.store.add(mapping(vec![("item", tagged_value)]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    // Assign with clobber_custom_tags = false should preserve the custom tag
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Assign(AssignPreferences {
                clobber_custom_tags: false,
                ..AssignPreferences::default()
            }))),
            update_assign: false,
        }),
        lhs: Some(
            create_traversal_tree(
                &[string_scalar("item")],
                TraversePreferences::default(),
                false,
            )
            .unwrap(),
        ),
        rhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(string_scalar("new_data"))).unwrap(),
            lhs: None,
            rhs: None,
        })),
    };

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(
        result.is_ok(),
        "assign with custom tag preservation should succeed"
    );

    let root = engine.store.get(root_id);
    let value = map_value(root, "item");
    assert_eq!(value.value, "new_data");
    assert_eq!(value.tag, "!custom");
}

#[test]
fn assign_with_update_assign_operator() {
    // The |= operator (update_assign = true) evaluates RHS in the context
    // of each LHS candidate.
    let mut engine = TreeEngine::default();
    let root_id = engine
        .store
        .add(mapping(vec![("a", int_scalar(1)), ("b", int_scalar(2))]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    // update_assign = true with a traversal RHS
    let mut expr =
        assign_with_traversal_rhs(vec![string_scalar("a")], vec![string_scalar("b")], true);

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "update_assign with |= should succeed");
}

#[test]
fn assign_multiple_path_assignment() {
    // Assign to a nested path like .x.y = "value"
    let mut engine = TreeEngine::default();

    let inner = mapping(vec![("y", string_scalar("old"))]);
    let root_id = engine.store.add(mapping(vec![("x", inner)]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    let mut expr = assign_expression(
        vec![string_scalar("x"), string_scalar("y")],
        string_scalar("new"),
        false,
    );

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "multi-path assign should succeed");
}

#[test]
fn assign_to_array_index() {
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(sequence(vec![
        string_scalar("first"),
        string_scalar("second"),
    ]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    let mut expr = assign_expression(vec![int_scalar(0)], string_scalar("updated_first"), false);

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "assign to array index should succeed");
}

#[test]
fn assign_with_only_write_null_preference() {
    let mut engine = TreeEngine::default();
    let root_id = engine
        .store
        .add(mapping(vec![("a", string_scalar("exists"))]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    // only_write_null = true: only assign if the target is null/nil
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Assign(AssignPreferences {
                only_write_null: true,
                ..AssignPreferences::default()
            }))),
            update_assign: false,
        }),
        lhs: Some(
            create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
                .unwrap(),
        ),
        rhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(string_scalar("should_not_overwrite")))
                .unwrap(),
            lhs: None,
            rhs: None,
        })),
    };

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(result.is_ok(), "assign with only_write_null should succeed");

    let root = engine.store.get(root_id);
    let value = map_value(root, "a");
    assert_eq!(value.value, "exists");
    assert_eq!(value.tag, SemType::Str.tag());
}

// ── Expression-level scenarios (aligned with assign.zig) ──────────

#[test]
fn create_yaml_file_with_multiple_assignments() {
    // .a.b = "cat" | .x = "frog" — chained assignments building nested structure
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![empty_map()],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".a.b = \"cat\" | .x = \"frog\"",
    )
    .expect("chained assignment should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    // Should have keys "a" and "x"
    let root = &out.matching_nodes[0];
    assert!(
        root.content.len() >= 4,
        "map should have at least two key-value pairs"
    );
}

#[test]
fn update_assign_with_self_reference() {
    // .a |= . — update-assign where RHS is self-reference
    // Document: a: {b: 3}
    let mut engine = TreeEngine::default();
    let inner = mapping(vec![("b", int_scalar(3))]);
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", inner)])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a |= .")
        .expect("update-assign with self-reference should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn update_assign_with_traversal_on_empty_map() {
    // .a |= .b on empty map — should create the path
    // Document: {}
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![empty_map()],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a |= .b")
        .expect("update-assign with traversal on empty map should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn assign_traversal_to_traversal_on_empty_map() {
    // .a = .b on empty map — both sides are traversals
    // Document: {}
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![empty_map()],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a = .b")
        .expect("assign traversal to traversal on empty map should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn self_reference_wrapping_in_array() {
    // .a = [.a] — wrap self in array
    // Document: a: cat
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", string_scalar("cat"))])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a = [.a]")
        .expect("self-reference wrapping should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn change_string_3_to_number_3() {
    // .a = 3 — type coercion from string "3" to int 3
    // Document: a: "3"
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", string_scalar("3"))])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a = 3")
        .expect("string to int coercion should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn change_string_true_to_bool_true() {
    // .a = true — type coercion from string "true" to bool true
    // Document: a: "true"
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", string_scalar("true"))])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a = true")
        .expect("string to bool coercion should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn custom_tag_preservation_on_assign() {
    // !cat "meow" -> assign "woof" -> !cat "woof" (custom tag is preserved)
    // Document: a: !cat "meow"
    let mut engine = TreeEngine::default();

    let mut tagged_value = string_scalar("meow");
    tagged_value.tag = "!cat".to_owned();
    let root_id = engine.store.add(mapping(vec![("a", tagged_value)]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Assign(AssignPreferences {
                clobber_custom_tags: false,
                ..AssignPreferences::default()
            }))),
            update_assign: false,
        }),
        lhs: Some(
            create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
                .unwrap(),
        ),
        rhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(string_scalar("woof"))).unwrap(),
            lhs: None,
            rhs: None,
        })),
    };

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(
        result.is_ok(),
        "assign with custom tag preservation should succeed"
    );

    let root = engine.store.get(root_id);
    let value = map_value(root, "a");
    assert_eq!(value.value, "woof");
    assert_eq!(value.tag, "!cat");
}

#[test]
fn update_node_to_child_value() {
    // .a |= .b — update-assign with traversal to child
    // Document: {a: {b: {g: foof}}}
    let mut engine = TreeEngine::default();
    let deepest = mapping(vec![("g", string_scalar("foof"))]);
    let middle = mapping(vec![("b", deepest)]);
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", middle)])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a |= .b")
        .expect("update node to child value should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn double_array_elements() {
    // .[] |= . * 2 — multiply each array element by 2
    // Document: [1, 2, 3]
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".[] |= . * 2")
        .expect("double array elements should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
}

#[test]
fn update_to_sibling_value() {
    // .a = .b — assign traversal result
    // Document: {a: {b: child}, b: sibling}
    let mut engine = TreeEngine::default();
    let a_val = mapping(vec![("b", string_scalar("child"))]);
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", a_val), ("b", string_scalar("sibling"))])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a = .b")
        .expect("update to sibling value should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn multiple_path_assignment() {
    // (.a, .c) = "potato" — assign same value to multiple paths
    // Document: {a: fieldA, b: fieldB, c: fieldC}
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("a", string_scalar("fieldA")),
            ("b", string_scalar("fieldB")),
            ("c", string_scalar("fieldC")),
        ])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "(.a, .c) = \"potato\"")
        .expect("multiple path assignment should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn update_string_via_assign() {
    // .a.b = "frog" — update nested string value
    // Document: {a: {b: apple}}
    let mut engine = TreeEngine::default();
    let inner = mapping(vec![("b", string_scalar("apple"))]);
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", inner)])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b = \"frog\"")
        .expect("update string via assign should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn update_string_via_update_assign() {
    // .a.b |= "frog" — update nested string via |=
    // Document: {a: {b: apple}}
    let mut engine = TreeEngine::default();
    let inner = mapping(vec![("b", string_scalar("apple"))]);
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", inner)])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b |= \"frog\"")
        .expect("update string via |= should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn pipe_with_self_assign() {
    // .a.b | (. |= "frog") — pipe then assign to self
    // Document: {a: {b: apple}}
    let mut engine = TreeEngine::default();
    let inner = mapping(vec![("b", string_scalar("apple"))]);
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", inner)])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b | (. |= \"frog\")")
        .expect("pipe with self-assign should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "frog");
}

#[test]
fn update_nested_with_int() {
    // .a.b |= 5 — update nested with integer
    // Document: {a: {b: apple}}
    let mut engine = TreeEngine::default();
    let inner = mapping(vec![("b", string_scalar("apple"))]);
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", inner)])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b |= 5")
        .expect("update nested with int should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn update_nested_with_float() {
    // .a.b |= 3.142 — update nested with float
    // Document: {a: {b: apple}}
    let mut engine = TreeEngine::default();
    let inner = mapping(vec![("b", string_scalar("apple"))]);
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", inner)])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b |= 3.142")
        .expect("update nested with float should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn deeply_selected_results_update_with_select() {
    // (.a[] | select(. == "apple")) = "frog" — select then update-assign
    // Document: {a: {b: apple, c: cactus}}
    let mut engine = TreeEngine::default();
    let inner = mapping(vec![
        ("b", string_scalar("apple")),
        ("c", string_scalar("cactus")),
    ]);
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", inner)])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "(.a[] | select(. == \"apple\")) = \"frog\"",
    )
    .expect("deeply selected results update should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn deeply_selected_results_update_with_explicit_splat_select() {
    let mut engine = TreeEngine::default();
    let inner = mapping(vec![
        ("b", string_scalar("apple")),
        ("c", string_scalar("cactus")),
    ]);
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", inner)])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "(.a.[] | select(. == \"apple\")) = \"frog\"",
    )
    .expect("deeply selected explicit splat update should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let root = &out.matching_nodes[0];
    let nested = map_value(root, "a");
    assert_eq!(map_value(nested, "b").value, "frog");
    assert_eq!(map_value(nested, "c").value, "cactus");
}

#[test]
fn array_value_update_with_select_wildcard() {
    // (.[] | select(. == "*andy")) = "bogs" — select with wildcard then update
    // Document: [candy, apple, sandy]
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            string_scalar("candy"),
            string_scalar("apple"),
            string_scalar("sandy"),
        ])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "(.[] | select(. == \"*andy\")) = \"bogs\"",
    )
    .expect("array value update with select wildcard should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
}

#[test]
fn update_empty_object() {
    // .a.b |= "bogs" on empty input — creates nested structure
    // Document: {}
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![empty_map()],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b |= \"bogs\"")
        .expect("update empty object should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn anchor_preservation_on_update() {
    // a: &cool cat -> .a = "dog" -> a: &cool dog (anchor is preserved)
    // Document: a: &cool cat
    let mut engine = TreeEngine::default();

    let mut anchored_value = string_scalar("cat");
    anchored_value.anchor = "cool".to_owned();
    let root_id = engine.store.add(mapping(vec![("a", anchored_value)]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Assign(AssignPreferences {
                dont_overwrite_anchor: false,
                ..AssignPreferences::default()
            }))),
            update_assign: false,
        }),
        lhs: Some(
            create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
                .unwrap(),
        ),
        rhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(string_scalar("dog"))).unwrap(),
            lhs: None,
            rhs: None,
        })),
    };

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(
        result.is_ok(),
        "assign with anchor preservation should succeed"
    );

    let root = engine.store.get(root_id);
    let value = map_value(root, "a");
    assert_eq!(value.value, "dog");
    assert_eq!(value.anchor, "cool");
}

#[test]
fn nested_empty_object_array_creation_index_0() {
    // .a.b.[0] |= "bogs" on empty input — creates nested object and array
    // Document: {}
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![empty_map()],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b.[0] |= \"bogs\"")
        .expect("nested empty object/array creation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn nested_empty_object_array_creation_index_1() {
    // .a.b.[1].c |= "bogs" on empty input — creates nested structure with index 1
    // Document: {}
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![empty_map()],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b.[1].c |= \"bogs\"")
        .expect("nested empty object/array creation with index 1 should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
}

#[test]
fn custom_type_maintenance_by_default() {
    // .a = .b — custom types are maintained by default (clobber_custom_tags = false)
    // Document: a: !cat meow, b: !dog woof
    let mut engine = TreeEngine::default();

    let mut a_val = string_scalar("meow");
    a_val.tag = "!cat".to_owned();
    let mut b_val = string_scalar("woof");
    b_val.tag = "!dog".to_owned();
    let root_id = engine.store.add(mapping(vec![("a", a_val), ("b", b_val)]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    // .a = .b with clobber_custom_tags = false (default)
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Assign(AssignPreferences {
                clobber_custom_tags: false,
                ..AssignPreferences::default()
            }))),
            update_assign: false,
        }),
        lhs: Some(
            create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
                .unwrap(),
        ),
        rhs: Some(
            create_traversal_tree(&[string_scalar("b")], TraversePreferences::default(), false)
                .unwrap(),
        ),
    };

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(
        result.is_ok(),
        "assign with custom type maintenance should succeed"
    );

    let root = engine.store.get(root_id);
    let a_value = map_value(root, "a");
    let b_value = map_value(root, "b");
    assert_eq!(a_value.value, "woof");
    assert_eq!(a_value.tag, "!cat");
    assert_eq!(b_value.value, "woof");
    assert_eq!(b_value.tag, "!dog");
}

#[test]
fn custom_type_clobber_with_c_option() {
    // .a =c .b — use `c` option to clobber custom tags
    // Document: a: !cat meow, b: !dog woof
    let mut engine = TreeEngine::default();

    let mut a_val = string_scalar("meow");
    a_val.tag = "!cat".to_owned();
    let mut b_val = string_scalar("woof");
    b_val.tag = "!dog".to_owned();
    let root_id = engine.store.add(mapping(vec![("a", a_val), ("b", b_val)]));

    let ctx = Context {
        matching_nodes: vec![engine.store.get(root_id).clone()],
        ..Context::default()
    };

    // .a =c .b with clobber_custom_tags = true
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Assign(AssignPreferences {
                clobber_custom_tags: true,
                ..AssignPreferences::default()
            }))),
            update_assign: false,
        }),
        lhs: Some(
            create_traversal_tree(&[string_scalar("a")], TraversePreferences::default(), false)
                .unwrap(),
        ),
        rhs: Some(
            create_traversal_tree(&[string_scalar("b")], TraversePreferences::default(), false)
                .unwrap(),
        ),
    };

    let result = assign_update_operator(ctx, &mut engine, &mut expr);
    assert!(
        result.is_ok(),
        "assign with custom type clobber should succeed"
    );

    let root = engine.store.get(root_id);
    let a_value = map_value(root, "a");
    assert_eq!(a_value.value, "woof");
    assert_eq!(a_value.tag, "!dog");
}
