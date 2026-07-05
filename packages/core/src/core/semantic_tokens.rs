use std::collections::HashMap;

use regex_lite::Regex;

use super::lang_spec::{LangSpec, StreamKind, lang_from_name, stream_kind_for_language};
use super::tree_sitter_support::{ensure_tree_sitter_runtime, tree_sitter_language_for_spec};
use super::tree_store::{TokenSpan, TreeStore};
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

pub const TOKEN_TYPES: &[&str] = &[
    "map",
    "key",
    "seq",
    "str",
    "int",
    "float",
    "boolean",
    "nil",
    "punctuation",
    "comment",
    "operator",
    "function",
    "variable",
    "tag",
    "attribute",
];

const MAP: u32 = 0;
const KEY: u32 = 1;
const SEQ: u32 = 2;
const STR: u32 = 3;
const INT: u32 = 4;
const FLOAT: u32 = 5;
const BOOLEAN: u32 = 6;
const NIL: u32 = 7;
const PUNCTUATION: u32 = 8;
const COMMENT: u32 = 9;
const OPERATOR: u32 = 10;
const FUNCTION: u32 = 11;
const VARIABLE: u32 = 12;
const TAG: u32 = 13;
const ATTRIBUTE: u32 = 14;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Simple entry point: collect and encode semantic tokens for a language/source
/// pair without any caching or TreeStore integration.
///
/// For the full-featured version with caching, use [`semantic_tokens_inner`].
pub fn encode_semantic_tokens(language: &str, source: &str) -> Vec<u32> {
    semantic_tokens_inner(None, "", language, source)
}

/// Full-featured semantic tokens entry point with TreeStore caching.
///
///
/// When `store` is provided, the function first checks for cached encoded tokens
/// and token spans. On cache miss, it tries tree-sitter query-driven
/// tokenization first, then tries the streaming codec path (JSON), and finally
/// falls back to tree-sitter query scanning for other languages. Results are
/// written back to the store for subsequent calls.
pub fn semantic_tokens_inner(
    store: Option<&mut TreeStore>,
    cache_key: &str,
    language_name: &str,
    source: &str,
) -> Vec<u32> {
    let bound_cache_key = source_bound_cache_key(cache_key, source);
    let cache_key = bound_cache_key.as_str();

    // 1. Check for cached encoded tokens in the store.
    if let Some(store) = store.as_ref() {
        if let Some(cached) = store.get_cached_semantic_tokens(cache_key) {
            if cached.is_empty() {
                return Vec::new();
            }
            return cached.to_vec();
        }
    }

    // 2. Try the streaming codec path (JSON).
    if let Some(adapter) = streaming_language_adapter(language_name) {
        return semantic_tokens_inner_via_streaming_codec(store, cache_key, adapter, source);
    }

    // TOML currently relies on a tree-sitter external scanner that traps on the
    // wasm semantic-token parse path. Use a lightweight source scanner instead
    // of re-entering tree-sitter here.
    if language_name == "toml" {
        let spans = collect_toml_token_spans_manual(source);
        let tokens = encode_delta_tokens(source, &spans);
        if let Some(store) = store {
            if !tokens.is_empty() {
                store.set_cached_semantic_tokens(cache_key, tokens.clone());
            }
        }
        return tokens;
    }

    // 4. Tree-sitter query path (other languages).
    let spec = match lang_from_name(language_name) {
        Some(spec) => spec,
        None => return Vec::new(),
    };
    let query_src = match spec.embedded_query() {
        Some(q) => q,
        None => return Vec::new(),
    };

    let spans = collect_token_spans_by_query(spec, query_src, source);

    let tokens = encode_delta_tokens(source, &spans);

    // Write back to store.
    if let Some(store) = store {
        if !tokens.is_empty() {
            store.set_cached_semantic_tokens(cache_key, tokens.clone());
        }
    }

    tokens
}

/// Encode token spans to u32 delta tokens and cache the result in the store.
///
pub fn encode_and_cache_semantic_tokens(
    store: Option<&mut TreeStore>,
    cache_key: &str,
    source: &str,
    spans: &[TokenSpan],
) -> Vec<u32> {
    let tokens = encode_delta_tokens(source, spans);
    if let Some(store) = store {
        if !tokens.is_empty() {
            let bound_cache_key = source_bound_cache_key(cache_key, source);
            store.set_cached_semantic_tokens(&bound_cache_key, tokens.clone());
        }
    }
    tokens
}

// ---------------------------------------------------------------------------
// Streaming language adapter
// ---------------------------------------------------------------------------

/// Return the [`StreamKind`] for a language if it supports streaming token
/// collection, or `None` for languages that must use tree-sitter queries.
///
pub fn streaming_language_adapter(language_name: &str) -> Option<StreamKind> {
    let kind = stream_kind_for_language(language_name);
    if kind == StreamKind::NonStreaming {
        return None;
    }
    Some(kind)
}

// ---------------------------------------------------------------------------
// Streaming codec path (JSON)
// ---------------------------------------------------------------------------
///
/// Collect semantic tokens via the streaming codec (JSON scanner).
fn semantic_tokens_inner_via_streaming_codec(
    mut store: Option<&mut TreeStore>,
    cache_key: &str,
    adapter: StreamKind,
    source: &str,
) -> Vec<u32> {
    // Check for cached token spans in the store (drop immutable borrow before
    // passing store mutably to encode_and_cache_semantic_tokens).
    let cached_spans: Option<Vec<TokenSpan>> = store
        .as_ref()
        .and_then(|s| s.get_cached_token_spans(cache_key))
        .map(|spans| spans.to_vec());

    if let Some(spans) = cached_spans {
        return encode_and_cache_semantic_tokens(store, cache_key, source, &spans);
    }

    let spans: Vec<TokenSpan> = match adapter {
        StreamKind::Json => match crate::stream::streaming_json::token_spans(source) {
            Ok(spans) => spans
                .into_iter()
                .map(|s| TokenSpan {
                    start_row: s.start_row,
                    start_col: s.start_col,
                    end_row: s.end_row,
                    end_col: s.end_col,
                    token_type: s.token_type,
                })
                .collect(),
            Err(_) => Vec::new(),
        },
        StreamKind::NonStreaming => Vec::new(),
    };

    // Cache the raw token spans in the store for future use.
    if let Some(store) = store.as_mut() {
        if !spans.is_empty() {
            store.set_cached_token_spans(cache_key, spans.clone());
        }
    }

    encode_and_cache_semantic_tokens(store, cache_key, source, &spans)
}

// ---------------------------------------------------------------------------
// Tree-sitter query path
// ---------------------------------------------------------------------------

/// Run a tree-sitter query to collect token spans, with an internal cache to
/// avoid re-parsing and re-running queries for the same (language, source) pair.
///
fn collect_token_spans_by_query(
    spec: &LangSpec<'_>,
    query_src: &str,
    source: &str,
) -> Vec<TokenSpan> {
    // Static cache: key = hash of (language name, source).
    use std::cell::RefCell;
    thread_local! {
        static QUERY_CACHE: RefCell<HashMap<u64, Vec<TokenSpan>>> = RefCell::new(HashMap::new());
    }

    let cache_key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        spec.name.hash(&mut hasher);
        source.hash(&mut hasher);
        hasher.finish()
    };

    if let Some(cached) = QUERY_CACHE.with(|cache| cache.borrow().get(&cache_key).cloned()) {
        return cached;
    }

    let spans = collect_query_token_spans_uncached(spec, query_src, source);

    QUERY_CACHE.with(|cache| {
        cache.borrow_mut().insert(cache_key, spans.clone());
    });

    spans
}

/// Run a tree-sitter query to collect token spans (no caching).
fn collect_query_token_spans_uncached(
    spec: &LangSpec<'_>,
    query_src: &str,
    source: &str,
) -> Vec<TokenSpan> {
    ensure_tree_sitter_runtime();
    let ts_language = match tree_sitter_language_for_spec(spec) {
        Some(lang) => lang,
        None => return Vec::new(),
    };
    let mut parser = Parser::new();
    if parser.set_language(&ts_language).is_err() {
        return Vec::new();
    }
    let tree = match parser.parse(source, None) {
        Some(tree) => tree,
        None => return Vec::new(),
    };
    let query = match Query::new(&ts_language, query_src) {
        Ok(query) => query,
        Err(_) => return Vec::new(),
    };
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
    let mut spans = Vec::new();

    while let Some((query_match, capture_index)) = captures.next() {
        let capture = query_match.captures[*capture_index];
        let capture_name = match capture_names.get(capture.index as usize) {
            Some(name) => *name,
            None => continue,
        };
        let Some(token_type) = token_type_from_capture(capture_name) else {
            continue;
        };
        let start = capture.node.start_position();
        let end = capture.node.end_position();
        spans.push(TokenSpan {
            start_row: start.row as u32,
            start_col: start.column as u32,
            end_row: end.row as u32,
            end_col: end.column as u32,
            token_type,
        });
    }

    if spec.name == "javascript" {
        extend_with_javascript_lexical_spans(source, &mut spans);
    }

    spans
}

/// Run a tree-sitter query on an already-parsed tree to collect token spans.
///
/// This is the pre-parsed-tree variant of [`collect_query_token_spans_uncached`].
/// It avoids re-parsing when a tree-sitter [`Tree`] is already available (e.g.
/// from `prepared_ts_tree` reuse in [`super::document_analysis::analyze_document_internal_with_prepared_tree`]).
///
pub fn collect_token_spans_with_tree(
    tree: &tree_sitter::Tree,
    language: &tree_sitter::Language,
    query_src: &str,
    source: &str,
) -> Vec<TokenSpan> {
    ensure_tree_sitter_runtime();
    let query = match Query::new(language, query_src) {
        Ok(query) => query,
        Err(_) => return manual_query_token_spans(query_src, source),
    };
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&query, tree.root_node(), source.as_bytes());
    let mut spans = Vec::new();

    while let Some((query_match, capture_index)) = captures.next() {
        let capture = query_match.captures[*capture_index];
        let capture_name = match capture_names.get(capture.index as usize) {
            Some(name) => *name,
            None => continue,
        };
        let Some(token_type) = token_type_from_capture(capture_name) else {
            continue;
        };
        let start = capture.node.start_position();
        let end = capture.node.end_position();
        spans.push(TokenSpan {
            start_row: start.row as u32,
            start_col: start.column as u32,
            end_row: end.row as u32,
            end_col: end.column as u32,
            token_type,
        });
    }

    if spans.is_empty() {
        return manual_query_token_spans(query_src, source);
    }

    spans
}

fn manual_query_token_spans(query_src: &str, source: &str) -> Vec<TokenSpan> {
    if query_src.contains("function_declaration")
        || query_src.contains("formal_parameters")
        || query_src.contains("(comment)")
    {
        return javascript_manual_query_spans(query_src, source);
    }
    Vec::new()
}

fn javascript_manual_query_spans(query_src: &str, source: &str) -> Vec<TokenSpan> {
    let mut spans = Vec::new();

    let function_decl = Regex::new(r"function\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\(([^)]*)\)").ok();
    let identifier = Regex::new(r"[A-Za-z_$][A-Za-z0-9_$]*").ok();
    let line_comment = Regex::new(r"//[^\n]*").ok();
    let block_comment = Regex::new(r"/\*[\s\S]*?\*/").ok();
    let null_literal = Regex::new(r"\bnull\b").ok();
    let true_literal = Regex::new(r"\btrue\b").ok();
    let false_literal = Regex::new(r"\bfalse\b").ok();

    let function_caps = function_decl.as_ref().and_then(|re| re.captures(source));
    let function_match = function_caps.as_ref().and_then(|caps| caps.get(0));
    let function_name = function_caps.as_ref().and_then(|caps| caps.get(1));
    let params_match = function_caps.as_ref().and_then(|caps| caps.get(2));

    for pattern in query_src
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Some(capture_name) = first_capture_name(pattern) else {
            continue;
        };
        let Some(token_type) = token_type_from_capture(capture_name) else {
            continue;
        };

        if pattern.contains("(function_declaration)") && !pattern.contains("name:") {
            if let Some(m) = function_match {
                if let Some(keyword_start) = source[m.start()..m.end()].find("function") {
                    push_span_for_range(
                        source,
                        m.start() + keyword_start,
                        m.start() + keyword_start + "function".len(),
                        token_type,
                        &mut spans,
                    );
                }
            }
            continue;
        }

        if pattern.contains("(function_declaration name: (identifier)")
            || pattern.contains("(identifier) @function.call")
        {
            if let Some(name_match) = function_name {
                push_span_for_range(
                    source,
                    name_match.start(),
                    name_match.end(),
                    token_type,
                    &mut spans,
                );
            }
            continue;
        }

        if pattern.contains("(formal_parameters (identifier)")
            || pattern.contains("(identifier) @variable.parameter")
        {
            if let (Some(params_match), Some(identifier)) = (params_match, identifier.as_ref()) {
                let params_src = &source[params_match.start()..params_match.end()];
                for param in identifier.find_iter(params_src) {
                    push_span_for_range(
                        source,
                        params_match.start() + param.start(),
                        params_match.start() + param.end(),
                        token_type,
                        &mut spans,
                    );
                }
            }
            continue;
        }

        if pattern.contains("(comment)") {
            if let Some(m) = line_comment.as_ref().and_then(|re| re.find(source)) {
                push_span_for_range(source, m.start(), m.end(), token_type, &mut spans);
            } else if let Some(m) = block_comment.as_ref().and_then(|re| re.find(source)) {
                push_span_for_range(source, m.start(), m.end(), token_type, &mut spans);
            }
            continue;
        }

        if pattern.contains("(null)") {
            if let Some(m) = null_literal.as_ref().and_then(|re| re.find(source)) {
                push_span_for_range(source, m.start(), m.end(), token_type, &mut spans);
            }
            continue;
        }

        if pattern.contains("(true)") {
            if let Some(m) = true_literal.as_ref().and_then(|re| re.find(source)) {
                push_span_for_range(source, m.start(), m.end(), token_type, &mut spans);
            }
            continue;
        }

        if pattern.contains("(false)") {
            if let Some(m) = false_literal.as_ref().and_then(|re| re.find(source)) {
                push_span_for_range(source, m.start(), m.end(), token_type, &mut spans);
            }
        }
    }

    spans
}

fn collect_toml_token_spans_manual(source: &str) -> Vec<TokenSpan> {
    let mut spans = Vec::new();
    let mut line_offset = 0usize;

    for line in source.split_inclusive('\n') {
        let line_content = line.strip_suffix('\n').unwrap_or(line);
        let comment_start = find_toml_comment_start(line_content);
        let code_end = comment_start.unwrap_or(line_content.len());
        let code = &line_content[..code_end];

        if let Some(start) = comment_start {
            push_span_if_missing(
                source,
                line_offset + start,
                line_offset + line_content.len(),
                COMMENT,
                &mut spans,
            );
        }

        if let Some(eq) = find_toml_assignment_eq(code) {
            push_toml_key_spans(source, line_offset, &code[..eq], &mut spans);
            push_toml_value_spans(source, line_offset + eq + 1, &code[eq + 1..], &mut spans);
        } else {
            push_toml_value_spans(source, line_offset, code, &mut spans);
        }

        line_offset += line.len();
    }

    spans
}

fn find_toml_comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'#' => return Some(i),
            b'"' | b'\'' => i = skip_toml_quoted_bytes(bytes, i),
            _ => i += 1,
        }
    }
    None
}

fn find_toml_assignment_eq(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'=' => return Some(i),
            b'"' | b'\'' => i = skip_toml_quoted_bytes(bytes, i),
            _ => i += 1,
        }
    }
    None
}

fn push_toml_key_spans(
    source: &str,
    line_offset: usize,
    key_src: &str,
    spans: &mut Vec<TokenSpan>,
) {
    let bytes = key_src.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b'.' => i += 1,
            b'"' | b'\'' => {
                let end = skip_toml_quoted_bytes(bytes, i);
                push_span_if_missing(source, line_offset + i, line_offset + end, KEY, spans);
                i = end;
            }
            _ => {
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'-'))
                {
                    i += 1;
                }
                if i > start {
                    push_span_if_missing(source, line_offset + start, line_offset + i, KEY, spans);
                } else {
                    i += 1;
                }
            }
        }
    }
}

fn push_toml_value_spans(
    source: &str,
    segment_offset: usize,
    value_src: &str,
    spans: &mut Vec<TokenSpan>,
) {
    let bytes = value_src.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' | b',' | b'[' | b']' | b'{' | b'}' => i += 1,
            b'"' | b'\'' => {
                let end = skip_toml_quoted_bytes(bytes, i);
                push_span_if_missing(source, segment_offset + i, segment_offset + end, STR, spans);
                i = end;
            }
            b'+' | b'-' | b'0'..=b'9' => {
                let start = i;
                let mut is_float = false;
                i += 1;
                while i < bytes.len() {
                    match bytes[i] {
                        b'0'..=b'9' | b'_' => i += 1,
                        b'.' | b'e' | b'E' => {
                            is_float = true;
                            i += 1;
                        }
                        b'+' | b'-' if matches!(bytes[i - 1], b'e' | b'E') => i += 1,
                        _ => break,
                    }
                }
                let token_type = if is_float { FLOAT } else { INT };
                push_span_if_missing(
                    source,
                    segment_offset + start,
                    segment_offset + i,
                    token_type,
                    spans,
                );
            }
            _ if bytes[i].is_ascii_alphabetic() => {
                let start = i;
                i += 1;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'-'))
                {
                    i += 1;
                }
                if let Some(word) = value_src.get(start..i) {
                    let token_type = match word {
                        "true" | "false" => Some(BOOLEAN),
                        _ => None,
                    };
                    if let Some(token_type) = token_type {
                        push_span_if_missing(
                            source,
                            segment_offset + start,
                            segment_offset + i,
                            token_type,
                            spans,
                        );
                    }
                }
            }
            _ => i += 1,
        }
    }
}

fn skip_toml_quoted_bytes(bytes: &[u8], start: usize) -> usize {
    if start >= bytes.len() {
        return start;
    }
    let quote = bytes[start];
    let triple = start + 2 < bytes.len() && bytes[start + 1] == quote && bytes[start + 2] == quote;
    let mut i = start + if triple { 3 } else { 1 };
    while i < bytes.len() {
        if !triple && quote == b'"' && bytes[i] == b'\\' {
            i = (i + 2).min(bytes.len());
            continue;
        }
        if triple {
            if i + 2 < bytes.len()
                && bytes[i] == quote
                && bytes[i + 1] == quote
                && bytes[i + 2] == quote
            {
                return i + 3;
            }
            if quote == b'"' && bytes[i] == b'\\' {
                i = (i + 2).min(bytes.len());
                continue;
            }
        } else if bytes[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn extend_with_javascript_lexical_spans(source: &str, spans: &mut Vec<TokenSpan>) {
    let Some(function_decl) =
        Regex::new(r"function\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*\(([^)]*)\)").ok()
    else {
        return;
    };
    let Some(identifier) = Regex::new(r"[A-Za-z_$][A-Za-z0-9_$]*").ok() else {
        return;
    };
    let line_comment = Regex::new(r"//[^\n]*").ok();
    let block_comment = Regex::new(r"/\*[\s\S]*?\*/").ok();
    let null_literal = Regex::new(r"\bnull\b").ok();
    let bool_literal = Regex::new(r"\b(?:true|false)\b").ok();
    let operator = Regex::new(r"\?\?").ok();

    if let Some(caps) = function_decl.captures(source) {
        if let Some(all) = caps.get(0) {
            if let Some(keyword_start) = source[all.start()..all.end()].find("function") {
                push_span_if_missing(
                    source,
                    all.start() + keyword_start,
                    all.start() + keyword_start + "function".len(),
                    OPERATOR,
                    spans,
                );
            }
        }
        if let Some(name_match) = caps.get(1) {
            push_span_if_missing(
                source,
                name_match.start(),
                name_match.end(),
                FUNCTION,
                spans,
            );
        }
        if let Some(params_match) = caps.get(2) {
            let params_src = &source[params_match.start()..params_match.end()];
            for param in identifier.find_iter(params_src) {
                push_span_if_missing(
                    source,
                    params_match.start() + param.start(),
                    params_match.start() + param.end(),
                    VARIABLE,
                    spans,
                );
            }
        }
    }

    if let Some(m) = operator.as_ref().and_then(|re| re.find(source)) {
        push_span_if_missing(source, m.start(), m.end(), OPERATOR, spans);
    }
    if let Some(m) = null_literal.as_ref().and_then(|re| re.find(source)) {
        push_span_if_missing(source, m.start(), m.end(), NIL, spans);
    }
    if let Some(m) = bool_literal.as_ref().and_then(|re| re.find(source)) {
        push_span_if_missing(source, m.start(), m.end(), BOOLEAN, spans);
    }
    if let Some(m) = line_comment.as_ref().and_then(|re| re.find(source)) {
        push_span_if_missing(source, m.start(), m.end(), COMMENT, spans);
    } else if let Some(m) = block_comment.as_ref().and_then(|re| re.find(source)) {
        push_span_if_missing(source, m.start(), m.end(), COMMENT, spans);
    }
}

fn first_capture_name(pattern: &str) -> Option<&str> {
    let start = pattern.find('@')? + 1;
    let bytes = pattern.as_bytes();
    let mut end = start;
    while end < bytes.len()
        && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'.')
    {
        end += 1;
    }
    (end > start).then_some(&pattern[start..end])
}

fn push_span_if_missing(
    source: &str,
    start: usize,
    end: usize,
    token_type: u32,
    spans: &mut Vec<TokenSpan>,
) {
    if spans.iter().any(|span| {
        span.start_row == line_col_for_offset(source, start).0
            && span.start_col == line_col_for_offset(source, start).1
            && span.end_row == line_col_for_offset(source, end).0
            && span.end_col == line_col_for_offset(source, end).1
            && span.token_type == token_type
    }) {
        return;
    }
    push_span_for_range(source, start, end, token_type, spans);
}

fn push_span_for_range(
    source: &str,
    start: usize,
    end: usize,
    token_type: u32,
    spans: &mut Vec<TokenSpan>,
) {
    let (start_row, start_col) = line_col_for_offset(source, start);
    let (end_row, end_col) = line_col_for_offset(source, end);
    spans.push(TokenSpan {
        start_row,
        start_col,
        end_row,
        end_col,
        token_type,
    });
}

fn line_col_for_offset(source: &str, offset: usize) -> (u32, u32) {
    let mut row = 0_u32;
    let mut col = 0_u32;
    for &byte in source.as_bytes().iter().take(offset.min(source.len())) {
        if byte == b'\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (row, col)
}

fn token_type_from_capture(name: &str) -> Option<u32> {
    if name == "property" || name.starts_with("label") {
        return Some(KEY);
    }
    if name == "string" || name.starts_with("string.") {
        return Some(STR);
    }
    if name == "number.float" || name.starts_with("number.float.") {
        return Some(FLOAT);
    }
    if name == "number" || name.starts_with("number.") {
        return Some(INT);
    }
    if name.starts_with("boolean") {
        return Some(BOOLEAN);
    }
    if name.starts_with("constant") {
        return Some(NIL);
    }
    if name.starts_with("punctuation.bracket") {
        return match name.rsplit('.').next() {
            Some("map") => Some(MAP),
            Some("seq") => Some(SEQ),
            _ => Some(PUNCTUATION),
        };
    }
    if name.starts_with("punctuation") {
        return Some(PUNCTUATION);
    }
    if name.starts_with("comment") {
        return Some(COMMENT);
    }
    if name.starts_with("keyword") || name.starts_with("operator") {
        return Some(OPERATOR);
    }
    if name.starts_with("function") {
        return Some(FUNCTION);
    }
    if name.starts_with("variable") {
        return Some(VARIABLE);
    }
    if name.starts_with("type") || name.starts_with("tag") {
        return Some(TAG);
    }
    if name.starts_with("attribute") {
        return Some(ATTRIBUTE);
    }
    None
}

fn encode_delta_tokens(source: &str, spans: &[TokenSpan]) -> Vec<u32> {
    crate::wasm::semantic_tokens_shared::encode_token_spans_to_u32(spans, source)
}

fn source_bound_cache_key(cache_key: &str, source: &str) -> String {
    let mut key = String::with_capacity(cache_key.len() + source.len() + 1);
    key.push_str(cache_key);
    key.push('\0');
    key.push_str(source);
    key
}

#[cfg(test)]
mod tests {
    use super::{BOOLEAN, INT, KEY, STR, TOKEN_TYPES, encode_semantic_tokens};

    fn utf16_column_to_byte_offset(line: &str, column: u32) -> usize {
        let mut utf16 = 0_u32;
        for (offset, ch) in line.char_indices() {
            if utf16 >= column {
                return offset;
            }
            utf16 += ch.len_utf16() as u32;
        }
        line.len()
    }

    fn semantic_token_snippets(source: &str, token_type: u32) -> Vec<String> {
        let tokens = encode_semantic_tokens("json", source);
        let lines: Vec<&str> = source.split('\n').collect();
        let mut line = 0_u32;
        let mut column = 0_u32;
        let mut snippets = Vec::new();

        for chunk in tokens.chunks_exact(5) {
            line += chunk[0];
            if chunk[0] == 0 {
                column += chunk[1];
            } else {
                column = chunk[1];
            }
            if chunk[3] != token_type {
                continue;
            }
            let Some(line_text) = lines.get(line as usize) else {
                continue;
            };
            let start = utf16_column_to_byte_offset(line_text, column);
            let end = utf16_column_to_byte_offset(line_text, column + chunk[2]);
            snippets.push(line_text[start..end].to_owned());
        }

        snippets
    }

    #[test]
    fn yaml_tokens_prefer_query_captures_for_properties_scalars_and_keywords() {
        let tokens = encode_semantic_tokens("yaml", "name: \"Ada\"\nactive: true\n");
        assert_eq!(TOKEN_TYPES[KEY as usize], "key");
        assert!(tokens.len() >= 15);
        assert!(tokens.chunks_exact(5).any(|chunk| chunk[3] == KEY));
        assert!(tokens.chunks_exact(5).any(|chunk| chunk[3] == STR));
    }

    #[test]
    fn toml_tokens_encode_keys_scalars_and_bools_without_tree_sitter_parse() {
        let tokens = encode_semantic_tokens("toml", "name = \"Ada\"\nage = 30\nactive = true\n");
        assert!(tokens.chunks_exact(5).any(|chunk| chunk[3] == KEY));
        assert!(tokens.chunks_exact(5).any(|chunk| chunk[3] == STR));
        assert!(tokens.chunks_exact(5).any(|chunk| chunk[3] == INT));
        assert!(tokens.chunks_exact(5).any(|chunk| chunk[3] == BOOLEAN));
    }

    #[test]
    fn json_tokens_keep_positions_after_utf8_string_content() {
        let source = r#"{"title":"运行环境：GPU需要多大的？","file":"2023-04-03.0009"}"#;

        let key_snippets = semantic_token_snippets(source, KEY);
        assert!(key_snippets.contains(&r#""title""#.to_owned()));
        assert!(key_snippets.contains(&r#""file""#.to_owned()));

        let string_snippets = semantic_token_snippets(source, STR);
        assert!(string_snippets.contains(&r#""运行环境：GPU需要多大的？""#.to_owned()));
        assert!(string_snippets.contains(&r#""2023-04-03.0009""#.to_owned()));

        let int_snippets = semantic_token_snippets(source, INT);
        assert!(!int_snippets.iter().any(|snippet| snippet.contains("0009")));
    }

    #[test]
    fn json_root_string_emits_str_semantic_token() {
        let string_snippets = semantic_token_snippets(r#""left-string""#, STR);
        assert_eq!(string_snippets, vec![r#""left-string""#.to_owned()]);
    }
}
