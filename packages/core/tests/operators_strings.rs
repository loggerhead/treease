use treease_core::operators::strings::{
    capture_operator, change_case_operator, join_string_operator, match_operator,
    split_string_operator, string_interpolation_operator, substitute_string_operator,
    test_operator, to_string_operator, trim_space_operator,
};
use treease_core::operators::{
    BLOCK_OP_TYPE, CAPTURE_OP_TYPE, CHANGE_CASE_OP_TYPE, Context, ExpressionNode,
    JOIN_STRING_OP_TYPE, MATCH_OP_TYPE, NodeKind, Operation, OperationPreference,
    SPLIT_STRING_OP_TYPE, STRING_INTERPOLATION_OP_TYPE, SUB_STRING_OP_TYPE, SemType, TEST_OP_TYPE,
    TO_STRING_OP_TYPE, TRIM_OP_TYPE, TreeEngine, TreeNode, create_value_operation,
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

fn expression(op_type: &'static treease_core::operators::OperationType) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: op_type,
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

fn expression_with_rhs(
    op_type: &'static treease_core::operators::OperationType,
    rhs: ExpressionNode,
) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: op_type,
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

fn value_expression(node: TreeNode) -> ExpressionNode {
    ExpressionNode {
        operation: create_value_operation(Box::new(node)).unwrap(),
        lhs: None,
        rhs: None,
    }
}

fn block_expression(lhs: ExpressionNode, rhs: ExpressionNode) -> ExpressionNode {
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

#[test]
fn trim_space_operator_trims_whitespace() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("  cat  ")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&TRIM_OP_TYPE);

    let out = trim_space_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cat");
}

#[test]
fn change_case_operator_uppercases() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &CHANGE_CASE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::ChangeCase(
                treease_core::operators::ChangeCasePrefs {
                    to_upper_case: true,
                },
            ))),
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    };

    let out = change_case_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "CAT");
}

#[test]
fn split_string_operator_splits_by_delimiter() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("a,b,c")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_rhs(&SPLIT_STRING_OP_TYPE, value_expression(string_scalar(",")));

    let out = split_string_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.kind, NodeKind::Sequence);
    assert_eq!(seq.content.len(), 3);
    assert_eq!(seq.content[0].value, "a");
    assert_eq!(seq.content[1].value, "b");
    assert_eq!(seq.content[2].value, "c");
}

#[test]
fn join_string_operator_joins_with_delimiter() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![string_scalar("cat"), string_scalar("dog")])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_rhs(&JOIN_STRING_OP_TYPE, value_expression(string_scalar("-")));

    let out = join_string_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cat-dog");
}

#[test]
fn substitute_string_operator_replaces_regex_matches() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let block = block_expression(
        value_expression(string_scalar("a")),
        value_expression(string_scalar("o")),
    );
    let mut expr = expression_with_rhs(&SUB_STRING_OP_TYPE, block);

    let out = substitute_string_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cot");
}

#[test]
fn test_operator_returns_boolean_for_regex_match() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_rhs(&TEST_OP_TYPE, value_expression(string_scalar("cat")));

    let out = test_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "true");
}

#[test]
fn match_operator_returns_match_info_map() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_rhs(&MATCH_OP_TYPE, value_expression(string_scalar("c.t")));

    let out = match_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let m = &out.matching_nodes[0];
    assert_eq!(m.kind, NodeKind::Mapping);
    // Should have "string", "offset", "length" keys
    let has_string = m
        .content
        .chunks(2)
        .any(|chunk| chunk[0].value == "string" && chunk[1].value == "cat");
    assert!(has_string);
}

#[test]
fn capture_operator_returns_capture_map() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_rhs(&CAPTURE_OP_TYPE, value_expression(string_scalar("c.t")));

    let out = capture_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let m = &out.matching_nodes[0];
    assert_eq!(m.kind, NodeKind::Mapping);
    // Should have "0" key with value "cat"
    let has_zero = m
        .content
        .chunks(2)
        .any(|chunk| chunk[0].value == "0" && chunk[1].value == "cat");
    assert!(has_zero);
}

#[test]
fn to_string_operator_converts_value_to_string_scalar() {
    let ctx = Context {
        matching_nodes: vec![int_scalar(12)],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&TO_STRING_OP_TYPE);

    let out = to_string_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].tag, SemType::Str.to_string());
    assert_eq!(out.matching_nodes[0].value, "12");
}

#[test]
fn string_interpolation_operator_expands_expressions() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", string_scalar("cat"))])],
        string_interpolation_enabled: true,
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &STRING_INTERPOLATION_OP_TYPE,
            value: None,
            string_value: "value=\\(.a)".to_owned(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    };

    let out = string_interpolation_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "value=cat");
}

#[test]
fn string_interpolation_operator_returns_raw_string_when_disabled() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("ignored")],
        string_interpolation_enabled: false,
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &STRING_INTERPOLATION_OP_TYPE,
            value: None,
            string_value: "value=\\(.a)".to_owned(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    };

    let out = string_interpolation_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "value=\\(.a)");
}
