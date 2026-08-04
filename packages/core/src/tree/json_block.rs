use crate::{
    core::TreeNodeKind,
    formats::{Decode, JsonDecoder},
};

use crate::analysis::line_index::LineIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonBlockSpan {
    pub found: bool,
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_row: u32,
    pub start_column: u32,
    pub end_row: u32,
    pub end_column: u32,
}

impl JsonBlockSpan {
    pub const EMPTY: Self = Self {
        found: false,
        start_byte: 0,
        end_byte: 0,
        start_row: 0,
        start_column: 0,
        end_row: 0,
        end_column: 0,
    };
}

pub fn find_json_block_at_position(
    language: &str,
    source: &str,
    row: u32,
    column: u32,
) -> JsonBlockSpan {
    if !language.eq_ignore_ascii_case("json") {
        return JsonBlockSpan::EMPTY;
    }

    let index = LineIndex::build(source);
    let cursor = index.line_column_to_offset(row, column);
    if matches!(source.as_bytes().get(cursor), Some(b'{' | b'[')) {
        return JsonBlockSpan::EMPTY;
    }
    let Some((start, end)) = smallest_valid_json_container_span(source, cursor) else {
        return JsonBlockSpan::EMPTY;
    };
    let start_lc = index.offset_to_line_column(start);
    let end_lc = index.offset_to_line_column(end);
    JsonBlockSpan {
        found: true,
        start_byte: start as u32,
        end_byte: end as u32,
        start_row: start_lc.line,
        start_column: start_lc.column,
        end_row: end_lc.line,
        end_column: end_lc.column,
    }
}

fn smallest_valid_json_container_span(source: &str, cursor: usize) -> Option<(usize, usize)> {
    let cursor = cursor.min(source.len());
    let mut index = 0;

    while index < source.len() {
        let open = source.as_bytes()[index];
        if matching_close(open).is_none() {
            index += 1;
            continue;
        }
        if index >= cursor {
            break;
        }

        let Some(close_index) = find_matching_bracket(source, index) else {
            index += 1;
            continue;
        };
        let end = close_index + 1;
        if end <= cursor {
            index = end;
            continue;
        }

        if is_valid_json_container(&source[index..end]) {
            return Some((index, end));
        }

        index += 1;
    }

    None
}

fn matching_close(open: u8) -> Option<u8> {
    match open {
        b'{' => Some(b'}'),
        b'[' => Some(b']'),
        _ => None,
    }
}

fn find_matching_bracket(source: &str, start_index: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let open = *bytes.get(start_index)?;
    let close = matching_close(open)?;
    let mut depth = 1usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut index = start_index + 1;

    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        if byte == b'"' {
            in_string = true;
        } else if byte == open {
            depth += 1;
        } else if byte == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
        index += 1;
    }

    None
}

fn is_valid_json_container(candidate: &str) -> bool {
    let Ok(document) = JsonDecoder::default().decode_str(candidate) else {
        return false;
    };
    matches!(
        document.store.get(document.root).map(|node| node.kind),
        Some(TreeNodeKind::Sequence | TreeNodeKind::Mapping)
    )
}

#[cfg(test)]
mod tests {
    use super::{JsonBlockSpan, find_json_block_at_position};

    #[test]
    fn finds_smallest_json_container_at_cursor() {
        let span = find_json_block_at_position("json", "{\"a\":{\"b\":1}}", 0, 7);
        assert_eq!(
            span,
            JsonBlockSpan {
                found: true,
                start_byte: 0,
                end_byte: 13,
                start_row: 0,
                start_column: 0,
                end_row: 0,
                end_column: 13,
            }
        );
    }

    #[test]
    fn returns_empty_for_invalid_json_or_other_language() {
        assert_eq!(
            find_json_block_at_position("yaml", "{\"a\":1}", 0, 1),
            JsonBlockSpan::EMPTY
        );
        assert_eq!(
            find_json_block_at_position("json", "{\"a\":1", 0, 1),
            JsonBlockSpan::EMPTY
        );
    }

    #[test]
    fn does_not_find_json_block_after_object_end() {
        assert_eq!(
            find_json_block_at_position("json", "{\"a\":1}", 0, 7),
            JsonBlockSpan::EMPTY
        );
    }

    #[test]
    fn does_not_find_json_block_before_object_start() {
        assert_eq!(
            find_json_block_at_position("json", "{\"a\":1}", 0, 0),
            JsonBlockSpan::EMPTY
        );
    }

    #[test]
    fn does_not_find_json_block_after_array_end() {
        assert_eq!(
            find_json_block_at_position("json", "[1]", 0, 3),
            JsonBlockSpan::EMPTY
        );
    }

    #[test]
    fn does_not_find_json_block_before_array_start() {
        assert_eq!(
            find_json_block_at_position("json", "[1]", 0, 0),
            JsonBlockSpan::EMPTY
        );
    }

    #[test]
    fn finds_json_block_before_array_end() {
        assert!(find_json_block_at_position("json", "[1]", 0, 2).found);
    }

    #[test]
    fn still_finds_json_block_before_container_end() {
        assert!(find_json_block_at_position("json", "{\"a\":1}", 0, 5).found);
    }

    #[test]
    fn does_not_find_json_block_after_container_end() {
        assert_eq!(
            find_json_block_at_position("json", "{\"a\":1}", 0, 7),
            JsonBlockSpan::EMPTY
        );
    }

    #[test]
    fn still_finds_json_block_before_nested_end() {
        assert!(find_json_block_at_position("json", "{\"a\":{\"b\":1}}", 0, 11).found);
    }

    #[test]
    fn does_not_fall_back_to_outer_container_before_nested_start() {
        assert_eq!(
            find_json_block_at_position("json", "{\"a\":[1]}", 0, 5),
            JsonBlockSpan::EMPTY
        );
    }
}
