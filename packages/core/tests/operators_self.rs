use treease_core::operators::operator_helpers::identity_operator;
use treease_core::operators::{
    Context, ExpressionNode, NodeKind, Operation, SELF_REFERENCE_OP_TYPE, SemType, TreeEngine,
    TreeNode,
};

fn expression() -> ExpressionNode {
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

fn string_scalar(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: SemType::Str.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

#[test]
fn self_operator_returns_context_unchanged() {
    let root = string_scalar("root");
    let mut ctx = Context {
        matching_nodes: vec![root.clone()],
        ..Context::default()
    };
    ctx.set_variable("x", vec![root.clone()]).unwrap();
    let mut engine = TreeEngine::default();
    let mut expr = expression();

    let out = identity_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "root");
    assert_eq!(out.get_variable("x").unwrap().len(), 1);
    assert_eq!(out.get_variable("x").unwrap()[0].value, "root");
}
