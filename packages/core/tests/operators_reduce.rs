use treease_core::expression_pipeline;
use treease_core::operators::reduce::reduce_operator;
use treease_core::operators::{
    ADD_OP_TYPE, ASSIGN_OP_TYPE, ASSIGN_VARIABLE_OP_TYPE, BLOCK_OP_TYPE, Context, CoreError,
    ExpressionNode, GET_VARIABLE_OP_TYPE, MULTIPLY_OP_TYPE, NodeKind, Operation, PIPE_OP_TYPE,
    REDUCE_OP_TYPE, SELF_REFERENCE_OP_TYPE, SemType, TRAVERSE_ARRAY_OP_TYPE, TraversePreferences,
    TreeEngine, TreeNode, create_traversal_tree, create_value_operation,
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

fn int_scalar(value: i64) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: value.to_string(),
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

fn sequence(items: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: items,
        ..TreeNode::default()
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

/// Build a reduce expression: `array_expr as $var reduce (initial; block)`
fn reduce_expression(
    array_expr: ExpressionNode,
    var_name: &str,
    initial_expr: ExpressionNode,
    block_expr: ExpressionNode,
) -> ExpressionNode {
    let var_rhs = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_VARIABLE_OP_TYPE,
            value: None,
            string_value: var_name.to_owned(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    };
    let assign_var = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_VARIABLE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(array_expr)),
        rhs: Some(Box::new(var_rhs)),
    };
    let block = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &BLOCK_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(initial_expr)),
        rhs: Some(Box::new(block_expr)),
    };
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &REDUCE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(assign_var)),
        rhs: Some(Box::new(block)),
    }
}

fn get_variable_expression(name: &str) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &GET_VARIABLE_OP_TYPE,
            value: None,
            string_value: name.to_owned(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

fn add_self_and_variable(var_name: &str) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ADD_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(self_expression())),
        rhs: Some(Box::new(get_variable_expression(var_name))),
    }
}

fn multiply_self_and_variable(var_name: &str) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &MULTIPLY_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(self_expression())),
        rhs: Some(Box::new(get_variable_expression(var_name))),
    }
}

fn pipe_expression(lhs: ExpressionNode, rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &PIPE_OP_TYPE,
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

fn assign_expression(lhs: ExpressionNode, rhs: ExpressionNode) -> ExpressionNode {
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

fn array_items_expression() -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &TRAVERSE_ARRAY_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(self_expression())),
        rhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(sequence(vec![]))).unwrap(),
            lhs: None,
            rhs: None,
        })),
    }
}

#[test]
fn reduce_operator_requires_variable_assignment_lhs() {
    let ctx = Context::default();
    let mut engine = TreeEngine::default();
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &REDUCE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(self_expression())),
        rhs: Some(Box::new(self_expression())),
    };

    let err = reduce_operator(ctx, &mut engine, &mut expr).unwrap_err();
    assert!(matches!(err, CoreError::OperatorMessage { .. }));
}

#[test]
fn reduce_operator_requires_block_rhs() {
    let ctx = Context::default();
    let mut engine = TreeEngine::default();
    let var_rhs = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_VARIABLE_OP_TYPE,
            value: None,
            string_value: "$x".to_owned(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    };
    let assign_var = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_VARIABLE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(self_expression())),
        rhs: Some(Box::new(var_rhs)),
    };
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &REDUCE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(assign_var)),
        rhs: Some(Box::new(self_expression())),
    };

    let err = reduce_operator(ctx, &mut engine, &mut expr).unwrap_err();
    assert!(matches!(err, CoreError::OperatorMessage { .. }));
}

#[test]
fn reduce_operator_sums_numbers() {
    // Zig: .[10,2,5,3] as $item reduce (0; . + $item) → 20
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            int_scalar(10),
            int_scalar(2),
            int_scalar(5),
            int_scalar(3),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = reduce_expression(
        array_items_expression(),
        "$item",
        value_expression(int_scalar(0)),
        add_self_and_variable("$item"),
    );

    let result = reduce_operator(ctx, &mut engine, &mut expr).unwrap();
    assert_eq!(result.matching_nodes.len(), 1);
    assert_eq!(result.matching_nodes[0].kind, NodeKind::Scalar);
    assert_eq!(result.matching_nodes[0].value, "20");
}

#[test]
fn reduce_operator_with_empty_input_returns_initial() {
    let ctx = Context {
        matching_nodes: vec![],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = reduce_expression(
        self_expression(),
        "$item",
        value_expression(int_scalar(42)),
        self_expression(),
    );

    let result = reduce_operator(ctx, &mut engine, &mut expr).unwrap();
    assert_eq!(result.matching_nodes.len(), 1);
    assert_eq!(result.matching_nodes[0].value, "42");
}

#[test]
fn reduce_operator_merges_documents_together() {
    // Zig: merge {a: cat} + {b: dog} via . * $item → {a: cat, b: dog}
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("a", int_scalar(1)), ("b", int_scalar(2))]),
            mapping(vec![("c", int_scalar(3)), ("d", int_scalar(4))]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = reduce_expression(
        array_items_expression(),
        "$item",
        value_expression(mapping(vec![])),
        multiply_self_and_variable("$item"),
    );

    let result = reduce_operator(ctx, &mut engine, &mut expr).unwrap();
    // Result should be a mapping with all four key-value pairs merged
    assert_eq!(result.matching_nodes.len(), 1);
    let map = &result.matching_nodes[0];
    assert_eq!(map.kind, NodeKind::Mapping);
    assert_eq!(map.content.len(), 8); // 4 key-value pairs
    // Verify key-value pairs are present (order may vary)
    let mut found: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut i = 0;
    while i + 1 < map.content.len() {
        found.insert(
            map.content[i].value.as_str(),
            map.content[i + 1].value.as_str(),
        );
        i += 2;
    }
    assert_eq!(found.get("a"), Some(&"1"));
    assert_eq!(found.get("b"), Some(&"2"));
    assert_eq!(found.get("c"), Some(&"3"));
    assert_eq!(found.get("d"), Some(&"4"));
}

#[test]
fn reduce_operator_converts_array_to_object() {
    // Zig-style dynamic key assignment during reduce:
    // [{key:name, value:harry}, {key:pet, value:cat}] -> {name: harry, pet: cat}
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("key", string_scalar("name")),
                ("value", string_scalar("harry")),
            ]),
            mapping(vec![
                ("key", string_scalar("pet")),
                ("value", string_scalar("cat")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let key_path = create_traversal_tree(
        &[string_scalar("key")],
        TraversePreferences::default(),
        false,
    )
    .unwrap();
    let value_path = create_traversal_tree(
        &[string_scalar("value")],
        TraversePreferences::default(),
        false,
    )
    .unwrap();
    let dynamic_key_path = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &TRAVERSE_ARRAY_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(self_expression())),
        rhs: Some(Box::new(pipe_expression(
            get_variable_expression("$item"),
            *key_path,
        ))),
    };
    let assignment = assign_expression(
        dynamic_key_path,
        pipe_expression(get_variable_expression("$item"), *value_path),
    );
    let mut expr = reduce_expression(
        array_items_expression(),
        "$item",
        value_expression(mapping(vec![])),
        assignment,
    );

    let result = reduce_operator(ctx, &mut engine, &mut expr).unwrap();
    assert_eq!(result.matching_nodes.len(), 1);
    let map = &result.matching_nodes[0];
    assert_eq!(map.kind, NodeKind::Mapping);
    assert_eq!(map.content.len(), 4);
    let mut found: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut i = 0;
    while i + 1 < map.content.len() {
        found.insert(
            map.content[i].value.as_str(),
            map.content[i + 1].value.as_str(),
        );
        i += 2;
    }
    assert_eq!(found.get("name"), Some(&"harry"));
    assert_eq!(found.get("pet"), Some(&"cat"));
}

#[test]
fn reduce_pipeline_sums_numbers_like_zig_reduce_scenario() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            int_scalar(10),
            int_scalar(2),
            int_scalar(5),
            int_scalar(3),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".[] as $item reduce (0; . + $item)",
    )
    .expect("reduce pipeline should sum numbers");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "20");
}

#[test]
fn reduce_pipeline_merges_documents_like_zig_reduce_scenario() {
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![("a", string_scalar("cat"))]),
            mapping(vec![("b", string_scalar("dog"))]),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ". as $item reduce ({}; . * $item )",
    )
    .expect("reduce pipeline should merge documents");

    assert_eq!(out.matching_nodes.len(), 1);
    let map = &out.matching_nodes[0];
    assert_eq!(map.kind, NodeKind::Mapping);
    let mut found = std::collections::HashMap::new();
    let mut i = 0;
    while i + 1 < map.content.len() {
        found.insert(
            map.content[i].value.as_str(),
            map.content[i + 1].value.as_str(),
        );
        i += 2;
    }
    assert_eq!(found.get("a"), Some(&"cat"));
    assert_eq!(found.get("b"), Some(&"dog"));
}

#[test]
fn reduce_pipeline_converts_array_to_object_like_zig_reduce_scenario() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("name", string_scalar("Cathy")),
                ("has", string_scalar("apples")),
            ]),
            mapping(vec![
                ("name", string_scalar("Bob")),
                ("has", string_scalar("bananas")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".[] as $item reduce ({}; .[$item | .name] = ($item | .has) )",
    )
    .expect("reduce pipeline should convert array to object");

    assert_eq!(out.matching_nodes.len(), 1);
    let map = &out.matching_nodes[0];
    assert_eq!(map.kind, NodeKind::Mapping);
    let mut found = std::collections::HashMap::new();
    let mut i = 0;
    while i + 1 < map.content.len() {
        found.insert(
            map.content[i].value.as_str(),
            map.content[i + 1].value.as_str(),
        );
        i += 2;
    }
    assert_eq!(found.get("Cathy"), Some(&"apples"));
    assert_eq!(found.get("Bob"), Some(&"bananas"));
}
