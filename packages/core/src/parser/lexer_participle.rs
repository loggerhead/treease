use crate::registry::expression::{Operation, OperationId};
use crate::registry::operation_prefs::{
    AssignPreferences, ChangeCasePrefs, DecoderPreferences, EncoderPreferences,
    ExpressionOpPreferences, FlattenPreferences, OperationPreferences, ParentOpPreferences,
    RecursiveDescentPreferences, RelationalPref, TraversePreferences,
};

use super::lexer::{self, Token, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParticipleLexerError {
    UnknownToken { offset: usize, lexeme: String },
    UnterminatedString { start: usize },
    UnexpectedCharacter { offset: usize, ch: char },
}

// ── keyword constants ──────────────────────────────────────────────

const KW_TRUE: &str = "true";
const KW_FALSE: &str = "false";
const KW_NULL: &str = "null";
const KW_SELF: &str = "self";

const KW_ARRAY_TO_MAP: &str = "array_to_map";
const KW_ROOT: &str = "root";
const KW_FLATTEN: &str = "flatten";
const KW_PARENT: &str = "parent";

const EXPAND_ARRAY_TO_MAP: &str = "(.[] | select(. != null)) as $i | reduce({}; .[$i | key] = $i)";
const EXPAND_ROOT: &str = "parent(-1)";

// ── public API ─────────────────────────────────────────────────────

/// Tokenise an expression using the participle lexer (context-aware).
pub fn lex_participle(input: &str) -> Result<Vec<Token>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    let mut tokens: Vec<Token> = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;

        if let Some(res) = try_parse_variable(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_signed_number(input, start, &tokens)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_number(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_double_quoted_string(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_recursive_descent(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_punctuations(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_math_operator(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_pipe_and_assignment(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_traverse(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_at_encode_decode(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_encode_decode_functions(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }
        if let Some(res) = try_parse_builtins_and_literals(input, start)? {
            tokens.push(res.token);
            i = res.next_index;
            continue;
        }

        return Err(ParticipleLexerError::UnknownToken {
            offset: start,
            lexeme: input[start..i.min(input.len())].to_string(),
        });
    }

    Ok(lexer::post_process_tokens(&tokens))
}

// ── parse result helper ────────────────────────────────────────────

struct ParseResult {
    token: Token,
    next_index: usize,
}

// ── variable ($foo) ────────────────────────────────────────────────

fn try_parse_variable(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() || bytes[start] != b'$' {
        return Ok(None);
    }
    if start + 1 >= bytes.len() || !is_ident_start(bytes[start + 1] as char) {
        return Err(ParticipleLexerError::UnknownToken {
            offset: start,
            lexeme: "$".to_string(),
        });
    }
    let mut end = start + 1;
    while end < bytes.len() && is_ident_char(bytes[end] as char) {
        end += 1;
    }
    let full = &input[start..end];
    Ok(Some(ParseResult {
        token: make_op_token(
            Operation::new(OperationId::GetVariable, full, 0, 55),
            start,
            end,
            false,
            false,
        ),
        next_index: end,
    }))
}

// ── signed number (context-aware) ──────────────────────────────────

fn try_parse_signed_number(
    input: &str,
    start: usize,
    tokens: &[Token],
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() {
        return Ok(None);
    }
    let c = bytes[start] as char;
    if !(c == '-'
        && start + 1 < bytes.len()
        && is_digit(bytes[start + 1] as char)
        && can_start_signed_number(tokens))
    {
        return Ok(None);
    }
    parse_signed_number_token(input, start).map(Some)
}

fn can_start_signed_number(tokens: &[Token]) -> bool {
    if tokens.is_empty() {
        return true;
    }
    let last = &tokens[tokens.len() - 1];
    match &last.kind {
        TokenKind::TraverseArrayCollect => true,
        TokenKind::OpenParen
        | TokenKind::OpenBracket
        | TokenKind::OpenBrace
        | TokenKind::OpenCollectObject => true,
        TokenKind::CloseParen
        | TokenKind::CloseBracket
        | TokenKind::CloseBrace
        | TokenKind::CloseCollectObject => false,
        TokenKind::Operation(op) => {
            let id = op.operation_type.id;
            !matches!(
                id,
                OperationId::Value
                    | OperationId::SelfRef
                    | OperationId::TraversePath
                    | OperationId::GetVariable
                    | OperationId::RecursiveDescent
            )
        }
        _ => false,
    }
}

fn parse_signed_number_token(
    input: &str,
    start: usize,
) -> Result<ParseResult, ParticipleLexerError> {
    let bytes = input.as_bytes();
    let mut end = start;
    if bytes[end] == b'-' {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if digits_start == end {
        return Err(ParticipleLexerError::UnknownToken {
            offset: start,
            lexeme: input[start..end].to_string(),
        });
    }

    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        end += 1;
        if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
            end += 1;
        }
        let exp_start = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if exp_start == end {
            return Err(ParticipleLexerError::UnknownToken {
                offset: start,
                lexeme: input[start..end].to_string(),
            });
        }
    }

    let raw = &input[start..end];
    Ok(ParseResult {
        token: make_value_token(raw, start, end),
        next_index: end,
    })
}

// ── unsigned number ────────────────────────────────────────────────

fn try_parse_number(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() || !bytes[start].is_ascii_digit() {
        return Ok(None);
    }
    parse_number_token(input, start).map(Some)
}

fn parse_number_token(input: &str, start: usize) -> Result<ParseResult, ParticipleLexerError> {
    let bytes = input.as_bytes();
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }

    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        end += 1;
        if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
            end += 1;
        }
        let exp_start = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if exp_start == end {
            return Err(ParticipleLexerError::UnknownToken {
                offset: start,
                lexeme: input[start..end].to_string(),
            });
        }
    }

    let raw = &input[start..end];
    Ok(ParseResult {
        token: make_value_token(raw, start, end),
        next_index: end,
    })
}

// ── double-quoted string ───────────────────────────────────────────

fn try_parse_double_quoted_string(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() || bytes[start] != b'"' {
        return Ok(None);
    }
    parse_double_quoted_string(input, start).map(Some)
}

fn parse_double_quoted_string(
    input: &str,
    start_quote: usize,
) -> Result<ParseResult, ParticipleLexerError> {
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = start_quote + 1;

    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' {
            let has_interp = out.contains("\\(");
            let op = if has_interp {
                Operation::new(OperationId::StringInterp, out.as_str(), 0, 50)
            } else {
                Operation::value(out.as_str())
            };
            return Ok(ParseResult {
                token: make_value_op_token(op, start_quote, i + 1),
                next_index: i + 1,
            });
        }
        if c == '\\' {
            if i + 1 >= bytes.len() {
                return Err(ParticipleLexerError::UnterminatedString { start: start_quote });
            }
            let esc = bytes[i + 1] as char;
            let mapped = match esc {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '"' => '"',
                '\\' => '\\',
                _ => esc,
            };
            out.push(mapped);
            i += 1;
        } else {
            out.push(c);
        }
        i += 1;
    }
    Err(ParticipleLexerError::UnterminatedString { start: start_quote })
}

// ── recursive descent (.. / ...) ───────────────────────────────────

fn try_parse_recursive_descent(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start + 1 >= bytes.len() {
        return Ok(None);
    }
    if !(bytes[start] == b'.' && bytes[start + 1] == b'.') {
        return Ok(None);
    }
    let mut end = start;
    while end < bytes.len() && bytes[end] == b'.' {
        end += 1;
    }
    let dots = end - start;
    if !(dots == 2 || dots == 3) {
        return Ok(None);
    }

    let prefs = RecursiveDescentPreferences {
        traverse_preferences: TraversePreferences {
            include_map_keys: dots == 3,
            dont_follow_alias: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut op = Operation::new(OperationId::RecursiveDescent, &input[start..end], 0, 50);
    op.preferences = Some(Box::new(OperationPreferences::RecursiveDescent(prefs)));

    Ok(Some(ParseResult {
        token: make_op_token(op, start, end, false, false),
        next_index: end,
    }))
}

// ── punctuations ───────────────────────────────────────────────────

fn try_parse_punctuations(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() {
        return Ok(None);
    }
    let c = bytes[start] as char;

    // ] and ]? (optional close)
    if c == ']' {
        let mut match_str = "]";
        let mut next = start + 1;
        if next < bytes.len() && bytes[next] == b'?' {
            match_str = "]?";
            next += 1;
        }
        return Ok(Some(ParseResult {
            token: make_literal(TokenKind::CloseBracket, match_str, true, start, next),
            next_index: next,
        }));
    }

    match c {
        '(' => Ok(Some(ParseResult {
            token: make_literal(TokenKind::OpenParen, "(", false, start, start + 1),
            next_index: start + 1,
        })),
        ')' => Ok(Some(ParseResult {
            token: make_literal(TokenKind::CloseParen, ")", true, start, start + 1),
            next_index: start + 1,
        })),
        '{' => Ok(Some(ParseResult {
            token: make_literal(TokenKind::OpenCollectObject, "{", false, start, start + 1),
            next_index: start + 1,
        })),
        '}' => Ok(Some(ParseResult {
            token: make_literal(TokenKind::CloseCollectObject, "}", true, start, start + 1),
            next_index: start + 1,
        })),
        '[' => Ok(Some(ParseResult {
            token: make_literal(TokenKind::OpenBracket, "[", false, start, start + 1),
            next_index: start + 1,
        })),
        ':' => Ok(Some(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::CreateMap, ":", 15),
                start,
                start + 1,
                false,
                false,
            ),
            next_index: start + 1,
        })),
        ';' => Ok(Some(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::Block, ";", 10),
                start,
                start + 1,
                false,
                false,
            ),
            next_index: start + 1,
        })),
        ',' => Ok(Some(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::Union, ",", 10),
                start,
                start + 1,
                false,
                false,
            ),
            next_index: start + 1,
        })),
        _ => Ok(None),
    }
}

// ── math operators (+ - * / %) ─────────────────────────────────────

fn try_parse_math_operator(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() {
        return Ok(None);
    }
    let c = bytes[start] as char;
    if !matches!(c, '+' | '-' | '*' | '/' | '%') {
        return Ok(None);
    }

    // // alternative operator
    if c == '/' && start + 1 < bytes.len() && bytes[start + 1] == b'/' {
        return Ok(Some(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::Alternative, "//", 42),
                start,
                start + 2,
                false,
                false,
            ),
            next_index: start + 2,
        }));
    }

    parse_math_op_token(input, start).map(Some)
}

fn parse_math_op_token(input: &str, i: usize) -> Result<ParseResult, ParticipleLexerError> {
    let bytes = input.as_bytes();
    let c = bytes[i] as char;
    match c {
        '+' => Ok(ParseResult {
            token: make_math_op_token_with_assign(
                Operation::binary(OperationId::Add, "+", 42),
                Some(Operation::binary(OperationId::AddAssign, "+=", 40)),
                i,
                i + 1,
            ),
            next_index: i + 1,
        }),
        '-' => Ok(ParseResult {
            token: make_math_op_token_with_assign(
                Operation::binary(OperationId::Subtract, "-", 42),
                Some(Operation::binary(OperationId::SubtractAssign, "-=", 40)),
                i,
                i + 1,
            ),
            next_index: i + 1,
        }),
        '*' => Ok(ParseResult {
            token: make_math_op_token_with_assign(
                Operation::binary(OperationId::Multiply, "*", 42),
                Some(Operation::binary(OperationId::MultiplyAssign, "*=", 42)),
                i,
                i + 1,
            ),
            next_index: i + 1,
        }),
        '/' => Ok(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::Divide, "/", 42),
                i,
                i + 1,
                false,
                false,
            ),
            next_index: i + 1,
        }),
        '%' => Ok(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::Modulo, "%", 42),
                i,
                i + 1,
                false,
                false,
            ),
            next_index: i + 1,
        }),
        _ => Err(ParticipleLexerError::UnknownToken {
            offset: i,
            lexeme: input[i..i + 1].to_string(),
        }),
    }
}

fn make_math_op_token_with_assign(
    operation: Operation,
    assign_operation: Option<Operation>,
    start: usize,
    end: usize,
) -> Token {
    let lexeme = operation.string_value.clone();
    Token {
        kind: TokenKind::Operation(operation),
        lexeme,
        start_offset: start,
        end_offset: end,
        check_for_post_traverse: false,
        assign_operation,
    }
}

// ── pipe, relational, assignment ───────────────────────────────────

fn try_parse_pipe_and_assignment(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() {
        return Ok(None);
    }
    let c = bytes[start] as char;

    // < > relational (with optional =)
    if c == '<' || c == '>' {
        let mut end = start + 1;
        let mut or_equal = false;
        if end < bytes.len() && bytes[end] == b'=' {
            or_equal = true;
            end += 1;
        }
        let lexeme = &input[start..end];
        let mut op = Operation::binary(OperationId::Relational, lexeme, 40);
        op.preferences = Some(Box::new(OperationPreferences::Relational(RelationalPref {
            or_equal,
            greater: c == '>',
        })));
        return Ok(Some(ParseResult {
            token: make_op_token(op, start, end, false, false),
            next_index: end,
        }));
    }

    // |= pipe assign
    if c == '|' && start + 1 < bytes.len() && bytes[start + 1] == b'=' {
        return Ok(Some(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::Assign, "|=", 40),
                start,
                start + 2,
                false,
                true,
            ),
            next_index: start + 2,
        }));
    }

    // | pipe
    if c == '|' {
        return Ok(Some(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::Pipe, "|", 30),
                start,
                start + 1,
                false,
                false,
            ),
            next_index: start + 1,
        }));
    }

    // != not equals
    if c == '!' && start + 1 < bytes.len() && bytes[start + 1] == b'=' {
        return Ok(Some(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::NotEquals, "!=", 40),
                start,
                start + 2,
                false,
                false,
            ),
            next_index: start + 2,
        }));
    }

    // == equals
    if c == '=' && start + 1 < bytes.len() && bytes[start + 1] == b'=' {
        return Ok(Some(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::Equals, "==", 40),
                start,
                start + 2,
                false,
                false,
            ),
            next_index: start + 2,
        }));
    }

    // =c assign with clobber_custom_tags
    if c == '=' {
        if start + 1 < bytes.len() && bytes[start + 1] == b'c' {
            let boundary = start + 2 >= bytes.len() || !is_ident_char(bytes[start + 2] as char);
            if boundary {
                let mut op = Operation::binary(OperationId::Assign, "=c", 40);
                op.preferences = Some(Box::new(OperationPreferences::Assign(AssignPreferences {
                    clobber_custom_tags: true,
                    ..Default::default()
                })));
                return Ok(Some(ParseResult {
                    token: make_op_token(op, start, start + 2, false, false),
                    next_index: start + 2,
                }));
            }
        }
        // = assign
        return Ok(Some(ParseResult {
            token: make_op_token(
                Operation::binary(OperationId::Assign, "=", 40),
                start,
                start + 1,
                false,
                false,
            ),
            next_index: start + 1,
        }));
    }

    Ok(None)
}

// ── traverse (.[  .name  .name?  .) ────────────────────────────────

fn try_parse_traverse(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() || bytes[start] != b'.' {
        return Ok(None);
    }

    // .[
    if start + 1 < bytes.len() && bytes[start + 1] == b'[' {
        return Ok(Some(ParseResult {
            token: make_literal(
                TokenKind::TraverseArrayCollect,
                ".[",
                false,
                start,
                start + 2,
            ),
            next_index: start + 2,
        }));
    }

    // .name or .name?
    if start + 1 < bytes.len() && is_ident_start(bytes[start + 1] as char) {
        let mut end = start + 1;
        while end < bytes.len() && is_ident_char(bytes[end] as char) {
            end += 1;
        }
        let name = &input[start + 1..end];

        let mut optional = false;
        if end < bytes.len() && bytes[end] == b'?' {
            optional = true;
            end += 1;
        }

        let mut op = Operation::new(OperationId::TraversePath, name, 0, 55);
        if optional {
            op.preferences = Some(Box::new(OperationPreferences::Traverse(
                TraversePreferences {
                    optional_traverse: true,
                    ..Default::default()
                },
            )));
        }

        return Ok(Some(ParseResult {
            token: make_op_token_with_post(op, start, end, true),
            next_index: end,
        }));
    }

    // . (self)
    Ok(Some(ParseResult {
        token: make_op_token_with_post(
            Operation::new(OperationId::SelfRef, "SELF", 0, 55),
            start,
            start + 1,
            true,
        ),
        next_index: start + 1,
    }))
}

// ── @encode/@decode ────────────────────────────────────────────────

fn try_parse_at_encode_decode(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() || bytes[start] != b'@' {
        return Ok(None);
    }
    if start + 1 >= bytes.len() || !is_ident_start(bytes[start + 1] as char) {
        return Err(ParticipleLexerError::UnknownToken {
            offset: start,
            lexeme: "@".to_string(),
        });
    }

    let mut end = start + 1;
    while end < bytes.len() && is_ident_char(bytes[end] as char) {
        end += 1;
    }
    let full_name = &input[start + 1..end];
    if full_name.is_empty() {
        return Err(ParticipleLexerError::UnknownToken {
            offset: start,
            lexeme: input[start..end].to_string(),
        });
    }

    let decode = full_name.as_bytes()[full_name.len() - 1] == b'd';
    let base = if decode {
        &full_name[..full_name.len() - 1]
    } else {
        full_name
    };

    let def = match base {
        "yaml" => Some(("yaml", "yaml", 2i32)),
        "json" => Some(("json", "yaml", 0)),
        "toml" => Some(("toml", "toml", 2)),
        "csv" => Some(("csv", "csv", 0)),
        "uri" => Some(("uri", "uri", 0)),
        _ => None,
    };

    let (encode_fmt, decode_fmt, default_indent) = match def {
        Some(d) => d,
        None => return Ok(None),
    };

    let fmt = if decode { decode_fmt } else { encode_fmt };

    if decode {
        let mut op = Operation::new(OperationId::Decode, &input[start..end], 0, 50);
        op.preferences = Some(Box::new(OperationPreferences::Decoder(
            DecoderPreferences {
                format: fmt.to_string(),
            },
        )));
        return Ok(Some(ParseResult {
            token: make_op_token_with_post(op, start, end, true),
            next_index: end,
        }));
    }

    let mut op = Operation::new(OperationId::Encode, &input[start..end], 0, 50);
    op.preferences = Some(Box::new(OperationPreferences::Encoder(
        EncoderPreferences {
            format: fmt.to_string(),
            indent: default_indent,
        },
    )));
    Ok(Some(ParseResult {
        token: make_op_token_with_post(op, start, end, true),
        next_index: end,
    }))
}

// ── to_yaml / from_yaml etc. ───────────────────────────────────────

fn try_parse_encode_decode_functions(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() || !is_ident_start(bytes[start] as char) {
        return Ok(None);
    }

    // Encoders: to_yaml, to_json, to_toml, to_csv
    let encoders: &[(&str, &str, i32, bool)] = &[
        ("to_yaml", "yaml", 2, true),
        ("to_json", "json", 2, true),
        ("to_toml", "toml", 2, true),
        ("to_csv", "csv", 0, false),
    ];

    for &(keyword, fmt, default_indent, allow_optional_indent) in encoders {
        if let Some(end0) = try_match_keyword(input, start, keyword) {
            if allow_optional_indent {
                return parse_encode_fn_with_optional_indent(
                    input,
                    start,
                    end0,
                    keyword,
                    fmt,
                    default_indent,
                )
                .map(Some);
            }
            if end0 < bytes.len() && bytes[end0] == b'(' {
                return Err(ParticipleLexerError::UnknownToken {
                    offset: start,
                    lexeme: keyword.to_string(),
                });
            }
            let mut op = Operation::new(OperationId::Encode, keyword, 0, 50);
            op.preferences = Some(Box::new(OperationPreferences::Encoder(
                EncoderPreferences {
                    format: fmt.to_string(),
                    indent: default_indent,
                },
            )));
            return Ok(Some(ParseResult {
                token: make_op_token_with_post(op, start, end0, true),
                next_index: end0,
            }));
        }
    }

    // Decoders: from_yaml, from_json, from_csv
    let decoders: &[(&str, &str)] = &[
        ("from_yaml", "yaml"),
        ("from_json", "yaml"),
        ("from_csv", "csv"),
    ];

    for &(keyword, fmt) in decoders {
        if let Some(end0) = try_match_keyword(input, start, keyword) {
            if end0 < bytes.len() && bytes[end0] == b'(' {
                return Err(ParticipleLexerError::UnknownToken {
                    offset: start,
                    lexeme: keyword.to_string(),
                });
            }
            let mut op = Operation::new(OperationId::Decode, keyword, 0, 50);
            op.preferences = Some(Box::new(OperationPreferences::Decoder(
                DecoderPreferences {
                    format: fmt.to_string(),
                },
            )));
            return Ok(Some(ParseResult {
                token: make_op_token_with_post(op, start, end0, true),
                next_index: end0,
            }));
        }
    }

    Ok(None)
}

fn parse_encode_fn_with_optional_indent(
    input: &str,
    start: usize,
    end0: usize,
    string_value: &str,
    fmt: &str,
    default_indent: i32,
) -> Result<ParseResult, ParticipleLexerError> {
    let bytes = input.as_bytes();
    let mut end = end0;
    let mut indent = default_indent;
    if end < bytes.len() && bytes[end] == b'(' {
        end += 1;
        let digits_start = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if digits_start == end {
            return Err(ParticipleLexerError::UnknownToken {
                offset: start,
                lexeme: string_value.to_string(),
            });
        }
        if end >= bytes.len() || bytes[end] != b')' {
            return Err(ParticipleLexerError::UnknownToken {
                offset: start,
                lexeme: string_value.to_string(),
            });
        }
        indent = input[digits_start..end].parse::<i32>().map_err(|_| {
            ParticipleLexerError::UnknownToken {
                offset: start,
                lexeme: string_value.to_string(),
            }
        })?;
        end += 1;
    }

    let mut op = Operation::new(OperationId::Encode, string_value, 0, 50);
    op.preferences = Some(Box::new(OperationPreferences::Encoder(
        EncoderPreferences {
            format: fmt.to_string(),
            indent,
        },
    )));
    Ok(ParseResult {
        token: make_op_token_with_post(op, start, end, true),
        next_index: end,
    })
}

// ── builtins and literals ──────────────────────────────────────────

fn try_parse_builtins_and_literals(
    input: &str,
    start: usize,
) -> Result<Option<ParseResult>, ParticipleLexerError> {
    let bytes = input.as_bytes();
    if start >= bytes.len() {
        return Ok(None);
    }

    // flatten / flatten(depth)
    if input[start..].starts_with(KW_FLATTEN) {
        let end_name = start + KW_FLATTEN.len();
        if end_name == bytes.len()
            || bytes[end_name] == b'('
            || !is_ident_char(bytes[end_name] as char)
        {
            return parse_flatten_token(input, start).map(Some);
        }
    }

    // parent / parent(level)
    if input[start..].starts_with(KW_PARENT) {
        let end_name = start + KW_PARENT.len();
        if end_name == bytes.len()
            || bytes[end_name] == b'('
            || !is_ident_char(bytes[end_name] as char)
        {
            if end_name < bytes.len() && bytes[end_name] == b'(' {
                let mut i = end_name + 1;
                let digits_start = i;
                if i < bytes.len() && bytes[i] == b'-' {
                    i += 1;
                }
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if digits_start == i {
                    return Err(ParticipleLexerError::UnknownToken {
                        offset: start,
                        lexeme: KW_PARENT.to_string(),
                    });
                }
                if i >= bytes.len() || bytes[i] != b')' {
                    return Err(ParticipleLexerError::UnknownToken {
                        offset: start,
                        lexeme: KW_PARENT.to_string(),
                    });
                }
                let level: i32 = input[digits_start..i].parse().map_err(|_| {
                    ParticipleLexerError::UnknownToken {
                        offset: start,
                        lexeme: KW_PARENT.to_string(),
                    }
                })?;
                i += 1;

                let mut op = Operation::new(OperationId::GetParent, KW_PARENT, 0, 50);
                op.preferences = Some(Box::new(OperationPreferences::Parent(
                    ParentOpPreferences { level },
                )));
                return Ok(Some(ParseResult {
                    token: make_op_token(op, start, i, false, false),
                    next_index: i,
                }));
            }
        }
    }

    // change_case: upcase, downcase, ascii_upcase, etc.
    let change_case_defs: &[(&str, bool)] = &[
        ("upcase", true),
        ("ascii_upcase", true),
        ("asciiupcase", true),
        ("downcase", false),
        ("ascii_downcase", false),
        ("asciidowncase", false),
    ];
    for &(keyword, upper) in change_case_defs {
        if let Some(end_case) = try_match_keyword(input, start, keyword) {
            let mut op = Operation::new(OperationId::ChangeCase, keyword, 0, 50);
            op.preferences = Some(Box::new(OperationPreferences::ChangeCase(
                ChangeCasePrefs {
                    to_upper_case: upper,
                },
            )));
            return Ok(Some(ParseResult {
                token: make_op_token(op, start, end_case, false, false),
                next_index: end_case,
            }));
        }
    }

    if !is_ident_start(bytes[start] as char) {
        return Ok(None);
    }

    let mut end = start + 1;
    while end < bytes.len() && is_ident_char(bytes[end] as char) {
        end += 1;
    }
    let ident = &input[start..end];

    // Check builtins table
    if let Some(result) = lookup_builtin(ident, start, end) {
        return Ok(Some(result));
    }

    // Literals: true, false, null
    if matches!(ident, KW_TRUE | KW_FALSE | KW_NULL) {
        return Ok(Some(ParseResult {
            token: make_value_token(ident, start, end),
            next_index: end,
        }));
    }

    // self
    if ident == KW_SELF {
        return Ok(Some(ParseResult {
            token: make_op_token_with_post(
                Operation::new(OperationId::SelfRef, KW_SELF, 0, 55),
                start,
                end,
                true,
            ),
            next_index: end,
        }));
    }

    // Bare identifiers are not valid participle tokens unless they are
    // recognised builtins, literals, or `self`.
    let _ = is_reserved_identifier(ident);
    Err(ParticipleLexerError::UnknownToken {
        offset: start,
        lexeme: ident.to_string(),
    })
}

fn lookup_builtin(ident: &str, start: usize, end: usize) -> Option<ParseResult> {
    // array_to_map → expression expansion
    if ident == KW_ARRAY_TO_MAP {
        let mut op = Operation::new(OperationId::Exp, KW_ARRAY_TO_MAP, 0, 50);
        op.preferences = Some(Box::new(OperationPreferences::Expression(
            ExpressionOpPreferences {
                expression: EXPAND_ARRAY_TO_MAP.to_string(),
            },
        )));
        return Some(ParseResult {
            token: make_op_token(op, start, end, false, false),
            next_index: end,
        });
    }

    // root → expression expansion
    if ident == KW_ROOT {
        let mut op = Operation::new(OperationId::Exp, KW_ROOT, 0, 50);
        op.preferences = Some(Box::new(OperationPreferences::Expression(
            ExpressionOpPreferences {
                expression: EXPAND_ROOT.to_string(),
            },
        )));
        return Some(ParseResult {
            token: make_op_token(op, start, end, false, false),
            next_index: end,
        });
    }

    // Map ident to (OperationId, check_for_post, update_assign)
    let (op_id, check_for_post, update_assign): (OperationId, bool, bool) = match ident {
        "to_entries" => (OperationId::ToEntries, true, false),
        "from_entries" | "fromEntries" => (OperationId::FromEntries, false, false),
        "with_entries" | "withEntries" => (OperationId::WithEntries, false, false),
        "select" => (OperationId::Select, true, false),
        "filter" => (OperationId::Filter, true, false),
        "compact" => (OperationId::Compact, true, false),
        "has" => (OperationId::Has, true, false),
        "contains" => (OperationId::Contains, false, false),
        "path" => (OperationId::GetPath, true, false),
        "setpath" => (OperationId::SetPath, true, false),
        "parent" => (OperationId::GetParent, false, false),
        "parents" => (OperationId::GetParents, false, false),
        "type" | "tag" => (OperationId::GetTag, false, false),
        "kind" => (OperationId::GetKind, false, false),
        "map_values" => (OperationId::MapValues, true, false),
        "map" => (OperationId::Map, true, false),
        "with" => (OperationId::With, true, false),
        "any_c" => (OperationId::AnyCondition, true, false),
        "all_c" => (OperationId::AllCondition, true, false),
        "any" => (OperationId::Any, true, false),
        "all" => (OperationId::All, true, false),
        "or" => (OperationId::Or, false, false),
        "and" => (OperationId::And, false, false),
        "not" => (OperationId::Not, true, false),
        "min" => (OperationId::Min, true, false),
        "max" => (OperationId::Max, true, false),
        "sub" => (OperationId::Substr, true, false),
        "match" => (OperationId::Match, true, false),
        "capture" => (OperationId::Capture, true, false),
        "test" => (OperationId::Test, true, false),
        "split" => (OperationId::Split, true, false),
        "trim" => (OperationId::Trim, false, false),
        "to_string" | "tostring" => (OperationId::ToString, false, false),
        "to_number" | "tonumber" => (OperationId::ToNumber, false, false),
        "key" => (OperationId::GetKey, false, false),
        "keys" => (OperationId::Keys, true, false),
        "length" => (OperationId::Length, false, false),
        "is_key" | "iskey" => (OperationId::IsKey, false, false),
        "unique_by" => (OperationId::UniqueBy, true, false),
        "unique" => (OperationId::Unique, true, false),
        "sort_by" => (OperationId::SortBy, true, false),
        "sort" => (OperationId::Sort, true, false),
        "sort_keys" | "sortKeys" => (OperationId::SortKeys, true, false),
        "first" => (OperationId::First, true, false),
        "reverse" => (OperationId::Reverse, true, false),
        "shuffle" => (OperationId::Shuffle, true, false),
        "join" => (OperationId::Join, true, false),
        "pick" => (OperationId::Pick, true, false),
        "omit" => (OperationId::Omit, true, false),
        "group_by" => (OperationId::GroupBy, true, false),
        "delpaths" => (OperationId::DelPaths, true, false),
        "del" => (OperationId::Delete, true, false),
        "as" => (OperationId::AssignVariable, false, false),
        "reduce" => (OperationId::Reduce, false, false),
        _ => return None,
    };

    let num_args = match op_id {
        OperationId::Or | OperationId::And | OperationId::Reduce | OperationId::AssignVariable => 2,
        OperationId::Select
        | OperationId::Filter
        | OperationId::Has
        | OperationId::Contains
        | OperationId::GetPath
        | OperationId::SetPath
        | OperationId::Map
        | OperationId::MapValues
        | OperationId::WithEntries
        | OperationId::With
        | OperationId::AnyCondition
        | OperationId::AllCondition
        | OperationId::Min
        | OperationId::Max
        | OperationId::Substr
        | OperationId::Match
        | OperationId::Capture
        | OperationId::Test
        | OperationId::Split
        | OperationId::UniqueBy
        | OperationId::SortBy
        | OperationId::SortKeys
        | OperationId::Join
        | OperationId::Pick
        | OperationId::Omit
        | OperationId::GroupBy
        | OperationId::DelPaths
        | OperationId::Delete => 1,
        OperationId::Any
        | OperationId::All
        | OperationId::Not
        | OperationId::Keys
        | OperationId::Reverse
        | OperationId::Shuffle
        | OperationId::First => 0,
        _ => 0,
    };
    let precedence = match op_id {
        OperationId::Or | OperationId::And => 20,
        OperationId::Reduce => 35,
        OperationId::AssignVariable => 40,
        OperationId::Min | OperationId::Max | OperationId::Delete => 40,
        OperationId::Map
        | OperationId::MapValues
        | OperationId::ToEntries
        | OperationId::Select
        | OperationId::Filter
        | OperationId::GetPath
        | OperationId::DelPaths
        | OperationId::SortBy
        | OperationId::First
        | OperationId::Reverse
        | OperationId::SortKeys
        | OperationId::Split
        | OperationId::Keys
        | OperationId::Unique
        | OperationId::UniqueBy
        | OperationId::GroupBy => 52,
        _ => 50,
    };
    let op = Operation::new(op_id, ident, num_args, precedence);
    Some(ParseResult {
        token: make_op_token(op, start, end, check_for_post, update_assign),
        next_index: end,
    })
}

fn parse_flatten_token(input: &str, start: usize) -> Result<ParseResult, ParticipleLexerError> {
    let bytes = input.as_bytes();
    let mut depth: i32 = -1;
    let mut i = start + KW_FLATTEN.len();
    if i < bytes.len() && bytes[i] == b'(' {
        i += 1;
        let digits_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if digits_start == i {
            return Err(ParticipleLexerError::UnknownToken {
                offset: start,
                lexeme: KW_FLATTEN.to_string(),
            });
        }
        if i >= bytes.len() || bytes[i] != b')' {
            return Err(ParticipleLexerError::UnknownToken {
                offset: start,
                lexeme: KW_FLATTEN.to_string(),
            });
        }
        depth = input[digits_start..i].parse::<i32>().map_err(|_| {
            ParticipleLexerError::UnknownToken {
                offset: start,
                lexeme: KW_FLATTEN.to_string(),
            }
        })?;
        i += 1;
    }

    let mut op = Operation::new(OperationId::Flatten, KW_FLATTEN, 0, 52);
    op.preferences = Some(Box::new(OperationPreferences::Flatten(
        FlattenPreferences { depth },
    )));
    Ok(ParseResult {
        token: make_op_token_with_post(op, start, i, true),
        next_index: i,
    })
}

// ── keyword matching ───────────────────────────────────────────────

fn try_match_keyword(input: &str, start: usize, keyword: &str) -> Option<usize> {
    if start > input.len() {
        return None;
    }
    if !input[start..].starts_with(keyword) {
        return None;
    }
    let end = start + keyword.len();
    let bytes = input.as_bytes();
    if end == bytes.len() || !is_ident_char(bytes[end] as char) {
        return Some(end);
    }
    None
}

// ── token constructors ─────────────────────────────────────────────

fn make_op_token(
    mut op: Operation,
    start: usize,
    end: usize,
    check_for_post: bool,
    update_assign: bool,
) -> Token {
    op.update_assign = update_assign;
    let lexeme = op.string_value.clone();
    Token {
        kind: TokenKind::Operation(op),
        lexeme,
        start_offset: start,
        end_offset: end,
        check_for_post_traverse: check_for_post,
        assign_operation: None,
    }
}

fn make_op_token_with_post(op: Operation, start: usize, end: usize, check_for_post: bool) -> Token {
    let lexeme = op.string_value.clone();
    Token {
        kind: TokenKind::Operation(op),
        lexeme,
        start_offset: start,
        end_offset: end,
        check_for_post_traverse: check_for_post,
        assign_operation: None,
    }
}

fn make_value_token(raw: &str, start: usize, end: usize) -> Token {
    Token {
        kind: TokenKind::Operation(Operation::value(raw)),
        lexeme: raw.to_string(),
        start_offset: start,
        end_offset: end,
        check_for_post_traverse: false,
        assign_operation: None,
    }
}

fn make_value_op_token(op: Operation, start: usize, end: usize) -> Token {
    let lexeme = op.string_value.clone();
    Token {
        kind: TokenKind::Operation(op),
        lexeme,
        start_offset: start,
        end_offset: end,
        check_for_post_traverse: false,
        assign_operation: None,
    }
}

fn make_literal(
    kind: TokenKind,
    lexeme: &str,
    check_for_post: bool,
    start: usize,
    end: usize,
) -> Token {
    Token {
        kind,
        lexeme: lexeme.to_string(),
        start_offset: start,
        end_offset: end,
        check_for_post_traverse: check_for_post,
        assign_operation: None,
    }
}

// ── char predicates ────────────────────────────────────────────────

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_char(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit() || ch == '-' || ch == '_'
}

fn is_digit(ch: char) -> bool {
    ch.is_ascii_digit()
}

fn is_reserved_identifier(lexeme: &str) -> bool {
    matches!(
        lexeme,
        "load" | "load_str" | "fileIndex" | "fi" | "documentIndex" | "di"
    )
}
