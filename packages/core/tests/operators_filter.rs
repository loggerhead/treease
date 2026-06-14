use treease_core::expression_pipeline;
use treease_core::operators::filter::filter_operator;
use treease_core::operators::{
    Context, ExpressionNode, FILTER_OP_TYPE, NodeKind, Operation, OperationPreference,
    RELATIONAL_OP_TYPE, RelationalPref, SELF_REFERENCE_OP_TYPE, SemType, TreeEngine, TreeNode,
    create_value_operation,
};

fn filter_expression(rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &FILTER_OP_TYPE,
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

fn less_than_rhs(limit: i64) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &RELATIONAL_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Relational(RelationalPref {
                greater: false,
                or_equal: false,
            }))),
            update_assign: false,
        }),
        lhs: Some(Box::new(self_expression())),
        rhs: Some(Box::new(value_expression(int_scalar(limit)))),
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

fn int_scalar(value: i64) -> TreeNode {
    scalar(SemType::Int, &value.to_string())
}

fn string_scalar(value: &str) -> TreeNode {
    scalar(SemType::Str, value)
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
fn filter_operator_filters_sequence_items_by_relational_rhs() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)])],
        ..Context::default()
    };
    let mut expr = filter_expression(less_than_rhs(3));
    let mut engine = TreeEngine::default();

    let out = filter_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.kind, NodeKind::Sequence);
    assert_eq!(seq.content.len(), 2);
    assert_eq!(seq.content[0].value, "1");
    assert_eq!(seq.content[1].value, "2");
}

#[test]
fn filter_operator_returns_empty_sequence_when_nothing_matches() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)])],
        ..Context::default()
    };
    let mut expr = filter_expression(less_than_rhs(0));
    let mut engine = TreeEngine::default();

    let out = filter_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.kind, NodeKind::Sequence);
    assert!(seq.content.is_empty());
    assert_eq!(seq.value, "[]");
}

#[test]
fn filter_operator_keeps_only_nested_cool_mapping() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            (
                "c",
                mapping(vec![
                    ("things", string_scalar("cool")),
                    ("frog", string_scalar("yes")),
                ]),
            ),
            (
                "d",
                mapping(vec![
                    ("things", string_scalar("hot")),
                    ("frog", string_scalar("false")),
                ]),
            ),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "filter(.things == \"cool\")")
            .expect("filter nested equality pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 1);
    let kept = &out.matching_nodes[0].content[0];
    assert_eq!(kept.content[0].value, "things");
    assert_eq!(kept.content[1].value, "cool");
    assert_eq!(kept.content[2].value, "frog");
    assert_eq!(kept.content[3].value, "yes");
}

#[test]
fn filter_expression_supports_splat_output() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "filter(. < 3)[]")
        .expect("filter splat pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "1");
    assert_eq!(out.matching_nodes[1].value, "2");
}

#[test]
fn filter_expression_keeps_values_less_than_three() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "filter(. < 3)")
        .expect("filter less-than pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "1");
    assert_eq!(out.matching_nodes[0].content[1].value, "2");
}

#[test]
fn filter_expression_keeps_only_nested_cool_mapping() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            (
                "c",
                mapping(vec![
                    ("things", string_scalar("cool")),
                    ("frog", string_scalar("yes")),
                ]),
            ),
            (
                "d",
                mapping(vec![
                    ("things", string_scalar("hot")),
                    ("frog", string_scalar("false")),
                ]),
            ),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "filter(.things == \"cool\")")
            .expect("filter nested equality pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 1);
    let kept = &out.matching_nodes[0].content[0];
    assert_eq!(kept.content[0].value, "things");
    assert_eq!(kept.content[1].value, "cool");
    assert_eq!(kept.content[2].value, "frog");
    assert_eq!(kept.content[3].value, "yes");
}

#[test]
fn filter_expression_returns_empty_when_threshold_excludes_all() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "filter(. > 4)")
        .expect("filter greater-than-empty pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert!(out.matching_nodes[0].content.is_empty());
}

#[test]
fn filter_expression_keeps_values_greater_than_one() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(1), int_scalar(2), int_scalar(3)])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "filter(. > 1)")
        .expect("filter greater-than pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "2");
    assert_eq!(out.matching_nodes[0].content[1].value, "3");
}

#[test]
fn filter_expression_keeps_empty_array_empty() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "filter(. > 1)")
        .expect("filter empty array pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert!(out.matching_nodes[0].content.is_empty());
}

#[test]
fn filter_expression_supports_equality_conditions() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            string_scalar("cat"),
            string_scalar("dog"),
            string_scalar("dog"),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "filter(. == \"dog\")")
        .expect("filter equality pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "dog");
    assert_eq!(out.matching_nodes[0].content[1].value, "dog");
}
