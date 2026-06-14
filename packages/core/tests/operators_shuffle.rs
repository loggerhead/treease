use treease_core::operators::shuffle::shuffle_operator;
use treease_core::operators::{
    Context, CoreError, EvalError, ExpressionNode, NodeKind, Operation, SHUFFLE_OP_TYPE, SemType,
    TreeEngine, TreeNode,
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

fn int_scalar(value: i64) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: "!!int".to_owned(),
        value: value.to_string(),
        ..TreeNode::default()
    }
}

#[test]
fn shuffle_operator_returns_a_permutation_of_sequence_items() {
    let seq = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: "array".to_owned(),
        content: vec![
            string_scalar("a"),
            string_scalar("b"),
            string_scalar("c"),
            string_scalar("d"),
        ],
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![seq],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&SHUFFLE_OP_TYPE);

    let out = shuffle_operator(ctx, &mut engine, &mut expr).unwrap();
    let mut values: Vec<_> = out.matching_nodes[0]
        .content
        .iter()
        .map(|node| node.value.clone())
        .collect();
    values.sort();

    assert_eq!(values, vec!["a", "b", "c", "d"]);
}

#[test]
fn shuffle_operator_rejects_non_arrays() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&SHUFFLE_OP_TYPE);

    let err = shuffle_operator(ctx, &mut engine, &mut expr).unwrap_err();
    assert!(matches!(err, CoreError::Eval(EvalError::NodeIsNotArray)));
}

#[test]
fn shuffle_operator_resets_indices_and_returns_permutation() {
    let mut engine = TreeEngine::default();
    let seq = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: "array".to_owned(),
        content: ["a", "b", "c", "d"]
            .into_iter()
            .enumerate()
            .map(|(idx, value)| {
                let key_id = engine.store.add(int_scalar(idx as i64));
                let mut child = string_scalar(value);
                child.key = Some(key_id);
                child
            })
            .collect(),
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![seq],
        ..Context::default()
    };
    let mut expr = expression(&SHUFFLE_OP_TYPE);

    let out = shuffle_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let result_seq = &out.matching_nodes[0];
    assert_eq!(result_seq.kind, NodeKind::Sequence);
    assert_eq!(result_seq.content.len(), 4);

    let mut key_values = Vec::new();
    for child in &result_seq.content {
        let key_id = child.key.expect("shuffled child should retain a key node");
        let key = engine.store.get(key_id);
        assert_eq!(key.tag, "!!int");
        key_values.push(key.value.clone());
        assert!(
            matches!(child.value.as_str(), "a" | "b" | "c" | "d"),
            "unexpected child value: {}",
            child.value
        );
    }
    key_values.sort();
    assert_eq!(key_values, vec!["0", "1", "2", "3"]);

    let mut values: Vec<_> = result_seq
        .content
        .iter()
        .map(|node| node.value.clone())
        .collect();
    values.sort();
    assert_eq!(values, vec!["a", "b", "c", "d"]);
}
