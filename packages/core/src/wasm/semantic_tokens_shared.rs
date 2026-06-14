use std::cmp::Ordering;

use crate::core::tree_store::TokenSpan;
use crate::wasm_types::WasmProtocol;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Per-line UTF-8 to UTF-16 column mapping.
#[derive(Debug, Clone)]
pub struct LineInfo {
    /// Byte offset of the first character on this line (inclusive).
    pub start_byte: usize,
    /// Byte offset just past the last character on this line (exclusive).
    pub end_byte: usize,
    /// Total UTF-16 code units on this line.
    pub utf16_length: u32,
    /// Maps every byte offset (0..=line.len()) to its UTF-16 column.
    pub byte_to_utf16: Vec<u32>,
}

/// An error span for raw encoding (no UTF-16 normalization, no delta encoding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorSpan {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
    pub kind: u32,
}

// ---------------------------------------------------------------------------
// High-level entry point (existing)
// ---------------------------------------------------------------------------

/// Encode semantic tokens for a document given its language name and source.
///
/// This is the WASM-facing entry point that resolves the language protocol and
/// delegates to the core semantic-tokens pipeline.
pub fn encode_document_semantic_tokens(language: &str, source: &str) -> Vec<u32> {
    let Some(protocol) = WasmProtocol::from_name(language) else {
        return Vec::new();
    };
    crate::core::encode_semantic_tokens(protocol.canonical_name(), source)
}

// ---------------------------------------------------------------------------
// UTF-8 to UTF-16 column mapping
// ---------------------------------------------------------------------------

/// Build per-line UTF-8 to UTF-16 column mappings for `source`.
///
/// Iterates byte-by-byte through the source, recording the current UTF-16
/// column for every byte offset.  Multi-byte UTF-8 sequences are decoded and
/// their UTF-16 width (1 or 2 code units) is added.  Invalid UTF-8 bytes are
/// treated as 1 UTF-16 unit each.
pub fn build_line_infos(source: &str) -> Vec<LineInfo> {
    let bytes = source.as_bytes();
    let newline_count = bytes.iter().filter(|&&b| b == b'\n').count();
    let mut infos = Vec::with_capacity(newline_count + 1);
    let mut line_start = 0usize;

    for (index, &byte) in bytes.iter().enumerate() {
        if byte != b'\n' {
            continue;
        }
        let line_end = if index > line_start && bytes[index - 1] == b'\r' {
            index - 1
        } else {
            index
        };
        infos.push(build_line_info(source, line_start, line_end));
        line_start = index + 1;
    }

    let line_end = if bytes.len() > line_start && bytes[bytes.len() - 1] == b'\r' {
        bytes.len() - 1
    } else {
        bytes.len()
    };
    infos.push(build_line_info(source, line_start, line_end));

    infos
}

fn build_line_info(source: &str, start_byte: usize, end_byte: usize) -> LineInfo {
    let line = &source[start_byte..end_byte];
    let mut byte_to_utf16 = Vec::with_capacity(line.len() + 1);
    let mut utf16_length = 0_u32;
    byte_to_utf16.push(0);

    for ch in line.chars() {
        for _ in 1..ch.len_utf8() {
            byte_to_utf16.push(utf16_length);
        }
        utf16_length += ch.len_utf16() as u32;
        byte_to_utf16.push(utf16_length);
    }

    LineInfo {
        start_byte,
        end_byte,
        utf16_length,
        byte_to_utf16,
    }
}

// ---------------------------------------------------------------------------
// Span manipulation helpers
// ---------------------------------------------------------------------------

/// Convert a byte-column to a UTF-16 column using the per-line mapping.
fn byte_column_to_utf16(line_info: &LineInfo, byte_col: u32) -> u32 {
    let byte_col = byte_col as usize;
    if line_info.byte_to_utf16.is_empty() {
        return 0;
    }
    if byte_col >= line_info.byte_to_utf16.len() {
        return line_info.utf16_length;
    }
    line_info.byte_to_utf16[byte_col]
}

/// Normalize a token span by converting byte-columns to UTF-16 columns.
///
/// Returns `None` if the span is completely out of bounds.
pub fn normalize_token_span(span: TokenSpan, line_infos: &[LineInfo]) -> Option<TokenSpan> {
    if line_infos.is_empty() {
        return None;
    }
    let start_row = span.start_row as usize;
    if start_row >= line_infos.len() {
        return None;
    }
    let mut end_row = span.end_row as usize;
    if end_row >= line_infos.len() {
        end_row = line_infos.len() - 1;
    }
    if end_row < start_row {
        return None;
    }
    Some(TokenSpan {
        start_row: span.start_row,
        start_col: byte_column_to_utf16(&line_infos[start_row], span.start_col),
        end_row: end_row as u32,
        end_col: byte_column_to_utf16(&line_infos[end_row], span.end_col),
        token_type: span.token_type,
    })
}

/// Compare two (row, column) positions.
fn compare_token_position(a_row: u32, a_col: u32, b_row: u32, b_col: u32) -> Ordering {
    match a_row.cmp(&b_row) {
        Ordering::Equal => a_col.cmp(&b_col),
        other => other,
    }
}

/// Clip the start of `span` so it does not begin before (`start_row`, `start_col`).
///
/// Returns `false` if the span is entirely before the clip point (should be
/// discarded).  Returns `true` and mutates `span.start_*` if the clip point
/// falls inside the span.
pub fn clip_span_start(span: &mut TokenSpan, start_row: u32, start_col: u32) -> bool {
    if compare_token_position(span.start_row, span.start_col, start_row, start_col).is_ge() {
        return true;
    }
    if compare_token_position(span.end_row, span.end_col, start_row, start_col).is_le() {
        return false;
    }
    span.start_row = start_row;
    span.start_col = start_col;
    true
}

/// Check whether `outer` fully contains `inner`.
pub fn span_contains(outer: TokenSpan, inner: TokenSpan) -> bool {
    compare_token_position(
        outer.start_row,
        outer.start_col,
        inner.start_row,
        inner.start_col,
    )
    .is_le()
        && compare_token_position(outer.end_row, outer.end_col, inner.end_row, inner.end_col)
            .is_ge()
}

/// Emit a single token span as one or more LSP delta-encoded 5-tuples.
///
/// Multi-line spans are split into one tuple per line.  Each tuple is
/// `[delta_line, delta_start, length, token_type, 0]`.
pub fn emit_range(
    line_infos: &[LineInfo],
    out: &mut Vec<u32>,
    prev_position: &mut (u32, u32),
    span: TokenSpan,
) {
    if line_infos.is_empty() {
        return;
    }
    let start_row = span.start_row as usize;
    if start_row >= line_infos.len() {
        return;
    }

    let clamped_end_row = (span.end_row as usize).min(line_infos.len() - 1);
    let normalized_start_col = span.start_col.min(line_infos[start_row].utf16_length);
    let mut normalized_end_col = span.end_col.min(line_infos[clamped_end_row].utf16_length);

    if compare_token_position(
        span.start_row,
        normalized_start_col,
        clamped_end_row as u32,
        normalized_end_col,
    )
    .is_ge()
    {
        return;
    }

    // Single-line span.
    if start_row == clamped_end_row {
        if normalized_end_col > normalized_start_col {
            push_encoded_span(
                out,
                prev_position,
                start_row as u32,
                normalized_start_col,
                normalized_end_col - normalized_start_col,
                span.token_type,
            );
        }
        return;
    }

    // Multi-line span: first line.
    let start_line_len = line_infos[start_row].utf16_length;
    if start_line_len > normalized_start_col {
        push_encoded_span(
            out,
            prev_position,
            start_row as u32,
            normalized_start_col,
            start_line_len - normalized_start_col,
            span.token_type,
        );
    }

    // Middle lines (full lines).
    for row in (start_row + 1)..clamped_end_row {
        let line_len = line_infos[row].utf16_length;
        if line_len > 0 {
            push_encoded_span(out, prev_position, row as u32, 0, line_len, span.token_type);
        }
    }

    // Last line.
    normalized_end_col = normalized_end_col.min(line_infos[clamped_end_row].utf16_length);
    if normalized_end_col > 0 {
        push_encoded_span(
            out,
            prev_position,
            clamped_end_row as u32,
            0,
            normalized_end_col,
            span.token_type,
        );
    }
}

/// Push a single LSP delta-encoded 5-tuple: `[delta_line, delta_start, length, token_type, 0]`.
fn push_encoded_span(
    out: &mut Vec<u32>,
    prev_position: &mut (u32, u32),
    line: u32,
    column: u32,
    length: u32,
    token_type: u32,
) {
    if length == 0 {
        return;
    }
    let delta_line = line.saturating_sub(prev_position.0);
    let delta_start = if delta_line == 0 {
        column.saturating_sub(prev_position.1)
    } else {
        column
    };
    out.extend([delta_line, delta_start, length, token_type, 0]);
    *prev_position = (line, column);
}

// ---------------------------------------------------------------------------
// LSP-style 5-tuple encoding
// ---------------------------------------------------------------------------

/// Encode token spans into LSP-style delta-encoded u32 quintuples.
///
/// This is the canonical entry point for semantic token encoding.  It:
///
/// 1. Copies and sorts spans by (start_row, start_col, end_row, end_col, token_type).
/// 2. Builds per-line UTF-8 to UTF-16 column mappings.
/// 3. Normalizes byte-columns to UTF-16 columns.
/// 4. Deduplicates identical and contained spans.
/// 5. Clips overlapping spans so they don't overlap.
/// 6. Emits delta-encoded 5-tuples.
pub fn encode_token_spans_to_u32(spans: &[TokenSpan], source: &str) -> Vec<u32> {
    if spans.is_empty() {
        return Vec::new();
    }

    // 1. Copy and sort.
    let mut working = spans.to_vec();
    working.sort_unstable_by(|lhs, rhs| {
        lhs.start_row
            .cmp(&rhs.start_row)
            .then_with(|| lhs.start_col.cmp(&rhs.start_col))
            .then_with(|| lhs.end_row.cmp(&rhs.end_row))
            .then_with(|| lhs.end_col.cmp(&rhs.end_col))
            .then_with(|| lhs.token_type.cmp(&rhs.token_type))
    });

    // 2. Build line infos.
    let line_infos = build_line_infos(source);
    if line_infos.is_empty() {
        return Vec::new();
    }

    // 3-6. Normalize, dedup, clip, emit.
    let mut out = Vec::with_capacity(working.len() * 5);
    let mut prev_position: (u32, u32) = (0, 0);
    let mut prev_span: Option<TokenSpan> = None;

    for raw_span in working {
        let Some(mut span) = normalize_token_span(raw_span, &line_infos) else {
            continue;
        };

        if let Some(last) = prev_span {
            // Skip exact duplicates.
            if last.start_row == span.start_row
                && last.start_col == span.start_col
                && last.end_row == span.end_row
                && last.end_col == span.end_col
            {
                continue;
            }
            // Skip if the previous span (same type) already contains this one.
            if last.token_type == span.token_type && span_contains(span, last) {
                continue;
            }
            // Clip start to avoid overlap with previous span.
            if !clip_span_start(&mut span, last.end_row, last.end_col) {
                continue;
            }
            // Re-check for duplicates after clipping.
            if last.start_row == span.start_row
                && last.start_col == span.start_col
                && last.end_row == span.end_row
                && last.end_col == span.end_col
            {
                continue;
            }
        }

        emit_range(&line_infos, &mut out, &mut prev_position, span);
        prev_span = Some(span);
    }

    out
}

// ---------------------------------------------------------------------------
// Error span encoding
// ---------------------------------------------------------------------------

/// Encode error spans as raw u32 quintuples (no UTF-16 normalization, no delta
/// encoding).
///
/// Each span produces 5 u32 values: `[start_row, start_col, end_row, end_col, kind]`.
pub fn encode_error_spans_raw(spans: &[ErrorSpan]) -> Vec<u32> {
    if spans.is_empty() {
        return Vec::new();
    }
    let mut raw = Vec::with_capacity(spans.len() * 5);
    for span in spans {
        raw.extend([
            span.start_row,
            span.start_col,
            span.end_row,
            span.end_col,
            span.kind,
        ]);
    }
    raw
}

#[cfg(test)]
mod tests {
    use super::{
        ErrorSpan, build_line_infos, clip_span_start, encode_error_spans_raw,
        encode_token_spans_to_u32, span_contains,
    };
    use crate::core::tree_store::TokenSpan;

    #[test]
    fn build_line_infos_tracks_utf16_columns_for_multibyte_text() {
        let infos = build_line_infos("a😀\r\n中b\n");

        assert_eq!(infos.len(), 3);
        assert_eq!(infos[0].utf16_length, 3);
        assert_eq!(infos[0].byte_to_utf16, vec![0, 1, 1, 1, 1, 3]);
        assert_eq!(infos[1].utf16_length, 2);
        assert_eq!(infos[1].byte_to_utf16, vec![0, 0, 0, 1, 2]);
        assert_eq!(infos[2].utf16_length, 0);
        assert_eq!(infos[2].byte_to_utf16, vec![0]);
    }

    #[test]
    fn encode_token_spans_to_u32_normalizes_and_splits_multiline_ranges() {
        let source = "a😀\nxy\n";
        let spans = [TokenSpan {
            start_row: 0,
            start_col: 1,
            end_row: 1,
            end_col: 2,
            token_type: 7,
        }];

        let encoded = encode_token_spans_to_u32(&spans, source);

        assert_eq!(encoded, vec![0, 1, 2, 7, 0, 1, 0, 2, 7, 0]);
    }

    #[test]
    fn clip_span_start_and_span_contains_match_zig_overlap_rules() {
        let outer = TokenSpan {
            start_row: 0,
            start_col: 1,
            end_row: 0,
            end_col: 4,
            token_type: 2,
        };
        let inner = TokenSpan {
            start_row: 0,
            start_col: 2,
            end_row: 0,
            end_col: 3,
            token_type: 2,
        };
        assert!(span_contains(outer, inner));

        let mut overlapping = TokenSpan {
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 5,
            token_type: 2,
        };
        assert!(clip_span_start(&mut overlapping, 0, 3));
        assert_eq!(overlapping.start_col, 3);

        let mut entirely_before = TokenSpan {
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 2,
            token_type: 2,
        };
        assert!(!clip_span_start(&mut entirely_before, 0, 3));
    }

    #[test]
    fn encode_error_spans_raw_writes_quintuples_in_order() {
        let spans = [
            ErrorSpan {
                start_row: 1,
                start_col: 2,
                end_row: 3,
                end_col: 4,
                kind: 5,
            },
            ErrorSpan {
                start_row: 6,
                start_col: 7,
                end_row: 8,
                end_col: 9,
                kind: 10,
            },
        ];

        let encoded = encode_error_spans_raw(&spans);

        assert_eq!(encoded, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }
}
