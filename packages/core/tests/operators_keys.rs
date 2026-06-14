use treease_core::operators::keys::{get_key_operator, is_key_operator, keys_operator};
use treease_core::operators::{
    Context, CoreError, EvalError, ExpressionNode, GET_KEY_OP_TYPE, IS_KEY_OP_TYPE, KEYS_OP_TYPE,
    NodeKind, Operation, SemType, TreeEngine, TreeNode,
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
fn is_key_operator_returns_true_for_map_keys() {
    let key_node = TreeNode {
        is_map_key: true,
        ..string_scalar("a")
    };
    let ctx = Context {
        matching_nodes: vec![key_node],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&IS_KEY_OP_TYPE);

    let out = is_key_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes[0].value, "true");
    assert_eq!(out.matching_nodes[0].tag, SemType::Boolean.to_string());
}

#[test]
fn get_key_operator_returns_key_for_map_value() {
    let mut engine = TreeEngine::default();
    let key_id = engine.store.add(string_scalar("a"));
    let value_node = TreeNode {
        key: Some(key_id),
        ..string_scalar("b")
    };
    let ctx = Context {
        matching_nodes: vec![value_node],
        ..Context::default()
    };
    let mut expr = expression(&GET_KEY_OP_TYPE);

    let out = get_key_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "a");
}

#[test]
fn keys_operator_returns_map_keys_and_sequence_indices() {
    let map_node = TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: "object".to_owned(),
        content: vec![
            string_scalar("a"),
            string_scalar("1"),
            string_scalar("b"),
            string_scalar("2"),
        ],
        ..TreeNode::default()
    };
    let sequence_node = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: "array".to_owned(),
        content: vec![string_scalar("x"), string_scalar("y"), string_scalar("z")],
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![map_node, sequence_node],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&KEYS_OP_TYPE);

    let out = keys_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a");
    assert_eq!(out.matching_nodes[0].content[1].value, "b");
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[1].content.len(), 3);
    assert_eq!(out.matching_nodes[1].content[0].value, "0");
    assert_eq!(out.matching_nodes[1].content[1].value, "1");
    assert_eq!(out.matching_nodes[1].content[2].value, "2");
}

#[test]
fn keys_operator_rejects_non_collection_nodes() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&KEYS_OP_TYPE);

    let err = keys_operator(ctx, &mut engine, &mut expr).unwrap_err();

    assert!(matches!(
        err,
        CoreError::Eval(EvalError::KeysOnlyWorksForMapsAndArrays)
    ));
}
