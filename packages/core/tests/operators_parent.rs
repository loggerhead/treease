use treease_core::operators::parent::{get_parent_operator, get_parents_operator};
use treease_core::operators::{
    Context, ExpressionNode, GET_PARENT_OP_TYPE, GET_PARENTS_OP_TYPE, NodeId, NodeKind, Operation,
    OperationPreference, ParentOpPreferences, SemType, TreeEngine, TreeNode,
};

fn expression(
    operation: &'static treease_core::operators::OperationType,
    string_value: &str,
) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: operation,
            value: None,
            string_value: string_value.to_owned(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

fn map_node(parent: Option<NodeId>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: "object".to_owned(),
        parent,
        ..TreeNode::default()
    }
}

fn string_scalar(value: &str, parent: Option<NodeId>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: "string".to_owned(),
        value: value.to_owned(),
        parent,
        ..TreeNode::default()
    }
}

#[test]
fn get_parents_operator_returns_all_ancestors() {
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(map_node(None));
    let a_id = engine.store.add(map_node(Some(root_id)));
    let b_id = engine.store.add(map_node(Some(a_id)));
    let leaf = string_scalar("cat", Some(b_id));
    let ctx = Context {
        matching_nodes: vec![leaf],
        ..Context::default()
    };
    let mut expr = expression(&GET_PARENTS_OP_TYPE, "");

    let out = get_parents_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 3);
    assert_eq!(out.matching_nodes[0].content[0].parent, Some(a_id));
    assert_eq!(out.matching_nodes[0].content[1].parent, Some(root_id));
    assert_eq!(out.matching_nodes[0].content[2].parent, None);
}

#[test]
fn get_parent_operator_supports_positive_and_negative_levels() {
    let mut engine = TreeEngine::default();
    let root_id = engine.store.add(map_node(None));
    let a_id = engine.store.add(map_node(Some(root_id)));
    let b_id = engine.store.add(map_node(Some(a_id)));
    let leaf = string_scalar("cat", Some(b_id));
    let ctx = Context {
        matching_nodes: vec![leaf],
        ..Context::default()
    };

    let mut expr_level_one = expression(&GET_PARENT_OP_TYPE, "1");
    expr_level_one.operation.preferences =
        Some(Box::new(OperationPreference::Parent(ParentOpPreferences {
            level: 1,
        })));
    let out_one = get_parent_operator(ctx.clone(), &mut engine, &mut expr_level_one).unwrap();
    assert_eq!(out_one.matching_nodes.len(), 1);
    assert_eq!(out_one.matching_nodes[0].parent, Some(a_id));

    let mut expr_negative = expression(&GET_PARENT_OP_TYPE, "-1");
    expr_negative.operation.preferences =
        Some(Box::new(OperationPreference::Parent(ParentOpPreferences {
            level: -1,
        })));
    let out_root = get_parent_operator(ctx, &mut engine, &mut expr_negative).unwrap();
    assert_eq!(out_root.matching_nodes.len(), 1);
    assert_eq!(out_root.matching_nodes[0].parent, None);
}
