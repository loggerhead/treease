use std::io::Write;

use crate::core::{CoreError, LineIndex, NodeId, SemType, TokenSpan, TreeNodeKind, TreeStore};

use super::encoder_javascript::{is_js_identifier, is_safe_integer_literal};
use super::formats_helpers::write_quoted_string;
use super::preferences::FormatPreferences;
use super::smart_layout::encode_json_node_smart;
use super::{Encode, escape_json_string, is_truthy_literal, node, write_indent};

#[derive(Debug, Clone, Default)]
pub struct JsonEncoder {
    pub prefs: FormatPreferences,
}

impl JsonEncoder {
    pub fn new(prefs: FormatPreferences) -> Self {
        Self { prefs }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LanguageStyle {
    Json,
    Python,
    Javascript,
}

fn write_null(out: &mut String, style: LanguageStyle) {
    out.push_str(match style {
        LanguageStyle::Json | LanguageStyle::Javascript => "null",
        LanguageStyle::Python => "None",
    });
}

fn write_quoted(out: &mut String, value: &str, quote: char) {
    write_quoted_string(out, value, quote);
}

fn write_key(out: &mut String, node: &crate::core::TreeNode, style: LanguageStyle) {
    match style {
        LanguageStyle::Json => {
            out.push_str(&escape_json_string(&node.value));
        }
        LanguageStyle::Python => crate::formats::encoder_python::write_python_key(out, node),
        LanguageStyle::Javascript => {
            if is_js_identifier(&node.value) {
                out.push_str(&node.value);
            } else {
                write_quoted(out, &node.value, '\'');
            }
        }
    }
}

fn write_scalar(out: &mut String, node: &crate::core::TreeNode, style: LanguageStyle) {
    match style {
        LanguageStyle::Json => {
            out.push_str(&super::scalar_json_text(node).unwrap_or_else(|_| "null".to_owned()));
        }
        LanguageStyle::Python | LanguageStyle::Javascript => match node.resolved_sem_type() {
            Some(SemType::Nil) => write_null(out, style),
            Some(SemType::Boolean) => {
                let truthy = is_truthy_literal(&node.value);
                out.push_str(match style {
                    LanguageStyle::Python => {
                        if truthy {
                            "True"
                        } else {
                            "False"
                        }
                    }
                    LanguageStyle::Javascript => {
                        if truthy {
                            "true"
                        } else {
                            "false"
                        }
                    }
                    LanguageStyle::Json => unreachable!(),
                });
            }
            Some(SemType::Int) => match style {
                LanguageStyle::Python => out.push_str(&node.value),
                LanguageStyle::Javascript => {
                    if is_safe_integer_literal(&node.value) {
                        out.push_str(&node.value);
                    } else {
                        write_quoted(out, &node.value, '\'');
                    }
                }
                LanguageStyle::Json => unreachable!(),
            },
            Some(SemType::Float) => out.push_str(&node.value),
            None => write_null(out, style),
            _ => write_quoted(out, &node.value, '\''),
        },
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonNodeFormattedSpan {
    pub node_id: NodeId,
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonFormattedDocument {
    pub text: String,
    pub spans: Vec<JsonNodeFormattedSpan>,
    pub semantic_token_spans: Vec<TokenSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JsonFormattedTokenByteSpan {
    start_byte: u32,
    end_byte: u32,
    token_type: u32,
}

const TOKEN_TYPE_KEY: u32 = 1;
const TOKEN_TYPE_STRING: u32 = 3;
const TOKEN_TYPE_INT: u32 = 4;
const TOKEN_TYPE_BOOLEAN: u32 = 6;
const TOKEN_TYPE_NULL: u32 = 7;
const TOKEN_TYPE_PUNCTUATION: u32 = 8;

pub fn format_json_document_with_spans(
    decoded: &super::DecodedDocument,
    prefs: &FormatPreferences,
) -> Result<JsonFormattedDocument, CoreError> {
    let text = JsonEncoder::new(prefs.clone()).encode_to_string(&decoded.store, decoded.root)?;
    let mut spans = Vec::new();
    let mut semantic_token_byte_spans = Vec::new();
    let mut cursor = 0usize;
    let root = node(&decoded.store, decoded.root)?;
    if root.kind == TreeNodeKind::Scalar && prefs.unwrap_scalar {
        let end = root.value.len().min(text.len());
        spans.push(JsonNodeFormattedSpan {
            node_id: decoded.root,
            start_byte: 0,
            end_byte: end as u32,
        });
        push_scalar_token_byte_span(root, &text, 0, end, &mut semantic_token_byte_spans);
        let line_index = LineIndex::build(&text);
        let semantic_token_spans =
            encode_formatted_token_spans(&semantic_token_byte_spans, &line_index, text.len());
        return Ok(JsonFormattedDocument {
            text,
            spans,
            semantic_token_spans,
        });
    }
    record_formatted_span(
        &decoded.store,
        decoded.root,
        &text,
        &mut cursor,
        &mut spans,
        &mut semantic_token_byte_spans,
    )?;
    let line_index = LineIndex::build(&text);
    let semantic_token_spans =
        encode_formatted_token_spans(&semantic_token_byte_spans, &line_index, text.len());
    Ok(JsonFormattedDocument {
        text,
        spans,
        semantic_token_spans,
    })
}

fn record_formatted_span(
    store: &TreeStore,
    node_id: NodeId,
    text: &str,
    cursor: &mut usize,
    spans: &mut Vec<JsonNodeFormattedSpan>,
    semantic_token_byte_spans: &mut Vec<JsonFormattedTokenByteSpan>,
) -> Result<(), CoreError> {
    skip_json_ws(text, cursor);
    let start = *cursor;
    let current = node(store, node_id)?;
    match current.kind {
        TreeNodeKind::Scalar | TreeNodeKind::Unknown => {
            skip_json_value_token(text, cursor);
            push_scalar_token_byte_span(current, text, start, *cursor, semantic_token_byte_spans);
        }
        TreeNodeKind::Alias => {
            if let Some(alias) = current.alias {
                record_formatted_span(
                    store,
                    alias,
                    text,
                    cursor,
                    spans,
                    semantic_token_byte_spans,
                )?;
            } else {
                skip_json_value_token(text, cursor);
                push_scalar_token_byte_span(
                    current,
                    text,
                    start,
                    *cursor,
                    semantic_token_byte_spans,
                );
            }
        }
        TreeNodeKind::Sequence => {
            let bracket_start = *cursor;
            consume_json_byte(text, cursor, b'[')?;
            push_punctuation_token_byte_span(bracket_start, semantic_token_byte_spans);
            for (index, child) in current.content.iter().enumerate() {
                skip_json_ws(text, cursor);
                record_formatted_span(
                    store,
                    *child,
                    text,
                    cursor,
                    spans,
                    semantic_token_byte_spans,
                )?;
                skip_json_ws(text, cursor);
                if index + 1 < current.content.len() {
                    let comma_start = *cursor;
                    consume_json_byte(text, cursor, b',')?;
                    push_punctuation_token_byte_span(comma_start, semantic_token_byte_spans);
                }
            }
            skip_json_ws(text, cursor);
            let bracket_end = *cursor;
            consume_json_byte(text, cursor, b']')?;
            push_punctuation_token_byte_span(bracket_end, semantic_token_byte_spans);
        }
        TreeNodeKind::Mapping => {
            let brace_start = *cursor;
            consume_json_byte(text, cursor, b'{')?;
            push_punctuation_token_byte_span(brace_start, semantic_token_byte_spans);
            let pairs = current.content.chunks_exact(2);
            let pair_count = pairs.len();
            for (index, pair) in pairs.enumerate() {
                skip_json_ws(text, cursor);
                let key_start = *cursor;
                skip_json_string(text, cursor)?;
                let key_end = *cursor;
                spans.push(JsonNodeFormattedSpan {
                    node_id: pair[0],
                    start_byte: key_start as u32,
                    end_byte: key_end as u32,
                });
                semantic_token_byte_spans.push(JsonFormattedTokenByteSpan {
                    start_byte: key_start as u32,
                    end_byte: key_end as u32,
                    token_type: TOKEN_TYPE_KEY,
                });
                skip_json_ws(text, cursor);
                let colon_start = *cursor;
                consume_json_byte(text, cursor, b':')?;
                push_punctuation_token_byte_span(colon_start, semantic_token_byte_spans);
                skip_json_ws(text, cursor);
                record_formatted_span(
                    store,
                    pair[1],
                    text,
                    cursor,
                    spans,
                    semantic_token_byte_spans,
                )?;
                skip_json_ws(text, cursor);
                if index + 1 < pair_count {
                    let comma_start = *cursor;
                    consume_json_byte(text, cursor, b',')?;
                    push_punctuation_token_byte_span(comma_start, semantic_token_byte_spans);
                }
            }
            skip_json_ws(text, cursor);
            let brace_end = *cursor;
            consume_json_byte(text, cursor, b'}')?;
            push_punctuation_token_byte_span(brace_end, semantic_token_byte_spans);
        }
    }
    let end = *cursor;
    spans.push(JsonNodeFormattedSpan {
        node_id,
        start_byte: start as u32,
        end_byte: end as u32,
    });
    Ok(())
}

fn push_punctuation_token_byte_span(
    byte_offset: usize,
    spans: &mut Vec<JsonFormattedTokenByteSpan>,
) {
    spans.push(JsonFormattedTokenByteSpan {
        start_byte: byte_offset as u32,
        end_byte: byte_offset.saturating_add(1) as u32,
        token_type: TOKEN_TYPE_PUNCTUATION,
    });
}

fn push_scalar_token_byte_span(
    node: &crate::core::TreeNode,
    text: &str,
    start: usize,
    end: usize,
    spans: &mut Vec<JsonFormattedTokenByteSpan>,
) {
    if start >= end || end > text.len() {
        return;
    }
    let token_type = match node.resolved_sem_type() {
        Some(SemType::Boolean) => TOKEN_TYPE_BOOLEAN,
        Some(SemType::Nil) => TOKEN_TYPE_NULL,
        Some(SemType::Int | SemType::Float) => TOKEN_TYPE_INT,
        Some(SemType::Str) => TOKEN_TYPE_STRING,
        _ => match text.as_bytes().get(start).copied() {
            Some(b'"') => TOKEN_TYPE_STRING,
            Some(b't' | b'f') => TOKEN_TYPE_BOOLEAN,
            Some(b'n') => TOKEN_TYPE_NULL,
            _ => TOKEN_TYPE_INT,
        },
    };
    spans.push(JsonFormattedTokenByteSpan {
        start_byte: start as u32,
        end_byte: end as u32,
        token_type,
    });
}

fn encode_formatted_token_spans(
    spans: &[JsonFormattedTokenByteSpan],
    line_index: &LineIndex,
    text_len: usize,
) -> Vec<TokenSpan> {
    spans
        .iter()
        .filter_map(|span| {
            let start = span.start_byte as usize;
            let end = span.end_byte as usize;
            if start >= end || end > text_len {
                return None;
            }
            let start_pos = line_index.offset_to_line_column(start);
            let end_pos = line_index.offset_to_line_column(end);
            Some(TokenSpan {
                start_row: start_pos.line as u32,
                start_col: start_pos.column as u32,
                end_row: end_pos.line as u32,
                end_col: end_pos.column as u32,
                token_type: span.token_type,
            })
        })
        .collect()
}

fn skip_json_ws(text: &str, cursor: &mut usize) {
    while let Some(byte) = text.as_bytes().get(*cursor) {
        if !matches!(byte, b' ' | b'\n' | b'\r' | b'\t') {
            break;
        }
        *cursor += 1;
    }
}

fn consume_json_byte(text: &str, cursor: &mut usize, expected: u8) -> Result<(), CoreError> {
    skip_json_ws(text, cursor);
    if text.as_bytes().get(*cursor).copied() != Some(expected) {
        return Err(CoreError::Io(format!(
            "expected JSON byte {:?} at offset {}",
            expected as char, *cursor
        )));
    }
    *cursor += 1;
    Ok(())
}

fn skip_json_value_token(text: &str, cursor: &mut usize) {
    skip_json_ws(text, cursor);
    if text.as_bytes().get(*cursor).copied() == Some(b'"') {
        let _ = skip_json_string(text, cursor);
        return;
    }
    while let Some(byte) = text.as_bytes().get(*cursor) {
        if matches!(byte, b',' | b']' | b'}' | b'\n' | b'\r' | b'\t' | b' ') {
            break;
        }
        *cursor += 1;
    }
}

fn skip_json_string(text: &str, cursor: &mut usize) -> Result<(), CoreError> {
    if text.as_bytes().get(*cursor).copied() != Some(b'"') {
        return Err(CoreError::Io(format!(
            "expected JSON string at offset {}",
            *cursor
        )));
    }
    *cursor += 1;
    let bytes = text.as_bytes();
    while *cursor < bytes.len() {
        match bytes[*cursor] {
            b'\\' => {
                *cursor += 1;
                if *cursor < bytes.len() {
                    *cursor += 1;
                }
            }
            b'"' => {
                *cursor += 1;
                return Ok(());
            }
            _ => *cursor += 1,
        }
    }
    Err(CoreError::Io(
        "unterminated JSON string in formatted output".to_owned(),
    ))
}

/// JSON encoder for a TreeStore that already satisfies the JSON tree contract.
///
/// In same-language JSON document sessions, upstream writers keep numeric
/// scalar text valid for the node's semantic type. This encoder therefore
/// formats the existing tree and relies on tests to prove correctness; it must
/// not hide bugs with a post-encode JSON reparse.

impl Encode for JsonEncoder {
    fn encode(
        &self,
        store: &TreeStore,
        node_id: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        let root = node(store, node_id)?;
        if root.kind == TreeNodeKind::Scalar && self.prefs.unwrap_scalar {
            writer.write_all(root.value.as_bytes())?;
            return Ok(());
        }

        let mut out = String::new();
        if self.prefs.smart && self.prefs.indent > 0 {
            encode_json_node_smart(store, node_id, &self.prefs, 0, &mut out)?;
        } else {
            encode_node(
                store,
                node_id,
                self.prefs.indent,
                0,
                &mut out,
                LanguageStyle::Json,
            )?;
        }
        out.push('\n');
        writer.write_all(out.as_bytes())?;
        Ok(())
    }
}

pub(crate) fn encode_node(
    store: &TreeStore,
    node_id: NodeId,
    indent: i32,
    depth: usize,
    out: &mut String,
    style: LanguageStyle,
) -> Result<(), CoreError> {
    let current = node(store, node_id)?;
    match current.kind {
        TreeNodeKind::Scalar => {
            write_scalar(out, current, style);
            Ok(())
        }
        TreeNodeKind::Alias => {
            match style {
                LanguageStyle::Json => {
                    let Some(alias) = current.alias else {
                        write_null(out, style);
                        return Ok(());
                    };
                    encode_node(store, alias, indent, depth, out, style)?;
                }
                _ => {
                    write_null(out, style);
                }
            }
            Ok(())
        }
        TreeNodeKind::Sequence => {
            out.push('[');
            if !current.content.is_empty() {
                let pretty = indent > 0;
                if pretty {
                    out.push('\n');
                }
                for (index, child) in current.content.iter().enumerate() {
                    if pretty {
                        write_indent(out, depth + 1, indent);
                    }
                    encode_node(store, *child, indent, depth + 1, out, style)?;
                    if index + 1 < current.content.len() {
                        out.push(',');
                    }
                    if pretty {
                        out.push('\n');
                    }
                }
                if pretty {
                    write_indent(out, depth, indent);
                }
            }
            out.push(']');
            Ok(())
        }
        TreeNodeKind::Mapping => {
            out.push('{');
            if !current.content.is_empty() {
                let pretty = indent > 0;
                if pretty {
                    out.push('\n');
                }
                let pairs: Vec<_> = current.content.chunks_exact(2).collect();
                for (index, pair) in pairs.iter().enumerate() {
                    let key = node(store, pair[0])?;
                    if pretty {
                        write_indent(out, depth + 1, indent);
                    }
                    write_key(out, key, style);
                    out.push(':');
                    if pretty {
                        out.push(' ');
                    }
                    encode_node(store, pair[1], indent, depth + 1, out, style)?;
                    if index + 1 < pairs.len() {
                        out.push(',');
                    }
                    if pretty {
                        out.push('\n');
                    }
                }
                if pretty {
                    write_indent(out, depth, indent);
                }
            }
            out.push('}');
            Ok(())
        }
        TreeNodeKind::Unknown => {
            write_null(out, style);
            Ok(())
        }
    }
}

pub fn encode_json(store: &TreeStore, node: NodeId) -> Result<String, CoreError> {
    JsonEncoder::default().encode_to_string(store, node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{Decode, JsonDecoder};

    #[test]
    fn formatted_semantic_token_spans_match_fresh_encode() {
        let decoded = JsonDecoder
            .decode_str(r#"{"a":{"b":1,"c":[2,3]},"d":"hello","e":true,"f":null}"#)
            .expect("json should decode");
        let formatted = format_json_document_with_spans(
            &decoded,
            &FormatPreferences {
                indent: 2,
                smart: true,
                ..FormatPreferences::base()
            },
        )
        .expect("json should format");

        let remapped = crate::core::encode_and_cache_semantic_tokens(
            None,
            "",
            &formatted.text,
            &formatted.semantic_token_spans,
        );
        let fresh = crate::core::encode_semantic_tokens("json", &formatted.text);

        assert_eq!(remapped, fresh);
    }
}
