use crate::{
    core::{CoreError, ParseError, SemType},
    formats::DecodedDocument,
    stream::tree_builder::Builder,
};

use super::{
    diagnostics::{ErrorSpan, TokenSpan},
    streaming_parse::StreamingParser,
};

// ---------------------------------------------------------------------------
// NormalizedNumber
// ---------------------------------------------------------------------------

/// Result of normalising a JSON number literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedNumber {
    pub sem_type: SemType,
    pub value: String,
}

/// `parse_tree.normalizedNumberValue`.
///
/// * Integer literals (no `.`, `e`, or `E`) are returned as-is with
///   `SemType::Int`.
/// * Float literals that represent a whole number within `i64` range are
///   normalised to their integer representation (e.g. `"1.0e2"` becomes
///   `"100"` with `SemType::Int`).
/// * All other float literals are returned as-is with `SemType::Float`.
///
pub fn normalized_number_value(raw: &str) -> NormalizedNumber {
    normalized_number_value_inner(raw)
}

fn normalized_number_value_inner(raw: &str) -> NormalizedNumber {
    let has_float = raw.contains(|c: char| matches!(c, '.' | 'e' | 'E'));
    if !has_float {
        return NormalizedNumber {
            sem_type: SemType::Int,
            value: raw.to_string(),
        };
    }

    match raw.parse::<f64>() {
        Ok(f) if f.is_finite() && f == f.trunc() => {
            if f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                return NormalizedNumber {
                    sem_type: SemType::Int,
                    value: (f as i64).to_string(),
                };
            }
            NormalizedNumber {
                sem_type: SemType::Float,
                value: raw.to_string(),
            }
        }
        _ => NormalizedNumber {
            sem_type: SemType::Float,
            value: raw.to_string(),
        },
    }
}

// ---------------------------------------------------------------------------
// DecodeWithTokenSpansResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DecodeWithTokenSpansResult {
    pub document: DecodedDocument,
    pub token_spans: Vec<TokenSpan>,
    pub error_spans: Vec<ErrorSpan>,
}

/// Parse a JSON source string into a document tree using the incremental
/// [`StreamingParser`], with a configurable nesting depth for nested-JSON
/// expansion.
///
/// This is the primary entry point that wires together the real streaming
/// parser pipeline: scanner -> parser -> tree builder.
pub fn decode_slice_to_tree_with_depth(
    source: &str,
    nest_json: bool,
    emit_path: bool,
    nest_depth: u8,
) -> Result<DecodedDocument, CoreError> {
    let mut builder = Builder::new();
    let mut parser = StreamingParser::with_builder(nest_json, emit_path, &mut builder);
    parser.set_nest_depth(nest_depth);

    // Feed the source and finalise.
    parser
        .feed(source)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;

    parser
        .finish_without_events()
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;

    let document = builder.into_document()?;

    Ok(document)
}

// ---------------------------------------------------------------------------
// decode_slice_to_tree
// ---------------------------------------------------------------------------

/// Parse a JSON source string into a document tree.
///
/// Normalises the source (strips BOM, handles UTF-16, trims whitespace,
/// validates document boundaries) and delegates to
/// [`decode_slice_to_tree_with_depth`].
pub fn decode_slice_to_tree(source: &str) -> Result<DecodedDocument, CoreError> {
    // Use the byte-based normalizer to handle BOM and UTF-16, then trim.
    let normalized = super::diagnostics::normalize_source(source.as_bytes())
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    let normalized = normalized.trim().to_string();
    // Validate that the source contains at least one complete document.
    let ranges = super::diagnostics::split_documents(&normalized)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    if ranges.is_empty() {
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }
    decode_slice_to_tree_with_depth(&normalized, false, false, 0)
}

/// Parse a JSON source byte slice into a document tree.
///
/// Normalises the source (strips BOM, handles UTF-16, trims whitespace,
/// validates document boundaries) and delegates to
/// [`decode_slice_to_tree_with_depth`].
///
/// (raw bytes) rather than a validated UTF-8 string.
pub fn decode_bytes_to_tree(bytes: &[u8]) -> Result<DecodedDocument, CoreError> {
    let normalized = super::diagnostics::normalize_source(bytes)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    let normalized = normalized.trim().to_string();
    let ranges = super::diagnostics::split_documents(&normalized)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    if ranges.is_empty() {
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }
    decode_slice_to_tree_with_depth(&normalized, false, false, 0)
}

// ---------------------------------------------------------------------------
// decode_slice_to_document (convenience alias)
// ---------------------------------------------------------------------------

pub fn decode_slice_to_document(source: &str) -> Result<DecodedDocument, CoreError> {
    decode_slice_to_tree(source)
}

// ---------------------------------------------------------------------------
// decode_slice_to_tree_with_token_spans
// ---------------------------------------------------------------------------

/// Parse a JSON source string into a document tree, also collecting
/// [`TokenSpan`]s and [`ErrorSpan`]s via the streaming parser's built-in
/// collectors.
///
/// This uses the real streaming parser pipeline so that token spans are
/// collected incrementally during parsing rather than via a separate
/// post-hoc scan.
pub fn decode_slice_to_tree_with_token_spans(
    source: &str,
) -> Result<DecodeWithTokenSpansResult, CoreError> {
    let normalized = super::diagnostics::normalize_source(source.as_bytes())
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    let normalized = normalized.trim().to_string();

    let mut parser = StreamingParser::new(false);
    parser
        .feed(&normalized)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;

    let result = parser
        .finish_with_token_spans()
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;

    Ok(result)
}

// ---------------------------------------------------------------------------
// decode_slice_to_document_with_token_spans (convenience alias)
// ---------------------------------------------------------------------------

pub fn decode_slice_to_document_with_token_spans(
    source: &str,
) -> Result<DecodeWithTokenSpansResult, CoreError> {
    decode_slice_to_tree_with_token_spans(source)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{SemType, TreeNodeKind, get_map_entry};

    // ------------------------------------------------------------------
    // normalized_number_value
    // ------------------------------------------------------------------

    #[test]
    fn normalizes_integer_literal() {
        let result = normalized_number_value("42");
        assert_eq!(result.sem_type, SemType::Int);
        assert_eq!(result.value, "42");
    }

    #[test]
    fn normalizes_float_to_int() {
        let result = normalized_number_value("1.0e2");
        assert_eq!(result.sem_type, SemType::Int);
        assert_eq!(result.value, "100");
    }

    #[test]
    fn preserves_true_float() {
        let result = normalized_number_value("3.14");
        assert_eq!(result.sem_type, SemType::Float);
        assert_eq!(result.value, "3.14");
    }

    // ------------------------------------------------------------------
    // decode_slice_to_tree
    // ------------------------------------------------------------------

    #[test]
    fn decodes_simple_object() {
        let doc = decode_slice_to_tree(r#"{"key":"value"}"#).unwrap();
        let root = doc.store.get(doc.root).unwrap();
        assert_eq!(root.kind, TreeNodeKind::Mapping);
        let entry = get_map_entry(&doc.store, doc.root, "key").unwrap().unwrap();
        assert_eq!(doc.store.value_for(entry.value).unwrap(), "value");
    }

    #[test]
    fn decodes_nested_structure() {
        let doc = decode_slice_to_tree(r#"{"a":{"b":[1,2,3]}}"#).unwrap();
        let root = doc.store.get(doc.root).unwrap();
        assert_eq!(root.kind, TreeNodeKind::Mapping);
    }

    #[test]
    fn rejects_invalid_json() {
        let result = decode_slice_to_tree("{invalid");
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // decode_slice_to_tree_with_token_spans
    // ------------------------------------------------------------------

    #[test]
    fn collects_token_spans() {
        let result = decode_slice_to_tree_with_token_spans(r#"{"x":1}"#).unwrap();
        assert!(!result.token_spans.is_empty(), "should have token spans");
        // The document should still be valid.
        let root = result.document.store.get(result.document.root).unwrap();
        assert_eq!(root.kind, TreeNodeKind::Mapping);
    }

    // ------------------------------------------------------------------
    // decode_slice_to_tree_with_depth
    // ------------------------------------------------------------------

    #[test]
    fn depth_limits_nested_json_expansion() {
        // At the maximum nesting depth, nested JSON strings should not expand.
        let doc = decode_slice_to_tree_with_depth(r#"{"nested":"{\"inner\":1}"}"#, true, false, 8)
            .unwrap();
        let entry = get_map_entry(&doc.store, doc.root, "nested")
            .unwrap()
            .unwrap();
        // Should be a scalar string, not expanded.
        assert_eq!(
            doc.store.get(entry.value).unwrap().kind,
            TreeNodeKind::Scalar
        );
    }
}
