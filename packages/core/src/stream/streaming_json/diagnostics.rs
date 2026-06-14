use crate::core::ParseError;

use super::parser::JsonStreamError;
use super::scanner::{Position, Scanner, Token, TokenTag};

const TOKEN_TYPE_KEY: u32 = 1;
const TOKEN_TYPE_STRING: u32 = 3;
const TOKEN_TYPE_INT: u32 = 4;
const TOKEN_TYPE_BOOLEAN: u32 = 6;
const TOKEN_TYPE_NULL: u32 = 7;
const TOKEN_TYPE_PUNCTUATION: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenSpan {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
    pub token_type: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorSpan {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
    pub kind: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseFailure {
    pub start_byte: usize,
    pub end_byte: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerKind {
    Object,
    Array,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectPhase {
    KeyOrEnd,
    Colon,
    Value,
    CommaOrEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArrayPhase {
    ValueOrEnd,
    Value,
    CommaOrEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Object(ObjectPhase),
    Array(ArrayPhase),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContainerState {
    kind: ContainerKind,
    phase: Phase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScannerFailure {
    byte_offset: usize,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompleteDocumentResult {
    range: Option<DocRange>,
    failure: Option<ScannerFailure>,
}

pub fn split_documents(source: &str) -> Result<Vec<DocRange>, JsonStreamError> {
    let result = complete_document_result(source);
    if let Some(failure) = result.failure {
        return Err(scanner_failure_to_error(source, failure));
    }
    Ok(result.range.into_iter().collect())
}

pub fn first_parse_failure(source: &str, nest_json: bool) -> Option<ParseFailure> {
    let _ = nest_json;
    complete_document_result(source)
        .failure
        .map(|failure| ParseFailure {
            start_byte: failure.byte_offset,
            end_byte: {
                let source_bytes = source.as_bytes();
                let mut end = failure.byte_offset;
                let scan_limit = end + 100;
                while end < source.len()
                    && end < scan_limit
                    && !matches!(
                        source_bytes[end],
                        b'{' | b'}'
                            | b'['
                            | b']'
                            | b':'
                            | b','
                            | b'"'
                            | b' '
                            | b'\t'
                            | b'\n'
                            | b'\r'
                    )
                {
                    end += 1;
                }
                source.len().min(end.max(failure.byte_offset + 1))
            },
            line: failure.line,
            column: failure.column,
        })
}

pub fn token_spans(source: &str) -> Result<Vec<TokenSpan>, JsonStreamError> {
    if let Some(failure) = complete_document_result(source).failure {
        return Err(scanner_failure_to_error(source, failure));
    }
    scan_token_spans(source)
}

pub fn error_spans(source: &str) -> Vec<ErrorSpan> {
    first_parse_failure(source, false)
        .map(|failure| {
            let (start_row, start_col) = zero_based_line_column(source, failure.start_byte);
            let (end_row, end_col) = zero_based_line_column(source, failure.end_byte);
            vec![ErrorSpan {
                start_row,
                start_col,
                end_row,
                end_col,
                kind: 1,
            }]
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TextPosition {
    row: u32,
    column: u32,
}

/// Scan token spans in one forward pass.
///
/// Complexity contract: this MUST stay O(source.len()) for large documents.
/// Do not recompute line/column by rescanning from the beginning of `source`
/// for each token/span; that regresses close-time semantic-token generation to
/// O(n^2) and reintroduces the large-file hang fixed by this path.
fn scan_token_spans(source: &str) -> Result<Vec<TokenSpan>, JsonStreamError> {
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    let mut position = TextPosition::default();
    let mut stack = Vec::<ContainerState>::new();

    while cursor < source.len() {
        let ws_start = cursor;
        skip_ws(source, &mut cursor);
        position = advance_text_position(position, &source[ws_start..cursor]);
        if cursor >= source.len() {
            break;
        }

        let start = cursor;
        let start_position = position;
        let ch = peek(source, cursor).ok_or(JsonStreamError::UnexpectedEnd)?;
        match ch {
            '{' => {
                cursor += 1;
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    TOKEN_TYPE_PUNCTUATION,
                    &mut position,
                );
                update_parent_after_value(&mut stack);
                stack.push(ContainerState {
                    kind: ContainerKind::Object,
                    phase: Phase::Object(ObjectPhase::KeyOrEnd),
                });
            }
            '}' => {
                cursor += 1;
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    TOKEN_TYPE_PUNCTUATION,
                    &mut position,
                );
                stack.pop();
                set_parent_comma_or_end(&mut stack);
            }
            '[' => {
                cursor += 1;
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    TOKEN_TYPE_PUNCTUATION,
                    &mut position,
                );
                update_parent_after_value(&mut stack);
                stack.push(ContainerState {
                    kind: ContainerKind::Array,
                    phase: Phase::Array(ArrayPhase::ValueOrEnd),
                });
            }
            ']' => {
                cursor += 1;
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    TOKEN_TYPE_PUNCTUATION,
                    &mut position,
                );
                stack.pop();
                set_parent_comma_or_end(&mut stack);
            }
            ':' => {
                cursor += 1;
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    TOKEN_TYPE_PUNCTUATION,
                    &mut position,
                );
                if let Some(ContainerState {
                    phase: Phase::Object(phase),
                    ..
                }) = stack.last_mut()
                {
                    *phase = ObjectPhase::Value;
                }
            }
            ',' => {
                cursor += 1;
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    TOKEN_TYPE_PUNCTUATION,
                    &mut position,
                );
                if let Some(top) = stack.last_mut() {
                    match top.kind {
                        ContainerKind::Object => top.phase = Phase::Object(ObjectPhase::KeyOrEnd),
                        ContainerKind::Array => top.phase = Phase::Array(ArrayPhase::Value),
                    }
                }
            }
            '"' => {
                cursor = scan_string(source, cursor)?;
                let token_type = if matches!(
                    stack.last(),
                    Some(ContainerState {
                        phase: Phase::Object(ObjectPhase::KeyOrEnd),
                        ..
                    })
                ) {
                    TOKEN_TYPE_KEY
                } else {
                    TOKEN_TYPE_STRING
                };
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    token_type,
                    &mut position,
                );
                update_parent_after_scalar(&mut stack, token_type == TOKEN_TYPE_KEY);
            }
            '-' | '0'..='9' => {
                cursor = scan_number(source, cursor)?;
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    TOKEN_TYPE_INT,
                    &mut position,
                );
                update_parent_after_value(&mut stack);
            }
            't' => {
                cursor = consume_literal(source, cursor, "true")?;
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    TOKEN_TYPE_BOOLEAN,
                    &mut position,
                );
                update_parent_after_value(&mut stack);
            }
            'f' => {
                cursor = consume_literal(source, cursor, "false")?;
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    TOKEN_TYPE_BOOLEAN,
                    &mut position,
                );
                update_parent_after_value(&mut stack);
            }
            'n' => {
                cursor = consume_literal(source, cursor, "null")?;
                emit_token_span(
                    &mut spans,
                    start_position,
                    &source[start..cursor],
                    TOKEN_TYPE_NULL,
                    &mut position,
                );
                update_parent_after_value(&mut stack);
            }
            found => {
                return Err(JsonStreamError::UnexpectedToken {
                    offset: cursor,
                    found,
                });
            }
        }
    }

    Ok(spans)
}

fn complete_document_result(source: &str) -> CompleteDocumentResult {
    let mut scanner = Scanner::new(None, false, None);
    scanner.feed(source);

    let mut first_start = None;
    let mut last_end = 0usize;
    let mut stack = Vec::<ContainerState>::new();
    let mut has_root = false;
    let mut root_complete = false;

    loop {
        let tok = scanner.next_token(true);
        match tok.tag {
            TokenTag::Eof => {
                if !stack.is_empty() || (has_root && !root_complete) {
                    return CompleteDocumentResult {
                        range: None,
                        failure: Some(failure_at_offset(source, source.len())),
                    };
                }
                return CompleteDocumentResult {
                    range: first_start.map(|start| DocRange {
                        start,
                        end: last_end,
                    }),
                    failure: None,
                };
            }
            TokenTag::None => continue,
            TokenTag::Comment | TokenTag::Invalid => {
                return CompleteDocumentResult {
                    range: None,
                    failure: Some(failure_at_position(tok.span.start)),
                };
            }
            _ => {
                if first_start.is_none() {
                    first_start = Some(tok.span.start.offset);
                }
                if root_complete && stack.is_empty() {
                    return CompleteDocumentResult {
                        range: None,
                        failure: Some(failure_at_position(tok.span.start)),
                    };
                }
                if !apply_structural_token(&mut stack, &tok, &mut has_root, &mut root_complete) {
                    return CompleteDocumentResult {
                        range: None,
                        failure: Some(failure_at_position(tok.span.start)),
                    };
                }
                last_end = tok.span.end.offset;
            }
        }
    }
}

fn failure_at_position(pos: Position) -> ScannerFailure {
    ScannerFailure {
        byte_offset: pos.offset,
        line: pos.line as usize + 1,
        column: pos.column as usize + 1,
    }
}

fn failure_at_offset(source: &str, offset: usize) -> ScannerFailure {
    let (line, column) = line_column_at(source, offset);
    ScannerFailure {
        byte_offset: offset.min(source.len()),
        line,
        column,
    }
}

fn scanner_failure_to_error(source: &str, failure: ScannerFailure) -> JsonStreamError {
    if failure.byte_offset >= source.len() {
        return JsonStreamError::UnexpectedEnd;
    }
    JsonStreamError::UnexpectedToken {
        offset: failure.byte_offset,
        found: source[failure.byte_offset..].chars().next().unwrap_or('\0'),
    }
}

fn apply_structural_token(
    stack: &mut Vec<ContainerState>,
    tok: &Token,
    has_root: &mut bool,
    root_complete: &mut bool,
) -> bool {
    if stack.is_empty() {
        return apply_top_level_token(stack, tok, has_root, root_complete);
    }
    match stack.last().map(|item| item.kind) {
        Some(ContainerKind::Object) => apply_object_token(stack, tok, root_complete),
        Some(ContainerKind::Array) => apply_array_token(stack, tok, root_complete),
        None => false,
    }
}

fn apply_top_level_token(
    stack: &mut Vec<ContainerState>,
    tok: &Token,
    has_root: &mut bool,
    root_complete: &mut bool,
) -> bool {
    if *has_root {
        return false;
    }
    *has_root = true;
    match tok.tag {
        TokenTag::LeftBrace => {
            stack.push(ContainerState {
                kind: ContainerKind::Object,
                phase: Phase::Object(ObjectPhase::KeyOrEnd),
            });
            *root_complete = false;
            true
        }
        TokenTag::LeftBracket => {
            stack.push(ContainerState {
                kind: ContainerKind::Array,
                phase: Phase::Array(ArrayPhase::ValueOrEnd),
            });
            *root_complete = false;
            true
        }
        TokenTag::String
        | TokenTag::Number
        | TokenTag::TrueKw
        | TokenTag::FalseKw
        | TokenTag::NullKw => {
            *root_complete = true;
            true
        }
        _ => {
            *has_root = false;
            false
        }
    }
}

fn apply_object_token(
    stack: &mut Vec<ContainerState>,
    tok: &Token,
    root_complete: &mut bool,
) -> bool {
    let Some(top) = stack.last_mut() else {
        return false;
    };
    let Phase::Object(phase) = top.phase else {
        return false;
    };
    match phase {
        ObjectPhase::KeyOrEnd => match tok.tag {
            TokenTag::RightBrace => {
                close_container(stack, root_complete);
                true
            }
            TokenTag::String => {
                top.phase = Phase::Object(ObjectPhase::Colon);
                true
            }
            _ => false,
        },
        ObjectPhase::Colon => match tok.tag {
            TokenTag::Colon => {
                top.phase = Phase::Object(ObjectPhase::Value);
                true
            }
            _ => false,
        },
        ObjectPhase::Value => apply_value_token(stack, tok, ContainerKind::Object, root_complete),
        ObjectPhase::CommaOrEnd => match tok.tag {
            TokenTag::Comma => {
                top.phase = Phase::Object(ObjectPhase::KeyOrEnd);
                true
            }
            TokenTag::RightBrace => {
                close_container(stack, root_complete);
                true
            }
            _ => false,
        },
    }
}

fn apply_array_token(
    stack: &mut Vec<ContainerState>,
    tok: &Token,
    root_complete: &mut bool,
) -> bool {
    let Some(top) = stack.last_mut() else {
        return false;
    };
    let Phase::Array(phase) = top.phase else {
        return false;
    };
    match phase {
        ArrayPhase::ValueOrEnd => match tok.tag {
            TokenTag::RightBracket => {
                close_container(stack, root_complete);
                true
            }
            _ => apply_value_token(stack, tok, ContainerKind::Array, root_complete),
        },
        ArrayPhase::Value => apply_value_token(stack, tok, ContainerKind::Array, root_complete),
        ArrayPhase::CommaOrEnd => match tok.tag {
            TokenTag::Comma => {
                top.phase = Phase::Array(ArrayPhase::Value);
                true
            }
            TokenTag::RightBracket => {
                close_container(stack, root_complete);
                true
            }
            _ => false,
        },
    }
}

fn apply_value_token(
    stack: &mut Vec<ContainerState>,
    tok: &Token,
    parent_kind: ContainerKind,
    root_complete: &mut bool,
) -> bool {
    match tok.tag {
        TokenTag::LeftBrace => {
            stack.push(ContainerState {
                kind: ContainerKind::Object,
                phase: Phase::Object(ObjectPhase::KeyOrEnd),
            });
            *root_complete = false;
            true
        }
        TokenTag::LeftBracket => {
            stack.push(ContainerState {
                kind: ContainerKind::Array,
                phase: Phase::Array(ArrayPhase::ValueOrEnd),
            });
            *root_complete = false;
            true
        }
        TokenTag::String
        | TokenTag::Number
        | TokenTag::TrueKw
        | TokenTag::FalseKw
        | TokenTag::NullKw => {
            let Some(top) = stack.last_mut() else {
                return false;
            };
            top.phase = match parent_kind {
                ContainerKind::Object => Phase::Object(ObjectPhase::CommaOrEnd),
                ContainerKind::Array => Phase::Array(ArrayPhase::CommaOrEnd),
            };
            *root_complete = stack.len() == 1;
            true
        }
        _ => false,
    }
}

fn close_container(stack: &mut Vec<ContainerState>, root_complete: &mut bool) {
    stack.pop();
    if stack.is_empty() {
        *root_complete = true;
        return;
    }
    if let Some(parent) = stack.last_mut() {
        parent.phase = match parent.kind {
            ContainerKind::Object => Phase::Object(ObjectPhase::CommaOrEnd),
            ContainerKind::Array => Phase::Array(ArrayPhase::CommaOrEnd),
        };
    }
}

fn update_parent_after_scalar(stack: &mut [ContainerState], is_key: bool) {
    let Some(top) = stack.last_mut() else {
        return;
    };

    match (&mut top.phase, is_key) {
        (Phase::Object(phase), true) => *phase = ObjectPhase::Colon,
        (Phase::Object(phase), false) => *phase = ObjectPhase::CommaOrEnd,
        (Phase::Array(phase), false) => *phase = ArrayPhase::CommaOrEnd,
        _ => {}
    }
}

fn update_parent_after_value(stack: &mut [ContainerState]) {
    let Some(top) = stack.last_mut() else {
        return;
    };

    match &mut top.phase {
        Phase::Object(phase) => *phase = ObjectPhase::CommaOrEnd,
        Phase::Array(phase) => *phase = ArrayPhase::CommaOrEnd,
    }
}

fn set_parent_comma_or_end(stack: &mut [ContainerState]) {
    let Some(parent) = stack.last_mut() else {
        return;
    };

    match &mut parent.phase {
        Phase::Object(phase) => *phase = ObjectPhase::CommaOrEnd,
        Phase::Array(phase) => *phase = ArrayPhase::CommaOrEnd,
    }
}

fn scan_string(source: &str, start: usize) -> Result<usize, JsonStreamError> {
    let bytes = source.as_bytes();
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' => return Ok(cursor + 1),
            b'\\' => {
                cursor += 1;
                if cursor >= bytes.len() {
                    return Err(JsonStreamError::UnexpectedEnd);
                }
                if bytes[cursor] == b'u' {
                    cursor += 4;
                    if cursor >= bytes.len() {
                        return Err(JsonStreamError::InvalidUnicodeEscape { offset: start });
                    }
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    Err(JsonStreamError::UnterminatedString { offset: start })
}

fn scan_number(source: &str, start: usize) -> Result<usize, JsonStreamError> {
    let bytes = source.as_bytes();
    let mut cursor = start;

    if bytes.get(cursor) == Some(&b'-') {
        cursor += 1;
    }
    match bytes.get(cursor) {
        Some(b'0') => {
            cursor += 1;
            if matches!(bytes.get(cursor), Some(b'0'..=b'9')) {
                return Err(JsonStreamError::InvalidNumber { offset: start });
            }
        }
        Some(b'1'..=b'9') => {
            cursor += 1;
            while matches!(bytes.get(cursor), Some(b'0'..=b'9')) {
                cursor += 1;
            }
        }
        _ => return Err(JsonStreamError::InvalidNumber { offset: start }),
    }

    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        let digit_start = cursor;
        while matches!(bytes.get(cursor), Some(b'0'..=b'9')) {
            cursor += 1;
        }
        if digit_start == cursor {
            return Err(JsonStreamError::InvalidNumber { offset: start });
        }
    }

    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        cursor += 1;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            cursor += 1;
        }
        let digit_start = cursor;
        while matches!(bytes.get(cursor), Some(b'0'..=b'9')) {
            cursor += 1;
        }
        if digit_start == cursor {
            return Err(JsonStreamError::InvalidNumber { offset: start });
        }
    }

    Ok(cursor)
}

fn consume_literal(source: &str, start: usize, expected: &str) -> Result<usize, JsonStreamError> {
    let end = start + expected.len();
    if source.get(start..end) == Some(expected) {
        Ok(end)
    } else {
        let found = peek(source, start).unwrap_or('\0');
        Err(JsonStreamError::UnexpectedToken {
            offset: start,
            found,
        })
    }
}

fn token_span(start: TextPosition, end: TextPosition, token_type: u32) -> TokenSpan {
    TokenSpan {
        start_row: start.row,
        start_col: start.column,
        end_row: end.row,
        end_col: end.column,
        token_type,
    }
}

fn emit_token_span(
    spans: &mut Vec<TokenSpan>,
    start: TextPosition,
    fragment: &str,
    token_type: u32,
    position: &mut TextPosition,
) {
    let end = advance_text_position(start, fragment);
    spans.push(token_span(start, end, token_type));
    *position = end;
}

fn advance_text_position(mut position: TextPosition, fragment: &str) -> TextPosition {
    for &byte in fragment.as_bytes() {
        if byte == b'\n' {
            position.row += 1;
            position.column = 0;
        } else {
            position.column += 1;
        }
    }
    position
}

fn zero_based_line_column(source: &str, offset: usize) -> (u32, u32) {
    let (line, column) = line_column_at(source, offset);
    (
        (line.saturating_sub(1)) as u32,
        (column.saturating_sub(1)) as u32,
    )
}

fn line_column_at(source: &str, raw_offset: usize) -> (usize, usize) {
    let offset = raw_offset.min(source.len());
    let mut line = 1usize;
    let mut column = 1usize;
    for (index, ch) in source.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn skip_ws(source: &str, cursor: &mut usize) {
    while let Some(ch) = peek(source, *cursor) {
        if !ch.is_ascii_whitespace() {
            break;
        }
        *cursor += ch.len_utf8();
    }
}

fn peek(source: &str, cursor: usize) -> Option<char> {
    source.get(cursor..)?.chars().next()
}

// ---------------------------------------------------------------------------
// normalize_source — BOM removal + UTF-16 → UTF-8 transcoding
// ---------------------------------------------------------------------------

/// Normalize a raw JSON source buffer:
/// 1. Strip UTF-8 BOM (`EF BB BF`) if present.
/// 2. Detect and transcode UTF-16 (LE/BE, with or without BOM) to UTF-8.
/// 3. Otherwise return the source as a UTF-8 string.
pub fn normalize_source(source: &[u8]) -> Result<String, ParseError> {
    // UTF-8 BOM
    if source.len() >= 3 && source[0] == 0xEF && source[1] == 0xBB && source[2] == 0xBF {
        return String::from_utf8(source[3..].to_vec()).map_err(|_| ParseError::InvalidSyntax);
    }

    // UTF-16 with BOM
    if let Some(enc) = detect_utf16_bom(source) {
        return transcode_utf16_to_utf8(&source[enc.skip..], enc.little_endian);
    }

    // UTF-16 heuristic (no BOM)
    if let Some(enc) = detect_utf16_heuristic(source) {
        return transcode_utf16_to_utf8(&source[enc.skip..], enc.little_endian);
    }

    String::from_utf8(source.to_vec()).map_err(|_| ParseError::InvalidSyntax)
}

struct Utf16Encoding {
    little_endian: bool,
    skip: usize,
}

fn detect_utf16_bom(source: &[u8]) -> Option<Utf16Encoding> {
    if source.len() >= 2 {
        if source[0] == 0xFF && source[1] == 0xFE {
            return Some(Utf16Encoding {
                little_endian: true,
                skip: 2,
            });
        }
        if source[0] == 0xFE && source[1] == 0xFF {
            return Some(Utf16Encoding {
                little_endian: false,
                skip: 2,
            });
        }
    }
    None
}

fn detect_utf16_heuristic(source: &[u8]) -> Option<Utf16Encoding> {
    if source.len() < 4 || source.len() % 2 != 0 {
        return None;
    }
    let mut even_zero: usize = 0;
    let mut odd_zero: usize = 0;
    let pairs = (source.len() / 2).min(8);
    for i in 0..pairs {
        if source[i * 2] == 0 {
            even_zero += 1;
        }
        if source[i * 2 + 1] == 0 {
            odd_zero += 1;
        }
    }
    if even_zero >= pairs.saturating_sub(1) && odd_zero == 0 {
        return Some(Utf16Encoding {
            little_endian: false,
            skip: 0,
        });
    }
    if odd_zero >= pairs.saturating_sub(1) && even_zero == 0 {
        return Some(Utf16Encoding {
            little_endian: true,
            skip: 0,
        });
    }
    None
}

fn transcode_utf16_to_utf8(source: &[u8], little_endian: bool) -> Result<String, ParseError> {
    if source.len() % 2 != 0 {
        return Err(ParseError::InvalidSyntax);
    }
    let mut out = String::new();
    let mut pending_high: Option<u16> = None;
    let mut i = 0;
    while i < source.len() {
        let code_unit = if little_endian {
            u16::from_le_bytes([source[i], source[i + 1]])
        } else {
            u16::from_be_bytes([source[i], source[i + 1]])
        };
        append_utf16_code_unit(&mut out, &mut pending_high, code_unit)?;
        i += 2;
    }
    if let Some(high) = pending_high {
        append_wtf8_code_unit(&mut out, high);
    }
    Ok(out)
}

fn append_utf16_code_unit(
    out: &mut String,
    pending_high: &mut Option<u16>,
    code_unit: u16,
) -> Result<(), ParseError> {
    if let Some(high) = pending_high.take() {
        if (0xDC00..=0xDFFF).contains(&code_unit) {
            // Valid surrogate pair
            let high_ten = (high as u32) - 0xD800;
            let low_ten = (code_unit as u32) - 0xDC00;
            let scalar = 0x10000 + ((high_ten << 10) | low_ten);
            append_unicode_scalar(out, scalar)?;
            return Ok(());
        }
        // Orphaned high surrogate — emit as WTF-8
        append_wtf8_code_unit(out, high);
    }
    if (0xD800..=0xDBFF).contains(&code_unit) {
        *pending_high = Some(code_unit);
        return Ok(());
    }
    if (0xDC00..=0xDFFF).contains(&code_unit) {
        // Orphaned low surrogate
        append_wtf8_code_unit(out, code_unit);
        return Ok(());
    }
    append_unicode_scalar(out, code_unit as u32)
}

fn append_unicode_scalar(out: &mut String, scalar: u32) -> Result<(), ParseError> {
    char::from_u32(scalar)
        .map(|ch| out.push(ch))
        .ok_or(ParseError::InvalidSyntax)
}

fn append_wtf8_code_unit(out: &mut String, value: u16) {
    if value <= 0x7F {
        out.push(value as u8 as char);
    } else if value <= 0x7FF {
        out.push(char::from_u32(0xC0u32 | (value as u32 >> 6)).unwrap_or('\u{FFFD}'));
        out.push(char::from_u32(0x80u32 | (value as u32 & 0x3F)).unwrap_or('\u{FFFD}'));
    } else {
        out.push(char::from_u32(0xE0u32 | (value as u32 >> 12)).unwrap_or('\u{FFFD}'));
        out.push(char::from_u32(0x80u32 | ((value as u32 >> 6) & 0x3F)).unwrap_or('\u{FFFD}'));
        out.push(char::from_u32(0x80u32 | (value as u32 & 0x3F)).unwrap_or('\u{FFFD}'));
    }
}

#[cfg(test)]
mod tests {
    use super::{TOKEN_TYPE_KEY, TOKEN_TYPE_STRING, token_spans};

    #[test]
    fn token_spans_preserve_byte_columns_for_utf8_multiline_json() {
        let spans = token_spans("{\n  \"键\": \"值\"\n}").expect("utf8 json should tokenize");

        assert!(spans.iter().any(|span| {
            span.start_row == 1
                && span.start_col == 2
                && span.end_row == 1
                && span.end_col == 7
                && span.token_type == TOKEN_TYPE_KEY
        }));
        assert!(spans.iter().any(|span| {
            span.start_row == 1
                && span.start_col == 9
                && span.end_row == 1
                && span.end_col == 14
                && span.token_type == TOKEN_TYPE_STRING
        }));
    }
}
