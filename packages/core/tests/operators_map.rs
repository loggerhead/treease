use treease_core::operators::map::{map_operator, map_values_operator};
use treease_core::operators::{
    ADD_OP_TYPE, Context, ExpressionNode, MAP_OP_TYPE, MAP_VALUES_OP_TYPE, NodeKind, Operation,
    SELF_REFERENCE_OP_TYPE, SemType, TreeEngine, TreeNode, create_value_operation,
};

fn expression_with_rhs(
    operation: &'static treease_core::operators::OperationType,
    rhs: ExpressionNode,
) -> ExpressionNode {
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
        rhs: Some(Box::new(rhs)),
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

fn add_one_expression() -> ExpressionNode {
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
        rhs: Some(Box::new(value_expression(int_scalar(1)))),
    }
}

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

#[test]
fn map_operator_maps_over_sequence_with_addition_rhs() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(1), int_scalar(2)])],
        ..Context::default()
    };
    let mut expr = expression_with_rhs(&MAP_OP_TYPE, add_one_expression());
    let mut engine = TreeEngine::default();

    let out = map_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let mapped = &out.matching_nodes[0];
    assert_eq!(mapped.kind, NodeKind::Sequence);
    assert_eq!(mapped.content[0].value, "2");
    assert_eq!(mapped.content[1].value, "3");
}

#[test]
fn map_values_operator_updates_map_values() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("b", int_scalar(12)),
            ("name", string_scalar("Ada")),
        ])],
        ..Context::default()
    };
    let mut expr = expression_with_rhs(&MAP_VALUES_OP_TYPE, value_expression(int_scalar(3)));
    let mut engine = TreeEngine::default();

    let out = map_values_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let mapped = &out.matching_nodes[0];
    assert_eq!(mapped.kind, NodeKind::Mapping);
    assert_eq!(mapped.content[1].value, "3");
    assert_eq!(mapped.content[3].value, "3");
}
