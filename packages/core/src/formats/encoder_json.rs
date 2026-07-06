use std::io::Write;

use crate::core::{
    CoreError, LineIndex, NodeId, SemType, TokenSpan, TreeNodeKind, TreeStore, ValueRep,
};
use crate::evaluator::Value as EvalValue;

use super::encoder_javascript::{is_js_identifier, is_safe_integer_literal};
use super::formats_helpers::write_quoted_string;
use super::preferences::FormatPreferences;
use super::smart_layout::{TextSink, encode_json_node_smart_into};
use super::{Encode, is_truthy_literal, node};

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

fn write_null_into<Sink: TextSink>(out: &mut Sink, style: LanguageStyle) -> Result<(), CoreError> {
    out.push_str(match style {
        LanguageStyle::Json | LanguageStyle::Javascript => "null",
        LanguageStyle::Python => "None",
    })
}

fn write_quoted_into<Sink: TextSink>(
    out: &mut Sink,
    value: &str,
    quote: char,
) -> Result<(), CoreError> {
    let mut quoted = String::new();
    write_quoted_string(&mut quoted, value, quote);
    out.push_str(&quoted)
}

pub(crate) fn json_quoted_len(value: &str) -> usize {
    let mut len = 2;
    for ch in value.chars() {
        len += match ch {
            '"' | '\\' | '\u{08}' | '\u{0C}' | '\n' | '\r' | '\t' => 2,
            ch if ch <= '\u{1F}' => 6,
            _ => ch.len_utf8(),
        };
    }
    len
}

pub(crate) fn write_json_quoted_into<Sink: TextSink>(
    out: &mut Sink,
    value: &str,
) -> Result<(), CoreError> {
    out.push_char('"')?;
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\"")?,
            '\\' => out.push_str("\\\\")?,
            '\u{08}' => out.push_str("\\b")?,
            '\u{0C}' => out.push_str("\\f")?,
            '\n' => out.push_str("\\n")?,
            '\r' => out.push_str("\\r")?,
            '\t' => out.push_str("\\t")?,
            ch if ch <= '\u{1F}' => {
                let escaped = format!("\\u{:04x}", ch as u32);
                out.push_str(&escaped)?;
            }
            _ => {
                let mut buf = [0_u8; 4];
                out.push_str(ch.encode_utf8(&mut buf))?;
            }
        }
    }
    out.push_char('"')
}

pub(crate) fn json_scalar_len(store: &TreeStore, node_id: NodeId) -> Result<usize, CoreError> {
    let node = node(store, node_id)?;
    let value = store.value_for(node_id)?;
    match node.resolved_sem_type() {
        Some(SemType::Boolean) => Ok(if value.eq_ignore_ascii_case("true") {
            4
        } else {
            5
        }),
        Some(SemType::Int | SemType::Float) => Ok(value.len()),
        Some(SemType::Nil) => Ok(4),
        Some(SemType::Str) => Ok(json_quoted_len(value)),
        _ => match store.value_rep_for(node_id)? {
            ValueRep::Nil => Ok(4),
            ValueRep::Boolean(value) => Ok(if value { 4 } else { 5 }),
            ValueRep::Int(value) => Ok(value.to_string().len()),
            ValueRep::Float(value) => Ok(value.to_string().len()),
            ValueRep::Str(value) => Ok(json_quoted_len(&value)),
        },
    }
}

pub(crate) fn write_json_scalar_into<Sink: TextSink>(
    store: &TreeStore,
    node_id: NodeId,
    out: &mut Sink,
) -> Result<(), CoreError> {
    let node = node(store, node_id)?;
    let value = store.value_for(node_id)?;
    match node.resolved_sem_type() {
        Some(SemType::Boolean) => out.push_str(if value.eq_ignore_ascii_case("true") {
            "true"
        } else {
            "false"
        }),
        Some(SemType::Int | SemType::Float) => out.push_str(value),
        Some(SemType::Nil) => out.push_str("null"),
        Some(SemType::Str) => write_json_quoted_into(out, value),
        _ => match store.value_rep_for(node_id)? {
            ValueRep::Nil => out.push_str("null"),
            ValueRep::Boolean(value) => out.push_str(if value { "true" } else { "false" }),
            ValueRep::Int(value) => out.push_str(&value.to_string()),
            ValueRep::Float(value) => out.push_str(&value.to_string()),
            ValueRep::Str(value) => write_json_quoted_into(out, &value),
        },
    }
}

pub(crate) fn eval_scalar_unquoted_text(value: &EvalValue) -> Option<String> {
    match value {
        EvalValue::Null => Some("null".to_owned()),
        EvalValue::Bool(value) => Some(if *value { "true" } else { "false" }.to_owned()),
        EvalValue::Number(value) => Some(value.to_string()),
        EvalValue::String(value) => Some(value.clone()),
        EvalValue::Array(_) | EvalValue::Object(_) => None,
    }
}

fn eval_scalar_len(value: &EvalValue, style: LanguageStyle) -> usize {
    match value {
        EvalValue::Null => 4,
        EvalValue::Bool(value) => {
            if *value {
                4
            } else {
                5
            }
        }
        EvalValue::Number(value) => value.to_string().len(),
        EvalValue::String(value) => match style {
            LanguageStyle::Json => json_quoted_len(value),
            LanguageStyle::Python | LanguageStyle::Javascript => value.len() + 2,
        },
        EvalValue::Array(_) | EvalValue::Object(_) => 0,
    }
}

fn write_eval_scalar_into<Sink: TextSink>(
    value: &EvalValue,
    style: LanguageStyle,
    out: &mut Sink,
) -> Result<(), CoreError> {
    match value {
        EvalValue::Null => write_null_into(out, style),
        EvalValue::Bool(value) => out.push_str(match style {
            LanguageStyle::Json | LanguageStyle::Javascript => {
                if *value {
                    "true"
                } else {
                    "false"
                }
            }
            LanguageStyle::Python => {
                if *value {
                    "True"
                } else {
                    "False"
                }
            }
        }),
        EvalValue::Number(value) => out.push_str(&value.to_string()),
        EvalValue::String(value) => match style {
            LanguageStyle::Json => write_json_quoted_into(out, value),
            LanguageStyle::Python | LanguageStyle::Javascript => {
                write_quoted_into(out, value, '\'')
            }
        },
        EvalValue::Array(_) | EvalValue::Object(_) => Ok(()),
    }
}

fn write_eval_key_into<Sink: TextSink>(
    key: &str,
    style: LanguageStyle,
    out: &mut Sink,
) -> Result<(), CoreError> {
    match style {
        LanguageStyle::Json => write_json_quoted_into(out, key),
        LanguageStyle::Python => write_quoted_into(out, key, '\''),
        LanguageStyle::Javascript => {
            if is_js_identifier(key) {
                out.push_str(key)
            } else {
                write_quoted_into(out, key, '\'')
            }
        }
    }
}

pub(crate) fn write_eval_value_into<Sink: TextSink>(
    value: &EvalValue,
    indent: i32,
    depth: usize,
    style: LanguageStyle,
    out: &mut Sink,
) -> Result<(), CoreError> {
    match value {
        EvalValue::Null | EvalValue::Bool(_) | EvalValue::Number(_) | EvalValue::String(_) => {
            write_eval_scalar_into(value, style, out)
        }
        EvalValue::Array(values) => {
            out.push_char('[')?;
            if !values.is_empty() {
                let pretty = indent > 0;
                if pretty {
                    out.push_char('\n')?;
                }
                for (index, value) in values.iter().enumerate() {
                    if pretty {
                        write_indent_into(out, depth + 1, indent)?;
                    }
                    write_eval_value_into(value, indent, depth + 1, style, out)?;
                    if index + 1 < values.len() {
                        out.push_char(',')?;
                    }
                    if pretty {
                        out.push_char('\n')?;
                    }
                }
                if pretty {
                    write_indent_into(out, depth, indent)?;
                }
            }
            out.push_char(']')
        }
        EvalValue::Object(values) => {
            out.push_char('{')?;
            if !values.is_empty() {
                let pretty = indent > 0;
                if pretty {
                    out.push_char('\n')?;
                }
                let pair_count = values.len();
                for (index, (key, value)) in values.iter().enumerate() {
                    if pretty {
                        write_indent_into(out, depth + 1, indent)?;
                    }
                    write_eval_key_into(key, style, out)?;
                    out.push_char(':')?;
                    if pretty {
                        out.push_char(' ')?;
                    }
                    write_eval_value_into(value, indent, depth + 1, style, out)?;
                    if index + 1 < pair_count {
                        out.push_char(',')?;
                    }
                    if pretty {
                        out.push_char('\n')?;
                    }
                }
                if pretty {
                    write_indent_into(out, depth, indent)?;
                }
            }
            out.push_char('}')
        }
    }
}

fn eval_inline_complexity_exceeds(value: &EvalValue, max: usize, count: &mut usize) -> bool {
    match value {
        EvalValue::Null | EvalValue::Bool(_) | EvalValue::Number(_) | EvalValue::String(_) => false,
        EvalValue::Array(values) => {
            *count += 1;
            if *count > max {
                return true;
            }
            values
                .iter()
                .any(|value| eval_inline_complexity_exceeds(value, max, count))
        }
        EvalValue::Object(values) => {
            *count += 1;
            if *count > max {
                return true;
            }
            values
                .values()
                .any(|value| eval_inline_complexity_exceeds(value, max, count))
        }
    }
}

fn eval_inline_len(value: &EvalValue, style: LanguageStyle) -> usize {
    match value {
        EvalValue::Null | EvalValue::Bool(_) | EvalValue::Number(_) | EvalValue::String(_) => {
            eval_scalar_len(value, style)
        }
        EvalValue::Array(values) => {
            if values.is_empty() {
                return 2;
            }
            2 + values
                .iter()
                .enumerate()
                .map(|(index, value)| eval_inline_len(value, style) + usize::from(index > 0) * 2)
                .sum::<usize>()
        }
        EvalValue::Object(values) => {
            if values.is_empty() {
                return 2;
            }
            2 + values
                .iter()
                .enumerate()
                .map(|(index, (key, value))| {
                    let key_len = match style {
                        LanguageStyle::Json => json_quoted_len(key),
                        LanguageStyle::Python => key.len() + 2,
                        LanguageStyle::Javascript => {
                            if is_js_identifier(key) {
                                key.len()
                            } else {
                                key.len() + 2
                            }
                        }
                    };
                    key_len + 2 + eval_inline_len(value, style) + usize::from(index > 0) * 2
                })
                .sum::<usize>()
        }
    }
}

fn eval_can_inline(
    value: &EvalValue,
    prefs: &FormatPreferences,
    depth: usize,
    style: LanguageStyle,
) -> bool {
    let mut count = 0usize;
    if eval_inline_complexity_exceeds(
        value,
        prefs.max_inline_complexity.max(0) as usize,
        &mut count,
    ) {
        return false;
    }
    eval_inline_len(value, style) + depth * prefs.indent.max(0) as usize
        <= prefs.max_line_length.max(0) as usize
}

fn write_eval_inline_value<Sink: TextSink>(
    value: &EvalValue,
    style: LanguageStyle,
    out: &mut Sink,
) -> Result<(), CoreError> {
    match value {
        EvalValue::Null | EvalValue::Bool(_) | EvalValue::Number(_) | EvalValue::String(_) => {
            write_eval_value_into(value, 0, 0, style, out)
        }
        EvalValue::Array(values) => {
            out.push_char('[')?;
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ")?;
                }
                write_eval_inline_value(value, style, out)?;
            }
            out.push_char(']')
        }
        EvalValue::Object(values) => {
            out.push_char('{')?;
            for (index, (key, value)) in values.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ")?;
                }
                write_eval_key_into(key, style, out)?;
                out.push_str(": ")?;
                write_eval_inline_value(value, style, out)?;
            }
            out.push_char('}')
        }
    }
}

pub(crate) fn write_eval_value_smart<Sink: TextSink>(
    value: &EvalValue,
    prefs: &FormatPreferences,
    depth: usize,
    style: LanguageStyle,
    out: &mut Sink,
) -> Result<(), CoreError> {
    match value {
        EvalValue::Null | EvalValue::Bool(_) | EvalValue::Number(_) | EvalValue::String(_) => {
            write_eval_value_into(value, 0, depth, style, out)
        }
        EvalValue::Array(values) => {
            if values.is_empty() {
                return out.push_str("[]");
            }
            if eval_can_inline(value, prefs, depth, style) {
                return write_eval_inline_value(value, style, out);
            }
            out.push_char('[')?;
            out.push_char('\n')?;
            for (index, value) in values.iter().enumerate() {
                write_indent_into(out, depth + 1, prefs.indent)?;
                write_eval_value_smart(value, prefs, depth + 1, style, out)?;
                if index + 1 < values.len() {
                    out.push_char(',')?;
                }
                out.push_char('\n')?;
            }
            write_indent_into(out, depth, prefs.indent)?;
            out.push_char(']')
        }
        EvalValue::Object(values) => {
            if values.is_empty() {
                return out.push_str("{}");
            }
            if eval_can_inline(value, prefs, depth, style) {
                return write_eval_inline_value(value, style, out);
            }
            out.push_char('{')?;
            out.push_char('\n')?;
            let pair_count = values.len();
            for (index, (key, value)) in values.iter().enumerate() {
                write_indent_into(out, depth + 1, prefs.indent)?;
                write_eval_key_into(key, style, out)?;
                out.push_str(": ")?;
                write_eval_value_smart(value, prefs, depth + 1, style, out)?;
                if index + 1 < pair_count {
                    out.push_char(',')?;
                }
                out.push_char('\n')?;
            }
            write_indent_into(out, depth, prefs.indent)?;
            out.push_char('}')
        }
    }
}

fn write_indent_into<Sink: TextSink>(
    out: &mut Sink,
    depth: usize,
    indent: i32,
) -> Result<(), CoreError> {
    let count = depth.saturating_mul(indent.max(0) as usize);
    for _ in 0..count {
        out.push_char(' ')?;
    }
    Ok(())
}

fn write_key_into<Sink: TextSink>(
    store: &TreeStore,
    node_id: NodeId,
    out: &mut Sink,
    style: LanguageStyle,
) -> Result<(), CoreError> {
    let _node = node(store, node_id)?;
    let value = store.value_for(node_id)?;
    match style {
        LanguageStyle::Json => {
            write_json_quoted_into(out, value)?;
        }
        LanguageStyle::Python => {
            let mut key = String::new();
            crate::formats::encoder_python::write_python_key(store, node_id, &mut key);
            out.push_str(&key)?;
        }
        LanguageStyle::Javascript => {
            if is_js_identifier(value) {
                out.push_str(value)?;
            } else {
                write_quoted_into(out, value, '\'')?;
            }
        }
    }
    Ok(())
}

fn write_scalar_into<Sink: TextSink>(
    store: &TreeStore,
    node_id: NodeId,
    out: &mut Sink,
    style: LanguageStyle,
) -> Result<(), CoreError> {
    let node = node(store, node_id)?;
    let value = store.value_for(node_id)?;
    match style {
        LanguageStyle::Json => {
            write_json_scalar_into(store, node_id, out)?;
        }
        LanguageStyle::Python | LanguageStyle::Javascript => match node.resolved_sem_type() {
            Some(SemType::Nil) => write_null_into(out, style)?,
            Some(SemType::Boolean) => {
                let truthy = is_truthy_literal(value);
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
                })?;
            }
            Some(SemType::Int) => match style {
                LanguageStyle::Python => out.push_str(value)?,
                LanguageStyle::Javascript => {
                    if is_safe_integer_literal(value) {
                        out.push_str(value)?;
                    } else {
                        write_quoted_into(out, value, '\'')?;
                    }
                }
                LanguageStyle::Json => unreachable!(),
            },
            Some(SemType::Float) => out.push_str(value)?,
            None => write_null_into(out, style)?,
            _ => write_quoted_into(out, value, '\'')?,
        },
    }
    Ok(())
}

struct BufferedTextWriter<'a> {
    inner: &'a mut dyn Write,
    buffer: String,
}

impl<'a> BufferedTextWriter<'a> {
    const FLUSH_THRESHOLD: usize = 64 * 1024;

    fn new(inner: &'a mut dyn Write) -> Self {
        Self {
            inner,
            buffer: String::with_capacity(Self::FLUSH_THRESHOLD),
        }
    }

    fn flush_buffer(&mut self) -> Result<(), CoreError> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        self.inner.write_all(self.buffer.as_bytes())?;
        self.buffer.clear();
        Ok(())
    }

    fn finish(&mut self) -> Result<(), CoreError> {
        self.flush_buffer()
    }
}

impl TextSink for BufferedTextWriter<'_> {
    fn push_char(&mut self, ch: char) -> Result<(), CoreError> {
        self.buffer.push(ch);
        if self.buffer.len() >= Self::FLUSH_THRESHOLD {
            self.flush_buffer()?;
        }
        Ok(())
    }

    fn push_str(&mut self, value: &str) -> Result<(), CoreError> {
        self.buffer.push_str(value);
        if self.buffer.len() >= Self::FLUSH_THRESHOLD {
            self.flush_buffer()?;
        }
        Ok(())
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
        let end = decoded.store.value_for(decoded.root)?.len().min(text.len());
        spans.push(JsonNodeFormattedSpan {
            node_id: decoded.root,
            start_byte: 0,
            end_byte: end as u32,
        });
        push_scalar_token_byte_span(
            &decoded.store,
            decoded.root,
            &text,
            0,
            end,
            &mut semantic_token_byte_spans,
        );
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
            push_scalar_token_byte_span(
                store,
                node_id,
                text,
                start,
                *cursor,
                semantic_token_byte_spans,
            );
        }
        TreeNodeKind::Alias => {
            if let Some(alias) = current.alias() {
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
                    store,
                    node_id,
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
    store: &TreeStore,
    node_id: NodeId,
    text: &str,
    start: usize,
    end: usize,
    spans: &mut Vec<JsonFormattedTokenByteSpan>,
) {
    if start >= end || end > text.len() {
        return;
    }
    let Some(node) = store.get(node_id) else {
        return;
    };
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
            writer.write_all(store.value_for(node_id)?.as_bytes())?;
            return Ok(());
        }

        let mut out = BufferedTextWriter::new(writer);
        if self.prefs.smart && self.prefs.indent > 0 {
            encode_json_node_smart_into(store, node_id, &self.prefs, 0, &mut out)?;
        } else {
            encode_node_into(
                store,
                node_id,
                self.prefs.indent,
                0,
                &mut out,
                LanguageStyle::Json,
            )?;
        }
        out.push_char('\n')?;
        out.finish()
    }

    fn encode_evaluated_value(
        &self,
        value: &EvalValue,
        writer: &mut dyn Write,
    ) -> Result<bool, CoreError> {
        if self.prefs.unwrap_scalar {
            if let Some(text) = eval_scalar_unquoted_text(value) {
                writer.write_all(text.as_bytes())?;
                return Ok(true);
            }
        }

        let mut out = BufferedTextWriter::new(writer);
        if self.prefs.smart && self.prefs.indent > 0 {
            write_eval_value_smart(value, &self.prefs, 0, LanguageStyle::Json, &mut out)?;
        } else {
            write_eval_value_into(value, self.prefs.indent, 0, LanguageStyle::Json, &mut out)?;
        }
        out.push_char('\n')?;
        out.finish()?;
        Ok(true)
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
    encode_node_into(store, node_id, indent, depth, out, style)
}

pub(crate) fn encode_node_into<Sink: TextSink>(
    store: &TreeStore,
    node_id: NodeId,
    indent: i32,
    depth: usize,
    out: &mut Sink,
    style: LanguageStyle,
) -> Result<(), CoreError> {
    let current = node(store, node_id)?;
    match current.kind {
        TreeNodeKind::Scalar => {
            write_scalar_into(store, node_id, out, style)?;
            Ok(())
        }
        TreeNodeKind::Alias => {
            match style {
                LanguageStyle::Json => {
                    let Some(alias) = current.alias() else {
                        write_null_into(out, style)?;
                        return Ok(());
                    };
                    encode_node_into(store, alias, indent, depth, out, style)?;
                }
                _ => {
                    write_null_into(out, style)?;
                }
            }
            Ok(())
        }
        TreeNodeKind::Sequence => {
            out.push_char('[')?;
            if !current.content.is_empty() {
                let pretty = indent > 0;
                if pretty {
                    out.push_char('\n')?;
                }
                for (index, child) in current.content.iter().enumerate() {
                    if pretty {
                        write_indent_into(out, depth + 1, indent)?;
                    }
                    encode_node_into(store, *child, indent, depth + 1, out, style)?;
                    if index + 1 < current.content.len() {
                        out.push_char(',')?;
                    }
                    if pretty {
                        out.push_char('\n')?;
                    }
                }
                if pretty {
                    write_indent_into(out, depth, indent)?;
                }
            }
            out.push_char(']')?;
            Ok(())
        }
        TreeNodeKind::Mapping => {
            out.push_char('{')?;
            if !current.content.is_empty() {
                let pretty = indent > 0;
                if pretty {
                    out.push_char('\n')?;
                }
                let pair_count = current.content.len() / 2;
                for (index, pair) in current.content.chunks_exact(2).enumerate() {
                    if pretty {
                        write_indent_into(out, depth + 1, indent)?;
                    }
                    write_key_into(store, pair[0], out, style)?;
                    out.push_char(':')?;
                    if pretty {
                        out.push_char(' ')?;
                    }
                    encode_node_into(store, pair[1], indent, depth + 1, out, style)?;
                    if index + 1 < pair_count {
                        out.push_char(',')?;
                    }
                    if pretty {
                        out.push_char('\n')?;
                    }
                }
                if pretty {
                    write_indent_into(out, depth, indent)?;
                }
            }
            out.push_char('}')?;
            Ok(())
        }
        TreeNodeKind::Unknown => {
            write_null_into(out, style)?;
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

    #[test]
    fn smart_encode_streams_to_writer_without_changing_output() {
        let decoded = JsonDecoder
            .decode_str(r#"{"a":{"b":1},"items":[1,2,3],"flag":true}"#)
            .expect("json should decode");
        let prefs = FormatPreferences {
            indent: 2,
            smart: true,
            unwrap_scalar: false,
            ..FormatPreferences::base()
        };

        let mut bytes = Vec::new();
        JsonEncoder::new(prefs)
            .encode(&decoded.store, decoded.root, &mut bytes)
            .expect("smart encode should succeed");

        let output = String::from_utf8(bytes).expect("encoded bytes should be utf-8");
        assert_eq!(
            output,
            "{\n  \"a\": {\"b\": 1},\n  \"items\": [1, 2, 3],\n  \"flag\": true\n}\n"
        );
    }
}
