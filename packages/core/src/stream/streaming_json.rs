pub mod config;
pub mod diagnostics;
pub mod nested_json;
pub mod parse_tree;
pub mod parser;
pub mod scanner;
pub mod streaming_parse;

pub use config::{DEFAULT_SCANNER_COMPACT_THRESHOLD, JsonScannerConfig, scanner_compact_threshold};
pub use diagnostics::{
    DocRange, ErrorSpan, ParseFailure, TokenSpan, error_spans, first_parse_failure,
    normalize_source, split_documents, token_spans,
};
pub use nested_json::{is_nested_json_candidate, trim_ascii_whitespace};
pub use parse_tree::{
    DecodeWithTokenSpansResult, NormalizedNumber, decode_bytes_to_tree, decode_slice_to_document,
    decode_slice_to_document_with_token_spans, decode_slice_to_tree,
    decode_slice_to_tree_with_depth, decode_slice_to_tree_with_token_spans,
    normalized_number_value,
};
pub use parser::{JsonStreamError, decode};
pub use scanner::{Position, Scanner, Span, Token, TokenCache, TokenTag};
pub use streaming_parse::{
    Decoder, ErrorSpanCollector, SourceRewrite, StreamDecoder, StreamingParser, TokenSpanCollector,
};
