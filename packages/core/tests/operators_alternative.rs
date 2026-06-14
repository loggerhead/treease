use treease_core::core::OperationId;
use treease_core::evaluator::{AllAtOnceEvaluator, Value};
use treease_core::parser::{TokenKind, lex, parse_expression};

#[test]
fn lexer_recognizes_double_slash_as_alternative_operator() {
    let tokens = lex("1 // 2 * 3").unwrap();

    assert_eq!(tokens.len(), 5);
    match &tokens[1].kind {
        TokenKind::Operation(operation) => {
            assert_eq!(operation.operation_type.id, OperationId::Alternative);
            assert_eq!(tokens[1].lexeme, "//");
        }
        _ => panic!("expected alternative operator token"),
    }
}

#[test]
fn parser_and_evaluator_support_alternative_operator() {
    let null_fallback = parse_expression("null // 1").unwrap().unwrap();
    let null_result = AllAtOnceEvaluator::new()
        .evaluate(&Value::Null, Some(&null_fallback))
        .unwrap();
    assert_eq!(null_result, Value::Number(1.0));

    let false_fallback = parse_expression("false // 7").unwrap().unwrap();
    let false_result = AllAtOnceEvaluator::new()
        .evaluate(&Value::Null, Some(&false_fallback))
        .unwrap();
    assert_eq!(false_result, Value::Number(7.0));

    let true_keeps_lhs = parse_expression("true // 7").unwrap().unwrap();
    let true_result = AllAtOnceEvaluator::new()
        .evaluate(&Value::Null, Some(&true_keeps_lhs))
        .unwrap();
    assert_eq!(true_result, Value::Bool(true));

    let zero_fallback = parse_expression("0 // 2").unwrap().unwrap();
    let zero_result = AllAtOnceEvaluator::new()
        .evaluate(&Value::Null, Some(&zero_fallback))
        .unwrap();
    assert_eq!(zero_result, Value::Number(2.0));

    let left_truthy = parse_expression("1 // 2 * 3").unwrap().unwrap();
    let truthy_result = AllAtOnceEvaluator::new()
        .evaluate(&Value::Null, Some(&left_truthy))
        .unwrap();
    assert_eq!(truthy_result, Value::Number(1.0));

    let divide_then_alternative = parse_expression("6 / 3 // 0").unwrap().unwrap();
    let divide_result = AllAtOnceEvaluator::new()
        .evaluate(&Value::Null, Some(&divide_then_alternative))
        .unwrap();
    assert_eq!(divide_result, Value::Number(2.0));
}
