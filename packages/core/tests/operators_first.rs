use treease_core::operators::first::first_operator;
use treease_core::operators::{
    Context, EQUALS_OP_TYPE, ExpressionNode, FIRST_OP_TYPE, NodeKind, Operation,
    SELF_REFERENCE_OP_TYPE, SemType, TreeEngine, TreeNode, create_scalar_node_bool,
    create_value_operation,
};

fn expression(rhs: Option<TreeNode>) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &FIRST_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: rhs.map(|node| {
            Box::new(ExpressionNode {
                operation: create_value_operation(Box::new(node)).unwrap(),
                lhs: None,
                rhs: None,
            })
        }),
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

fn sequence(items: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: items,
        ..TreeNode::default()
    }
}

fn first_with_rhs_expression(rhs_expr: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &FIRST_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(rhs_expr)),
    }
}

fn string_scalar(value: &str) -> TreeNode {
    scalar(SemType::Str, value)
}

#[test]
fn first_operator_without_rhs_returns_first_child() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            scalar(SemType::Int, "10"),
            scalar(SemType::Int, "20"),
            scalar(SemType::Int, "30"),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(None);

    let out = first_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "10");
}

#[test]
fn first_operator_with_truthy_rhs_still_returns_first_matching_child() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            scalar(SemType::Int, "1"),
            scalar(SemType::Int, "2"),
            scalar(SemType::Int, "3"),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(Some(*create_scalar_node_bool(true).unwrap()));

    let out = first_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "1");
}

#[test]
fn first_operator_with_rhs_selectively_matches_value_b() {
    // Zig: sequence ["a","b","c"] with RHS that matches only "b" → returns "b"
    // Build RHS expression: self == "b"
    let self_expr = ExpressionNode {
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
    };
    let b_value_expr = ExpressionNode {
        operation: create_value_operation(Box::new(string_scalar("b"))).unwrap(),
        lhs: None,
        rhs: None,
    };
    let equals_rhs = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &EQUALS_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(self_expr)),
        rhs: Some(Box::new(b_value_expr)),
    };

    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            string_scalar("a"),
            string_scalar("b"),
            string_scalar("c"),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = first_with_rhs_expression(equals_rhs);

    let out = first_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "b");
}
