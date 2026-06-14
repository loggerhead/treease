use treease_core::expression_pipeline;
use treease_core::operators::with::with_operator;
use treease_core::operators::{
    ASSIGN_OP_TYPE, BLOCK_OP_TYPE, Context, CoreError, ExpressionNode, NodeKind, Operation,
    PIPE_OP_TYPE, SELF_REFERENCE_OP_TYPE, SemType, TRAVERSE_ARRAY_OP_TYPE, TRAVERSE_PATH_OP_TYPE,
    TreeEngine, TreeNode, VALUE_OP_TYPE, WITH_OP_TYPE, create_value_operation, splat,
};

fn string_scalar(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: SemType::Str.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

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

fn self_reference_expr() -> ExpressionNode {
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

fn traverse_path_expr(path: &str) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &TRAVERSE_PATH_OP_TYPE,
            value: None,
            string_value: path.to_string(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

fn value_expr(node: TreeNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &VALUE_OP_TYPE,
            value: None,
            string_value: node.value.clone(),
            tree_node: Some(Box::new(node)),
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

fn assign_expr(lhs: ExpressionNode, rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
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

fn block_expr(lhs: ExpressionNode, rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &BLOCK_OP_TYPE,
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

fn with_expr(block: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &WITH_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(block)),
    }
}

#[test]
fn with_operator_requires_block_rhs() {
    let ctx = Context::default();
    let mut engine = TreeEngine::default();
    // Use a non-block expression as rhs
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &WITH_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(self_reference_expr())),
    };

    let err = with_operator(ctx, &mut engine, &mut expr).unwrap_err();

    assert!(matches!(
        err,
        CoreError::OperatorMessage { ref op, ref message }
            if op == "with" && message.contains("must be given a block")
    ));
}

#[test]
fn with_operator_updates_nested_property() {
    // Build: with(.a; . = "newValue")
    // This traverses to .a on each matching node and assigns "newValue".
    let inner_map = mapping(vec![("nested", string_scalar("oldValue"))]);
    let outer_map = mapping(vec![("a", inner_map)]);

    let ctx = Context {
        matching_nodes: vec![outer_map],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let target_path = traverse_path_expr("a");
    let update = assign_expr(self_reference_expr(), value_expr(string_scalar("newValue")));
    let block = block_expr(target_path, update);
    let mut expr = with_expr(block);

    let out = with_operator(ctx, &mut engine, &mut expr).unwrap();

    // The with_operator returns the original context unchanged.
    assert_eq!(out.matching_nodes.len(), 1);
}

#[test]
fn with_operator_works_on_array_elements() {
    // Build a sequence node and use with(.[]; . = "updated")
    let seq = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: vec![string_scalar("a"), string_scalar("b"), string_scalar("c")],
        ..TreeNode::default()
    };

    let ctx = Context {
        matching_nodes: vec![seq],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let target_path = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &TRAVERSE_ARRAY_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(self_reference_expr())),
        rhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(TreeNode {
                kind: NodeKind::Sequence,
                sem_type: Some(SemType::Seq),
                tag: SemType::Seq.tag().to_owned(),
                ..TreeNode::default()
            }))
            .unwrap(),
            lhs: None,
            rhs: None,
        })),
    };
    let update = assign_expr(self_reference_expr(), value_expr(string_scalar("updated")));
    let block = block_expr(target_path, update);
    let mut expr = with_expr(block);

    let out = with_operator(ctx, &mut engine, &mut expr).unwrap();

    // The with_operator returns the original context unchanged.
    assert_eq!(out.matching_nodes.len(), 1);
}

#[test]
fn with_operator_with_add_assign_update() {
    // Build: with(.a; . += "suffix")
    // This traverses to .a and appends "suffix" to the value.
    let inner_map = mapping(vec![("name", string_scalar("hello"))]);
    let outer_map = mapping(vec![("a", inner_map)]);

    let ctx = Context {
        matching_nodes: vec![outer_map],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let target_path = traverse_path_expr("a");
    let update = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: true,
        }),
        lhs: Some(Box::new(self_reference_expr())),
        rhs: Some(Box::new(value_expr(string_scalar("suffix")))),
    };
    let block = block_expr(target_path, update);
    let mut expr = with_expr(block);

    let out = with_operator(ctx, &mut engine, &mut expr).unwrap();

    // The with_operator returns the original context unchanged.
    assert_eq!(out.matching_nodes.len(), 1);
}

#[test]
fn with_operator_splat() {
    // Tests: with(.a.deeply.nested; . = "newValue")[]
    let inner_inner = mapping(vec![("nested", string_scalar("value"))]);
    let inner_map = mapping(vec![("deeply", inner_inner)]);
    let outer_map = mapping(vec![("a", inner_map)]);

    let ctx = Context {
        matching_nodes: vec![outer_map],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let target_path = traverse_path_expr("a.deeply.nested");
    let update = assign_expr(self_reference_expr(), value_expr(string_scalar("newValue")));
    let block = block_expr(target_path, update);
    let mut expr = with_expr(block);

    let out = with_operator(ctx, &mut engine, &mut expr).unwrap();
    assert_eq!(out.matching_nodes.len(), 1);
}

#[test]
fn with_operator_updates_multiple_deeply_nested_properties() {
    // Tests: with(.a.deeply; .nested = "newValue" | .other = "newThing")
    let inner_map = mapping(vec![
        ("nested", string_scalar("value")),
        ("other", string_scalar("thing")),
    ]);
    let outer_map = mapping(vec![("a", mapping(vec![("deeply", inner_map)]))]);

    let ctx = Context {
        matching_nodes: vec![outer_map],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let target_path = traverse_path_expr("a.deeply");
    let assign_nested = assign_expr(
        traverse_path_expr("nested"),
        value_expr(string_scalar("newValue")),
    );
    let assign_other = assign_expr(
        traverse_path_expr("other"),
        value_expr(string_scalar("newThing")),
    );
    let pipe = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &PIPE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(assign_nested)),
        rhs: Some(Box::new(assign_other)),
    };
    let block = block_expr(target_path, pipe);
    let mut expr = with_expr(block);

    let out = with_operator(ctx, &mut engine, &mut expr).unwrap();
    assert_eq!(out.matching_nodes.len(), 1);
}

#[test]
fn with_expression_result_can_be_splatted_after_update() {
    let outer_map = mapping(vec![(
        "a",
        mapping(vec![(
            "deeply",
            mapping(vec![("nested", string_scalar("value"))]),
        )]),
    )]);
    let ctx = Context {
        matching_nodes: vec![outer_map],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let updated = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "with(.a.deeply.nested; . = \"newValue\")",
    )
    .expect("with pipeline should succeed");
    let splatted = splat(
        updated,
        treease_core::operators::TraversePreferences::default(),
    )
    .expect("splat should succeed");

    assert_eq!(splatted.matching_nodes.len(), 1);
    assert_eq!(splatted.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(splatted.matching_nodes[0].content[0].value, "deeply");
}

#[test]
fn with_pipeline_updates_and_styles_nested_value_like_zig() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "a",
            mapping(vec![(
                "deeply",
                mapping(vec![("nested", string_scalar("value"))]),
            )]),
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "with(.a.deeply.nested; . = \"newValue\")",
    )
    .expect("with pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let root = &out.matching_nodes[0];
    assert_eq!(root.content[0].value, "a");
    assert_eq!(root.content[1].content[0].value, "deeply");
    assert_eq!(root.content[1].content[1].content[0].value, "nested");
    assert_eq!(root.content[1].content[1].content[1].value, "newValue");
}

#[test]
fn with_pipeline_updates_multiple_deep_properties_in_one_block() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "a",
            mapping(vec![(
                "deeply",
                mapping(vec![
                    ("nested", string_scalar("value")),
                    ("other", string_scalar("thing")),
                ]),
            )]),
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "with(.a.deeply; .nested = \"newValue\" | .other= \"newThing\")",
    )
    .expect("multi-update with pipeline should succeed");

    let root = &out.matching_nodes[0];
    let deeply = &root.content[1].content[1];
    assert_eq!(deeply.content[0].value, "nested");
    assert_eq!(deeply.content[1].value, "newValue");
    assert_eq!(deeply.content[2].value, "other");
    assert_eq!(deeply.content[3].value, "newThing");
}

#[test]
fn with_pipeline_updates_array_elements_relatively() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "myArray",
            TreeNode {
                kind: NodeKind::Sequence,
                sem_type: Some(SemType::Seq),
                tag: SemType::Seq.tag().to_owned(),
                content: vec![
                    mapping(vec![("a", string_scalar("apple"))]),
                    mapping(vec![("a", string_scalar("banana"))]),
                ],
                ..TreeNode::default()
            },
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        "with(.myArray[]; .b = .a + \" yum\")",
    )
    .expect("relative with pipeline should succeed");

    let items = &out.matching_nodes[0].content[1];
    assert_eq!(items.kind, NodeKind::Sequence);
    assert_eq!(items.content[0].content[2].value, "b");
    assert_eq!(items.content[0].content[3].value, "apple yum");
    assert_eq!(items.content[1].content[2].value, "b");
    assert_eq!(items.content[1].content[3].value, "banana yum");
}

#[test]
fn with_pipeline_supports_relative_add_assign_updates() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "myArray",
            TreeNode {
                kind: NodeKind::Sequence,
                sem_type: Some(SemType::Seq),
                tag: SemType::Seq.tag().to_owned(),
                content: vec![
                    mapping(vec![("a", string_scalar("apple"))]),
                    mapping(vec![("a", string_scalar("banana"))]),
                ],
                ..TreeNode::default()
            },
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "with(.myArray[]; .a += .a)")
            .expect("relative add-assign pipeline should succeed");

    let items = &out.matching_nodes[0].content[1];
    assert_eq!(items.content[0].content[1].value, "appleapple");
    assert_eq!(items.content[1].content[1].value, "bananabanana");
}
