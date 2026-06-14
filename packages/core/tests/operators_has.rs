use treease_core::operators::has::has_operator;
use treease_core::operators::{
    Context, ExpressionNode, HAS_OP_TYPE, NodeKind, Operation, SemType, TreeEngine, TreeNode,
    create_scalar_node_i64, create_string_scalar_node, create_value_operation,
};

fn expression_with_rhs(rhs: TreeNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &HAS_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(rhs)).unwrap(),
            lhs: None,
            rhs: None,
        })),
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

fn string_scalar(value: &str) -> TreeNode {
    *create_string_scalar_node(value).unwrap()
}

fn int_scalar(value: i64) -> TreeNode {
    *create_scalar_node_i64(value).unwrap()
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
fn has_operator_checks_mapping_keys() {
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![("a", int_scalar(1))]),
            mapping(vec![("b", int_scalar(2))]),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_rhs(string_scalar("a"));

    let out = has_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes[0].value, "true");
    assert_eq!(out.matching_nodes[1].value, "false");
}

#[test]
fn has_operator_checks_sequence_indices() {
    let ctx = Context {
        matching_nodes: vec![
            sequence(vec![scalar(SemType::Int, "10"), scalar(SemType::Int, "20")]),
            sequence(vec![scalar(SemType::Int, "10")]),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_rhs(int_scalar(1));

    let out = has_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes[0].value, "true");
    assert_eq!(out.matching_nodes[1].value, "false");
}
