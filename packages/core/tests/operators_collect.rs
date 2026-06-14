use treease_core::operators::collect::collect_operator;
use treease_core::operators::{
    ADD_OP_TYPE, COLLECT_OP_TYPE, Context, ExpressionNode, NodeKind, Operation,
    SELF_REFERENCE_OP_TYPE, SemType, TreeEngine, TreeNode, create_value_operation,
};

/// Build a collect expression whose RHS appends "!" to each node value,
/// mirroring the Zig `rhsEchoHandler` in `collect.zig`.
fn expression_with_append_bang_rhs() -> ExpressionNode {
    // self + "!"
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
    let bang_value = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: SemType::Str.tag().to_owned(),
        value: "!".to_owned(),
        ..TreeNode::default()
    };
    let bang_expr = ExpressionNode {
        operation: create_value_operation(Box::new(bang_value)).unwrap(),
        lhs: None,
        rhs: None,
    };
    let append_rhs = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ADD_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(self_expr)),
        rhs: Some(Box::new(bang_expr)),
    };
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &COLLECT_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(append_rhs)),
    }
}

fn string_scalar(value: &str, evaluate_together: bool) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: SemType::Str.tag().to_owned(),
        value: value.to_owned(),
        evaluate_together,
        ..TreeNode::default()
    }
}

#[test]
fn collect_operator_returns_empty_sequence_for_empty_input() {
    let ctx = Context::default();
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_append_bang_rhs();

    let out = collect_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert!(out.matching_nodes[0].content.is_empty());
}

#[test]
fn collect_operator_collects_all_nodes_into_one_sequence_when_evaluate_together() {
    // Zig: two nodes with evaluate_together=true → one sequence with "a!" and "b!"
    let ctx = Context {
        matching_nodes: vec![string_scalar("a", true), string_scalar("b", true)],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_append_bang_rhs();

    let out = collect_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a!");
    assert_eq!(out.matching_nodes[0].content[1].value, "b!");
}

#[test]
fn collect_operator_collects_per_node_when_evaluate_together_is_mixed() {
    // Zig: one node with evaluate_together=false, one with true → two sequences
    let ctx = Context {
        matching_nodes: vec![string_scalar("a", false), string_scalar("b", true)],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_append_bang_rhs();

    let out = collect_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content[0].value, "a!");
    assert_eq!(out.matching_nodes[1].content[0].value, "b!");
}
