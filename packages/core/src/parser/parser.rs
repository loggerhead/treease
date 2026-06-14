use std::cell::RefCell;
use std::rc::Rc;

use crate::core::diagnostics::{DiagnosticStage, Diagnostics, ParseErrorInfo};
use crate::core::expression::{ExpressionNode, Operation, OperationId};
use crate::core::{
    ExpressionBuildError, ParsedKey, build_expression_tree_from_postfix_ops,
    build_traversal_expression,
};

use super::lexer::{LexerError, Token, TokenKind};
use super::lexer_participle::{self, ParticipleLexerError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParserError {
    Lexer(LexerError),
    ParticipleLexer(ParticipleLexerError),
    Build(ExpressionBuildError),
    UnexpectedEnd,
    UnexpectedToken(String),
    MissingClosingDelimiter(char),
    InvalidSyntax,
}

impl From<LexerError> for ParserError {
    fn from(value: LexerError) -> Self {
        ParserError::Lexer(value)
    }
}

impl From<ParticipleLexerError> for ParserError {
    fn from(value: ParticipleLexerError) -> Self {
        ParserError::ParticipleLexer(value)
    }
}

impl From<ExpressionBuildError> for ParserError {
    fn from(value: ExpressionBuildError) -> Self {
        ParserError::Build(value)
    }
}

// ── public API ─────────────────────────────────────────────────────

/// Parse an expression string into an expression tree.
/// Uses the participle lexer for context-aware tokenisation.
pub fn parse_expression(input: &str) -> Result<Option<Box<ExpressionNode>>, ParserError> {
    parse_expression_with_diagnostics(input, None)
}

/// Parse an expression string with optional diagnostics for error reporting.
/// When `diagnostics` is provided, parse errors are recorded via `set_parse_errorf`.
pub fn parse_expression_with_diagnostics(
    input: &str,
    diagnostics: Option<Rc<RefCell<Diagnostics>>>,
) -> Result<Option<Box<ExpressionNode>>, ParserError> {
    // Strip comments (string-aware) and trim
    let normalized = strip_comments(input);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    // Validate balanced delimiters
    validate_balanced_delimiters(trimmed, diagnostics.as_ref())?;

    // Check for illegal single colon or equals
    if trimmed == ":" {
        if let Some(d) = diagnostics.as_ref() {
            d.borrow_mut().set_parse_errorf(
                ParseErrorInfo {
                    op_id: Some(OperationId::CreateMap as u16),
                    expected_args: Some(2),
                    actual_args: Some(0),
                    ..Default::default()
                },
                "':' expects 2 args but there is 0",
            );
        }
        return Err(ParserError::InvalidSyntax);
    }
    if trimmed == "=" {
        if let Some(d) = diagnostics.as_ref() {
            d.borrow_mut().set_parse_errorf(
                ParseErrorInfo {
                    op_id: Some(OperationId::Assign as u16),
                    expected_args: Some(2),
                    actual_args: Some(0),
                    ..Default::default()
                },
                "'=' expects 2 args but there is 0",
            );
        }
        return Err(ParserError::InvalidSyntax);
    }

    // Check assignment argument count
    if has_standalone_binary(trimmed, '=') {
        let parts = split_once(trimmed, '=');
        let lhs = parts.lhs.trim();
        let rhs = parts.rhs.trim();
        let count =
            (if !lhs.is_empty() { 1usize } else { 0 }) + (if !rhs.is_empty() { 1usize } else { 0 });
        if count != 2 {
            if let Some(d) = diagnostics.as_ref() {
                d.borrow_mut().set_parse_errorf(
                    ParseErrorInfo {
                        op_id: Some(OperationId::Assign as u16),
                        expected_args: Some(2),
                        actual_args: Some(count as u32),
                        ..Default::default()
                    },
                    format!("'=' expects 2 args but there is {}", count),
                );
            }
            return Err(ParserError::InvalidSyntax);
        }
    }

    // Try simple traversal first
    if let Some(expression) = parse_simple_traversal(trimmed)? {
        return Ok(Some(Box::new(expression)));
    }

    // Use participle lexer for context-aware tokenisation
    let mut tokens = match lexer_participle::lex_participle(trimmed) {
        Ok(tokens) => tokens,
        Err(participle_error) => {
            if let Err(lexer_error) = super::lexer::lex(trimmed) {
                return Err(ParserError::Lexer(lexer_error));
            }
            return Err(ParserError::ParticipleLexer(participle_error));
        }
    };
    if tokens.is_empty() {
        return Ok(None);
    }

    let token_count = tokens.len();
    for index in 0..token_count {
        let next_is_open_paren = matches!(
            tokens.get(index + 1).map(|token| &token.kind),
            Some(TokenKind::OpenParen)
        );
        if let Token {
            kind: TokenKind::Operation(operation),
            ..
        } = &mut tokens[index]
        {
            if operation.operation_type.id == OperationId::Not && index == 0 && token_count > 1 {
                operation.operation_type.num_args = 1;
            }

            if operation.operation_type.id == OperationId::GetPath && !next_is_open_paren {
                operation.operation_type.num_args = 0;
            }
        }
    }

    let postfix = to_postfix(&tokens, diagnostics.as_ref())?;
    match build_expression_tree_from_postfix_ops(&postfix) {
        Ok(tree) => Ok(tree),
        Err(ExpressionBuildError::InvalidStackState { .. })
            if postfix.len() > 1 && postfix.iter().all(|op| op.operation_type.num_args == 0) =>
        {
            Err(ParserError::UnexpectedToken(
                postfix
                    .last()
                    .expect("postfix should contain at least one operation")
                    .string_value
                    .clone(),
            ))
        }
        Err(
            ExpressionBuildError::InsufficientOperands { .. }
            | ExpressionBuildError::InvalidStackState { .. }
            | ExpressionBuildError::UnsupportedArity { .. },
        ) => Err(ParserError::InvalidSyntax),
    }
}

// ── comment stripping (string-aware) ───────────────────────────────

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i] as char;

        // Track string boundaries (respecting backslash escaping)
        if c == '"' {
            let mut backslashes = 0usize;
            let mut j = i as isize;
            while j - 1 >= 0 && bytes[(j - 1) as usize] == b'\\' {
                backslashes += 1;
                j -= 1;
            }
            if backslashes % 2 == 0 {
                in_string = !in_string;
            }
            out.push(c);
            i += 1;
            continue;
        }

        // If not in string and hit '#', skip to end of line
        if !in_string && c == '#' {
            while i + 1 < bytes.len() && bytes[i + 1] != b'\n' && bytes[i + 1] != b'\r' {
                i += 1;
            }
            i += 1;
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

// ── delimiter validation ───────────────────────────────────────────

fn validate_balanced_delimiters(
    s: &str,
    diagnostics: Option<&Rc<RefCell<Diagnostics>>>,
) -> Result<(), ParserError> {
    let bytes = s.as_bytes();
    let mut stack: Vec<(u8, usize)> = Vec::new();
    let mut i = 0;
    let mut in_string = false;

    while i < bytes.len() {
        let c = bytes[i] as char;

        // Ignore characters inside strings
        if c == '"' {
            let mut backslashes = 0usize;
            let mut j = i as isize;
            while j - 1 >= 0 && bytes[(j - 1) as usize] == b'\\' {
                backslashes += 1;
                j -= 1;
            }
            if backslashes % 2 == 0 {
                in_string = !in_string;
            }
            i += 1;
            continue;
        }
        if in_string {
            i += 1;
            continue;
        }

        // Push open delimiters
        if c == '(' || c == '[' || c == '{' {
            stack.push((c as u8, i));
            i += 1;
            continue;
        }

        // Check close delimiters
        if c == ')' || c == ']' || c == '}' {
            if stack.is_empty() {
                i += 1;
                continue;
            }
            let top = stack[stack.len() - 1].0 as char;
            let want = match top {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => '\0',
            };
            if c == want {
                stack.pop();
            }
        }

        i += 1;
    }

    if stack.is_empty() {
        return Ok(());
    }

    let (top, top_index) = stack[stack.len() - 1];
    let top = top as char;
    if let Some(d) = diagnostics {
        let msg = match top {
            '[' => "bad expression, could not find matching `]`",
            '{' => "bad expression, could not find matching `}`",
            '(' => "bad expression, could not find matching `)`",
            _ => "bad expression, unbalanced delimiter",
        };
        d.borrow_mut()
            .set_message(DiagnosticStage::ParseExpression, msg);
    }
    if stack.len() == 1 && top == '(' {
        let preceded_by_ident = top_index > 0 && is_ident_continue(bytes[top_index - 1] as char);
        if !preceded_by_ident {
            return Err(ParserError::MissingClosingDelimiter(')'));
        }
    }
    Err(ParserError::InvalidSyntax)
}

// ── standalone binary check ────────────────────────────────────────

fn has_standalone_binary(s: &str, needle: char) -> bool {
    s.contains(needle)
}

fn split_once(s: &str, needle: char) -> SplitOnce<'_> {
    match s.find(needle) {
        Some(idx) => SplitOnce {
            lhs: &s[..idx],
            rhs: &s[idx + 1..],
        },
        None => SplitOnce { lhs: s, rhs: "" },
    }
}

struct SplitOnce<'a> {
    lhs: &'a str,
    rhs: &'a str,
}

// ── simple traversal ───────────────────────────────────────────────

fn parse_simple_traversal(input: &str) -> Result<Option<ExpressionNode>, ParserError> {
    if !input.starts_with('.') {
        return Ok(None);
    }
    if input == "." {
        return Ok(Some(build_traversal_expression(&[])));
    }

    let bytes = input.as_bytes();
    let mut position = 0usize;
    let mut path = Vec::new();

    while position < bytes.len() {
        if bytes[position] != b'.' {
            return Ok(None);
        }
        position += 1;
        if position >= bytes.len() {
            break;
        }
        match bytes[position] as char {
            '[' => {
                position += 1;
                let start = position;
                while position < bytes.len() && bytes[position] != b']' {
                    position += 1;
                }
                if position >= bytes.len() {
                    return Err(ParserError::MissingClosingDelimiter(']'));
                }
                let segment = input[start..position].trim();
                let Ok(index) = segment.parse::<i64>() else {
                    return Ok(None);
                };
                path.push(ParsedKey::Int(index));
                position += 1;
            }
            ch if is_ident_start(ch) => {
                let start = position;
                position += 1;
                while position < bytes.len() && is_ident_continue(bytes[position] as char) {
                    position += 1;
                }
                path.push(ParsedKey::Str(input[start..position].to_string()));
            }
            _ => return Ok(None),
        }
    }

    Ok(Some(build_traversal_expression(&path)))
}

// ── postfix conversion (Shunting-yard) ─────────────────────────────

#[derive(Debug)]
enum StackItem {
    OpenParen,
    OpenBracket,
    OpenCollectObject,
    Operation(Operation),
}

fn to_postfix(
    tokens: &[Token],
    diagnostics: Option<&Rc<RefCell<Diagnostics>>>,
) -> Result<Vec<Operation>, ParserError> {
    let mut output = Vec::with_capacity(tokens.len());
    let mut stack: Vec<StackItem> = Vec::new();

    stack.push(StackItem::OpenParen);

    for (_idx, token) in tokens.iter().enumerate() {
        match &token.kind {
            // Open delimiters → push to stack
            TokenKind::OpenParen
            | TokenKind::OpenBracket
            | TokenKind::OpenCollectObject
            | TokenKind::OpenBrace => {
                let item = match &token.kind {
                    TokenKind::OpenParen => StackItem::OpenParen,
                    TokenKind::OpenBracket => StackItem::OpenBracket,
                    TokenKind::OpenCollectObject | TokenKind::OpenBrace => {
                        StackItem::OpenCollectObject
                    }
                    _ => unreachable!(),
                };
                stack.push(item);
            }

            // Close collect / close collect object
            TokenKind::CloseBracket | TokenKind::CloseCollectObject | TokenKind::CloseBrace => {
                let (opener_kind, collect_op) = match &token.kind {
                    TokenKind::CloseBracket => (
                        StackItemKind::OpenBracket,
                        Operation::new(OperationId::Collect, "collect", 1, 50),
                    ),
                    TokenKind::CloseCollectObject | TokenKind::CloseBrace => (
                        StackItemKind::OpenCollectObject,
                        Operation::new(OperationId::CollectObject, "collect_object", 0, 50),
                    ),
                    _ => unreachable!(),
                };

                // Pop until matching opener, validating no open tokens
                pop_until_with_validation(
                    &mut stack,
                    &mut output,
                    opener_kind,
                    token,
                    diagnostics,
                )?;

                // Push collect operation
                output.push(collect_op.clone());

                // For collect_object, push short_pipe
                if collect_op.operation_type.id == OperationId::CollectObject {
                    output.push(Operation::binary(OperationId::ShortPipe, "short_pipe", 45));
                }

                // traverse_array_op_type flush: check if stack top is traverse_array
                // and flush it with optional_traverse from ]?
                if let Some(StackItem::Operation(top_op)) = stack.last() {
                    if top_op.operation_type.id == OperationId::TraverseArray {
                        // Check for optional traverse (]? suffix)
                        let optional = token.lexeme.ends_with('?');
                        if optional {
                            // Mark as optional traverse
                            // In the full implementation, this would set
                            // TraversePreferences { optional_traverse: true }
                        }
                        let popped = stack.pop().unwrap();
                        if let StackItem::Operation(op) = popped {
                            output.push(op);
                        }
                    }
                }

                // length_op_type filler: if create_map is on stack and next is close_collect
                // This is handled by post_process_tokens in the lexer
            }

            // Close paren
            TokenKind::CloseParen => {
                pop_until_with_validation(
                    &mut stack,
                    &mut output,
                    StackItemKind::OpenParen,
                    token,
                    diagnostics,
                )?;
            }

            // Comma → push union operator
            TokenKind::Comma => {
                push_operator(
                    &mut stack,
                    &mut output,
                    Operation::binary(OperationId::Union, ",", 10),
                );
            }

            // TraverseArrayCollect should have been expanded by post_process_tokens
            TokenKind::TraverseArrayCollect => {
                return Err(ParserError::UnexpectedToken(".[".to_string()));
            }

            // Operation tokens
            TokenKind::Operation(operation) => {
                if operation.operation_type.num_args == 0 {
                    output.push(operation.clone());
                } else {
                    push_operator(&mut stack, &mut output, operation.clone());
                }
            }
        }
    }

    // Process implicit close bracket
    {
        let close_token = Token {
            kind: TokenKind::CloseParen,
            lexeme: ")".to_string(),
            start_offset: 0,
            end_offset: 0,
            check_for_post_traverse: false,
            assign_operation: None,
        };
        pop_until_with_validation(
            &mut stack,
            &mut output,
            StackItemKind::OpenParen,
            &close_token,
            diagnostics,
        )?;
    }

    // Check for remaining unclosed delimiters
    if !stack.is_empty() {
        let last = stack.last().unwrap();
        if let Some(d) = diagnostics {
            let msg = match last {
                StackItem::OpenParen => "bad expression - probably missing close bracket",
                StackItem::OpenBracket => "bad expression - probably missing close bracket on [",
                StackItem::OpenCollectObject => {
                    "bad expression - probably missing close bracket on {"
                }
                StackItem::Operation(op) => {
                    return Err(ParserError::UnexpectedToken(op.string_value.clone()));
                }
            };
            d.borrow_mut()
                .set_message(DiagnosticStage::ParseExpression, msg);
        }
        return Err(ParserError::InvalidSyntax);
    }

    Ok(output)
}

fn push_operator(stack: &mut Vec<StackItem>, output: &mut Vec<Operation>, operation: Operation) {
    while let Some(StackItem::Operation(top)) = stack.last() {
        if top.operation_type.precedence > operation.operation_type.precedence {
            let StackItem::Operation(popped) = stack.pop().expect("stack item must exist") else {
                unreachable!();
            };
            output.push(popped);
        } else {
            break;
        }
    }
    stack.push(StackItem::Operation(operation));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StackItemKind {
    OpenParen,
    OpenBracket,
    OpenCollectObject,
}

fn pop_until_with_validation(
    stack: &mut Vec<StackItem>,
    output: &mut Vec<Operation>,
    stop: StackItemKind,
    current_token: &Token,
    diagnostics: Option<&Rc<RefCell<Diagnostics>>>,
) -> Result<(), ParserError> {
    loop {
        if stack.is_empty() {
            // Report missing opening bracket
            if let Some(d) = diagnostics {
                let msg = match stop {
                    StackItemKind::OpenParen => {
                        "bad expression, got close brackets without matching opening bracket"
                    }
                    StackItemKind::OpenBracket => {
                        "bad path expression, got close collect brackets without matching opening bracket"
                    }
                    StackItemKind::OpenCollectObject => {
                        "bad expression, got close collect object without matching opening bracket"
                    }
                };
                d.borrow_mut().set_parse_errorf(
                    ParseErrorInfo {
                        token_start: Some(current_token.start_offset),
                        token_end: Some(current_token.end_offset),
                        ..Default::default()
                    },
                    msg,
                );
            }
            return Err(ParserError::InvalidSyntax);
        }

        let item = stack.pop().unwrap();
        match item {
            StackItem::Operation(operation) => output.push(operation),
            StackItem::OpenParen if stop == StackItemKind::OpenParen => return Ok(()),
            StackItem::OpenBracket if stop == StackItemKind::OpenBracket => return Ok(()),
            StackItem::OpenCollectObject if stop == StackItemKind::OpenCollectObject => {
                return Ok(());
            }
            // Validate no open tokens before reporting error
            other => {
                validate_no_open_tokens(&other, current_token, diagnostics)?;
                return Err(ParserError::MissingClosingDelimiter(match stop {
                    StackItemKind::OpenParen => ')',
                    StackItemKind::OpenBracket => ']',
                    StackItemKind::OpenCollectObject => '}',
                }));
            }
        }
    }
}

/// Validate that a stack item is not an unclosed open token.
/// Emits diagnostics for unclosed `[`, `{`, or `(`.
fn validate_no_open_tokens(
    item: &StackItem,
    _current_token: &Token,
    diagnostics: Option<&Rc<RefCell<Diagnostics>>>,
) -> Result<(), ParserError> {
    match item {
        StackItem::OpenBracket => {
            if let Some(d) = diagnostics {
                d.borrow_mut().set_message(
                    DiagnosticStage::ParseExpression,
                    "bad expression, could not find matching `]`",
                );
            }
            Err(ParserError::InvalidSyntax)
        }
        StackItem::OpenCollectObject => {
            if let Some(d) = diagnostics {
                d.borrow_mut().set_message(
                    DiagnosticStage::ParseExpression,
                    "bad expression, could not find matching `}`",
                );
            }
            Err(ParserError::InvalidSyntax)
        }
        StackItem::OpenParen => {
            if let Some(d) = diagnostics {
                d.borrow_mut().set_message(
                    DiagnosticStage::ParseExpression,
                    "bad expression, could not find matching `)`",
                );
            }
            Err(ParserError::InvalidSyntax)
        }
        _ => Ok(()),
    }
}

// ── char predicates ────────────────────────────────────────────────

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')
}

// ── tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::expression::OperationId;

    #[test]
    fn parses_precedence_with_pratt_parser() {
        let tree = parse_expression("1 + 2 * 3")
            .expect("parse should succeed")
            .expect("tree should exist");

        assert_eq!(tree.operation.operation_type.id, OperationId::Add);
        assert_eq!(
            tree.rhs.as_ref().unwrap().operation.operation_type.id,
            OperationId::Multiply
        );
    }

    #[test]
    fn collect_object_postfix_uses_zero_arity_collect_object() {
        let tokens =
            lexer_participle::lex_participle("{\"name\": \"mike\"}").expect("lex should succeed");
        let postfix = to_postfix(&tokens, None).expect("postfix conversion should succeed");
        let ids: Vec<_> = postfix.iter().map(|op| op.operation_type.id).collect();

        assert_eq!(
            ids,
            vec![
                OperationId::Value,
                OperationId::Value,
                OperationId::CreateMap,
                OperationId::CollectObject,
                OperationId::ShortPipe,
            ]
        );
        assert_eq!(postfix[3].operation_type.num_args, 0);
        build_expression_tree_from_postfix_ops(&postfix).expect("collect_object tree should build");
    }

    #[test]
    fn array_collect_postfix_uses_unary_collect() {
        let tokens = lexer_participle::lex_participle(".[] | {.name: \"great\"}")
            .expect("lex should succeed");
        let postfix = to_postfix(&tokens, None).expect("postfix conversion should succeed");

        let collect = postfix
            .iter()
            .find(|op| op.operation_type.id == OperationId::Collect)
            .expect("postfix should contain collect for []");

        assert_eq!(collect.operation_type.num_args, 1);
        build_expression_tree_from_postfix_ops(&postfix)
            .expect("array collect expression tree should build");
    }

    #[test]
    fn grouped_call_can_apply_postfix_collect() {
        let tokens = lexer_participle::lex_participle("select(.)[]").expect("lex should succeed");
        let postfix = to_postfix(&tokens, None).expect("postfix conversion should succeed");
        let tree = build_expression_tree_from_postfix_ops(&postfix)
            .expect("grouped postfix collect tree should build");

        let ids: Vec<_> = postfix.iter().map(|op| op.operation_type.id).collect();
        assert_eq!(
            ids,
            vec![
                OperationId::SelfRef,
                OperationId::Select,
                OperationId::Empty,
                OperationId::Collect,
                OperationId::TraverseArray,
            ]
        );

        let tree = tree.expect("tree should exist");
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
    }
}
