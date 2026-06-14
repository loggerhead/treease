use treease_core::operators::reverse::reverse_operator;
use treease_core::operators::to_number::to_number_operator;
use treease_core::operators::{
    Context, CoreError, EvalError, ExpressionNode, NodeKind, Operation, REVERSE_OP_TYPE, SemType,
    TO_NUMBER_OP_TYPE, TreeEngine, TreeNode,
};

fn expression(operation: &'static treease_core::operators::OperationType) -> ExpressionNode {
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
        rhs: None,
    }
}

fn string_scalar(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: "string".to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

#[test]
fn reverse_operator_reverses_array_order() {
    let sequence = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: "array".to_owned(),
        content: vec![string_scalar("a"), string_scalar("b"), string_scalar("c")],
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![sequence],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&REVERSE_OP_TYPE);

    let out = reverse_operator(ctx, &mut engine, &mut expr).unwrap();
    let values: Vec<_> = out.matching_nodes[0]
        .content
        .iter()
        .map(|node| node.value.as_str())
        .collect();

    assert_eq!(values, ["c", "b", "a"]);
}

#[test]
fn reverse_operator_rejects_non_arrays() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&REVERSE_OP_TYPE);

    let err = reverse_operator(ctx, &mut engine, &mut expr).unwrap_err();

    assert!(matches!(err, CoreError::Eval(EvalError::NodeIsNotArray)));
}

#[test]
fn to_number_operator_converts_numeric_strings_and_keeps_numbers() {
    let int_node = string_scalar("3");
    let float_node = string_scalar("3.1");
    let sci_node = string_scalar("-1e3");
    let kept_int = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: "integer".to_owned(),
        value: "7".to_owned(),
        ..TreeNode::default()
    };
    let kept_float = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Float),
        tag: "number".to_owned(),
        value: "2.5".to_owned(),
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![
            int_node,
            float_node,
            sci_node,
            kept_int.clone(),
            kept_float.clone(),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&TO_NUMBER_OP_TYPE);

    let out = to_number_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes[0].tag, "!!int");
    assert_eq!(out.matching_nodes[0].value, "3");
    assert_eq!(out.matching_nodes[1].tag, "!!float");
    assert_eq!(out.matching_nodes[1].value, "3.1");
    assert_eq!(out.matching_nodes[2].tag, "!!float");
    assert_eq!(out.matching_nodes[2].value, "-1e3");
    assert_eq!(out.matching_nodes[3].tag, kept_int.tag);
    assert_eq!(out.matching_nodes[3].value, kept_int.value);
    assert_eq!(out.matching_nodes[4].tag, kept_float.tag);
    assert_eq!(out.matching_nodes[4].value, kept_float.value);
}

#[test]
fn to_number_operator_rejects_null_scalars() {
    let null_node = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Nil),
        tag: "null".to_owned(),
        value: "null".to_owned(),
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![null_node],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&TO_NUMBER_OP_TYPE);

    let err = to_number_operator(ctx, &mut engine, &mut expr).unwrap_err();

    assert!(matches!(
        err,
        CoreError::Eval(EvalError::CannotConvertValueToNumber)
    ));
}
