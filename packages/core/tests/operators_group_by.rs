use treease_core::operators::group_by::group_by;
use treease_core::operators::{
    Context, ExpressionNode, GROUP_BY_OP_TYPE, NodeKind, Operation, SemType, TraversePreferences,
    TreeEngine, TreeNode, create_traversal_tree,
};

fn group_by_expression(rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &GROUP_BY_OP_TYPE,
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
fn group_by_operator_groups_sequence_by_rhs_key() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("foo", int_scalar(1)), ("bar", string_scalar("a"))]),
            mapping(vec![("foo", int_scalar(3)), ("bar", string_scalar("b"))]),
            mapping(vec![("foo", int_scalar(1)), ("bar", string_scalar("c"))]),
        ])],
        ..Context::default()
    };
    let rhs = create_traversal_tree(
        &[string_scalar("foo")],
        TraversePreferences::default(),
        false,
    )
    .unwrap();
    let mut expr = group_by_expression(*rhs);
    let mut engine = TreeEngine::default();

    let out = group_by(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let grouped = &out.matching_nodes[0];
    assert_eq!(grouped.kind, NodeKind::Sequence);
    assert_eq!(grouped.content.len(), 2);
    assert_eq!(grouped.content[0].content.len(), 2);
    assert_eq!(grouped.content[0].content[0].content[3].value, "a");
    assert_eq!(grouped.content[0].content[1].content[3].value, "c");
    assert_eq!(grouped.content[1].content.len(), 1);
    assert_eq!(grouped.content[1].content[0].content[3].value, "b");
}
