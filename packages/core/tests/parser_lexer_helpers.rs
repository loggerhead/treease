use treease_core::core::OperationId;
use treease_core::parser::{
    ParameterError, TokenKind, extract_number_parameter, has_option_parameter,
    make_operation_token, make_simple_token, post_process_tokens, unwrap,
};

#[test]
fn lexer_unwrap_handles_quoted_and_short_inputs() {
    assert_eq!(unwrap("\"abc\""), "abc");
    assert_eq!(unwrap("'x'"), "x");
    assert_eq!(unwrap("\"a\\\"b\""), "a\"b");
    assert_eq!(unwrap("\"\""), "");
    assert_eq!(unwrap("a"), "");
    assert_eq!(unwrap(""), "");
}

#[test]
fn lexer_extract_number_parameter_parses_and_rejects_invalid_inputs() {
    assert_eq!(extract_number_parameter("slice(3)").unwrap(), 3);
    assert_eq!(
        extract_number_parameter("slice").unwrap_err(),
        ParameterError::MissingParameter
    );
    assert_eq!(
        extract_number_parameter("slice()").unwrap_err(),
        ParameterError::MissingParameter
    );
    assert_eq!(
        extract_number_parameter("slice(a)").unwrap_err(),
        ParameterError::InvalidCharacter
    );
}

#[test]
fn lexer_has_option_parameter_detects_options() {
    assert!(has_option_parameter("sort_keys(ignorecase)", "ignorecase"));
    assert!(has_option_parameter(
        "sort_keys(ignorecase, numeric)",
        "numeric"
    ));
    assert!(!has_option_parameter("sort_keys()", "ignorecase"));
    assert!(!has_option_parameter("sort_keys", "ignorecase"));
    assert!(!has_option_parameter("sort_keys(ignorecase", "ignorecase"));
    assert!(has_option_parameter(
        "sort_keys( ignorecase  , numeric )",
        "numeric"
    ));
}

#[test]
fn lexer_post_process_expands_traverse_array_collect_and_empty() {
    let input = vec![
        make_simple_token(TokenKind::TraverseArrayCollect, ".[", 0, 2),
        make_simple_token(TokenKind::CloseBracket, "]", 2, 3),
    ];

    let out = post_process_tokens(&input);

    assert_eq!(out.len(), 5);
    assert_eq!(out[0].lexeme, "self");
    assert_eq!(out[1].lexeme, "traverse_array");
    assert!(matches!(out[2].kind, TokenKind::OpenBracket));
    assert_eq!(out[3].lexeme, "empty");
    assert!(matches!(out[4].kind, TokenKind::CloseBracket));
}

#[test]
fn lexer_post_process_inserts_short_pipe_after_post_traverse_tokens() {
    let mut first = make_operation_token(
        treease_core::core::Operation::new(OperationId::Select, "select", 1, 52),
        0,
        6,
    );
    first.check_for_post_traverse = true;
    let second = make_operation_token(
        treease_core::core::Operation::new(OperationId::TraversePath, "traverse_path", 0, u32::MAX),
        6,
        8,
    );

    let out = post_process_tokens(&[first, second]);

    assert_eq!(out.len(), 3);
    assert_eq!(out[0].lexeme, "select");
    assert_eq!(out[1].lexeme, "short_pipe");
    assert_eq!(out[2].lexeme, "traverse_path");
}

#[test]
fn lexer_post_process_expands_empty_object_collect() {
    let input = vec![
        make_simple_token(TokenKind::OpenCollectObject, "{", 0, 1),
        make_simple_token(TokenKind::CloseCollectObject, "}", 1, 2),
    ];

    let out = post_process_tokens(&input);

    assert_eq!(out.len(), 3);
    assert!(matches!(out[0].kind, TokenKind::OpenCollectObject));
    assert_eq!(out[1].lexeme, "empty");
    assert!(matches!(out[2].kind, TokenKind::CloseCollectObject));
}

#[test]
fn lexer_post_process_handles_grouped_expression_collect_like_zig() {
    let mut close_paren = make_simple_token(TokenKind::CloseParen, ")", 10, 11);
    close_paren.check_for_post_traverse = true;
    let input = vec![
        close_paren,
        make_simple_token(TokenKind::OpenBracket, "[", 11, 12),
        make_simple_token(TokenKind::CloseBracket, "]", 12, 13),
    ];

    let out = post_process_tokens(&input);

    assert_eq!(out.len(), 5);
    assert!(matches!(out[0].kind, TokenKind::CloseParen));
    assert_eq!(out[1].lexeme, "traverse_array");
    assert!(matches!(out[2].kind, TokenKind::OpenBracket));
    assert_eq!(out[3].lexeme, "empty");
    assert!(matches!(out[4].kind, TokenKind::CloseBracket));
}
