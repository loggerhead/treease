use treease_core::operators::omit::omit_operator;
use treease_core::operators::{
    Context, ExpressionNode, NodeKind, OMIT_OP_TYPE, Operation, SemType, TreeEngine, TreeNode,
    create_value_operation,
};

fn expression_with_rhs(rhs: TreeNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &OMIT_OP_TYPE,
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
    scalar(SemType::Str, value)
}

fn int_scalar(value: i64) -> TreeNode {
    scalar(SemType::Int, &value.to_string())
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
fn omit_operator_omits_mapping_keys_and_keeps_leading_content() {
    let mut node = mapping(vec![
        ("cat", string_scalar("meow")),
        ("dog", string_scalar("bark")),
        ("hamster", string_scalar("squeak")),
    ]);
    node.leading_content = "# abc\n".to_owned();

    let ctx = Context {
        matching_nodes: vec![node],
        ..Context::default()
    };
    let rhs = sequence(vec![string_scalar("dog")]);
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_rhs(rhs);

    let out = omit_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let map = &out.matching_nodes[0];
    assert_eq!(map.leading_content, "# abc\n");
    assert_eq!(map.content.len(), 4);
    assert_eq!(map.content[0].value, "cat");
    assert_eq!(map.content[1].value, "meow");
    assert_eq!(map.content[2].value, "hamster");
    assert_eq!(map.content[3].value, "squeak");
}

#[test]
fn omit_operator_omits_sequence_indices() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            string_scalar("cat"),
            string_scalar("leopard"),
            string_scalar("lion"),
        ])],
        ..Context::default()
    };
    let rhs = sequence(vec![int_scalar(2), int_scalar(0)]);
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_rhs(rhs);

    let out = omit_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.content.len(), 1);
    assert_eq!(seq.content[0].value, "leopard");
}
