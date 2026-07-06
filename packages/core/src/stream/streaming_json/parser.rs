use std::cell::Cell;

use crate::language::SemType;

use super::super::streaming_events::{Meta, StreamingEvent};
use super::nested_json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonStreamError {
    UnexpectedEnd,
    UnexpectedToken { offset: usize, found: char },
    UnterminatedString { offset: usize },
    InvalidNumber { offset: usize },
    InvalidEscape { offset: usize },
    InvalidUnicodeEscape { offset: usize },
    TrailingCharacters { offset: usize },
}

pub fn decode(input: &str) -> Result<Vec<StreamingEvent>, JsonStreamError> {
    decode_with_nesting(input, false)
}

pub(crate) fn decode_with_nesting(
    input: &str,
    nest_json: bool,
) -> Result<Vec<StreamingEvent>, JsonStreamError> {
    let mut parser = JsonStreamParser::new(input, nest_json, 0);
    parser
        .events
        .push(StreamingEvent::DocStart(parser.meta(0, "$")));
    parser.parse_value()?;
    parser.skip_ws();
    if parser.peek().is_some() {
        return Err(JsonStreamError::TrailingCharacters {
            offset: parser.position,
        });
    }
    parser
        .events
        .push(StreamingEvent::DocEnd(parser.meta(parser.position, "$")));
    Ok(parser.events)
}

struct JsonStreamParser<'a> {
    input: &'a str,
    position: usize,
    events: Vec<StreamingEvent>,
    path: Vec<String>,
    nest_json: bool,
    nest_depth: usize,
    line_column_offset: Cell<usize>,
    line_column_line: Cell<usize>,
    line_column_column: Cell<usize>,
}

impl<'a> JsonStreamParser<'a> {
    fn new(input: &'a str, nest_json: bool, nest_depth: usize) -> Self {
        Self {
            input,
            position: 0,
            events: Vec::new(),
            path: Vec::new(),
            nest_json,
            nest_depth,
            line_column_offset: Cell::new(0),
            line_column_line: Cell::new(1),
            line_column_column: Cell::new(1),
        }
    }

    fn parse_value(&mut self) -> Result<(), JsonStreamError> {
        self.skip_ws();
        match self.peek().ok_or(JsonStreamError::UnexpectedEnd)? {
            '{' => self.parse_object(),
            '[' => self.parse_array(),
            '"' => {
                let start = self.position;
                let string = self.parse_string()?;
                let meta = self.meta_with_sem_type(
                    start,
                    &self.current_path(),
                    Some(SemType::Str),
                    "!!str",
                );
                if self.try_emit_nested_json(&string, &meta)? {
                    return Ok(());
                }
                self.events.push(StreamingEvent::Scalar {
                    value: string,
                    meta,
                });
                Ok(())
            }
            't' => self.parse_keyword("true", SemType::Boolean),
            'f' => self.parse_keyword("false", SemType::Boolean),
            'n' => self.parse_keyword("null", SemType::Nil),
            '-' | '0'..='9' => self.parse_number(),
            found => Err(JsonStreamError::UnexpectedToken {
                offset: self.position,
                found,
            }),
        }
    }

    fn parse_object(&mut self) -> Result<(), JsonStreamError> {
        let start = self.position;
        self.consume('{')?;
        let path = self.current_path();
        self.events
            .push(StreamingEvent::MapStart(self.meta_with_sem_type(
                start,
                &path,
                Some(SemType::Map),
                "!!map",
            )));
        self.skip_ws();
        if self.peek() == Some('}') {
            self.consume('}')?;
            self.events
                .push(StreamingEvent::MapEnd(self.meta_with_sem_type(
                    self.position,
                    &path,
                    Some(SemType::Map),
                    "!!map",
                )));
            return Ok(());
        }

        loop {
            self.skip_ws();
            let key_start = self.position;
            let key = self.parse_string()?;
            let key_path = self.path_with_segment(&key);
            self.events.push(StreamingEvent::MapKey {
                value: key.clone(),
                meta: self.meta(key_start, &key_path),
            });
            self.skip_ws();
            self.consume(':')?;
            self.path.push(key);
            self.parse_value()?;
            self.path.pop();
            self.skip_ws();
            match self.peek().ok_or(JsonStreamError::UnexpectedEnd)? {
                ',' => {
                    self.consume(',')?;
                }
                '}' => {
                    self.consume('}')?;
                    self.events
                        .push(StreamingEvent::MapEnd(self.meta_with_sem_type(
                            self.position,
                            &path,
                            Some(SemType::Map),
                            "!!map",
                        )));
                    break;
                }
                found => {
                    return Err(JsonStreamError::UnexpectedToken {
                        offset: self.position,
                        found,
                    });
                }
            }
        }
        Ok(())
    }

    fn parse_array(&mut self) -> Result<(), JsonStreamError> {
        let start = self.position;
        self.consume('[')?;
        let path = self.current_path();
        self.events
            .push(StreamingEvent::SeqStart(self.meta_with_sem_type(
                start,
                &path,
                Some(SemType::Seq),
                "!!seq",
            )));
        self.skip_ws();
        if self.peek() == Some(']') {
            self.consume(']')?;
            self.events
                .push(StreamingEvent::SeqEnd(self.meta_with_sem_type(
                    self.position,
                    &path,
                    Some(SemType::Seq),
                    "!!seq",
                )));
            return Ok(());
        }

        let mut index = 0usize;
        loop {
            self.path.push(index.to_string());
            self.parse_value()?;
            self.path.pop();
            index += 1;
            self.skip_ws();
            match self.peek().ok_or(JsonStreamError::UnexpectedEnd)? {
                ',' => {
                    self.consume(',')?;
                }
                ']' => {
                    self.consume(']')?;
                    self.events
                        .push(StreamingEvent::SeqEnd(self.meta_with_sem_type(
                            self.position,
                            &path,
                            Some(SemType::Seq),
                            "!!seq",
                        )));
                    break;
                }
                found => {
                    return Err(JsonStreamError::UnexpectedToken {
                        offset: self.position,
                        found,
                    });
                }
            }
        }
        Ok(())
    }

    fn parse_number(&mut self) -> Result<(), JsonStreamError> {
        let start = self.position;
        if self.peek() == Some('-') {
            self.bump_char()?;
        }

        match self.peek() {
            Some('0') => {
                self.bump_char()?;
                if matches!(self.peek(), Some('0'..='9')) {
                    return Err(JsonStreamError::InvalidNumber { offset: start });
                }
            }
            Some('1'..='9') => {
                self.bump_char()?;
                while matches!(self.peek(), Some('0'..='9')) {
                    self.bump_char()?;
                }
            }
            _ => return Err(JsonStreamError::InvalidNumber { offset: start }),
        }

        let mut sem_type = SemType::Int;
        if self.peek() == Some('.') {
            sem_type = SemType::Float;
            self.bump_char()?;
            let digit_start = self.position;
            while matches!(self.peek(), Some('0'..='9')) {
                self.bump_char()?;
            }
            if digit_start == self.position {
                return Err(JsonStreamError::InvalidNumber { offset: start });
            }
        }

        if matches!(self.peek(), Some('e' | 'E')) {
            sem_type = SemType::Float;
            self.bump_char()?;
            if matches!(self.peek(), Some('+' | '-')) {
                self.bump_char()?;
            }
            let digit_start = self.position;
            while matches!(self.peek(), Some('0'..='9')) {
                self.bump_char()?;
            }
            if digit_start == self.position {
                return Err(JsonStreamError::InvalidNumber { offset: start });
            }
        }

        let literal = &self.input[start..self.position];
        let (sem_type, value) = normalize_number(literal, sem_type, start)?;
        self.events.push(StreamingEvent::Scalar {
            value,
            meta: self.meta_with_sem_type(
                start,
                &self.current_path(),
                Some(sem_type),
                sem_type.tag(),
            ),
        });
        Ok(())
    }

    fn parse_keyword(&mut self, keyword: &str, sem_type: SemType) -> Result<(), JsonStreamError> {
        let start = self.position;
        for expected in keyword.chars() {
            let found = self.peek().ok_or(JsonStreamError::UnexpectedEnd)?;
            if found != expected {
                return Err(JsonStreamError::UnexpectedToken {
                    offset: self.position,
                    found,
                });
            }
            self.bump_char()?;
        }
        self.events.push(StreamingEvent::Scalar {
            value: keyword.to_string(),
            meta: self.meta_with_sem_type(
                start,
                &self.current_path(),
                Some(sem_type),
                sem_type.tag(),
            ),
        });
        Ok(())
    }

    fn parse_string(&mut self) -> Result<String, JsonStreamError> {
        self.consume('"')?;
        let mut output = String::new();
        while let Some(ch) = self.peek() {
            match ch {
                '"' => {
                    self.bump_char()?;
                    return Ok(output);
                }
                '\\' => {
                    let escape_offset = self.position;
                    self.bump_char()?;
                    let escaped = self.peek().ok_or(JsonStreamError::UnexpectedEnd)?;
                    let translated = match escaped {
                        '"' => {
                            self.bump_char()?;
                            '"'
                        }
                        '\\' => {
                            self.bump_char()?;
                            '\\'
                        }
                        '/' => {
                            self.bump_char()?;
                            '/'
                        }
                        'b' => {
                            self.bump_char()?;
                            '\x08'
                        }
                        'f' => {
                            self.bump_char()?;
                            '\x0c'
                        }
                        'n' => {
                            self.bump_char()?;
                            '\n'
                        }
                        'r' => {
                            self.bump_char()?;
                            '\r'
                        }
                        't' => {
                            self.bump_char()?;
                            '\t'
                        }
                        'u' => {
                            self.bump_char()?;
                            self.parse_unicode_escape(escape_offset)?
                        }
                        _ => {
                            return Err(JsonStreamError::InvalidEscape {
                                offset: escape_offset,
                            });
                        }
                    };
                    output.push(translated);
                }
                ch if ch.is_control() => {
                    return Err(JsonStreamError::UnexpectedToken {
                        offset: self.position,
                        found: ch,
                    });
                }
                _ => {
                    output.push(ch);
                    self.bump_char()?;
                }
            }
        }
        Err(JsonStreamError::UnterminatedString {
            offset: self.position,
        })
    }

    fn try_emit_nested_json(
        &mut self,
        value: &str,
        outer_meta: &Meta,
    ) -> Result<bool, JsonStreamError> {
        if !self.nest_json || self.nest_depth >= 8 || !nested_json::is_nested_json_candidate(value)
        {
            return Ok(false);
        }
        let trimmed = nested_json::trim_ascii_whitespace(value);
        let nested_events = match decode_with_nesting_depth(trimmed, true, self.nest_depth + 1) {
            Ok(events) => events,
            Err(_) => return Ok(false),
        };
        for event in nested_events {
            if let Some(event) = rebase_nested_event(event, outer_meta) {
                self.events.push(event);
            }
        }
        Ok(true)
    }

    fn current_path(&self) -> String {
        if self.path.is_empty() {
            return "$".to_string();
        }
        let mut out = String::from("$");
        for segment in &self.path {
            append_path_segment(&mut out, segment);
        }
        out
    }

    fn path_with_segment(&self, segment: &str) -> String {
        let mut out = self.current_path();
        append_path_segment(&mut out, segment);
        out
    }

    fn meta(&self, start: usize, path: &str) -> Meta {
        self.meta_with_sem_type(start, path, None, "")
    }

    fn meta_with_sem_type(
        &self,
        start: usize,
        path: &str,
        sem_type: Option<SemType>,
        tag: &str,
    ) -> Meta {
        let (line, column) = self.line_column_at(start);
        Meta {
            tag: tag.to_string(),
            sem_type,
            start_byte: start as u32,
            end_byte: self.position as u32,
            line: line as i32,
            column: column as i32,
            path: path.to_string(),
            ..Meta::default()
        }
    }

    fn line_column_at(&self, raw_offset: usize) -> (usize, usize) {
        let offset = raw_offset.min(self.input.len());
        let mut cached_offset = self.line_column_offset.get();
        let mut line = self.line_column_line.get();
        let mut column = self.line_column_column.get();

        if offset < cached_offset {
            cached_offset = 0;
            line = 1;
            column = 1;
        }

        let scan_start = cached_offset;
        for (relative_index, ch) in self.input[scan_start..].char_indices() {
            let index = scan_start + relative_index;
            if index >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
            cached_offset = index + ch.len_utf8();
        }

        self.line_column_offset.set(cached_offset);
        self.line_column_line.set(line);
        self.line_column_column.set(column);
        (line, column)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(ch) if ch.is_ascii_whitespace()) {
            let _ = self.bump_char();
        }
    }

    fn consume(&mut self, expected: char) -> Result<(), JsonStreamError> {
        match self.peek() {
            Some(found) if found == expected => {
                self.bump_char()?;
                Ok(())
            }
            Some(found) => Err(JsonStreamError::UnexpectedToken {
                offset: self.position,
                found,
            }),
            None => Err(JsonStreamError::UnexpectedEnd),
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn bump_char(&mut self) -> Result<char, JsonStreamError> {
        let ch = self.peek().ok_or(JsonStreamError::UnexpectedEnd)?;
        self.position += ch.len_utf8();
        Ok(ch)
    }

    fn parse_unicode_escape(&mut self, offset: usize) -> Result<char, JsonStreamError> {
        let first = self.parse_hex_quad(offset)?;
        if !is_high_surrogate(first) {
            return char::from_u32(first as u32)
                .ok_or(JsonStreamError::InvalidUnicodeEscape { offset });
        }

        if self.peek() != Some('\\') {
            return Err(JsonStreamError::InvalidUnicodeEscape { offset });
        }
        self.bump_char()?;
        if self.peek() != Some('u') {
            return Err(JsonStreamError::InvalidUnicodeEscape { offset });
        }
        self.bump_char()?;
        let second = self.parse_hex_quad(offset)?;
        if !is_low_surrogate(second) {
            return Err(JsonStreamError::InvalidUnicodeEscape { offset });
        }

        let codepoint = 0x1_0000 + (((first as u32 - 0xD800) << 10) | (second as u32 - 0xDC00));
        char::from_u32(codepoint).ok_or(JsonStreamError::InvalidUnicodeEscape { offset })
    }

    fn parse_hex_quad(&mut self, offset: usize) -> Result<u16, JsonStreamError> {
        let mut value = 0u16;
        for _ in 0..4 {
            let ch = self.bump_char()?;
            let digit = ch
                .to_digit(16)
                .ok_or(JsonStreamError::InvalidUnicodeEscape { offset })?;
            value = (value << 4) | digit as u16;
        }
        Ok(value)
    }
}

fn append_path_segment(out: &mut String, segment: &str) {
    if segment.chars().all(|ch| ch.is_ascii_digit()) {
        out.push('[');
        out.push_str(segment);
        out.push(']');
    } else {
        out.push('.');
        out.push_str(segment);
    }
}

pub(crate) fn decode_with_nesting_depth(
    input: &str,
    nest_json: bool,
    nest_depth: usize,
) -> Result<Vec<StreamingEvent>, JsonStreamError> {
    let mut parser = JsonStreamParser::new(input, nest_json, nest_depth);
    parser
        .events
        .push(StreamingEvent::DocStart(parser.meta(0, "$")));
    parser.parse_value()?;
    parser.skip_ws();
    if parser.peek().is_some() {
        return Err(JsonStreamError::TrailingCharacters {
            offset: parser.position,
        });
    }
    parser
        .events
        .push(StreamingEvent::DocEnd(parser.meta(parser.position, "$")));
    Ok(parser.events)
}

fn rebase_nested_event(event: StreamingEvent, outer_meta: &Meta) -> Option<StreamingEvent> {
    match event {
        StreamingEvent::DocStart(_) | StreamingEvent::DocEnd(_) => None,
        StreamingEvent::MapStart(meta) => Some(StreamingEvent::MapStart(rebase_nested_meta(
            meta, outer_meta,
        ))),
        StreamingEvent::MapKey { value, meta } => Some(StreamingEvent::MapKey {
            value,
            meta: rebase_nested_meta(meta, outer_meta),
        }),
        StreamingEvent::MapEnd(meta) => {
            Some(StreamingEvent::MapEnd(rebase_nested_meta(meta, outer_meta)))
        }
        StreamingEvent::SeqStart(meta) => Some(StreamingEvent::SeqStart(rebase_nested_meta(
            meta, outer_meta,
        ))),
        StreamingEvent::SeqEnd(meta) => {
            Some(StreamingEvent::SeqEnd(rebase_nested_meta(meta, outer_meta)))
        }
        StreamingEvent::Scalar { value, meta } => Some(StreamingEvent::Scalar {
            value,
            meta: rebase_nested_meta(meta, outer_meta),
        }),
        StreamingEvent::Alias { anchor, meta } => Some(StreamingEvent::Alias {
            anchor,
            meta: rebase_nested_meta(meta, outer_meta),
        }),
        StreamingEvent::ParseError { message, meta } => Some(StreamingEvent::ParseError {
            message,
            meta: rebase_nested_meta(meta, outer_meta),
        }),
    }
}

fn rebase_nested_meta(mut meta: Meta, outer_meta: &Meta) -> Meta {
    meta.start_byte = outer_meta.start_byte;
    meta.end_byte = outer_meta.end_byte;
    meta.line = outer_meta.line;
    meta.column = outer_meta.column;
    meta.path = rebase_nested_path(&meta.path, &outer_meta.path);
    meta.path_supplier = None;
    meta
}

fn rebase_nested_path(inner_path: &str, outer_path: &str) -> String {
    if outer_path.is_empty() {
        return inner_path.to_string();
    }
    if inner_path.is_empty() || inner_path == "$" {
        return outer_path.to_string();
    }
    let suffix = inner_path.strip_prefix('$').unwrap_or(inner_path);
    format!("{outer_path}{suffix}")
}

fn normalize_number(
    literal: &str,
    sem_type: SemType,
    offset: usize,
) -> Result<(SemType, String), JsonStreamError> {
    match sem_type {
        SemType::Int => {
            if literal.parse::<i64>().is_err() && literal.parse::<u64>().is_err() {
                return Err(JsonStreamError::InvalidNumber { offset });
            }
            Ok((SemType::Int, literal.to_string()))
        }
        SemType::Float => {
            let parsed = literal
                .parse::<f64>()
                .map_err(|_| JsonStreamError::InvalidNumber { offset })?;
            if parsed.is_finite()
                && parsed == parsed.trunc()
                && parsed >= i64::MIN as f64
                && parsed <= i64::MAX as f64
            {
                return Ok((SemType::Int, (parsed as i64).to_string()));
            }
            Ok((SemType::Float, literal.to_string()))
        }
        _ => Err(JsonStreamError::InvalidNumber { offset }),
    }
}
fn is_high_surrogate(value: u16) -> bool {
    (0xD800..=0xDBFF).contains(&value)
}

fn is_low_surrogate(value: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_json_into_streaming_events() {
        let events = decode("{\"a\":[1,true]}").expect("json decode should succeed");
        assert!(events.iter().any(|event| matches!(
            event,
            StreamingEvent::MapKey { value, .. } if value == "a"
        )));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, StreamingEvent::SeqStart(_)))
        );
    }

    #[test]
    fn reports_one_based_char_line_columns_without_rescanning_from_start() {
        let events =
            decode("{\n  \"a\": [\n    \"é\"\n  ]\n}").expect("json decode should succeed");

        let key_meta = events
            .iter()
            .find_map(|event| match event {
                StreamingEvent::MapKey { value, meta } if value == "a" => Some(meta),
                _ => None,
            })
            .expect("map key metadata should be emitted");
        assert_eq!((key_meta.line, key_meta.column), (2, 3));

        let scalar_meta = events
            .iter()
            .find_map(|event| match event {
                StreamingEvent::Scalar { value, meta } if value == "é" => Some(meta),
                _ => None,
            })
            .expect("scalar metadata should be emitted");
        assert_eq!((scalar_meta.line, scalar_meta.column), (3, 5));
    }

    #[test]
    fn normalizes_integer_like_float_literals() {
        let events = decode(r#"{"value":1.0e2}"#).expect("json decode should succeed");
        assert!(events.iter().any(|event| matches!(
            event,
            StreamingEvent::Scalar { value, meta, .. }
                if value == "100" && meta.sem_type == Some(SemType::Int)
        )));
    }

    #[test]
    fn decodes_unicode_escape_sequences() {
        let events = decode(r#"{"value":"\u4f60\u597d"}"#).expect("json decode should succeed");
        assert!(events.iter().any(|event| matches!(
            event,
            StreamingEvent::Scalar { value, .. } if value == "你好"
        )));
    }
}
