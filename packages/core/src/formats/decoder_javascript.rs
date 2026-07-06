use crate::core::{CoreError, NodeId, ParseError, SemType, TreeStore, tree_sitter_support};

use super::formats_helpers::{
    self, missing_tree_node_error, new_map, new_scalar, new_seq, set_node_range,
};

fn js_language() -> tree_sitter::Language {
    tree_sitter_support::ensure_tree_sitter_runtime();
    tree_sitter::Language::new(tree_sitter_javascript::LANGUAGE)
}

// ---------------------------------------------------------------------------
// JavaScript string unquoting
// ---------------------------------------------------------------------------

/// Parse a hexadecimal nibble (0-15), returning None for invalid input.
fn parse_hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Append a Unicode codepoint as UTF-8 to the output buffer.
fn append_utf8_codepoint(out: &mut Vec<u8>, cp: u32) {
    if let Some(ch) = char::from_u32(cp) {
        append_utf8_char(out, ch);
    } else {
        append_utf8_char(out, char::REPLACEMENT_CHARACTER);
    }
}

fn append_utf8_char(out: &mut Vec<u8>, ch: char) {
    let mut buf = [0; 4];
    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
}

/// Unquote a JavaScript string literal, handling common escape sequences.
///
/// Supports single-quoted, double-quoted, and template literal (backtick)
/// strings.  Template literals are returned as-is without interpolation.
fn js_unquote(literal: &str) -> Result<String, CoreError> {
    let bytes = literal.as_bytes();
    if bytes.len() < 2 {
        return Err(CoreError::Parse(ParseError::InvalidJavaScript));
    }

    // Template literal: strip backticks, return content as-is.
    if bytes[0] == b'`' && bytes[bytes.len() - 1] == b'`' {
        return Ok(literal[1..literal.len() - 1].to_string());
    }

    // Single-quoted strings must not contain raw newlines.
    if bytes[0] == b'\'' && bytes.len() >= 2 && bytes[bytes.len() - 1] == b'\'' {
        if literal[1..literal.len() - 1].contains('\n')
            || literal[1..literal.len() - 1].contains('\r')
        {
            return Err(CoreError::Parse(ParseError::InvalidJavaScript));
        }
    }

    let q = bytes[0];
    if (q != b'"' && q != b'\'') || bytes[bytes.len() - 1] != q {
        return Err(CoreError::Parse(ParseError::InvalidJavaScript));
    }

    let inner = &bytes[1..bytes.len() - 1];
    let mut out = Vec::with_capacity(inner.len());
    let mut i = 0;

    while i < inner.len() {
        let c = inner[i];
        if c != b'\\' {
            let text = std::str::from_utf8(&inner[i..])
                .map_err(|_| CoreError::Parse(ParseError::InvalidJavaScript))?;
            let ch = text
                .chars()
                .next()
                .ok_or(CoreError::Parse(ParseError::InvalidJavaScript))?;
            append_utf8_char(&mut out, ch);
            i += ch.len_utf8();
            continue;
        }
        if i + 1 >= inner.len() {
            return Err(CoreError::Parse(ParseError::InvalidJavaScript));
        }
        let esc = inner[i + 1];
        match esc {
            b'\\' | b'"' | b'\'' | b'/' => {
                out.push(esc);
                i += 2;
            }
            b'b' => {
                out.push(0x08);
                i += 2;
            }
            b'f' => {
                out.push(0x0c);
                i += 2;
            }
            b'n' => {
                out.push(b'\n');
                i += 2;
            }
            b'r' => {
                out.push(b'\r');
                i += 2;
            }
            b't' => {
                out.push(b'\t');
                i += 2;
            }
            b'x' => {
                if i + 3 >= inner.len() {
                    return Err(CoreError::Parse(ParseError::InvalidJavaScript));
                }
                let hi = parse_hex_nibble(inner[i + 2])
                    .ok_or(CoreError::Parse(ParseError::InvalidJavaScript))?;
                let lo = parse_hex_nibble(inner[i + 3])
                    .ok_or(CoreError::Parse(ParseError::InvalidJavaScript))?;
                out.push((hi << 4) | lo);
                i += 4;
            }
            b'u' => {
                if i + 2 >= inner.len() {
                    return Err(CoreError::Parse(ParseError::InvalidJavaScript));
                }
                if inner[i + 2] == b'{' {
                    // Braced Unicode escape: \u{...}
                    let mut j = i + 3;
                    while j < inner.len() && inner[j] != b'}' {
                        j += 1;
                    }
                    if j >= inner.len() {
                        return Err(CoreError::Parse(ParseError::InvalidJavaScript));
                    }
                    let hex_str = std::str::from_utf8(&inner[i + 3..j])
                        .map_err(|_| CoreError::Parse(ParseError::InvalidJavaScript))?;
                    if hex_str.is_empty() || !hex_str.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err(CoreError::Parse(ParseError::InvalidJavaScript));
                    }
                    let v = u32::from_str_radix(hex_str, 16)
                        .map_err(|_| CoreError::Parse(ParseError::InvalidJavaScript))?;
                    append_utf8_codepoint(&mut out, v);
                    i = j + 1;
                } else {
                    // Fixed-length Unicode escape: \uXXXX
                    if i + 5 >= inner.len() {
                        return Err(CoreError::Parse(ParseError::InvalidJavaScript));
                    }
                    let hex_str = std::str::from_utf8(&inner[i + 2..i + 6])
                        .map_err(|_| CoreError::Parse(ParseError::InvalidJavaScript))?;
                    if !hex_str.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err(CoreError::Parse(ParseError::InvalidJavaScript));
                    }
                    let v = u32::from_str_radix(hex_str, 16)
                        .map_err(|_| CoreError::Parse(ParseError::InvalidJavaScript))?;
                    append_utf8_codepoint(&mut out, v);
                    i += 6;
                }
            }
            _ => {
                out.push(esc);
                i += 2;
            }
        }
    }

    String::from_utf8(out).map_err(|_| CoreError::Parse(ParseError::InvalidJavaScript))
}

// ---------------------------------------------------------------------------
// Node-type dispatch
// ---------------------------------------------------------------------------

/// Tree-sitter JavaScript node types that map to treease value nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsTsNodeKind {
    Object,
    Array,
    String,
    TemplateString,
    Number,
    TrueLit,
    FalseLit,
    NullLit,
}

/// Map a tree-sitter node type string to the dispatch enum.
fn js_ts_node_kind(node_type: &str) -> Option<JsTsNodeKind> {
    match node_type {
        "object" => Some(JsTsNodeKind::Object),
        "array" => Some(JsTsNodeKind::Array),
        "string" => Some(JsTsNodeKind::String),
        "template_string" => Some(JsTsNodeKind::TemplateString),
        "number" => Some(JsTsNodeKind::Number),
        "true" => Some(JsTsNodeKind::TrueLit),
        "false" => Some(JsTsNodeKind::FalseLit),
        "null" => Some(JsTsNodeKind::NullLit),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Recursive tree-sitter node -> TreeNode conversion
// ---------------------------------------------------------------------------

/// Recursively convert a tree-sitter JavaScript node into treease TreeNodes
/// stored in `store`.
///
/// Returns the `NodeId` of the newly-created root for this sub-tree.
fn build_candidate_from_ts_node(
    store: &mut TreeStore,
    source: &[u8],
    node: tree_sitter::Node,
    base_offset: usize,
) -> Result<NodeId, CoreError> {
    let ty = node.kind();

    // Unwrap parenthesized_expression.
    if ty == "parenthesized_expression" {
        if node.named_child_count() == 0 {
            return Err(CoreError::Parse(ParseError::InvalidJavaScript));
        }
        return build_candidate_from_ts_node(
            store,
            source,
            node.named_child(0)
                .ok_or(CoreError::Parse(ParseError::InvalidJavaScript))?,
            base_offset,
        );
    }

    let kind = js_ts_node_kind(ty).ok_or(CoreError::Parse(ParseError::InvalidJavaScript))?;

    match kind {
        JsTsNodeKind::Object => {
            let mut map_node = new_map();
            set_node_range(&mut map_node, base_offset, node);
            let map_id = store.add(map_node);

            let n = node.named_child_count();
            for _i in 0..n {
                let pair = node
                    .named_child(_i as _)
                    .ok_or(CoreError::Parse(ParseError::InvalidJavaScript))?;
                if pair.kind() != "pair" {
                    continue;
                }
                if pair.named_child_count() < 2 {
                    return Err(CoreError::Parse(ParseError::InvalidJavaScript));
                }
                let key_ts = pair
                    .named_child(0)
                    .ok_or(CoreError::Parse(ParseError::InvalidJavaScript))?;
                let val_ts = pair
                    .named_child(1)
                    .ok_or(CoreError::Parse(ParseError::InvalidJavaScript))?;

                // Build the key node and add it to the store.
                let key_value = if key_ts.kind() == "string" {
                    let raw = node_text(source, key_ts);
                    js_unquote(&raw)?
                } else {
                    node_text(source, key_ts)
                };

                let mut key_node = new_scalar(SemType::Str, key_value);
                key_node.parent = Some(map_id);
                key_node.is_map_key = true;
                set_node_range(&mut key_node, base_offset, key_ts);
                let key_id = store.add(key_node);

                // Build the value node recursively.
                let val_id = build_candidate_from_ts_node(store, source, val_ts, base_offset)?;

                // Link the value back to its key and parent.
                {
                    let val = store.get_mut(val_id).ok_or_else(missing_tree_node_error)?;
                    val.parent = Some(map_id);
                    val.set_key(Some(key_id));
                }

                let map = store.get_mut(map_id).ok_or_else(missing_tree_node_error)?;
                map.content.push(key_id);
                map.content.push(val_id);
            }

            Ok(map_id)
        }
        JsTsNodeKind::Array => {
            let mut seq_node = new_seq();
            set_node_range(&mut seq_node, base_offset, node);
            let seq_id = store.add(seq_node);

            let n = node.named_child_count();
            for i in 0..n {
                let item_ts = node
                    .named_child(i as _)
                    .ok_or(CoreError::Parse(ParseError::InvalidJavaScript))?;

                let item_id = build_candidate_from_ts_node(store, source, item_ts, base_offset)?;

                let key_value = i.to_string();
                let mut key_node = new_scalar(SemType::Int, key_value);
                key_node.parent = Some(seq_id);
                key_node.is_map_key = true;
                let key_id = store.add(key_node);

                // Link the item to the sequence and set its key back-link.
                {
                    let item = store.get_mut(item_id).ok_or_else(missing_tree_node_error)?;
                    item.parent = Some(seq_id);
                    item.set_key(Some(key_id));
                    item.set_sequence_index(Some(i as u32));
                }

                let seq = store.get_mut(seq_id).ok_or_else(missing_tree_node_error)?;
                seq.content.push(item_id);
            }

            Ok(seq_id)
        }
        JsTsNodeKind::String | JsTsNodeKind::TemplateString => {
            let raw = node_text(source, node);
            let unq = js_unquote(&raw)?;
            let mut scalar = new_scalar(SemType::Str, unq);
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
        JsTsNodeKind::Number => {
            let raw = node_text(source, node);
            let has_float_marker = raw.contains('.') || raw.contains('e') || raw.contains('E');
            let tag = if has_float_marker {
                SemType::Float
            } else {
                SemType::Int
            };
            let mut scalar = new_scalar(tag, raw);
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
        JsTsNodeKind::TrueLit => {
            let mut scalar = new_scalar(SemType::Boolean, "true");
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
        JsTsNodeKind::FalseLit => {
            let mut scalar = new_scalar(SemType::Boolean, "false");
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
        JsTsNodeKind::NullLit => {
            let mut scalar = new_scalar(SemType::Nil, "null");
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
    }
}

/// Extract the source text covered by a tree-sitter node.
fn node_text<'a>(source: &'a [u8], node: tree_sitter::Node) -> String {
    let start = node.start_byte();
    let end = node.end_byte();
    String::from_utf8_lossy(&source[start..end]).into_owned()
}

// ---------------------------------------------------------------------------
// JavascriptObjectDecoder
// ---------------------------------------------------------------------------

/// JavaScript object-literal decoder backed by tree-sitter.
///
/// Unlike the previous normalize-to-JSON fallback, this decoder uses the
/// real `tree-sitter-javascript` grammar to parse the input, then walks the
/// CST to build treease `TreeNode`s directly.  This preserves source
/// positions (`set_node_range`), correctly handles `parenthesized_expression`
/// and `expression_statement` unwrapping, and splits a `program` node into
/// individual top-level expressions (the `Decode` trait returns the first
#[derive(Debug, Clone, Copy, Default)]
pub struct JavascriptObjectDecoder;

impl super::Decode for JavascriptObjectDecoder {
    fn decode_str(&self, input: &str) -> Result<super::DecodedDocument, CoreError> {
        decode_javascript(input)
    }
}

/// Parse `input` as JavaScript and convert it into a `DecodedDocument`.
pub fn decode_javascript(input: &str) -> Result<super::DecodedDocument, CoreError> {
    let source = input.as_bytes();

    if source.is_empty() {
        return Err(CoreError::Parse(ParseError::InvalidJavaScript));
    }

    let syntax_source = tree_sitter_support::tree_sitter_syntax_source("javascript", source);
    let tree = formats_helpers::ts_parse_checked_fail_fast(js_language(), syntax_source.as_ref())
        .map_err(|_| CoreError::Parse(ParseError::InvalidJavaScript))?;

    let root = tree.root_node();
    let root_type = root.kind();

    // Collect top-level expression nodes.
    let top_level_nodes: Vec<tree_sitter::Node> = collect_top_level_nodes(root, root_type)?;

    if top_level_nodes.is_empty() {
        return Err(CoreError::Parse(ParseError::InvalidJavaScript));
    }

    let mut store = TreeStore::new();

    // returning the first top-level expression instead of wrapping all of them.
    let root_id = build_top_level(&mut store, source, top_level_nodes[0])?;
    Ok(super::DecodedDocument::new(store, root_id))
}

/// Collect the top-level expression nodes from the tree-sitter root.
///
/// Handles `program`, `expression_statement`, `object`, and
/// `init` logic.
fn collect_top_level_nodes<'a>(
    root: tree_sitter::Node<'a>,
    root_type: &str,
) -> Result<Vec<tree_sitter::Node<'a>>, CoreError> {
    if root.has_error() {
        return Err(CoreError::Parse(ParseError::InvalidJavaScript));
    }

    match root_type {
        "program" => {
            let n = root.named_child_count();
            if n == 0 {
                return Err(CoreError::Parse(ParseError::InvalidJavaScript));
            }
            // If there's exactly one child and it's an expression_statement
            // wrapping a parenthesized_expression, unwrap through to the
            if n == 1 {
                if let Some(only) = root.named_child(0) {
                    if only.kind() == "expression_statement" && only.named_child_count() == 1 {
                        if let Some(expr) = only.named_child(0) {
                            if expr.kind() == "parenthesized_expression"
                                && expr.named_child_count() == 1
                            {
                                if let Some(inner) = expr.named_child(0) {
                                    return Ok(vec![inner]);
                                }
                            }
                        }
                    }
                }
            }
            // General case: collect all named children of the program.
            let mut nodes = Vec::with_capacity(n);
            for i in 0..n {
                if let Some(child) = root.named_child(i as _) {
                    nodes.push(child);
                }
            }
            Ok(nodes)
        }
        "expression_statement" => {
            if root.named_child_count() == 0 {
                return Err(CoreError::Parse(ParseError::InvalidJavaScript));
            }
            // Unwrap the expression from the statement.
            if let Some(expr) = root.named_child(0) {
                Ok(vec![expr])
            } else {
                Err(CoreError::Parse(ParseError::InvalidJavaScript))
            }
        }
        "object" | "parenthesized_expression" => Ok(vec![root]),
        _ => {
            // For any other root type, collect named children.
            let n = root.named_child_count();
            if n == 0 {
                return Err(CoreError::Parse(ParseError::InvalidJavaScript));
            }
            let mut nodes = Vec::with_capacity(n);
            for i in 0..n {
                if let Some(child) = root.named_child(i as _) {
                    nodes.push(child);
                }
            }
            Ok(nodes)
        }
    }
}

/// Build a TreeNode from a top-level expression node, handling
/// `expression_statement` unwrapping.
fn build_top_level(
    store: &mut TreeStore,
    source: &[u8],
    node: tree_sitter::Node,
) -> Result<NodeId, CoreError> {
    let ty = node.kind();
    if ty == "expression_statement" && node.named_child_count() != 0 {
        let expr = node
            .named_child(0)
            .ok_or(CoreError::Parse(ParseError::InvalidJavaScript))?;
        build_candidate_from_ts_node(store, source, expr, 0)
    } else {
        build_candidate_from_ts_node(store, source, node, 0)
    }
}
