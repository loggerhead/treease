use treease_core::core::{
    OperationId as CoreOperationId,
    operation_prefs::OperationPreferences as CoreOperationPreferences,
};
use treease_core::operators::relational::{max_operator, min_operator, relational_operator};
use treease_core::operators::{
    Context, ExpressionNode, MAX_OP_TYPE, MIN_OP_TYPE, NodeKind, Operation, OperationPreference,
    RELATIONAL_OP_TYPE, RelationalPref, SemType, TreeEngine, TreeNode, create_value_operation,
};
use treease_core::parser::lex_participle;
use treease_core::parser::lexer::TokenKind;

fn expression(operation: &'static treease_core::operators::OperationType) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: operation,
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

fn relational_expression(lhs: TreeNode, rhs: TreeNode, prefs: RelationalPref) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &RELATIONAL_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Relational(prefs))),
            update_assign: false,
        }),
        lhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(lhs)).unwrap(),
            lhs: None,
            rhs: None,
        })),
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

fn int_scalar(value: i64) -> TreeNode {
    scalar(SemType::Int, &value.to_string())
}

fn float_scalar(value: &str) -> TreeNode {
    scalar(SemType::Float, value)
}

fn string_scalar(value: &str) -> TreeNode {
    scalar(SemType::Str, value)
}

fn null_scalar() -> TreeNode {
    scalar(SemType::Nil, "null")
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
fn relational_operator_supports_int_float_string_and_null() {
    let mut engine = TreeEngine::default();

    let mut ints = relational_expression(
        int_scalar(5),
        int_scalar(4),
        RelationalPref {
            greater: true,
            or_equal: false,
        },
    );
    let mut floats = relational_expression(
        float_scalar("5.2"),
        float_scalar("5.2"),
        RelationalPref {
            greater: true,
            or_equal: true,
        },
    );
    let mut strings = relational_expression(
        string_scalar("zoo"),
        string_scalar("apple"),
        RelationalPref {
            greater: true,
            or_equal: false,
        },
    );
    let mut nulls = relational_expression(
        null_scalar(),
        null_scalar(),
        RelationalPref {
            greater: false,
            or_equal: true,
        },
    );

    let empty = Context::default();
    assert_eq!(
        relational_operator(empty.clone(), &mut engine, &mut ints)
            .unwrap()
            .matching_nodes[0]
            .value,
        "true"
    );
    assert_eq!(
        relational_operator(empty.clone(), &mut engine, &mut floats)
            .unwrap()
            .matching_nodes[0]
            .value,
        "true"
    );
    assert_eq!(
        relational_operator(empty.clone(), &mut engine, &mut strings)
            .unwrap()
            .matching_nodes[0]
            .value,
        "true"
    );
    assert_eq!(
        relational_operator(empty, &mut engine, &mut nulls)
            .unwrap()
            .matching_nodes[0]
            .value,
        "true"
    );
}

#[test]
fn min_and_max_operator_pick_extrema_from_sequence() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            int_scalar(99),
            int_scalar(16),
            int_scalar(12),
            int_scalar(6),
            int_scalar(66),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut min_expr = expression(&MIN_OP_TYPE);
    let mut max_expr = expression(&MAX_OP_TYPE);

    let min_out = min_operator(ctx.clone(), &mut engine, &mut min_expr).unwrap();
    let max_out = max_operator(ctx, &mut engine, &mut max_expr).unwrap();

    assert_eq!(min_out.matching_nodes.len(), 1);
    assert_eq!(min_out.matching_nodes[0].value, "6");
    assert_eq!(max_out.matching_nodes.len(), 1);
    assert_eq!(max_out.matching_nodes[0].value, "99");
}

#[test]
fn lexer_parses_relational_operators() {
    let tokens = lex_participle(".a <= .b").unwrap();

    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0].lexeme, "a");
    assert_eq!(tokens[1].lexeme, "<=");
    assert_eq!(tokens[2].lexeme, "b");

    match &tokens[0].kind {
        TokenKind::Operation(operation) => {
            assert_eq!(operation.operation_type.id, CoreOperationId::TraversePath);
        }
        other => panic!("expected traverse-path token, got {other:?}"),
    }

    match &tokens[1].kind {
        TokenKind::Operation(operation) => {
            assert_eq!(operation.operation_type.id, CoreOperationId::Relational);
            let Some(preferences) = operation.preferences.as_deref() else {
                panic!("expected relational preferences");
            };
            match preferences {
                CoreOperationPreferences::Relational(pref) => {
                    assert!(!pref.greater);
                    assert!(pref.or_equal);
                }
                other => panic!("expected relational preference, got {other:?}"),
            }
        }
        other => panic!("expected relational token, got {other:?}"),
    }

    match &tokens[2].kind {
        TokenKind::Operation(operation) => {
            assert_eq!(operation.operation_type.id, CoreOperationId::TraversePath);
        }
        other => panic!("expected traverse-path token, got {other:?}"),
    }
}

#[test]
fn lexer_parses_min_max_keywords() {
    let tokens = lex_participle("min").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].lexeme, "min");

    let tokens = lex_participle("max").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].lexeme, "max");
}
