use crate::errors::{CoreError, ParseError};
use crate::formats::DecodedDocument;
use crate::io::CodecService;
use crate::operators::{NodeKind, SemType, TreeNode};
use crate::tree::infer_scalar_tag;

#[derive(Debug, Clone)]
pub struct DocumentWithMeta {
    pub format: String,
    pub source_byte_length: usize,
    pub document: DecodedDocument,
}

pub fn read_documents_with_meta(
    codec: &CodecService,
    format_name: &str,
    sources: &[String],
) -> Result<Vec<DocumentWithMeta>, CoreError> {
    let mut documents = Vec::with_capacity(sources.len());
    for source in sources {
        let document = codec.decode(format_name, source)?;
        documents.push(DocumentWithMeta {
            format: format_name.to_string(),
            source_byte_length: source.len(),
            document,
        });
    }
    Ok(documents)
}

pub fn recursive_node_compare(left: &TreeNode, right: &TreeNode) -> bool {
    if left.kind != right.kind {
        return false;
    }

    if left.kind == NodeKind::Scalar {
        let left_tag = infer_scalar_tag(&left.tag, &left.value);
        let right_tag = infer_scalar_tag(&right.tag, &right.value);
        if left_tag != right_tag {
            return false;
        }
    }

    if left.resolved_sem_type() == Some(SemType::Nil)
        && right.resolved_sem_type() == Some(SemType::Nil)
    {
        return true;
    }

    match left.kind {
        NodeKind::Scalar | NodeKind::Alias => left.value == right.value,
        NodeKind::Sequence => recursive_array_equal(left, right),
        NodeKind::Mapping => recursive_mapping_equal(left, right),
        NodeKind::Unknown => false,
    }
}

fn recursive_array_equal(left: &TreeNode, right: &TreeNode) -> bool {
    left.content.len() == right.content.len()
        && left
            .content
            .iter()
            .zip(&right.content)
            .all(|(lhs, rhs)| recursive_node_compare(lhs, rhs))
}

fn recursive_mapping_equal(left: &TreeNode, right: &TreeNode) -> bool {
    let left_entries = mapping_entries(left);
    let right_entries = mapping_entries(right);
    if left_entries.len() != right_entries.len() {
        return false;
    }

    let mut matched = vec![false; right_entries.len()];
    'outer: for (left_key, left_value) in left_entries {
        for (index, (right_key, right_value)) in right_entries.iter().enumerate() {
            if matched[index] {
                continue;
            }
            if recursive_node_compare(left_key, right_key)
                && recursive_node_compare(left_value, right_value)
            {
                matched[index] = true;
                continue 'outer;
            }
        }
        return false;
    }

    true
}

fn mapping_entries(node: &TreeNode) -> Vec<(&TreeNode, &TreeNode)> {
    let mut entries = Vec::new();
    let mut index = 0;
    while index + 1 < node.content.len() {
        let key = &node.content[index];
        let value = &node.content[index + 1];
        entries.push((key, value));
        index += 2;
    }
    entries
}

pub fn require_single_document<T>(documents: &[T]) -> Result<(), CoreError> {
    if documents.len() == 1 {
        Ok(())
    } else {
        Err(ParseError::InvalidSyntax.into())
    }
}

// ── find_in_array / find_key_in_map ──────────────────────────────

/// Find the index of a node within an array node using recursive semantic
/// comparison. Returns the index as i32, or -1 if not found.
pub fn find_in_array(array: &TreeNode, item: &TreeNode) -> i32 {
    for (i, child) in array.content.iter().enumerate() {
        if recursive_node_compare(child, item) {
            return i as i32;
        }
    }
    -1
}

/// Find the index of a key within a mapping node (stepping by 2).
/// Returns the key index as i32, or -1 if not found.
pub fn find_key_in_map(data_map: &TreeNode, item: &TreeNode) -> i32 {
    let mut i: usize = 0;
    while i < data_map.content.len() {
        if recursive_node_compare(&data_map.content[i], item) {
            return i as i32;
        }
        i += 2;
    }
    -1
}

// ── parse_snippet ────────────────────────────────────────────────

/// Parse a single snippet string into a TreeNode.
///
/// - Empty string returns a nil scalar node.
/// - `":"` returns `InvalidYaml` error.
/// - Strings starting with `"#"` return a nil scalar node with `line_comment` set.
/// - `"null"` returns a nil scalar node.
/// - Otherwise infers the scalar tag and sem_type from the value.
pub fn parse_snippet(value: &str) -> Result<TreeNode, CoreError> {
    if value.is_empty() {
        let mut n = TreeNode::default();
        n.kind = NodeKind::Scalar;
        n.sem_type = Some(SemType::Nil);
        n.tag = SemType::Nil.to_string().into();
        return Ok(n);
    }
    if value == ":" {
        return Err(ParseError::InvalidYaml.into());
    }

    if value.starts_with('#') {
        let mut n = TreeNode::default();
        n.kind = NodeKind::Scalar;
        n.sem_type = Some(SemType::Nil);
        n.tag = SemType::Nil.to_string().into();
        n.line_comment = value.to_string();
        return Ok(n);
    }

    let mut n = TreeNode::default();
    n.kind = NodeKind::Scalar;
    n.value = value.to_string();

    if value == "null" {
        n.sem_type = Some(SemType::Nil);
        n.tag = SemType::Nil.to_string().into();
        return Ok(n);
    }

    let inferred = infer_scalar_tag(&n.tag, &n.value);
    n.tag = inferred.to_string();
    n.sem_type = SemType::from_string(&n.tag);
    Ok(n)
}

// ── float_to_string ──────────────────────────────────────────────

/// Convert an f64 to its string representation.
///
/// Special values: NaN → `"NaN"`, +Inf → `"+Inf"`, -Inf → `"-Inf"`.
pub fn float_to_string(value: f64) -> String {
    if value.is_infinite() {
        if value.is_sign_positive() {
            return "+Inf".to_string();
        }
        return "-Inf".to_string();
    }
    if value.is_nan() {
        return "NaN".to_string();
    }
    format!("{}", value)
}

// ── Comment helpers ──────────────────────────────────────────────

/// Return the head_comment with a leading `'#'` stripped.
pub fn head_comment(node: &TreeNode) -> &str {
    if let Some(stripped) = node.head_comment.strip_prefix('#') {
        stripped
    } else {
        &node.head_comment
    }
}

/// Return the line_comment with a leading `'#'` stripped.
pub fn line_comment(node: &TreeNode) -> &str {
    if let Some(stripped) = node.line_comment.strip_prefix('#') {
        stripped
    } else {
        &node.line_comment
    }
}

/// Return the foot_comment with a leading `'#'` stripped.
pub fn foot_comment(node: &TreeNode) -> &str {
    if let Some(stripped) = node.foot_comment.strip_prefix('#') {
        stripped
    } else {
        &node.foot_comment
    }
}

// ── Integer formatting / parsing ─────────────────────────────────

/// Result of `parse_int64`: the canonical format string and the parsed value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseInt64Result {
    pub fmt: String,
    pub value: i64,
}

/// Compute the unsigned magnitude of an i64 value.
fn int64_magnitude(value: i64) -> u64 {
    if value >= 0 {
        value as u64
    } else {
        (-(value + 1)) as u64 + 1
    }
}

/// Reconstruct an i64 from an unsigned magnitude and a sign flag.
fn signed_magnitude_to_i64(magnitude: u64, negative: bool) -> Result<i64, CoreError> {
    let max_positive = i64::MAX as u64;
    if !negative {
        if magnitude > max_positive {
            return Err(ParseError::InvalidSyntax.into());
        }
        return Ok(magnitude as i64);
    }

    let max_negative_magnitude = max_positive + 1;
    if magnitude > max_negative_magnitude {
        return Err(ParseError::InvalidSyntax.into());
    }
    if magnitude == max_negative_magnitude {
        return Ok(i64::MIN);
    }
    Ok(-(magnitude as i64))
}

/// Format an i64 according to a format string.
///
/// Supported formats:
/// - `"%v"` — decimal
/// - `"0x%X"` — hexadecimal with `0x` prefix
/// - `"0o%o"` — octal with `0o` prefix
/// - `"0b%b"` — binary with `0b` prefix
///
/// Returns an error for unknown format strings.
pub fn format_int64(fmt_str: &str, value: i64) -> Result<String, CoreError> {
    match fmt_str {
        "%v" => Ok(format!("{}", value)),
        "0x%X" => {
            let magnitude = int64_magnitude(value);
            if value < 0 {
                Ok(format!("-0x{:X}", magnitude))
            } else {
                Ok(format!("0x{:X}", magnitude))
            }
        }
        "0o%o" => {
            let magnitude = int64_magnitude(value);
            if value < 0 {
                Ok(format!("-0o{:o}", magnitude))
            } else {
                Ok(format!("0o{:o}", magnitude))
            }
        }
        "0b%b" => {
            let magnitude = int64_magnitude(value);
            if value < 0 {
                Ok(format!("-0b{:b}", magnitude))
            } else {
                Ok(format!("0b{:b}", magnitude))
            }
        }
        _ => Err(ParseError::InvalidSyntax.into()),
    }
}

/// Parse a number string (which may contain underscores and a sign) into a
/// format string and an i64 value.
///
/// Detects `0x`/`0X` (hex), `0o`/`0O` (octal), `0b`/`0B` (binary) prefixes.
/// Decimal is the fallback.
pub fn parse_int64(number_string: &str) -> Result<ParseInt64Result, CoreError> {
    // Strip underscores
    let cleaned: String = number_string.chars().filter(|&c| c != '_').collect();
    let mut s = cleaned.as_str();

    let negative = if !s.is_empty() && (s.starts_with('-') || s.starts_with('+')) {
        let neg = s.starts_with('-');
        s = &s[1..];
        neg
    } else {
        false
    };

    if s.starts_with("0x") || s.starts_with("0X") {
        let magnitude = u64::from_str_radix(&s[2..], 16).map_err(|_| ParseError::InvalidSyntax)?;
        Ok(ParseInt64Result {
            fmt: "0x%X".to_string(),
            value: signed_magnitude_to_i64(magnitude, negative)?,
        })
    } else if s.starts_with("0o") || s.starts_with("0O") {
        let magnitude = u64::from_str_radix(&s[2..], 8).map_err(|_| ParseError::InvalidSyntax)?;
        Ok(ParseInt64Result {
            fmt: "0o%o".to_string(),
            value: signed_magnitude_to_i64(magnitude, negative)?,
        })
    } else if s.starts_with("0b") || s.starts_with("0B") {
        let magnitude = u64::from_str_radix(&s[2..], 2).map_err(|_| ParseError::InvalidSyntax)?;
        Ok(ParseInt64Result {
            fmt: "0b%b".to_string(),
            value: signed_magnitude_to_i64(magnitude, negative)?,
        })
    } else {
        let magnitude = s.parse::<u64>().map_err(|_| ParseError::InvalidSyntax)?;
        Ok(ParseInt64Result {
            fmt: "%v".to_string(),
            value: signed_magnitude_to_i64(magnitude, negative)?,
        })
    }
}

/// Parse a number string into an i32.
///
/// Delegates to `parse_int64` and checks that the result fits in i32 range.
pub fn parse_int(number_string: &str) -> Result<i32, CoreError> {
    let r = parse_int64(number_string)?;
    if r.value > i32::MAX as i64 || r.value < i32::MIN as i64 {
        return Err(ParseError::InvalidSyntax.into());
    }
    Ok(r.value as i32)
}

/// Parse a boolean from a string value.
///
/// Accepts "true"/"false", "yes"/"no", "y"/"n" (case-insensitive).
/// Returns `None` when the value is not a recognized boolean representation.
pub fn parse_bool(value: &str) -> Option<bool> {
    if value.eq_ignore_ascii_case("true")
        || value.eq_ignore_ascii_case("yes")
        || value.eq_ignore_ascii_case("y")
    {
        return Some(true);
    }
    if value.eq_ignore_ascii_case("false")
        || value.eq_ignore_ascii_case("no")
        || value.eq_ignore_ascii_case("n")
    {
        return Some(false);
    }
    None
}

// ── Wildcard key matching ────────────────────────────────────────

/// Match a key name against a pattern that supports `*` (any sequence) and
/// `?` (any single character) wildcards.
///
/// - An empty pattern only matches an empty name.
/// - A lone `"*"` matches any name.
pub fn match_key(name: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return name == pattern;
    }
    if pattern.len() == 1 && pattern.as_bytes()[0] == b'*' {
        return true;
    }
    deep_match(name, pattern)
}

/// Full wildcard matching with backtracking support.
///
/// `*` matches zero or more characters; `?` matches exactly one character.
pub fn deep_match(name: &str, pattern: &str) -> bool {
    let name_bytes = name.as_bytes();
    let pat_bytes = pattern.as_bytes();
    let mut px: usize = 0;
    let mut nx: usize = 0;
    let mut next_px: usize = 0;
    let mut next_nx: usize = 0;

    while px < pat_bytes.len() || nx < name_bytes.len() {
        if px < pat_bytes.len() {
            match pat_bytes[px] {
                b'?' => {
                    if nx < name_bytes.len() {
                        px += 1;
                        nx += 1;
                        continue;
                    }
                }
                b'*' => {
                    next_px = px;
                    next_nx = nx + 1;
                    px += 1;
                    continue;
                }
                c => {
                    if nx < name_bytes.len() && name_bytes[nx] == c {
                        px += 1;
                        nx += 1;
                        continue;
                    }
                }
            }
        }

        if next_nx > 0 && next_nx <= name_bytes.len() {
            px = next_px;
            nx = next_nx;
            continue;
        }
        return false;
    }
    true
}

// ── Escape character processing ──────────────────────────────────

/// Process escape sequences in a string.
///
/// Recognized escapes:
/// - `\\` → `\`
/// - `\"` → `"`
/// - `\n` → newline
/// - `\t` → tab
/// - `\r` → carriage return
/// - `\f` → form feed (0x0C)
/// - `\v` → vertical tab (0x0B)
/// - `\b` → backspace (0x08)
/// - `\a` → bell (0x07)
///
/// Unrecognized escape sequences are left as-is.
pub fn process_escape_characters(original: &str) -> Result<String, CoreError> {
    if original.is_empty() {
        return Ok(original.to_string());
    }

    let bytes = original.as_bytes();
    let mut out = String::with_capacity(original.len());
    let mut i: usize = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            match next {
                b'\\' => {
                    out.push('\\');
                    i += 2;
                    continue;
                }
                b'"' => {
                    out.push('"');
                    i += 2;
                    continue;
                }
                b'n' => {
                    out.push('\n');
                    i += 2;
                    continue;
                }
                b't' => {
                    out.push('\t');
                    i += 2;
                    continue;
                }
                b'r' => {
                    out.push('\r');
                    i += 2;
                    continue;
                }
                b'f' => {
                    out.push(0x0C as char);
                    i += 2;
                    continue;
                }
                b'v' => {
                    out.push(0x0B as char);
                    i += 2;
                    continue;
                }
                b'b' => {
                    out.push(0x08 as char);
                    i += 2;
                    continue;
                }
                b'a' => {
                    out.push(0x07 as char);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }

    Ok(out)
}

// ── nodes_to_string ──────────────────────────────────────────────

/// Convert an array of nodes to a string representation.
///
/// Joins the value of each node, separated by newlines for scalar nodes.
pub fn nodes_to_string(nodes: &[TreeNode]) -> Result<String, CoreError> {
    let mut out = String::new();
    for (i, node) in nodes.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&node.value);
    }
    Ok(out)
}
