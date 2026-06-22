use crate::core::tree_sitter_support::tree_sitter_language;
use crate::core::{CoreError, ParseError, SemType, TreeNode, TreeNodeKind, TreeStore};

use super::formats_helpers::{set_node_range, ts_parse_checked_fail_fast};
use super::{Decode, DecodedDocument};

// ---------------------------------------------------------------------------
// DocRange – a half-open [start, end) byte range for a top-level expression
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct DocRange {
    start: usize,
    end: usize,
}

// ---------------------------------------------------------------------------
// split_top_level_python_expressions
// ---------------------------------------------------------------------------

/// Returns true if `c` is an ASCII whitespace byte (including newline).
fn is_whitespace(c: u8) -> bool {
    c.is_ascii_whitespace()
}

/// Skip a Python line comment starting at `i` (where `source[i] == b'#'`).
/// Returns the index after the comment (pointing at `\n` or `source.len()`).
fn skip_python_line_comment(source: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < source.len() && source[j] != b'\n' {
        j += 1;
    }
    j
}

/// Split `source` into top-level Python expression ranges without performing
/// a full semantic parse.  Rules (sufficient for dict/list/tuple and common
/// scalars):
///
/// - Whitespace and `# ...` line comments are skipped.
/// - At brace/bracket/paren depth 0, `\n` or `;` act as expression separators.
/// - Single/double-quoted strings and simple triple-quoted strings are
///   recognised; backslash escapes are skipped.
fn split_top_level_python_expressions(source: &[u8]) -> Result<Vec<DocRange>, CoreError> {
    let mut ranges: Vec<DocRange> = Vec::new();
    let mut i: usize = 0;

    while i < source.len() {
        // Skip whitespace and comments.
        loop {
            if i >= source.len() {
                break;
            }
            if is_whitespace(source[i]) {
                i += 1;
                continue;
            }
            if source[i] == b'#' {
                i = skip_python_line_comment(source, i);
                continue;
            }
            break;
        }
        if i >= source.len() {
            break;
        }

        let start = i;

        let mut brace_depth: usize = 0;
        let mut bracket_depth: usize = 0;
        let mut paren_depth: usize = 0;

        let mut in_string = false;
        let mut quote: u8 = 0;
        let mut triple = false;

        // did_break tracks whether the inner loop exited via `break`
        let mut did_break = false;

        while i < source.len() {
            let c = source[i];

            if in_string {
                if c == b'\\' {
                    if i + 1 < source.len() {
                        i += 1;
                    }
                    i += 1;
                    continue;
                }
                if triple {
                    if i + 2 < source.len()
                        && source[i] == quote
                        && source[i + 1] == quote
                        && source[i + 2] == quote
                    {
                        i += 2;
                        in_string = false;
                        triple = false;
                        i += 1;
                        continue;
                    }
                    i += 1;
                    continue;
                }
                if c == quote {
                    in_string = false;
                    i += 1;
                    continue;
                }
                i += 1;
                continue;
            }

            if c == b'#' {
                i = skip_python_line_comment(source, i);
                if i >= source.len() {
                    break;
                }
                // After skip_python_line_comment, `i` points at `\n`.
                // `continue` skips the `i += 1` at the bottom so we
                // re-examine this `\n` in the next iteration.
                continue;
            }

            if c == b'"' || c == b'\'' {
                quote = c;
                triple = i + 2 < source.len() && source[i + 1] == c && source[i + 2] == c;
                in_string = true;
                if triple {
                    i += 2;
                }
                i += 1;
                continue;
            }

            match c {
                b'{' => brace_depth += 1,
                b'}' => {
                    if brace_depth == 0 {
                        return Err(CoreError::Parse(ParseError::InvalidPython));
                    }
                    brace_depth -= 1;
                }
                b'[' => bracket_depth += 1,
                b']' => {
                    if bracket_depth == 0 {
                        return Err(CoreError::Parse(ParseError::InvalidPython));
                    }
                    bracket_depth -= 1;
                }
                b'(' => paren_depth += 1,
                b')' => {
                    if paren_depth == 0 {
                        return Err(CoreError::Parse(ParseError::InvalidPython));
                    }
                    paren_depth -= 1;
                }
                b'\n' | b';' => {
                    if brace_depth == 0 && bracket_depth == 0 && paren_depth == 0 {
                        let mut end = i;
                        while end > start && is_whitespace(source[end - 1]) {
                            end -= 1;
                        }
                        if end > start {
                            ranges.push(DocRange { start, end });
                        }
                        i += 1;
                        did_break = true;
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        if !did_break {
            let mut end = source.len();
            while end > start && is_whitespace(source[end - 1]) {
                end -= 1;
            }
            if end > start {
                ranges.push(DocRange { start, end });
            }
        }

        if brace_depth != 0 || bracket_depth != 0 || paren_depth != 0 || in_string {
            return Err(CoreError::Parse(ParseError::InvalidPython));
        }
    }

    Ok(ranges)
}

// ---------------------------------------------------------------------------
// py_unquote – Python string literal unquoting
// ---------------------------------------------------------------------------

/// Parse a hex string of exactly `N` digits into a `u32`.
fn parse_hex_digits(s: &[u8]) -> Option<u32> {
    let mut v: u32 = 0;
    for &c in s {
        let n = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return None,
        };
        v = (v << 4) | n as u32;
    }
    Some(v)
}

fn append_utf8_char(out: &mut Vec<u8>, ch: char) {
    let mut buf = [0; 4];
    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
}

/// Unquote a Python string literal, handling escape sequences.
///
/// Supports prefixes (r/R/u/U/b/B/f/F), single/double quotes, triple-quoted
/// strings, and the following escape sequences:
///   `\\`, `\"`, `\'`, `\n`, `\r`, `\t`, `\b`, `\f`, `\a`, `\v`
///   `\xNN`, `\uNNNN`, `\UNNNNNNNN`
///   `\0`–`\777` (octal, up to 3 digits)
///
/// Unknown escape sequences pass through the character after the backslash.
fn py_unquote(literal: &[u8]) -> Result<String, CoreError> {
    let mut s = literal;

    // Strip prefix characters.
    while !s.is_empty() {
        match s[0] {
            b'r' | b'R' => s = &s[1..],
            b'u' | b'U' | b'b' | b'B' | b'f' | b'F' => {
                s = &s[1..];
            }
            _ => break,
        }
    }

    if s.len() < 2 {
        return Err(CoreError::Parse(ParseError::InvalidPython));
    }

    let q = s[0];
    if q != b'"' && q != b'\'' {
        return Err(CoreError::Parse(ParseError::InvalidPython));
    }

    let triple = s.len() >= 6
        && s[1] == q
        && s[2] == q
        && s[s.len() - 1] == q
        && s[s.len() - 2] == q
        && s[s.len() - 3] == q;

    let start: usize = if triple { 3 } else { 1 };
    let end: usize = if triple { s.len() - 3 } else { s.len() - 1 };

    if end < start {
        return Err(CoreError::Parse(ParseError::InvalidPython));
    }

    let mut out = Vec::with_capacity(end - start);
    let mut i = start;

    while i < end {
        let c = s[i];
        if c != b'\\' {
            let text = std::str::from_utf8(&s[i..end])
                .map_err(|_| CoreError::Parse(ParseError::InvalidPython))?;
            let ch = text
                .chars()
                .next()
                .ok_or(CoreError::Parse(ParseError::InvalidPython))?;
            append_utf8_char(&mut out, ch);
            i += ch.len_utf8();
            continue;
        }
        if i + 1 >= end {
            break;
        }
        let esc = s[i + 1];
        match esc {
            b'\\' => {
                out.push(b'\\');
                i += 2;
            }
            b'"' => {
                out.push(b'"');
                i += 2;
            }
            b'\'' => {
                out.push(b'\'');
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
            b'b' => {
                out.push(0x08);
                i += 2;
            }
            b'f' => {
                out.push(0x0c);
                i += 2;
            }
            b'a' => {
                out.push(0x07);
                i += 2;
            }
            b'v' => {
                out.push(0x0b);
                i += 2;
            }
            b'x' => {
                if i + 3 >= end {
                    return Err(CoreError::Parse(ParseError::InvalidPython));
                }
                let v = parse_hex_digits(&s[i + 2..i + 4])
                    .ok_or(CoreError::Parse(ParseError::InvalidPython))?;
                out.push(v as u8);
                i += 4;
            }
            b'u' => {
                if i + 5 >= end {
                    return Err(CoreError::Parse(ParseError::InvalidPython));
                }
                let v = parse_hex_digits(&s[i + 2..i + 6])
                    .ok_or(CoreError::Parse(ParseError::InvalidPython))?;
                if let Some(ch) = char::from_u32(v) {
                    append_utf8_char(&mut out, ch);
                } else {
                    append_utf8_char(&mut out, char::REPLACEMENT_CHARACTER);
                }
                i += 6;
            }
            b'U' => {
                if i + 9 >= end {
                    return Err(CoreError::Parse(ParseError::InvalidPython));
                }
                let v = parse_hex_digits(&s[i + 2..i + 10])
                    .ok_or(CoreError::Parse(ParseError::InvalidPython))?;
                if let Some(ch) = char::from_u32(v) {
                    append_utf8_char(&mut out, ch);
                } else {
                    append_utf8_char(&mut out, char::REPLACEMENT_CHARACTER);
                }
                i += 10;
            }
            b'0'..=b'7' => {
                let mut j = i + 1;
                let mut count: usize = 0;
                let mut v: u16 = 0;
                while j < end && count < 3 {
                    let d = s[j];
                    if d < b'0' || d > b'7' {
                        break;
                    }
                    v = (v << 3) | (d - b'0') as u16;
                    count += 1;
                    j += 1;
                }
                out.push((v & 0xFF) as u8);
                i = j;
            }
            _ => {
                // Unknown escape – pass through the escaped character.
                out.push(esc);
                i += 2;
            }
        }
    }

    String::from_utf8(out).map_err(|_| CoreError::Parse(ParseError::InvalidPython))
}

// ---------------------------------------------------------------------------
// PyTsNodeKind – tree-sitter node type dispatch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PyTsNodeKind {
    Dictionary,
    List,
    Tuple,
    String,
    Integer,
    Float,
    TrueLit,
    FalseLit,
    NoneLit,
}

fn py_ts_node_kind(node_type: &str) -> Option<PyTsNodeKind> {
    match node_type {
        "dictionary" => Some(PyTsNodeKind::Dictionary),
        "list" => Some(PyTsNodeKind::List),
        "tuple" => Some(PyTsNodeKind::Tuple),
        "string" => Some(PyTsNodeKind::String),
        "integer" => Some(PyTsNodeKind::Integer),
        "float" => Some(PyTsNodeKind::Float),
        "true" => Some(PyTsNodeKind::TrueLit),
        "false" => Some(PyTsNodeKind::FalseLit),
        "none" => Some(PyTsNodeKind::NoneLit),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// build_candidate_from_ts_node – recursive tree-sitter node to TreeNode
// ---------------------------------------------------------------------------

/// Recursively convert a tree-sitter Python node into a `TreeNode` subtree
/// rooted at a new `NodeId` in `store`.
///
/// `base_offset` is added to every node's byte range so that ranges are
/// relative to the original multi-document source, not the individual slice.
fn build_candidate_from_ts_node(
    store: &mut TreeStore,
    source: &[u8],
    node: tree_sitter::Node,
    base_offset: usize,
) -> Result<crate::core::NodeId, CoreError> {
    let ty = node.kind();
    let kind = py_ts_node_kind(ty).ok_or(CoreError::Parse(ParseError::InvalidPython))?;

    match kind {
        PyTsNodeKind::Dictionary => {
            let mut map_node = TreeNode {
                kind: TreeNodeKind::Mapping,
                sem_type: Some(SemType::Map),
                tag: SemType::Map.to_string(),
                ..TreeNode::default()
            };
            set_node_range(&mut map_node, base_offset, node);
            let map_id = store.add(map_node);

            let n = node.named_child_count();
            for i in 0..n {
                let Some(pair) = node.named_child(i as _) else {
                    continue;
                };
                if pair.kind() != "pair" {
                    continue;
                }
                let pn = pair.named_child_count();
                if pn < 2 {
                    return Err(CoreError::Parse(ParseError::InvalidPython));
                }
                let Some(key_ts) = pair.named_child(0) else {
                    return Err(CoreError::Parse(ParseError::InvalidPython));
                };
                let Some(val_ts) = pair.named_child(1) else {
                    return Err(CoreError::Parse(ParseError::InvalidPython));
                };

                // Build the key node.
                let mut key_node = TreeNode {
                    kind: TreeNodeKind::Scalar,
                    is_map_key: true,
                    parent: Some(map_id),
                    ..TreeNode::default()
                };
                set_node_range(&mut key_node, base_offset, key_ts);

                let key_kind = py_ts_node_kind(key_ts.kind())
                    .ok_or(CoreError::Parse(ParseError::InvalidPython))?;
                match key_kind {
                    PyTsNodeKind::String => {
                        let raw = node_text(source, key_ts);
                        let unq = py_unquote(raw.as_bytes())?;
                        key_node.set_sem_type(SemType::Str);
                        key_node.value = unq;
                    }
                    PyTsNodeKind::Integer => {
                        key_node.set_sem_type(SemType::Int);
                        key_node.value = node_text(source, key_ts).to_string();
                    }
                    PyTsNodeKind::Float => {
                        key_node.set_sem_type(SemType::Float);
                        key_node.value = node_text(source, key_ts).to_string();
                    }
                    PyTsNodeKind::TrueLit => {
                        key_node.set_sem_type(SemType::Boolean);
                        key_node.value = "true".to_string();
                    }
                    PyTsNodeKind::FalseLit => {
                        key_node.set_sem_type(SemType::Boolean);
                        key_node.value = "false".to_string();
                    }
                    PyTsNodeKind::NoneLit => {
                        key_node.set_sem_type(SemType::Nil);
                        key_node.value = "null".to_string();
                    }
                    _ => return Err(CoreError::Parse(ParseError::InvalidPython)),
                }

                let key_id = store.add(key_node);

                // Build the value node.
                let val_id = build_candidate_from_ts_node(store, source, val_ts, base_offset)?;

                // Set back-links.
                if let Some(v) = store.get_mut(val_id) {
                    v.key = Some(key_id);
                    v.parent = Some(map_id);
                }

                // Push key+value into map content.
                if let Some(map) = store.get_mut(map_id) {
                    map.content.push(key_id);
                    map.content.push(val_id);
                }
            }

            Ok(map_id)
        }
        PyTsNodeKind::List | PyTsNodeKind::Tuple => {
            let mut seq_node = TreeNode {
                kind: TreeNodeKind::Sequence,
                sem_type: Some(SemType::Seq),
                tag: SemType::Seq.to_string(),
                ..TreeNode::default()
            };
            set_node_range(&mut seq_node, base_offset, node);
            let seq_id = store.add(seq_node);

            let n = node.named_child_count();
            for i in 0..n {
                let Some(item_ts) = node.named_child(i as _) else {
                    continue;
                };
                let child_id = build_candidate_from_ts_node(store, source, item_ts, base_offset)?;

                let key_id = store.add(TreeNode {
                    kind: TreeNodeKind::Scalar,
                    sem_type: Some(SemType::Int),
                    tag: SemType::Int.to_string(),
                    value: i.to_string(),
                    parent: Some(seq_id),
                    is_map_key: true,
                    ..TreeNode::default()
                });

                // Set parent and sequence_index on the child.
                if let Some(child) = store.get_mut(child_id) {
                    child.parent = Some(seq_id);
                    child.key = Some(key_id);
                    child.sequence_index = Some(i as i64);
                }

                // Push into sequence content.
                if let Some(seq) = store.get_mut(seq_id) {
                    seq.content.push(child_id);
                }
            }

            Ok(seq_id)
        }
        PyTsNodeKind::String => {
            let raw = node_text(source, node);
            let unq = py_unquote(raw.as_bytes())?;
            let mut scalar = TreeNode {
                kind: TreeNodeKind::Scalar,
                sem_type: Some(SemType::Str),
                tag: SemType::Str.to_string(),
                value: unq,
                ..TreeNode::default()
            };
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
        PyTsNodeKind::Integer => {
            let mut scalar = TreeNode {
                kind: TreeNodeKind::Scalar,
                sem_type: Some(SemType::Int),
                tag: SemType::Int.to_string(),
                value: node_text(source, node).to_string(),
                ..TreeNode::default()
            };
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
        PyTsNodeKind::Float => {
            let mut scalar = TreeNode {
                kind: TreeNodeKind::Scalar,
                sem_type: Some(SemType::Float),
                tag: SemType::Float.to_string(),
                value: node_text(source, node).to_string(),
                ..TreeNode::default()
            };
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
        PyTsNodeKind::TrueLit => {
            let mut scalar = TreeNode {
                kind: TreeNodeKind::Scalar,
                sem_type: Some(SemType::Boolean),
                tag: SemType::Boolean.to_string(),
                value: "true".to_string(),
                ..TreeNode::default()
            };
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
        PyTsNodeKind::FalseLit => {
            let mut scalar = TreeNode {
                kind: TreeNodeKind::Scalar,
                sem_type: Some(SemType::Boolean),
                tag: SemType::Boolean.to_string(),
                value: "false".to_string(),
                ..TreeNode::default()
            };
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
        PyTsNodeKind::NoneLit => {
            let mut scalar = TreeNode {
                kind: TreeNodeKind::Scalar,
                sem_type: Some(SemType::Nil),
                tag: SemType::Nil.to_string(),
                value: "null".to_string(),
                ..TreeNode::default()
            };
            set_node_range(&mut scalar, base_offset, node);
            Ok(store.add(scalar))
        }
    }
}

/// Extract the source text covered by a tree-sitter node.
fn node_text<'a>(source: &'a [u8], node: tree_sitter::Node) -> &'a str {
    let bytes = &source[node.start_byte()..node.end_byte()];
    std::str::from_utf8(bytes).unwrap_or("")
}

fn unwrap_python_value_node(mut node: tree_sitter::Node) -> tree_sitter::Node {
    loop {
        let next = match node.kind() {
            "module" | "source_file" | "expression_statement" | "parenthesized_expression" => {
                node.named_child(0)
            }
            _ => None,
        };
        match next {
            Some(child) => node = child,
            None => return node,
        }
    }
}

// ---------------------------------------------------------------------------
// PythonDecoder – the top-level decoder
// ---------------------------------------------------------------------------

/// A streaming Python object-literal decoder backed by tree-sitter.
///
/// Unlike the old normalize-to-JSON path, this decoder:
/// - Splits multi-document input via `split_top_level_python_expressions`
/// - Validates each expression with `ts_parse_checked_fail_fast`
/// - Recursively builds `TreeNode` subtrees with byte-range metadata
/// - Handles the full Python literal syntax including `\a`/`\v` escapes,
///   triple-quoted strings, octal escapes, and all scalar types
#[derive(Debug, Clone, Default)]
pub struct PythonDecoder;

impl Decode for PythonDecoder {
    fn decode_str(&self, input: &str) -> Result<DecodedDocument, CoreError> {
        let source = input.as_bytes();

        if source.is_empty() {
            return Err(CoreError::Parse(ParseError::InvalidPython));
        }

        let ranges = split_top_level_python_expressions(source)?;
        if ranges.is_empty() {
            return Err(CoreError::Parse(ParseError::InvalidPython));
        }

        // Decode the first top-level expression.
        let r = ranges[0];
        let slice = &source[r.start..r.end];

        let language =
            tree_sitter_language("python").ok_or(CoreError::Parse(ParseError::InvalidPython))?;

        let tree = ts_parse_checked_fail_fast(language, slice)?;
        let root = tree.root_node();

        let value_node = unwrap_python_value_node(root);

        if value_node.has_error() {
            return Err(CoreError::Parse(ParseError::InvalidPython));
        }

        let mut store = TreeStore::new();
        let root_id = build_candidate_from_ts_node(&mut store, slice, value_node, r.start)?;

        Ok(DecodedDocument::new(store, root_id))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- split_top_level_python_expressions ---------------------------------

    #[test]
    fn split_single_dict() {
        let ranges = split_top_level_python_expressions(b"{'a': 1}").unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(&b"{'a': 1}"[ranges[0].start..ranges[0].end], b"{'a': 1}");
    }

    #[test]
    fn split_two_dicts_newline() {
        let ranges = split_top_level_python_expressions(b"{'a': 1}\n{'b': 2}").unwrap();
        assert_eq!(ranges.len(), 2);
        assert_eq!(
            &b"{'a': 1}\n{'b': 2}"[ranges[0].start..ranges[0].end],
            b"{'a': 1}"
        );
        assert_eq!(
            &b"{'a': 1}\n{'b': 2}"[ranges[1].start..ranges[1].end],
            b"{'b': 2}"
        );
    }

    #[test]
    fn split_two_dicts_semicolon() {
        let ranges = split_top_level_python_expressions(b"{'a': 1};{'b': 2}").unwrap();
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn split_skips_comments() {
        let ranges =
            split_top_level_python_expressions(b"# comment\n{'a': 1}\n# another\n{'b': 2}")
                .unwrap();
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn split_nested_braces_not_split() {
        let ranges = split_top_level_python_expressions(b"{'a': {'nested': 1}}\n{'b': 2}").unwrap();
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn split_string_with_braces() {
        let ranges = split_top_level_python_expressions(b"{'a': '{\"x\": 1}'}\n[1, 2]").unwrap();
        assert_eq!(ranges.len(), 2);
    }

    // -- py_unquote ---------------------------------------------------------

    #[test]
    fn unquote_simple_single() {
        assert_eq!(py_unquote(b"'hello'").unwrap(), "hello");
    }

    #[test]
    fn unquote_simple_double() {
        assert_eq!(py_unquote(b"\"hello\"").unwrap(), "hello");
    }

    #[test]
    fn unquote_with_escapes() {
        assert_eq!(py_unquote(b"'line\\nfeed'").unwrap(), "line\nfeed");
        assert_eq!(py_unquote(b"'tab\\there'").unwrap(), "tab\there");
        assert_eq!(py_unquote(b"'backslash\\\\'").unwrap(), "backslash\\");
    }

    #[test]
    fn unquote_bell_and_vertical_tab() {
        assert_eq!(py_unquote(b"'\\a'").unwrap(), "\u{07}");
        assert_eq!(py_unquote(b"'\\v'").unwrap(), "\u{0b}");
    }

    #[test]
    fn unquote_hex_escape() {
        assert_eq!(py_unquote(b"'\\x41'").unwrap(), "A");
    }

    #[test]
    fn unquote_unicode_4digit() {
        assert_eq!(py_unquote(b"'\\u0041'").unwrap(), "A");
    }

    #[test]
    fn unquote_unicode_8digit() {
        assert_eq!(py_unquote(b"'\\U00000041'").unwrap(), "A");
    }

    #[test]
    fn unquote_octal() {
        assert_eq!(py_unquote(b"'\\101'").unwrap(), "A"); // octal 101 = 65 = 'A'
    }

    #[test]
    fn unquote_triple_single() {
        assert_eq!(py_unquote(b"'''hello'''").unwrap(), "hello");
    }

    #[test]
    fn unquote_triple_double() {
        assert_eq!(py_unquote(b"\"\"\"hello\"\"\"").unwrap(), "hello");
    }

    #[test]
    fn unquote_with_prefix() {
        assert_eq!(py_unquote(b"r'hello\\nworld'").unwrap(), "hello\nworld");
    }

    #[test]
    fn list_items_get_synthetic_index_keys_like_zig() {
        let decoded = PythonDecoder.decode_str("[1, 2]").unwrap();
        let root = decoded.store.get(decoded.root).unwrap();
        let first = decoded.store.get(root.content[0]).unwrap();
        let key = decoded.store.get(first.key.unwrap()).unwrap();

        assert_eq!(key.value, "0");
        assert_eq!(key.sem_type, Some(SemType::Int));
    }

    // -- py_ts_node_kind ----------------------------------------------------

    #[test]
    fn node_kind_mapping() {
        assert_eq!(
            py_ts_node_kind("dictionary"),
            Some(PyTsNodeKind::Dictionary)
        );
        assert_eq!(py_ts_node_kind("list"), Some(PyTsNodeKind::List));
        assert_eq!(py_ts_node_kind("tuple"), Some(PyTsNodeKind::Tuple));
        assert_eq!(py_ts_node_kind("string"), Some(PyTsNodeKind::String));
        assert_eq!(py_ts_node_kind("integer"), Some(PyTsNodeKind::Integer));
        assert_eq!(py_ts_node_kind("float"), Some(PyTsNodeKind::Float));
        assert_eq!(py_ts_node_kind("true"), Some(PyTsNodeKind::TrueLit));
        assert_eq!(py_ts_node_kind("false"), Some(PyTsNodeKind::FalseLit));
        assert_eq!(py_ts_node_kind("none"), Some(PyTsNodeKind::NoneLit));
        assert_eq!(py_ts_node_kind("unknown_type"), None);
    }
}
