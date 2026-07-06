use crate::registry::expression::{Operation, OperationId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Operation(Operation),
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    Comma,
    TraverseArrayCollect,
    OpenCollectObject,
    CloseCollectObject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub check_for_post_traverse: bool,
    pub assign_operation: Option<Operation>,
}

impl Token {
    /// Convert Token to a readable string.
    /// If `detail` is true, include precedence info for operation tokens.
    pub fn to_string(&self, detail: bool) -> String {
        match &self.kind {
            TokenKind::Operation(op) => {
                if detail {
                    format!("{} ({})", op.to_string_name(), op.operation_type.precedence)
                } else {
                    op.to_string_name().to_string()
                }
            }
            TokenKind::OpenParen => "(".to_string(),
            TokenKind::CloseParen => ")".to_string(),
            TokenKind::OpenBracket => "[".to_string(),
            TokenKind::CloseBracket => "]".to_string(),
            TokenKind::OpenBrace => "{".to_string(),
            TokenKind::CloseBrace => "}".to_string(),
            TokenKind::Comma => ",".to_string(),
            TokenKind::TraverseArrayCollect => ".[".to_string(),
            TokenKind::OpenCollectObject => "{".to_string(),
            TokenKind::CloseCollectObject => "}".to_string(),
        }
    }
}

/// Deep-free a vector of tokens. In Rust, this is handled by Drop,
pub fn destroy_tokens_deep(_tokens: Vec<Token>) {
    // Ownership transfer + drop handles cleanup in Rust
    drop(_tokens);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexerError {
    UnterminatedString { start: usize },
    UnexpectedCharacter { offset: usize, ch: char },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterError {
    MissingParameter,
    InvalidCharacter,
}

pub fn lex(input: &str) -> Result<Vec<Token>, LexerError> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i] as char;

        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        if ch == '#' {
            while i < bytes.len() && (bytes[i] as char) != '\n' {
                i += 1;
            }
            continue;
        }

        let start = i;
        match ch {
            '.' => {
                if bytes.get(i + 1) == Some(&b'.') {
                    i += 2;
                    if bytes.get(i) == Some(&b'.') {
                        i += 1;
                    }
                    let lexeme = &input[start..i];
                    tokens.push(op_token(
                        Operation::new(OperationId::RecursiveDescent, lexeme, 0, 50),
                        start,
                        i,
                    ));
                    continue;
                }
                if bytes.get(i + 1) == Some(&b'[') {
                    tokens.push(Token {
                        kind: TokenKind::TraverseArrayCollect,
                        lexeme: ".[".to_string(),
                        start_offset: start,
                        end_offset: start + 2,
                        check_for_post_traverse: false,
                        assign_operation: None,
                    });
                    i += 2;
                    continue;
                }
                i += 1;
                if i < bytes.len() && is_ident_start(bytes[i] as char) {
                    let path_start = i;
                    i += 1;
                    while i < bytes.len() && is_ident_continue(bytes[i] as char) {
                        i += 1;
                    }
                    let key = &input[path_start..i];
                    tokens.push(op_token_with_post(
                        Operation::new(OperationId::TraversePath, key, 0, u32::MAX),
                        start,
                        i,
                        true,
                    ));
                } else {
                    tokens.push(op_token_with_post(
                        Operation::new(OperationId::SelfRef, "self", 0, u32::MAX),
                        start,
                        i,
                        true,
                    ));
                }
            }
            '(' => {
                tokens.push(punct(TokenKind::OpenParen, "(", start));
                i += 1;
            }
            ')' => {
                tokens.push(punct_with_post(TokenKind::CloseParen, ")", start, true));
                i += 1;
            }
            '[' => {
                tokens.push(punct(TokenKind::OpenBracket, "[", start));
                i += 1;
            }
            ']' => {
                tokens.push(punct_with_post(TokenKind::CloseBracket, "]", start, true));
                i += 1;
            }
            '{' => {
                tokens.push(punct(TokenKind::OpenCollectObject, "{", start));
                i += 1;
            }
            '}' => {
                tokens.push(punct_with_post(
                    TokenKind::CloseCollectObject,
                    "}",
                    start,
                    true,
                ));
                i += 1;
            }
            ',' => {
                tokens.push(punct(TokenKind::Comma, ",", start));
                i += 1;
            }
            ';' => {
                tokens.push(op_token(
                    Operation::binary(OperationId::Block, ";", 10),
                    start,
                    start + 1,
                ));
                i += 1;
            }
            '"' | '\'' => {
                let quote = ch;
                i += 1;
                let mut escaped = false;
                while i < bytes.len() {
                    let current = bytes[i] as char;
                    if escaped {
                        escaped = false;
                    } else if current == '\\' {
                        escaped = true;
                    } else if current == quote {
                        break;
                    }
                    i += 1;
                }
                if i >= bytes.len() {
                    return Err(LexerError::UnterminatedString { start });
                }
                i += 1;
                let lexeme = &input[start..i];
                tokens.push(Token {
                    kind: TokenKind::Operation(Operation::value(lexeme)),
                    lexeme: lexeme.to_string(),
                    start_offset: start,
                    end_offset: i,
                    check_for_post_traverse: false,
                    assign_operation: None,
                });
            }
            '0'..='9' => {
                i += 1;
                while i < bytes.len() {
                    let current = bytes[i] as char;
                    if current.is_ascii_digit() || current == '.' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                let lexeme = &input[start..i];
                tokens.push(Token {
                    kind: TokenKind::Operation(Operation::value(lexeme)),
                    lexeme: lexeme.to_string(),
                    start_offset: start,
                    end_offset: i,
                    check_for_post_traverse: false,
                    assign_operation: None,
                });
            }
            '+' | '-' | '*' | '/' | '%' | '|' | ':' => {
                if ch == '/' && bytes.get(i + 1) == Some(&b'/') {
                    tokens.push(op_token(
                        Operation::binary(OperationId::Alternative, "//", 50),
                        start,
                        start + 2,
                    ));
                    i += 2;
                    continue;
                }
                if bytes.get(i + 1) == Some(&b'=') {
                    let operation = match ch {
                        '+' => Some(Operation::binary(OperationId::AddAssign, "+=", 40)),
                        '-' => Some(Operation::binary(OperationId::SubtractAssign, "-=", 40)),
                        '*' => Some(Operation::binary(OperationId::MultiplyAssign, "*=", 40)),
                        _ => None,
                    };
                    if let Some(operation) = operation {
                        tokens.push(op_token(operation, start, start + 2));
                        i += 2;
                        continue;
                    }
                }

                let operation = match ch {
                    '+' => Operation::binary(OperationId::Add, "+", 50),
                    '-' => Operation::binary(OperationId::Subtract, "-", 50),
                    '*' => Operation::binary(OperationId::Multiply, "*", 60),
                    '/' => Operation::binary(OperationId::Divide, "/", 60),
                    '%' => Operation::binary(OperationId::Modulo, "%", 60),
                    '|' => Operation::binary(OperationId::Pipe, "|", 30),
                    ':' => Operation::binary(OperationId::CreateMap, ":", 5),
                    _ => unreachable!(),
                };
                tokens.push(op_token(operation, start, start + 1));
                i += 1;
            }
            '=' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    tokens.push(op_token(
                        Operation::binary(OperationId::Equals, "==", 40),
                        start,
                        start + 2,
                    ));
                    i += 2;
                } else if bytes.get(i + 1) == Some(&b'c')
                    && bytes
                        .get(i + 2)
                        .is_none_or(|byte| !is_ident_continue(*byte as char))
                {
                    tokens.push(op_token(
                        Operation::binary(OperationId::Assign, "=c", 40),
                        start,
                        start + 2,
                    ));
                    i += 2;
                } else {
                    tokens.push(op_token(
                        Operation::binary(OperationId::Assign, "=", 40),
                        start,
                        start + 1,
                    ));
                    i += 1;
                }
            }
            '!' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    tokens.push(op_token(
                        Operation::binary(OperationId::NotEquals, "!=", 40),
                        start,
                        start + 2,
                    ));
                    i += 2;
                } else {
                    return Err(LexerError::UnexpectedCharacter { offset: start, ch });
                }
            }
            '<' | '>' => {
                i += 1;
                if bytes.get(i) == Some(&b'=') {
                    i += 1;
                }
                let lexeme = &input[start..i];
                tokens.push(op_token(
                    Operation::binary(OperationId::Relational, lexeme, 40),
                    start,
                    i,
                ));
            }
            _ if is_ident_start(ch) => {
                i += 1;
                while i < bytes.len() && is_ident_continue(bytes[i] as char) {
                    i += 1;
                }
                let ident = &input[start..i];
                let mut end = i;
                if bytes.get(i) == Some(&b'(') {
                    end = consume_call_suffix(input, i)?;
                    i = end;
                }
                let lexeme = &input[start..end];
                let (operation, check_for_post_traverse) = match ident {
                    "and" => (Operation::binary(OperationId::And, lexeme, 20), false),
                    "or" => (Operation::binary(OperationId::Or, lexeme, 15), false),
                    "not" => (Operation::unary(OperationId::Not, lexeme, 70), false),
                    "self" => (
                        Operation::new(OperationId::SelfRef, lexeme, 0, u32::MAX),
                        true,
                    ),
                    "true" | "false" | "null" => (Operation::value(lexeme), false),
                    _ => builtin_operation(ident, lexeme).unwrap_or_else(|| {
                        (
                            Operation::custom(ident.to_string(), lexeme, 0, u32::MAX),
                            false,
                        )
                    }),
                };
                tokens.push(op_token_with_post(
                    operation,
                    start,
                    end,
                    check_for_post_traverse,
                ));
            }
            _ => return Err(LexerError::UnexpectedCharacter { offset: start, ch }),
        }
    }

    Ok(tokens)
}

fn punct(kind: TokenKind, lexeme: &str, start: usize) -> Token {
    punct_with_post(kind, lexeme, start, false)
}

fn punct_with_post(
    kind: TokenKind,
    lexeme: &str,
    start: usize,
    check_for_post_traverse: bool,
) -> Token {
    Token {
        kind,
        lexeme: lexeme.to_string(),
        start_offset: start,
        end_offset: start + lexeme.len(),
        check_for_post_traverse,
        assign_operation: None,
    }
}

fn op_token(operation: Operation, start: usize, end: usize) -> Token {
    op_token_with_post(operation, start, end, false)
}

fn op_token_with_post(
    operation: Operation,
    start: usize,
    end: usize,
    check_for_post_traverse: bool,
) -> Token {
    let lexeme = operation.string_value.clone();
    Token {
        kind: TokenKind::Operation(operation),
        lexeme,
        start_offset: start,
        end_offset: end,
        check_for_post_traverse,
        assign_operation: None,
    }
}

pub fn unwrap(input: &str) -> String {
    if input.len() < 2 {
        return String::new();
    }
    let bytes = input.as_bytes();
    let quote = bytes[0];
    if (quote != b'"' && quote != b'\'') || bytes[input.len() - 1] != quote {
        return String::new();
    }
    input[1..input.len() - 1].replace("\\\"", "\"")
}

pub fn extract_number_parameter(input: &str) -> Result<i32, ParameterError> {
    let Some(open) = input.find('(') else {
        return Err(ParameterError::MissingParameter);
    };
    let Some(close) = input[open + 1..].find(')') else {
        return Err(ParameterError::MissingParameter);
    };
    let param = input[open + 1..open + 1 + close].trim();
    if param.is_empty() {
        return Err(ParameterError::MissingParameter);
    }
    param
        .parse::<i32>()
        .map_err(|_| ParameterError::InvalidCharacter)
}

pub fn has_option_parameter(input: &str, option: &str) -> bool {
    let Some(open) = input.find('(') else {
        return false;
    };
    let Some(close) = input[open + 1..].find(')') else {
        return false;
    };
    input[open + 1..open + 1 + close]
        .split(',')
        .map(str::trim)
        .any(|candidate| candidate == option)
}

pub fn post_process_tokens(input: &[Token]) -> Vec<Token> {
    let mut out = Vec::new();
    let mut index = 0usize;

    while index < input.len() {
        let token = &input[index];
        let mut current = token.clone();

        // SELF + TRAVERSE_ARRAY, then continue processing the current token
        // as a synthetic open_collect so the normal post-processing rules still apply.
        if matches!(current.kind, TokenKind::TraverseArrayCollect) {
            out.push(make_operation_token(
                Operation::new(OperationId::SelfRef, "self", 0, u32::MAX),
                current.start_offset,
                current.start_offset + 1,
            ));
            out.push(make_operation_token(
                Operation::binary(OperationId::TraverseArray, "traverse_array", 50),
                current.start_offset,
                current.end_offset,
            ));
            current = make_simple_token(
                TokenKind::OpenBracket,
                "[",
                current.start_offset,
                current.end_offset,
            );
        }

        // .[:n] desugars to .[0:n]
        if matches!(
            current.kind,
            TokenKind::Operation(ref op) if op.operation_type.id == OperationId::CreateMap
        ) && matches!(
            input.get(index.wrapping_sub(1)).map(|prev| &prev.kind),
            Some(TokenKind::TraverseArrayCollect)
        ) {
            out.push(make_operation_token(
                Operation::value("0"),
                current.start_offset,
                current.start_offset,
            ));
        }

        // Compound-assign: if token has assign_operation and next is assign, merge
        if current.assign_operation.is_some()
            && matches!(
                input.get(index + 1).map(|next| &next.kind),
                Some(TokenKind::Operation(op))
                    if op.operation_type.id == OperationId::Assign
            )
        {
            let assign_op = current.assign_operation.clone().unwrap();
            out.push(make_operation_token(
                assign_op,
                current.start_offset,
                current.end_offset,
            ));
            index += 2; // skip the assign token
            continue;
        }

        // Push current token
        out.push(current.clone());

        // create_map followed by close_collect → insert length_op_type
        if matches!(
            current.kind,
            TokenKind::Operation(ref op) if op.operation_type.id == OperationId::CreateMap
        ) {
            if matches!(
                input.get(index + 1).map(|next| &next.kind),
                Some(TokenKind::CloseBracket)
            ) {
                out.push(make_operation_token(
                    Operation::new(OperationId::Length, "", 0, 50),
                    current.start_offset,
                    current.end_offset,
                ));
            }
        }

        // Empty collect: open_collect/open_collect_object followed by close → insert empty
        if matches!(
            current.kind,
            TokenKind::OpenBracket | TokenKind::OpenCollectObject
        ) && matches!(
            input.get(index + 1).map(|next| &next.kind),
            Some(TokenKind::CloseBracket) | Some(TokenKind::CloseCollectObject)
        ) {
            out.push(make_operation_token(
                Operation::new(OperationId::Empty, "empty", 0, u32::MAX),
                current.end_offset,
                current.end_offset,
            ));
        }

        // check_for_post_traverse + traverse_path → short_pipe
        if current.check_for_post_traverse
            && matches!(
                input.get(index + 1).map(|next| &next.kind),
                Some(TokenKind::Operation(op))
                    if op.operation_type.id == OperationId::TraversePath
            )
        {
            out.push(make_operation_token(
                Operation::binary(OperationId::ShortPipe, "short_pipe", 45),
                current.end_offset,
                current.end_offset,
            ));
        }

        // check_for_post_traverse + traverse_array_collect → short_pipe
        if current.check_for_post_traverse
            && matches!(
                input.get(index + 1).map(|next| &next.kind),
                Some(TokenKind::TraverseArrayCollect)
            )
        {
            out.push(make_operation_token(
                Operation::binary(OperationId::ShortPipe, "short_pipe", 45),
                current.end_offset,
                current.end_offset,
            ));
        }

        // check_for_post_traverse + open_collect → traverse_array
        if current.check_for_post_traverse
            && matches!(
                input.get(index + 1).map(|next| &next.kind),
                Some(TokenKind::OpenBracket)
            )
        {
            out.push(make_operation_token(
                Operation::binary(OperationId::TraverseArray, "traverse_array", 50),
                current.end_offset,
                current.end_offset,
            ));
        }

        index += 1;
    }

    out
}

pub fn make_simple_token(kind: TokenKind, lexeme: &str, start: usize, end: usize) -> Token {
    Token {
        kind,
        lexeme: lexeme.to_string(),
        start_offset: start,
        end_offset: end,
        check_for_post_traverse: false,
        assign_operation: None,
    }
}

pub fn make_operation_token(operation: Operation, start: usize, end: usize) -> Token {
    Token {
        lexeme: operation.string_value.clone(),
        kind: TokenKind::Operation(operation),
        start_offset: start,
        end_offset: end,
        check_for_post_traverse: false,
        assign_operation: None,
    }
}

pub fn make_operation_token_with_assign(
    operation: Operation,
    assign_operation: Option<Operation>,
    start: usize,
    end: usize,
) -> Token {
    Token {
        lexeme: operation.string_value.clone(),
        kind: TokenKind::Operation(operation),
        start_offset: start,
        end_offset: end,
        check_for_post_traverse: false,
        assign_operation,
    }
}

fn builtin_operation(name: &str, string_value: &str) -> Option<(Operation, bool)> {
    let operation = match name {
        "to_entries" => Operation::new(OperationId::ToEntries, string_value, 0, 52),
        "from_entries" | "fromEntries" => {
            Operation::new(OperationId::FromEntries, string_value, 0, 50)
        }
        "with_entries" | "withEntries" => {
            Operation::new(OperationId::WithEntries, string_value, 1, 50)
        }
        "select" => Operation::new(OperationId::Select, string_value, 1, 52),
        "filter" => Operation::new(OperationId::Filter, string_value, 1, 52),
        "has" => Operation::new(OperationId::Has, string_value, 1, 50),
        "contains" => Operation::new(OperationId::Contains, string_value, 1, 50),
        "path" => Operation::new(OperationId::GetPath, string_value, 1, 52),
        "setpath" => Operation::new(OperationId::SetPath, string_value, 1, 50),
        "parent" => Operation::new(OperationId::GetParent, string_value, 0, 50),
        "parents" => Operation::new(OperationId::GetParents, string_value, 0, 50),
        "type" | "tag" => Operation::new(OperationId::GetTag, string_value, 0, 50),
        "kind" => Operation::new(OperationId::GetKind, string_value, 0, 50),
        "map_values" => Operation::new(OperationId::MapValues, string_value, 1, 52),
        "map" => Operation::new(OperationId::Map, string_value, 1, 52),
        "with" => Operation::new(OperationId::With, string_value, 1, 52),
        "any_c" => Operation::new(OperationId::AnyCondition, string_value, 1, 50),
        "all_c" => Operation::new(OperationId::AllCondition, string_value, 1, 50),
        "any" => Operation::new(OperationId::Any, string_value, 0, 50),
        "all" => Operation::new(OperationId::All, string_value, 0, 50),
        "min" => Operation::new(OperationId::Min, string_value, 0, 40),
        "max" => Operation::new(OperationId::Max, string_value, 0, 40),
        "to_yaml" | "to_json" | "to_toml" | "to_csv" | "to_uri" => {
            Operation::new(OperationId::Encode, string_value, 0, 50)
        }
        "from_yaml" | "from_json" | "from_csv" | "from_uri" => {
            Operation::new(OperationId::Decode, string_value, 0, 50)
        }
        "sub" => Operation::new(OperationId::Substr, string_value, 1, 50),
        "match" => Operation::new(OperationId::Match, string_value, 1, 50),
        "capture" => Operation::new(OperationId::Capture, string_value, 1, 50),
        "test" => Operation::new(OperationId::Test, string_value, 1, 50),
        "split" => Operation::new(OperationId::Split, string_value, 1, 52),
        "trim" => Operation::new(OperationId::Trim, string_value, 0, 50),
        "to_string" | "tostring" => Operation::new(OperationId::ToString, string_value, 0, 50),
        "upcase" | "ascii_upcase" | "asciiupcase" | "downcase" | "ascii_downcase"
        | "asciidowncase" => Operation::new(OperationId::ChangeCase, string_value, 0, 50),
        "to_number" | "tonumber" => Operation::new(OperationId::ToNumber, string_value, 0, 50),
        "key" => Operation::new(OperationId::GetKey, string_value, 0, 50),
        "keys" => Operation::new(OperationId::Keys, string_value, 0, 52),
        "length" => Operation::new(OperationId::Length, string_value, 0, 50),
        "is_key" | "iskey" => Operation::new(OperationId::IsKey, string_value, 0, 50),
        "unique" => Operation::new(OperationId::Unique, string_value, 0, 52),
        "unique_by" => Operation::new(OperationId::UniqueBy, string_value, 1, 52),
        "sort_by" => Operation::new(OperationId::SortBy, string_value, 1, 52),
        "sort" => Operation::new(OperationId::Sort, string_value, 0, 52),
        "sort_keys" | "sortKeys" => Operation::new(OperationId::SortKeys, string_value, 1, 52),
        "first" => Operation::new(OperationId::First, string_value, 1, 52),
        "reverse" => Operation::new(OperationId::Reverse, string_value, 0, 52),
        "shuffle" => Operation::new(OperationId::Shuffle, string_value, 0, 52),
        "join" => Operation::new(OperationId::Join, string_value, 1, 50),
        "flatten" => Operation::new(OperationId::Flatten, string_value, 0, 52),
        "pick" => Operation::new(OperationId::Pick, string_value, 1, 52),
        "omit" => Operation::new(OperationId::Omit, string_value, 1, 52),
        "group_by" => Operation::new(OperationId::GroupBy, string_value, 1, 52),
        "delpaths" => Operation::new(OperationId::DelPaths, string_value, 1, 52),
        "del" => Operation::new(OperationId::Delete, string_value, 1, 40),
        "as" => Operation::new(OperationId::AssignVariable, string_value, 2, 40),
        "reduce" => Operation::new(OperationId::Reduce, string_value, 2, 35),
        _ => return None,
    };

    let check_for_post_traverse = matches!(
        name,
        "to_entries"
            | "select"
            | "filter"
            | "sub"
            | "match"
            | "capture"
            | "test"
            | "path"
            | "map_values"
            | "map"
            | "with"
            | "to_yaml"
            | "to_json"
            | "to_toml"
            | "to_csv"
            | "to_uri"
            | "from_yaml"
            | "from_json"
            | "from_csv"
            | "from_uri"
            | "keys"
            | "unique"
            | "unique_by"
            | "sort_by"
            | "sort"
            | "sort_keys"
            | "sortKeys"
            | "first"
            | "reverse"
            | "shuffle"
            | "join"
            | "flatten"
            | "pick"
            | "omit"
            | "group_by"
            | "delpaths"
    );

    Some((operation, check_for_post_traverse))
}

fn consume_call_suffix(input: &str, start: usize) -> Result<usize, LexerError> {
    let bytes = input.as_bytes();
    let mut depth = 0usize;
    let mut i = start;
    let mut quote = None;
    let mut escaped = false;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            i += 1;
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                i += 1;
            }
            '(' => {
                depth += 1;
                i += 1;
            }
            ')' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return Ok(i);
                }
            }
            _ => i += 1,
        }
    }

    Err(LexerError::UnexpectedCharacter {
        offset: start,
        ch: '(',
    })
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::expression::OperationId;

    #[test]
    fn lexes_binary_expression_without_dependencies() {
        let tokens = lex("1 + 2 # trailing comment").expect("lex should succeed");

        assert_eq!(tokens.len(), 3);
        match &tokens[1].kind {
            TokenKind::Operation(operation) => {
                assert_eq!(operation.operation_type.id, OperationId::Add)
            }
            _ => panic!("expected operator token"),
        }
    }

    #[test]
    fn lexes_dot_path_and_self_reference() {
        let tokens = lex(".a | .").expect("lex should succeed");

        assert_eq!(tokens.len(), 3);
        match &tokens[0].kind {
            TokenKind::Operation(operation) => {
                assert_eq!(operation.operation_type.id, OperationId::TraversePath);
                assert_eq!(operation.string_value, "a");
            }
            _ => panic!("expected traverse path token"),
        }
        match &tokens[2].kind {
            TokenKind::Operation(operation) => {
                assert_eq!(operation.operation_type.id, OperationId::SelfRef);
            }
            _ => panic!("expected self reference token"),
        }
    }
}
