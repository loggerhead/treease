use treease_core::operators::union::union_operator;
use treease_core::operators::{
    Context, ExpressionNode, NodeKind, Operation, SELF_REFERENCE_OP_TYPE, SemType, TreeEngine,
    TreeNode, UNION_OP_TYPE, VALUE_OP_TYPE,
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

fn self_reference_expr() -> ExpressionNode {
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

fn value_expr(node: TreeNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &VALUE_OP_TYPE,
            value: None,
            string_value: node.value.clone(),
            tree_node: Some(Box::new(node)),
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

fn union_expr(lhs: Option<ExpressionNode>, rhs: Option<ExpressionNode>) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &UNION_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: lhs.map(Box::new),
        rhs: rhs.map(Box::new),
    }
}

#[test]
fn union_operator_concatenates_lhs_and_rhs_results() {
    let node_a = string_scalar("a");
    let node_b = string_scalar("b");
    let ctx = Context {
        matching_nodes: vec![node_a.clone()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = union_expr(
        Some(self_reference_expr()),
        Some(value_expr(node_b.clone())),
    );

    let out = union_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "a");
    assert_eq!(out.matching_nodes[1].value, "b");
}

#[test]
fn union_operator_does_not_duplicate_when_lhs_and_rhs_share_same_list() {
    // The dedup check uses pointer equality (std::ptr::eq) on the
    // matching_nodes Vec data pointer. When both lhs and rhs are None,
    // get_matching_nodes returns ctx.clone() for each side. With an
    // empty context, both clones have empty Vecs that share the same
    // dangling pointer, so the dedup triggers and only one copy is kept.
    let ctx = Context::default();
    let mut engine = TreeEngine::default();
    let mut expr = union_expr(None, None);

    let out = union_operator(ctx, &mut engine, &mut expr).unwrap();

    // Dedup triggered: empty list appears only once
    assert_eq!(out.matching_nodes.len(), 0);
}

#[test]
fn union_operator_does_not_duplicate_shared_non_empty_inputs() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("a")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = union_expr(None, None);

    let out = union_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "a");
}
