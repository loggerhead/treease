use treease_core::operators::expression::expression_operator;
use treease_core::operators::{
    Context, CoreError, EXPRESSION_OP_TYPE, ExpressionNode, ExpressionOpPreferences, NodeKind,
    Operation, OperationPreference, ParseError, SemType, TreeEngine, TreeNode,
};

fn expression_with_source(source: &str) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &EXPRESSION_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Expression(
                ExpressionOpPreferences {
                    expression: source.to_owned(),
                },
            ))),
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

#[test]
fn expression_operator_executes_expression_against_matching_node() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", string_scalar("cat"))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_source(".a");

    let out = expression_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cat");
}

#[test]
fn expression_operator_returns_invalid_syntax_for_bad_expression() {
    let ctx = Context {
        matching_nodes: vec![string_scalar("ignored")],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_source("(");

    let err = expression_operator(ctx, &mut engine, &mut expr).unwrap_err();

    assert!(matches!(err, CoreError::Parse(ParseError::InvalidSyntax)));
}
