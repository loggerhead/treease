use treease_core::expression_pipeline;
use treease_core::operators::tag::get_tag_operator;
use treease_core::operators::value::value_operator;
use treease_core::operators::{
    Context, ExpressionNode, GET_TAG_OP_TYPE, NodeKind, Operation, SemType, TreeEngine, TreeNode,
    VALUE_OP_TYPE,
};

fn expression(
    operation: &'static treease_core::operators::OperationType,
    tree_node: Option<TreeNode>,
) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: operation,
            value: None,
            string_value: String::new(),
            tree_node: tree_node.map(Box::new),
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

fn string_scalar(value: &str, tag: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: tag.to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

#[test]
fn get_tag_operator_returns_tag_strings() {
    let ctx = Context {
        matching_nodes: vec![
            string_scalar("cat", "!things"),
            string_scalar("dog", "!!str"),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&GET_TAG_OP_TYPE, None);

    let out = get_tag_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].tag, "!!str");
    assert_eq!(out.matching_nodes[0].value, "!things");
    assert_eq!(out.matching_nodes[1].value, "!!str");
}

#[test]
fn value_operator_clones_value_for_each_input_match_and_for_empty_ctx() {
    let template = string_scalar("cat", "string");
    let ctx = Context {
        matching_nodes: vec![string_scalar("a", "string"), string_scalar("b", "string")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&VALUE_OP_TYPE, Some(template.clone()));

    let out = value_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "cat");
    assert_eq!(out.matching_nodes[1].value, "cat");
    assert_eq!(out.matching_nodes[0].tag, "string");
    assert_eq!(out.matching_nodes[1].tag, "string");

    let empty_ctx = Context::default();
    let mut empty_expr = expression(&VALUE_OP_TYPE, Some(template));
    let empty_out = value_operator(empty_ctx, &mut engine, &mut empty_expr).unwrap();
    assert_eq!(empty_out.matching_nodes.len(), 1);
    assert_eq!(empty_out.matching_nodes[0].value, "cat");
}

#[test]
fn get_tag_operator_on_key_returns_str_tag() {
    let key_node = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: "!!str".to_owned(),
        value: "mykey".to_owned(),
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![key_node],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&GET_TAG_OP_TYPE, None);

    let out = get_tag_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "!!str");
}

#[test]
fn get_tag_operator_returns_tags_for_multiple_node_types() {
    let ctx = Context {
        matching_nodes: vec![
            TreeNode {
                kind: NodeKind::Mapping,
                sem_type: Some(SemType::Map),
                tag: "!!map".to_owned(),
                ..TreeNode::default()
            },
            TreeNode {
                kind: NodeKind::Scalar,
                sem_type: Some(SemType::Str),
                tag: "!!str".to_owned(),
                value: "hello".to_owned(),
                ..TreeNode::default()
            },
            TreeNode {
                kind: NodeKind::Scalar,
                sem_type: Some(SemType::Int),
                tag: "!!int".to_owned(),
                value: "42".to_owned(),
                ..TreeNode::default()
            },
            TreeNode {
                kind: NodeKind::Scalar,
                sem_type: Some(SemType::Float),
                tag: "!!float".to_owned(),
                value: "3.14".to_owned(),
                ..TreeNode::default()
            },
            TreeNode {
                kind: NodeKind::Scalar,
                sem_type: Some(SemType::Boolean),
                tag: "!!bool".to_owned(),
                value: "true".to_owned(),
                ..TreeNode::default()
            },
            TreeNode {
                kind: NodeKind::Sequence,
                sem_type: Some(SemType::Seq),
                tag: "!!seq".to_owned(),
                ..TreeNode::default()
            },
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&GET_TAG_OP_TYPE, None);

    let out = get_tag_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 6);
    assert_eq!(out.matching_nodes[0].value, "!!map");
    assert_eq!(out.matching_nodes[1].value, "!!str");
    assert_eq!(out.matching_nodes[2].value, "!!int");
    assert_eq!(out.matching_nodes[3].value, "!!float");
    assert_eq!(out.matching_nodes[4].value, "!!bool");
    assert_eq!(out.matching_nodes[5].value, "!!seq");
}

#[test]
fn get_tag_operator_on_map_returns_map_tag() {
    let map_node = TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: "!!map".to_owned(),
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![map_node],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&GET_TAG_OP_TYPE, None);

    let out = get_tag_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "!!map");
}

#[test]
fn expression_pipeline_treats_type_as_alias_for_tag() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("cat", "!things")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let type_out = expression_pipeline::execute_on_context(&mut engine, &ctx, "type")
        .expect("type alias should succeed");
    let tag_out = expression_pipeline::execute_on_context(&mut engine, &ctx, "tag")
        .expect("tag should succeed");

    assert_eq!(type_out.matching_nodes.len(), 1);
    assert_eq!(tag_out.matching_nodes.len(), 1);
    assert_eq!(type_out.matching_nodes[0].value, "!things");
    assert_eq!(tag_out.matching_nodes[0].value, "!things");
    assert_eq!(
        type_out.matching_nodes[0].value,
        tag_out.matching_nodes[0].value
    );
}

#[test]
fn expression_pipeline_tag_on_root_map_matches_zig_root_tag_scenario() {
    let ctx = Context {
        matching_nodes: vec![TreeNode {
            kind: NodeKind::Mapping,
            sem_type: Some(SemType::Map),
            tag: "!!map".to_owned(),
            ..TreeNode::default()
        }],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "tag")
        .expect("root tag expression should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].tag, "!!str");
    assert_eq!(out.matching_nodes[0].value, "!!map");
}
