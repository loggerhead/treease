use treease_core::core::{
    ExpressionBuildError, Operation, OperationId, SemType, build_expression_tree_from_postfix_ops,
    query_cursor_exec, query_cursor_new, query_new, tree_sitter_language,
};
use treease_core::evaluator::{AllAtOnceEvaluator, EvaluationError, StreamEvaluator, Value};
use treease_core::parser::{
    LexerError, ParserError, ParticipleLexerError, Token, TokenKind, lex, parse_expression,
};
use treease_core::stream::{Meta, StreamingEvent};

#[test]
fn lexer_strips_line_comments_from_expression_input() {
    let tokens = lex("self # trailing comment\n + 1").unwrap();

    let lexemes: Vec<_> = tokens.iter().map(|token| token.lexeme.as_str()).collect();
    assert_eq!(lexemes, ["self", "+", "1"]);
}

#[test]
fn parser_preserves_binary_operator_precedence() {
    let tree = parse_expression("1 + 2 * 3").unwrap().unwrap();

    assert_eq!(tree.operation.operation_type.id, OperationId::Add);
    assert_eq!(
        tree.rhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::Multiply
    );
}

#[test]
fn parser_builds_pipe_between_dot_paths() {
    let tree = parse_expression(".a | .b").unwrap().unwrap();

    assert_eq!(tree.operation.operation_type.id, OperationId::Pipe);
    assert_eq!(
        tree.lhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::TraversePath
    );
    assert_eq!(tree.lhs.as_ref().unwrap().operation.string_value, "a");
    assert_eq!(
        tree.rhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::TraversePath
    );
    assert_eq!(tree.rhs.as_ref().unwrap().operation.string_value, "b");
}

#[test]
fn parser_builds_nested_traversal_for_assignment_lhs() {
    let tree = parse_expression(".a.b = \"new\"").unwrap().unwrap();

    assert_eq!(tree.operation.operation_type.id, OperationId::Assign);
    assert_eq!(
        tree.lhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::ShortPipe
    );
    assert_eq!(
        tree.lhs
            .as_ref()
            .unwrap()
            .lhs
            .as_ref()
            .unwrap()
            .operation
            .operation_type
            .id,
        OperationId::TraversePath
    );
    assert_eq!(
        tree.lhs
            .as_ref()
            .unwrap()
            .rhs
            .as_ref()
            .unwrap()
            .operation
            .string_value,
        "b"
    );
}

#[test]
fn parser_preserves_precedence_for_dot_path_math_expression() {
    let tree = parse_expression(".a + .b * .c").unwrap().unwrap();

    assert_eq!(tree.operation.operation_type.id, OperationId::Add);
    assert_eq!(
        tree.lhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::TraversePath
    );
    assert_eq!(tree.lhs.as_ref().unwrap().operation.string_value, "a");
    assert_eq!(
        tree.rhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::Multiply
    );
    assert_eq!(
        tree.rhs
            .as_ref()
            .unwrap()
            .lhs
            .as_ref()
            .unwrap()
            .operation
            .string_value,
        "b"
    );
    assert_eq!(
        tree.rhs
            .as_ref()
            .unwrap()
            .rhs
            .as_ref()
            .unwrap()
            .operation
            .string_value,
        "c"
    );
}

#[test]
fn parser_respects_parentheses_for_dot_path_math_expression() {
    let tree = parse_expression("(.a + .b) * .c").unwrap().unwrap();

    assert_eq!(tree.operation.operation_type.id, OperationId::Multiply);
    assert_eq!(
        tree.lhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::Add
    );
    assert_eq!(
        tree.lhs
            .as_ref()
            .unwrap()
            .lhs
            .as_ref()
            .unwrap()
            .operation
            .string_value,
        "a"
    );
    assert_eq!(
        tree.lhs
            .as_ref()
            .unwrap()
            .rhs
            .as_ref()
            .unwrap()
            .operation
            .string_value,
        "b"
    );
    assert_eq!(
        tree.rhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::TraversePath
    );
    assert_eq!(tree.rhs.as_ref().unwrap().operation.string_value, "c");
}

#[test]
fn query_cursor_exec_keeps_distinct_matches_with_same_pattern_index() {
    let language = tree_sitter_language("json").expect("json tree-sitter should be available");
    let query = query_new(
        &language,
        "(pair key: (string) @property value: (number) @number)",
    )
    .expect("query should compile");

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .expect("parser should accept json");
    let source = br#"{"a":1,"b":2}"#;
    let tree = parser.parse(source, None).expect("json should parse");

    let mut cursor = query_cursor_new();
    let matches = query_cursor_exec(&mut cursor, &query, tree.root_node(), source);

    assert_eq!(matches.len(), 2);
    assert!(
        matches
            .iter()
            .all(|query_match| query_match.pattern_index == 0)
    );
    assert_eq!(matches[0].captures.len(), 2);
    assert_eq!(matches[1].captures.len(), 2);
}

#[test]
fn parser_reports_missing_closing_parenthesis() {
    let error = parse_expression("(1 + 2").unwrap_err();

    assert_eq!(error, ParserError::MissingClosingDelimiter(')'));
}

#[test]
fn lexer_reports_unterminated_string_start_offset() {
    let error = lex("  \"missing end").unwrap_err();

    assert_eq!(error, LexerError::UnterminatedString { start: 2 });
}

#[test]
fn lexer_tokenizes_brackets_without_treating_them_as_operations() {
    let tokens = lex("[1, 2]").unwrap();

    assert!(matches!(tokens[0].kind, TokenKind::OpenBracket));
    assert!(matches!(tokens[2].kind, TokenKind::Comma));
    assert!(matches!(tokens[4].kind, TokenKind::CloseBracket));
}

#[test]
fn lexer_keeps_hash_inside_quoted_string() {
    let tokens = lex("\"#not_comment\" # trailing comment").unwrap();

    assert_eq!(tokens[0].lexeme, "\"#not_comment\"");
    assert_eq!(tokens.len(), 1);
}

#[test]
fn lexer_tracks_offsets_for_double_char_operators() {
    let tokens = lex("self == 10 != 11").unwrap();

    assert_eq!(tokens.len(), 5);
    assert_eq!(tokens[1].lexeme, "==");
    assert_eq!(tokens[1].start_offset, 5);
    assert_eq!(tokens[1].end_offset, 7);
    assert_eq!(tokens[3].lexeme, "!=");
    assert_eq!(tokens[3].start_offset, 11);
    assert_eq!(tokens[3].end_offset, 13);
}

#[test]
fn lexer_reports_unexpected_character_offset() {
    let error = lex("self ! true").unwrap_err();

    assert_eq!(
        error,
        LexerError::UnexpectedCharacter { offset: 5, ch: '!' }
    );
}

#[test]
fn parser_returns_none_for_empty_expression() {
    let tree = parse_expression(" \n\t ").unwrap();

    assert!(tree.is_none());
}

#[test]
fn parser_builds_unary_not_expression() {
    let tree = parse_expression("not false").unwrap().unwrap();

    assert_eq!(tree.operation.operation_type.id, OperationId::Not);
    assert_eq!(
        tree.rhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::Value
    );
}

#[test]
fn parser_wraps_lexer_errors() {
    let error = parse_expression("self ! true").unwrap_err();

    assert_eq!(
        error,
        ParserError::Lexer(LexerError::UnexpectedCharacter { offset: 5, ch: '!' })
    );
}

#[test]
fn parser_reports_unexpected_trailing_token() {
    let error = parse_expression("self self").unwrap_err();

    assert_eq!(error, ParserError::UnexpectedToken("self".to_string()));
}

#[test]
fn parser_reports_invalid_syntax_for_unclosed_dot_path_parenthesis() {
    let error = parse_expression("(.a").unwrap_err();

    assert_eq!(error, ParserError::MissingClosingDelimiter(')'));
}

#[test]
fn expression_builder_reports_invalid_postfix_stack_state() {
    let postfix = vec![Operation::value("a"), Operation::value("b")];
    let error = build_expression_tree_from_postfix_ops(&postfix).unwrap_err();

    assert_eq!(
        error,
        ExpressionBuildError::InvalidStackState { remaining: 2 }
    );
}

#[test]
fn expression_builder_returns_none_for_empty_postfix_input() {
    let tree = build_expression_tree_from_postfix_ops(&[]).unwrap();

    assert!(tree.is_none());
}

#[test]
fn expression_builder_reports_missing_rhs_operand() {
    let postfix = vec![Operation::binary(OperationId::Add, "+", 50)];
    let error = build_expression_tree_from_postfix_ops(&postfix).unwrap_err();

    assert_eq!(
        error,
        ExpressionBuildError::InsufficientOperands {
            operation: "add".to_string(),
            expected: 2,
            actual: 0,
        }
    );
}

#[test]
fn expression_builder_reports_missing_operand_for_non_first_unary_ops() {
    let postfix = vec![Operation::unary(OperationId::Not, "not", 70)];
    let error = build_expression_tree_from_postfix_ops(&postfix).unwrap_err();

    assert_eq!(
        error,
        ExpressionBuildError::InsufficientOperands {
            operation: "not".to_string(),
            expected: 1,
            actual: 0,
        }
    );
}

#[test]
fn expression_builder_builds_unary_tree_from_postfix() {
    let postfix = vec![
        Operation::value("true"),
        Operation::unary(OperationId::Not, "not", 70),
    ];
    let tree = build_expression_tree_from_postfix_ops(&postfix)
        .unwrap()
        .unwrap();

    assert_eq!(tree.operation.operation_type.id, OperationId::Not);
    assert!(tree.lhs.is_none());
    assert_eq!(
        tree.rhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::Value
    );
}

#[test]
fn parser_literal_value_operation_keeps_typed_tree_node_like_zig() {
    let tree = parse_expression("41").unwrap().unwrap();

    assert_eq!(tree.operation.operation_type.id, OperationId::Value);
    let node = tree
        .operation
        .tree_node
        .as_ref()
        .expect("value operation should keep parsed scalar node");
    assert_eq!(node.sem_type, Some(SemType::Int));
    assert_eq!(node.tag.as_str(), Some("!!int"));
    assert_eq!(
        node.get_value_rep_with("41").unwrap(),
        treease_core::core::ValueRep::Int(41)
    );
}

#[test]
fn expression_builder_reports_unsupported_arity() {
    let postfix = vec![Operation::custom("ternary", "ternary", 3, 10)];
    let error = build_expression_tree_from_postfix_ops(&postfix).unwrap_err();

    assert_eq!(
        error,
        ExpressionBuildError::UnsupportedArity {
            operation: "ternary".to_string(),
            arity: 3,
        }
    );
}

#[test]
fn evaluator_returns_input_when_expression_is_empty() {
    let input = Value::String("world".to_string());
    let result = AllAtOnceEvaluator::new().evaluate(&input, None).unwrap();

    assert_eq!(result, input);
}

#[test]
fn evaluator_applies_pipe_to_self_reference() {
    let tree = parse_expression("self | self + 1").unwrap().unwrap();
    let result = AllAtOnceEvaluator::new()
        .evaluate(&Value::Number(41.0), Some(&tree))
        .unwrap();

    assert_eq!(result, Value::Number(42.0));
}

#[test]
fn evaluator_reports_division_by_zero() {
    let tree = parse_expression("1 / 0").unwrap().unwrap();
    let error = AllAtOnceEvaluator::new()
        .evaluate(&Value::Null, Some(&tree))
        .unwrap_err();

    assert_eq!(error, EvaluationError::DivisionByZero);
}

#[test]
fn stream_evaluator_evaluates_scalar_events() {
    let expression = parse_expression("self + 1").unwrap().unwrap();
    let events = vec![StreamingEvent::Scalar {
        value: "41".to_string(),
        meta: Meta {
            sem_type: Some(SemType::Int),
            ..Meta::default()
        },
    }];
    let results = StreamEvaluator::new()
        .evaluate_events(Some(&expression), &events)
        .unwrap();

    assert_eq!(results, vec![Value::Number(42.0)]);
}

// ── Gap Group 1: expression_parser.zig error-detection tests ────────

#[test]
fn parser_reports_colon_on_its_own() {
    let error = parse_expression(":").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_reports_no_matching_close_bracket() {
    let error = parse_expression(".cat | with(.;.bob").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_reports_no_matching_close_collect() {
    let error = parse_expression("[1,2").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_reports_no_matching_close_object_in_collect() {
    let error = parse_expression("[{\"b\": \"c\"]").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_reports_no_matching_close_in_collect() {
    let error = parse_expression("[(.a]").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_reports_no_matching_close_collect_object() {
    let error = parse_expression("{\"a\": \"b\"").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_reports_no_matching_close_collect_in_collect_object() {
    let error = parse_expression("{\"b\": [1}").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_reports_no_matching_close_bracket_in_collect_object() {
    let error = parse_expression("{\"b\": (1}").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_reports_no_args_for_two_arg_op() {
    let error = parse_expression("=").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_reports_one_lhs_args_for_two_arg_op() {
    let error = parse_expression(".a =").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_reports_one_rhs_args_for_two_arg_op() {
    let error = parse_expression("= .a").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_accepts_two_args_for_two_arg_op_like_zig() {
    let tree = parse_expression(".a = .b").unwrap();

    assert!(tree.is_some());
}

#[test]
fn parser_reports_select_without_arguments_like_zig() {
    let error = parse_expression("select").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_accepts_one_arg_for_one_arg_op_like_zig() {
    let tree = parse_expression("select(.)").unwrap();

    assert!(tree.is_some());
}

#[test]
fn parser_binds_postfix_collect_to_whole_grouped_call_like_zig() {
    let tree = parse_expression("select(.)[]")
        .unwrap()
        .expect("tree should exist");

    assert_eq!(tree.operation.operation_type.id, OperationId::TraverseArray);
    assert_eq!(
        tree.lhs
            .as_ref()
            .expect("lhs should exist")
            .operation
            .operation_type
            .id,
        OperationId::Select
    );
    assert_eq!(
        tree.rhs
            .as_ref()
            .expect("rhs should exist")
            .operation
            .operation_type
            .id,
        OperationId::Collect
    );
}

#[test]
fn parser_binds_postfix_collect_to_whole_delpaths_call_like_zig() {
    let tree = parse_expression("delpaths([[\"a\", 0]])[]")
        .unwrap()
        .expect("tree should exist");

    assert_eq!(tree.operation.operation_type.id, OperationId::TraverseArray);
    assert_eq!(
        tree.lhs
            .as_ref()
            .expect("lhs should exist")
            .operation
            .operation_type
            .id,
        OperationId::DelPaths
    );
    assert_eq!(
        tree.rhs
            .as_ref()
            .expect("rhs should exist")
            .operation
            .operation_type
            .id,
        OperationId::Collect
    );
}

#[test]
fn parser_reports_extra_args_without_pipe_like_zig() {
    let error = parse_expression("sortKeys(.) select(.)").unwrap_err();

    assert_eq!(error, ParserError::InvalidSyntax);
}

#[test]
fn parser_returns_tree_for_single_dot_operation() {
    let tree = parse_expression(".").unwrap();

    assert!(tree.is_some());
    let tree = tree.unwrap();
    assert!(!tree.operation.to_string_name().is_empty());
}

#[test]
fn token_string_uses_value_name_for_string_interpolation_like_zig() {
    let token = Token {
        kind: TokenKind::Operation(Operation::new(
            OperationId::StringInterp,
            r#"value=\(.a)"#,
            0,
            50,
        )),
        lexeme: r#"value=\(.a)"#.to_string(),
        start_offset: 0,
        end_offset: 11,
        check_for_post_traverse: false,
        assign_operation: None,
    };

    assert_eq!(token.to_string(false), "value");
}

#[test]
fn parser_returns_tree_for_first_op_with_zero_args() {
    let tree = parse_expression("first").unwrap();

    assert!(tree.is_some());
    assert_eq!(
        tree.unwrap().operation.operation_type.id,
        OperationId::First
    );
}

#[test]
fn parser_reports_invalid_postfix_with_multiple_roots_like_zig() {
    let postfix = vec![
        Operation::custom("RAW", "a", 0, 0),
        Operation::custom("RAW", "b", 0, 0),
    ];
    let error = build_expression_tree_from_postfix_ops(&postfix).unwrap_err();

    assert_eq!(
        error,
        ExpressionBuildError::InvalidStackState { remaining: 2 }
    );
}

#[test]
fn parser_reports_unknown_token_for_explode_like_zig() {
    let error = parse_expression("explode").unwrap_err();

    assert_eq!(
        error,
        ParserError::ParticipleLexer(ParticipleLexerError::UnknownToken {
            offset: 0,
            lexeme: "explode".to_string(),
        })
    );
}

// ── Gap Group 2: expression_builder.zig ─────────────────────────────

#[test]
fn expression_builder_builds_binary_tree_from_postfix() {
    let postfix = vec![
        Operation::value("1"),
        Operation::value("2"),
        Operation::binary(OperationId::Add, "+", 42),
    ];
    let tree = build_expression_tree_from_postfix_ops(&postfix)
        .unwrap()
        .unwrap();

    assert_eq!(tree.operation.operation_type.id, OperationId::Add);
    assert_eq!(
        tree.lhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::Value
    );
    assert_eq!(tree.lhs.as_ref().unwrap().operation.string_value, "1");
    assert_eq!(
        tree.rhs.as_ref().unwrap().operation.operation_type.id,
        OperationId::Value
    );
    assert_eq!(tree.rhs.as_ref().unwrap().operation.string_value, "2");
}
