use treease_core::operators::kind::get_kind_operator;
use treease_core::operators::length::length_operator;
use treease_core::operators::{
    Context, ExpressionNode, GET_KIND_OP_TYPE, LENGTH_OP_TYPE, NodeKind, Operation, SemType,
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

fn scalar(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: "string".to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

#[test]
fn get_kind_operator_maps_node_kinds_to_strings() {
    let ctx = Context {
        matching_nodes: vec![
            TreeNode {
                kind: NodeKind::Mapping,
                tag: "object".to_owned(),
                sem_type: Some(SemType::Map),
                ..TreeNode::default()
            },
            TreeNode {
                kind: NodeKind::Sequence,
                tag: "array".to_owned(),
                sem_type: Some(SemType::Seq),
                ..TreeNode::default()
            },
            scalar("x"),
            TreeNode {
                kind: NodeKind::Alias,
                tag: "alias".to_owned(),
                ..TreeNode::default()
            },
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&GET_KIND_OP_TYPE);

    let out = get_kind_operator(ctx, &mut engine, &mut expr).unwrap();

    let values: Vec<_> = out
        .matching_nodes
        .iter()
        .map(|node| node.value.as_str())
        .collect();
    assert_eq!(values, ["map", "seq", "scalar", "alias"]);
    assert!(
        out.matching_nodes
            .iter()
            .all(|node| node.tag == SemType::Str.to_string())
    );
}

#[test]
fn length_operator_counts_scalar_mapping_sequence_and_null() {
    let map_node = TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: "object".to_owned(),
        content: vec![scalar("a"), scalar("1"), scalar("b"), scalar("2")],
        ..TreeNode::default()
    };
    let sequence_node = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: "array".to_owned(),
        content: vec![scalar("x"), scalar("y"), scalar("z")],
        ..TreeNode::default()
    };
    let null_node = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Nil),
        tag: "null".to_owned(),
        value: String::new(),
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![scalar("cat"), map_node, sequence_node, null_node],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&LENGTH_OP_TYPE);

    let out = length_operator(ctx, &mut engine, &mut expr).unwrap();

    let values: Vec<_> = out
        .matching_nodes
        .iter()
        .map(|node| node.value.as_str())
        .collect();
    assert_eq!(values, ["3", "2", "3", "0"]);
    assert!(
        out.matching_nodes
            .iter()
            .all(|node| node.tag == SemType::Int.to_string())
    );
}
