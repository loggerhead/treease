use treease_core::operators::sort_keys::sort_keys_operator;
use treease_core::operators::{
    Context, ExpressionNode, NodeKind, Operation, SELF_REFERENCE_OP_TYPE, SORT_KEYS_OP_TYPE,
    SemType, TreeEngine, TreeNode,
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

fn sort_keys_expression(rhs: Option<ExpressionNode>) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &SORT_KEYS_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: rhs.map(Box::new),
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

#[test]
fn sort_keys_operator_sorts_map_keys_alphabetically() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("c", string_scalar("frog")),
            ("a", string_scalar("blah")),
            ("b", string_scalar("bing")),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = sort_keys_expression(Some(self_expression()));

    let out = sort_keys_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let map = &out.matching_nodes[0];
    assert_eq!(map.content.len(), 6);
    assert_eq!(map.content[0].value, "a");
    assert_eq!(map.content[2].value, "b");
    assert_eq!(map.content[4].value, "c");
}

#[test]
fn sort_keys_operator_on_empty_map() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = sort_keys_expression(Some(self_expression()));

    let out = sort_keys_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert!(out.matching_nodes[0].content.is_empty());
}

#[test]
fn sort_keys_operator_on_single_entry_map() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("z", string_scalar("only"))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = sort_keys_expression(Some(self_expression()));

    let out = sort_keys_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "z");
}

#[test]
fn sort_keys_operator_preserves_values() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("b", string_scalar("second")),
            ("a", string_scalar("first")),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = sort_keys_expression(Some(self_expression()));

    let out = sort_keys_operator(ctx, &mut engine, &mut expr).unwrap();

    let map = &out.matching_nodes[0];
    assert_eq!(map.content[0].value, "a");
    assert_eq!(map.content[1].value, "first");
    assert_eq!(map.content[2].value, "b");
    assert_eq!(map.content[3].value, "second");
}
