use crate::operators::{NodeKind, TreeNode};
use crate::wasm_types::{PathSeg, PathSegTag, PathSpan};

use super::tree_node::{NodeId, TreeNodeKind as CoreTreeNodeKind};
use super::tree_store::{TreeEntry, TreeStore};
use crate::analysis::line_index::LineIndex;
use crate::graph::graph_builder::PathSeg as GraphPathSeg;
use crate::language::lang_spec;
use crate::tree::tree_path_index::{OwnedPathSeg, TreePathIndex};

pub const EMPTY_PATH: &[PathSeg<'static>] = &[];

// ============================================================================
// Basic constructors / accessors (existing)
// ============================================================================

pub fn path_seg_key(key: &str) -> PathSeg<'_> {
    PathSeg {
        tag: PathSegTag::Key,
        key,
        index: 0,
    }
}

pub fn path_seg_index(index: i32) -> PathSeg<'static> {
    PathSeg {
        tag: PathSegTag::Index,
        key: "",
        index,
    }
}

pub fn path_seg_key_slice(seg: PathSeg<'_>) -> &str {
    seg.key
}

pub fn is_simple_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

// ============================================================================
// Formatting helpers (existing)
// ============================================================================

pub fn format_tree_path_segment(seg: PathSeg<'_>) -> String {
    if seg.tag == PathSegTag::Index {
        return format!("[{}]", seg.index);
    }
    if is_simple_key(seg.key) {
        seg.key.to_owned()
    } else {
        format!("[{}]", append_json_escaped(seg.key))
    }
}

pub fn build_tree_path_parts<'a>(segments: &'a [PathSeg<'a>]) -> Vec<String> {
    let mut parts = Vec::with_capacity(segments.len() + 1);
    parts.push("$".to_owned());
    parts.extend(segments.iter().map(|seg| format_tree_path_segment(*seg)));
    parts
}

pub fn format_tree_path(segments: &[PathSeg<'_>]) -> String {
    if segments.is_empty() {
        return "$".to_owned();
    }

    let mut out = String::from("$");
    for seg in segments {
        if seg.tag == PathSegTag::Index {
            out.push_str(&format!("[{}]", seg.index));
        } else if is_simple_key(seg.key) {
            out.push('.');
            out.push_str(seg.key);
        } else {
            out.push('[');
            out.push_str(&append_json_escaped(seg.key));
            out.push(']');
        }
    }
    out
}

pub fn borrowed_tree_path(segments: &[OwnedPathSeg]) -> Vec<PathSeg<'_>> {
    segments.iter().map(OwnedPathSeg::borrowed).collect()
}

pub fn format_owned_tree_path(segments: &[OwnedPathSeg]) -> String {
    format_tree_path(&borrowed_tree_path(segments))
}

/// Parse the protocol Tree Path syntax into an owned representation.
///
/// The empty string and `$` both denote the root. For compatibility, paths
/// without the leading `$` are accepted when they start with `.` or `[`. Keys
/// may use dot notation or JSON-string bracket notation; bracket indices are
/// signed `i32` values and are rejected later when they cannot address a
/// sequence node.
pub fn parse_tree_path(path: &str) -> Option<Vec<OwnedPathSeg>> {
    if path.is_empty() || path == "$" {
        return Some(Vec::new());
    }

    let bytes = path.as_bytes();
    let mut index = usize::from(bytes.first() == Some(&b'$'));
    let mut segments = Vec::new();

    while index < bytes.len() {
        match bytes[index] {
            b'.' => {
                index += 1;
                let start = index;
                while index < bytes.len()
                    && matches!(bytes[index], b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'$')
                {
                    index += 1;
                }
                if start == index {
                    return None;
                }
                segments.push(OwnedPathSeg::Key(path[start..index].to_owned()));
            }
            b'[' => {
                let inner_start = index + 1;
                let mut cursor = inner_start;
                let mut in_string = false;
                let mut escaped = false;
                let end = loop {
                    let byte = *bytes.get(cursor)?;
                    if in_string {
                        if escaped {
                            escaped = false;
                        } else if byte == b'\\' {
                            escaped = true;
                        } else if byte == b'"' {
                            in_string = false;
                        }
                    } else if byte == b'"' {
                        in_string = true;
                    } else if byte == b']' {
                        break cursor;
                    }
                    cursor += 1;
                };
                if in_string {
                    return None;
                }

                let inner = path[inner_start..end].trim();
                if inner.starts_with('"') {
                    let key = unescape_json_string(inner)?;
                    segments.push(OwnedPathSeg::Key(key));
                } else {
                    segments.push(OwnedPathSeg::Index(inner.parse::<i32>().ok()?));
                }
                index = end + 1;
            }
            _ => return None,
        }
    }

    Some(segments)
}

fn append_json_escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch < ' ' => out.push_str(&format!("\\u{:04X}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

// ============================================================================
// Graph-path functions (existing — compat TreeNode)
// ============================================================================

pub fn find_node_by_graph_path<'a>(
    root: &'a TreeNode,
    path: &[GraphPathSeg],
    prefer_key: bool,
) -> Option<&'a TreeNode> {
    if path.is_empty() {
        return Some(root);
    }

    let mut current = root;
    for (index, segment) in path.iter().enumerate() {
        let is_last = index + 1 == path.len();
        match (current.kind, segment) {
            (NodeKind::Mapping, GraphPathSeg::Key(key)) => {
                let (key_node, value_node) = find_mapping_entry(current, key)?;
                current = if is_last && prefer_key {
                    key_node
                } else {
                    value_node
                };
            }
            (NodeKind::Sequence, GraphPathSeg::Index(value)) => {
                current = current.content.get(*value)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

pub fn compute_graph_path_span(
    root: &TreeNode,
    path: &[GraphPathSeg],
    prefer_key: bool,
) -> PathSpan {
    let Some(node) = find_node_by_graph_path(root, path, prefer_key) else {
        return PathSpan::EMPTY;
    };

    PathSpan {
        start_byte: i32::try_from(node.start_byte).unwrap_or(-1),
        end_byte: i32::try_from(node.end_byte).unwrap_or(-1),
        row: node.line.saturating_sub(1),
        column: node.column.saturating_sub(1),
    }
}

fn find_mapping_entry<'a>(node: &'a TreeNode, key: &str) -> Option<(&'a TreeNode, &'a TreeNode)> {
    let mut index = 0;
    while index + 1 < node.content.len() {
        let key_node = &node.content[index];
        let value_node = &node.content[index + 1];
        if key_node.is_map_key && key_node.value == key {
            return Some((key_node, value_node));
        }
        index += 2;
    }
    None
}

// ============================================================================
// ============================================================================

/// Check whether `source` looks like a JSON document (starts with `{` or `[`
/// followed by `{` / `[`).
///
pub fn looks_like_json_document(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut start: usize = 0;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    if start >= bytes.len() {
        return false;
    }
    if bytes[start] == b'{' {
        return true;
    }
    if bytes[start] != b'[' {
        return false;
    }
    let mut i = start + 1;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= bytes.len() {
        return false;
    }
    bytes[i] == b'{' || bytes[i] == b'['
}

/// Check whether a tree-sitter node type name represents punctuation.
///
pub fn is_punctuation_type(node_type: &str) -> bool {
    if node_type.is_empty() {
        return true;
    }
    if node_type.len() == 1 {
        let ch = node_type.as_bytes()[0];
        if !ch.is_ascii_alphanumeric() && ch != b'_' {
            return true;
        }
    }
    matches!(
        node_type,
        ":" | "," | "{" | "}" | "[" | "]" | "(" | ")" | "<" | ">" | "/" | "?" | "=" | "-" | "+"
    )
}

/// Unescape a JSON double-quoted string (including the surrounding quotes).
///
/// Returns `None` when the input is not a valid JSON string.
///
pub fn unescape_json_string(text: &str) -> Option<String> {
    serde_json::from_str(text).ok()
}

/// Normalize key text: trim whitespace, strip surrounding quotes, unescape
/// JSON strings.
///
pub fn normalize_key_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= 1 {
        return trimmed.to_owned();
    }
    let first = trimmed.as_bytes()[0];
    let last = trimmed.as_bytes()[trimmed.len() - 1];
    if first == b'"' && last == b'"' {
        if let Some(value) = unescape_json_string(trimmed) {
            return value;
        }
        return trimmed[1..trimmed.len() - 1].to_owned();
    }
    if first == b'\'' && last == b'\'' {
        return trimmed[1..trimmed.len() - 1].to_owned();
    }
    trimmed.to_owned()
}

// ============================================================================
// ============================================================================

/// Return the first child of `node` whose type is not punctuation.
///
fn get_first_non_punctuation_child(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let count = node.child_count();
    for i in 0..count {
        let child = node.child(i as _)?;
        if is_punctuation_type(child.kind()) {
            continue;
        }
        return Some(child);
    }
    None
}

/// Collect TOML key parts from a tree-sitter key node (handles `bare_key`,
/// `quoted_key`, and `dotted_key`).
///
fn collect_toml_key_parts_from_node(
    source: &[u8],
    key_node: tree_sitter::Node<'_>,
    out: &mut Vec<String>,
) {
    let typ = key_node.kind();
    if typ == "bare_key" {
        if let Ok(text) = std::str::from_utf8(&source[key_node.start_byte()..key_node.end_byte()]) {
            out.push(text.to_owned());
        }
        return;
    }
    if typ == "quoted_key" {
        if let Ok(raw) = std::str::from_utf8(&source[key_node.start_byte()..key_node.end_byte()]) {
            out.push(normalize_key_text(raw));
        }
        return;
    }
    if typ == "dotted_key" {
        let count = key_node.child_count();
        for i in 0..count {
            let child = match key_node.child(i as _) {
                Some(c) => c,
                None => continue,
            };
            if is_punctuation_type(child.kind()) {
                continue;
            }
            collect_toml_key_parts_from_node(source, child, out);
        }
    }
}

/// Extract the key text from a pair node.
///
fn get_pair_key(pair_node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    let key_node = get_first_non_punctuation_child(pair_node)?;
    let key_text = std::str::from_utf8(&source[key_node.start_byte()..key_node.end_byte()]).ok()?;
    if key_text.is_empty() {
        return None;
    }
    Some(normalize_key_text(key_text))
}

/// Check whether `node` contains `target` (both start and end points of
/// `target` are within `node`).
///
fn node_contains_node(node: tree_sitter::Node<'_>, target: tree_sitter::Node<'_>) -> bool {
    let target_start = target.start_position();
    let target_end = target.end_position();
    let node_start = node.start_position();
    let node_end = node.end_position();
    point_in_range(target_start, node_start, node_end)
        && point_in_range(target_end, node_start, node_end)
}

/// Find the array index of `target` within `array_node`.
///
fn find_array_index(
    array_node: tree_sitter::Node<'_>,
    target: tree_sitter::Node<'_>,
    language_name: &str,
) -> Option<usize> {
    let count = array_node.child_count();
    let spec = lang_spec::node_type_spec_for_language(language_name);
    if let Some(item_type) = spec.array_item_type {
        if lang_spec::is_array_node_type(array_node.kind(), language_name) {
            let mut index: usize = 0;
            let mut matched_item_type = false;
            for i in 0..count {
                let child = array_node.child(i as _)?;
                if child.kind() == item_type {
                    matched_item_type = true;
                    if node_contains_node(child, target) {
                        return Some(index);
                    }
                    index += 1;
                }
            }
            if matched_item_type {
                return None;
            }
        }
    }
    let mut index: usize = 0;
    for i in 0..count {
        let child = array_node.child(i as _)?;
        if is_punctuation_type(child.kind()) {
            continue;
        }
        if node_contains_node(child, target) {
            return Some(index);
        }
        index += 1;
    }
    None
}

/// Build structured tree-path segments by walking up the tree-sitter AST from
/// `node`.
///
fn build_structured_tree_path_segments(
    node: tree_sitter::Node<'_>,
    language_name: &str,
    source: &[u8],
) -> Vec<OwnedPathSeg> {
    let mut segments = Vec::new();
    let mut current = node;

    loop {
        let parent = match current.parent() {
            Some(p) => p,
            None => break,
        };
        let parent_type = parent.kind();

        if lang_spec::is_pair_node_type(parent_type, language_name) {
            if language_name == "toml" {
                if let Some(key_node) = get_first_non_punctuation_child(parent) {
                    let mut parts: Vec<String> = Vec::new();
                    collect_toml_key_parts_from_node(source, key_node, &mut parts);
                    if !parts.is_empty() {
                        for part in parts.into_iter().rev() {
                            segments.push(OwnedPathSeg::Key(part));
                        }
                    } else if let Some(key) = get_pair_key(parent, source) {
                        segments.push(OwnedPathSeg::Key(key));
                    }
                }
            } else if let Some(key) = get_pair_key(parent, source) {
                segments.push(OwnedPathSeg::Key(key));
            }
            current = parent;
            continue;
        }

        if lang_spec::is_array_node_type(parent_type, language_name) {
            if let Some(index) = find_array_index(parent, current, language_name) {
                segments.push(OwnedPathSeg::Index(index as i32));
            }
            current = parent;
            continue;
        }

        if language_name == "toml"
            && (parent_type == "table" || parent_type == "table_array_element")
        {
            let count = parent.child_count();
            let mut key_node: Option<tree_sitter::Node<'_>> = None;
            for i in 0..count {
                let child = match parent.child(i as _) {
                    Some(c) => c,
                    None => continue,
                };
                let child_type = child.kind();
                if is_punctuation_type(child_type) {
                    continue;
                }
                if child_type == "pair" {
                    continue;
                }
                key_node = Some(child);
                break;
            }
            if let Some(kn) = key_node {
                let mut parts: Vec<String> = Vec::new();
                collect_toml_key_parts_from_node(source, kn, &mut parts);
                if !parts.is_empty() {
                    for part in parts.into_iter().rev() {
                        segments.push(OwnedPathSeg::Key(part));
                    }
                } else {
                    let raw =
                        std::str::from_utf8(&source[kn.start_byte()..kn.end_byte()]).unwrap_or("");
                    if !raw.is_empty() {
                        segments.push(OwnedPathSeg::Key(normalize_key_text(raw)));
                    }
                }
            }
            current = parent;
            continue;
        }

        current = parent;
    }

    if segments.is_empty() {
        return Vec::new();
    }
    segments.reverse();
    segments
}

/// Normalize a tree-sitter node: skip punctuation, `document`, `stream`, and
/// `content` wrapper nodes.
///
fn normalize_node<'a>(node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    let mut current = node;
    loop {
        let node_type = current.kind();
        if !is_punctuation_type(node_type)
            && node_type != "document"
            && node_type != "stream"
            && node_type != "content"
        {
            break;
        }
        current = current.parent()?;
    }
    Some(current)
}

// ============================================================================
// Tree-sitter point / position helpers (new)
// ============================================================================

/// Compare two [`tree_sitter::Point`] values.
///
/// Returns -1, 0, or 1.
///
fn compare_points(a: tree_sitter::Point, b: tree_sitter::Point) -> i32 {
    if a.row < b.row {
        return -1;
    }
    if a.row > b.row {
        return 1;
    }
    if a.column < b.column {
        return -1;
    }
    if a.column > b.column {
        return 1;
    }
    0
}

/// Check whether `point` is within the range `[start, end]`.
///
fn point_in_range(
    point: tree_sitter::Point,
    start: tree_sitter::Point,
    end: tree_sitter::Point,
) -> bool {
    compare_points(point, start) >= 0 && compare_points(point, end) <= 0
}

/// Check whether `node` contains `point`.
///
fn node_contains_point(node: tree_sitter::Node<'_>, point: tree_sitter::Point) -> bool {
    let start = node.start_position();
    let end = node.end_position();
    point_in_range(point, start, end)
}

/// Find the deepest node at `point` within the tree rooted at `node`.
///
fn find_node_at_position(
    node: tree_sitter::Node<'_>,
    point: tree_sitter::Point,
) -> Option<tree_sitter::Node<'_>> {
    if !node_contains_point(node, point) {
        return None;
    }
    let count = node.child_count();
    for i in 0..count {
        let child = node.child(i as _)?;
        if !node_contains_point(child, point) {
            continue;
        }
        if let Some(found) = find_node_at_position(child, point) {
            return Some(found);
        }
    }
    Some(node)
}

/// Check whether a tree-sitter tree has a root node with an error.
///
fn tree_has_error(tree: &tree_sitter::Tree) -> bool {
    let root = tree.root_node();
    root.has_error()
}
fn parse_structured_tree_for_call(language_name: &str, source: &str) -> Option<tree_sitter::Tree> {
    let tree = lang_spec::parse_tree(language_name, source.as_bytes())?;
    (!tree_has_error(&tree)).then_some(tree)
}

// ============================================================================
// TreeStore entry validation (new)
// ============================================================================

/// Get the current [`TreeEntry`] from the store, validating that it matches
/// the expected language, source, has no diagnostics, and the tree-sitter tree
/// (if present) has no errors.
///
fn get_current_tree_entry<'a>(
    store: Option<&'a TreeStore>,
    cache_key: &str,
    language_name: &str,
    source: &str,
) -> Option<&'a TreeEntry> {
    let store = store?;
    let entry = store.get_tree_entry(cache_key)?;
    if entry.language != language_name {
        return None;
    }
    if entry.source != source {
        return None;
    }
    if !entry.diagnostics_raw.is_empty() {
        return None;
    }
    if let Some(ref tree) = entry.ts_tree {
        if tree_has_error(tree) {
            return None;
        }
    }
    Some(entry)
}

// ============================================================================
// ============================================================================

/// Check whether a tree (rooted at `node_id`) has any byte spans.
///
fn tree_has_byte_spans(node_id: NodeId, store: &TreeStore) -> bool {
    let node = match store.get(node_id) {
        Some(n) => n,
        None => return false,
    };
    if node.end_byte > node.start_byte {
        return true;
    }
    if matches!(
        node.kind,
        CoreTreeNodeKind::Mapping | CoreTreeNodeKind::Sequence
    ) {
        for &child_id in &node.content {
            if tree_has_byte_spans(child_id, store) {
                return true;
            }
        }
    }
    false
}

/// Find the deepest tree node at `byte_offset` within the subtree rooted at
/// `node_id`.
///
fn find_tree_node_at_byte_offset(
    node_id: NodeId,
    byte_offset: u32,
    store: &TreeStore,
) -> Option<NodeId> {
    let node = store.get(node_id)?;
    if matches!(
        node.kind,
        CoreTreeNodeKind::Mapping | CoreTreeNodeKind::Sequence
    ) {
        for &child_id in &node.content {
            if let Some(found) = find_tree_node_at_byte_offset(child_id, byte_offset, store) {
                return Some(found);
            }
        }
    }
    if node.end_byte <= node.start_byte {
        return None;
    }
    if byte_offset < node.start_byte || byte_offset >= node.end_byte {
        return None;
    }
    Some(node_id)
}

/// Convert tree-node path segments (from `TreeStore::path_for`) into
/// [`PathSeg`] values.
///
fn tree_path_segments_at_byte_offset(
    tree_root: NodeId,
    byte_offset: u32,
    store: &TreeStore,
) -> Option<Vec<OwnedPathSeg>> {
    // First try find_node_at_offset-style lookup via byte spans.
    let found = find_tree_node_at_byte_offset(tree_root, byte_offset, store)?;
    let parsed_path = store.path_for(found).ok()?;
    if parsed_path.is_empty() {
        return Some(Vec::new());
    }

    let mut out = Vec::with_capacity(parsed_path.len());
    for seg in &parsed_path {
        match seg {
            super::tree_node::ParsedKey::Str(s) => out.push(OwnedPathSeg::Key(s.clone())),
            super::tree_node::ParsedKey::Int(v) => {
                if *v < 0 {
                    return None;
                }
                let Ok(idx) = usize::try_from(*v) else {
                    return None;
                };
                if idx > i32::MAX as usize {
                    return None;
                }
                out.push(OwnedPathSeg::Index(idx as i32));
            }
        }
    }
    Some(out)
}

/// Search outward from `byte_offset` (within `[line_start, line_end)`) for
/// the nearest tree node that yields non-empty path segments.
///
fn find_nearest_tree_path_segments(
    tree_root: NodeId,
    source: &str,
    line_start: usize,
    line_end: usize,
    byte_offset: usize,
    store: &TreeStore,
) -> Option<Vec<OwnedPathSeg>> {
    if line_start >= line_end || byte_offset >= source.len() {
        return None;
    }
    let left_delta = byte_offset.saturating_sub(line_start);
    let right_delta = line_end.saturating_sub(byte_offset.saturating_add(1));
    let max_delta = std::cmp::max(left_delta, right_delta);
    let mut delta: usize = 1;
    while delta <= max_delta {
        if byte_offset + delta < line_end {
            if let Some(segments) =
                tree_path_segments_at_byte_offset(tree_root, (byte_offset + delta) as u32, store)
            {
                if !segments.is_empty() {
                    return Some(segments);
                }
            }
        }
        if byte_offset >= line_start + delta {
            if let Some(segments) =
                tree_path_segments_at_byte_offset(tree_root, (byte_offset - delta) as u32, store)
            {
                if !segments.is_empty() {
                    return Some(segments);
                }
            }
        }
        delta += 1;
    }
    None
}

// ============================================================================
// ============================================================================

/// Compute tree-path segments for a given (row, column) position.
///
/// This is the main entry point that:
/// 1. Tries streaming codec path (JSON)
/// 2. Falls back to tree-node byte-span lookup
/// 3. Falls back to tree-sitter structured path building
///
pub fn compute_tree_path_segments(
    store: Option<&TreeStore>,
    cache_key: &str,
    language_name: &str,
    source: &str,
    row: u32,
    column: u32,
) -> Vec<OwnedPathSeg> {
    // TOML that looks like JSON → bail out early.
    if language_name == "toml" && looks_like_json_document(source) {
        return Vec::new();
    }

    let entry = match get_current_tree_entry(store, cache_key, language_name, source) {
        Some(e) => e,
        None => return Vec::new(),
    };
    let entry_source = &entry.source;
    if entry_source.is_empty() {
        return Vec::new();
    }

    let line_bounds = match entry.line_index.line_bounds(entry_source.len(), row) {
        Some(b) => b,
        None => return Vec::new(),
    };
    let byte_offset = entry
        .line_index
        .line_column_to_offset(row, column)
        .min(entry_source.len());
    let has_byte_spans = store.is_some_and(|s| tree_has_byte_spans(entry.root, s));

    // --- Phase 1: try byte-span lookup on the tree-node tree ---
    let store_ref = match store {
        Some(s) => s,
        None => return Vec::new(),
    };
    if has_byte_spans {
        if let Some(segments) =
            tree_path_segments_at_byte_offset(entry.root, byte_offset as u32, store_ref)
        {
            if !segments.is_empty() {
                return segments;
            }
            // If we're on whitespace and got empty segments, try nearest.
            if byte_offset < entry_source.len()
                && !entry_source.as_bytes()[byte_offset].is_ascii_whitespace()
            {
                return Vec::new();
            }
        }
        if let Some(segments) = find_nearest_tree_path_segments(
            entry.root,
            entry_source,
            line_bounds.start,
            line_bounds.end,
            byte_offset,
            store_ref,
        ) {
            return segments;
        }
    }

    // --- Phase 2: tree-sitter structured path ---
    let path_spec = lang_spec::find_spec(language_name);
    if path_spec.is_none() || !path_spec.unwrap().has_structured_path {
        return Vec::new();
    }

    let owned_tree: Option<tree_sitter::Tree>;
    let tree = match &entry.ts_tree {
        Some(t) => t,
        None => {
            owned_tree = parse_structured_tree_for_call(language_name, entry_source);
            let Some(tree) = owned_tree.as_ref() else {
                return Vec::new();
            };
            tree
        }
    };
    let root_node = tree.root_node();
    let point = tree_sitter::Point {
        row: row as usize,
        column: column as usize,
    };
    let found_ts = match find_node_at_position(root_node, point) {
        Some(n) => n,
        None => return Vec::new(),
    };
    let normalized = match normalize_node(found_ts) {
        Some(n) => n,
        None => return Vec::new(),
    };
    build_structured_tree_path_segments(normalized, language_name, entry_source.as_bytes())
}

pub fn compute_tree_path_segments_for_document(
    store: &TreeStore,
    root: NodeId,
    ts_tree: Option<&tree_sitter::Tree>,
    diagnostics_raw: &[u32],
    language_name: &str,
    source: &str,
    line_index: &LineIndex,
    row: u32,
    column: u32,
) -> Vec<OwnedPathSeg> {
    if language_name == "toml" && looks_like_json_document(source) {
        return Vec::new();
    }
    if !diagnostics_raw.is_empty() {
        return Vec::new();
    }

    let line_bounds = match line_index.line_bounds(source.len(), row) {
        Some(bounds) => bounds,
        None => return Vec::new(),
    };
    let byte_offset = line_index
        .line_column_to_offset(row, column)
        .min(source.len());

    if tree_has_byte_spans(root, store) {
        if let Some(segments) = tree_path_segments_at_byte_offset(root, byte_offset as u32, store) {
            if !segments.is_empty() {
                return segments;
            }
            if byte_offset < source.len() && !source.as_bytes()[byte_offset].is_ascii_whitespace() {
                return Vec::new();
            }
        }
        if let Some(segments) = find_nearest_tree_path_segments(
            root,
            source,
            line_bounds.start,
            line_bounds.end,
            byte_offset,
            store,
        ) {
            return segments;
        }
    }

    let path_spec = lang_spec::find_spec(language_name);
    if path_spec.is_none() || !path_spec.unwrap().has_structured_path {
        return Vec::new();
    }

    let owned_tree: Option<tree_sitter::Tree>;
    let tree = match ts_tree {
        Some(tree) if !tree_has_error(tree) => tree,
        _ => {
            owned_tree = parse_structured_tree_for_call(language_name, source);
            let Some(tree) = owned_tree.as_ref() else {
                return Vec::new();
            };
            tree
        }
    };
    let root_node = tree.root_node();
    let point = tree_sitter::Point {
        row: row as usize,
        column: column as usize,
    };
    let found_ts = match find_node_at_position(root_node, point) {
        Some(node) => node,
        None => return Vec::new(),
    };
    let normalized = match normalize_node(found_ts) {
        Some(node) => node,
        None => return Vec::new(),
    };
    build_structured_tree_path_segments(normalized, language_name, source.as_bytes())
}

fn path_segments_match(expected: &[PathSeg<'_>], actual: &[PathSeg<'_>]) -> bool {
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(lhs, rhs)| {
            lhs.tag == rhs.tag
                && match lhs.tag {
                    PathSegTag::Key => lhs.key == rhs.key,
                    PathSegTag::Index => lhs.index == rhs.index,
                }
        })
}

fn structured_pair_fields(
    pair_node: tree_sitter::Node<'_>,
) -> (Option<tree_sitter::Node<'_>>, Option<tree_sitter::Node<'_>>) {
    let key = pair_node.child_by_field_name("key");
    let value = pair_node.child_by_field_name("value");
    if key.is_some() || value.is_some() {
        return (key, value);
    }

    let mut children =
        (0..pair_node.named_child_count()).filter_map(|index| pair_node.named_child(index as _));
    (children.next(), children.next())
}

fn path_span_from_ts_node(node: tree_sitter::Node<'_>) -> PathSpan {
    let start = node.start_position();
    PathSpan {
        start_byte: i32::try_from(node.start_byte()).unwrap_or(-1),
        end_byte: i32::try_from(node.end_byte()).unwrap_or(-1),
        row: i32::try_from(start.row).unwrap_or(-1),
        column: i32::try_from(start.column).unwrap_or(-1),
    }
}

fn structured_path_span_for_pair(
    pair_node: tree_sitter::Node<'_>,
    language_name: &str,
    source: &[u8],
    path: &[PathSeg<'_>],
    prefer_key: bool,
) -> Option<PathSpan> {
    let (key_node, value_node) = structured_pair_fields(pair_node);
    let target_node = if prefer_key { key_node? } else { value_node? };
    let candidate = normalize_node(target_node)
        .map(|node| build_structured_tree_path_segments(node, language_name, source))
        .unwrap_or_default();
    path_segments_match(path, &borrowed_tree_path(&candidate))
        .then(|| path_span_from_ts_node(target_node))
}

fn recover_structured_path_span(
    node: tree_sitter::Node<'_>,
    language_name: &str,
    source: &[u8],
    path: &[PathSeg<'_>],
    prefer_key: bool,
) -> Option<PathSpan> {
    if lang_spec::is_pair_node_type(node.kind(), language_name) {
        if let Some(span) =
            structured_path_span_for_pair(node, language_name, source, path, prefer_key)
        {
            return Some(span);
        }
    }

    let candidate = normalize_node(node)
        .map(|normalized| build_structured_tree_path_segments(normalized, language_name, source))
        .unwrap_or_default();
    if path_segments_match(path, &borrowed_tree_path(&candidate)) {
        return Some(path_span_from_ts_node(node));
    }

    for index in 0..node.named_child_count() {
        let child = match node.named_child(index as _) {
            Some(child) => child,
            None => continue,
        };
        if let Some(span) =
            recover_structured_path_span(child, language_name, source, path, prefer_key)
        {
            return Some(span);
        }
    }

    None
}

// ============================================================================
// ============================================================================

/// Find a tree node by walking a [`PathSeg`] slice from `root`.
///
/// When `prefer_key` is true and the last segment is a key, the key node
/// itself is returned instead of the value node.
///
pub fn find_node_by_path(
    root: NodeId,
    path: &[PathSeg<'_>],
    prefer_key: bool,
    store: &TreeStore,
) -> Option<NodeId> {
    if path.is_empty() {
        return Some(root);
    }

    let mut current = root;
    for (i, seg) in path.iter().enumerate() {
        let is_last = i + 1 == path.len();
        let node = store.get(current)?;
        match node.kind {
            CoreTreeNodeKind::Mapping => {
                if seg.tag != PathSegTag::Key {
                    return None;
                }
                let key = seg.key;
                let (key_id, value_id) = find_map_entry_in_store(current, key, store)?;
                current = if is_last && prefer_key {
                    key_id
                } else {
                    value_id
                };
            }
            CoreTreeNodeKind::Sequence => {
                if seg.tag != PathSegTag::Index {
                    return None;
                }
                if seg.index < 0 {
                    return None;
                }
                let seq_index = seg.index as usize;
                let child_id = *node.content.get(seq_index)?;
                if is_last {
                    return Some(child_id);
                }
                current = child_id;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Find a tree node by walking a [`PathSeg`] slice from `root`, optionally
/// using a pre-built [`crate::tree::TreePathIndex`] to avoid linear scanning.
///
/// When `index` is `Some`, the index's hashmap is consulted first for the
/// value or key node. Falls back to [`find_node_by_path`] when the index is
/// `None` or the path segment is not found.
pub fn find_node_by_path_with_index(
    root: NodeId,
    path: &[PathSeg<'_>],
    prefer_key: bool,
    store: &TreeStore,
    index: Option<&crate::tree::TreePathIndex>,
) -> Option<NodeId> {
    if let Some(index) = index {
        if prefer_key
            && path
                .last()
                .is_some_and(|segment| segment.tag == PathSegTag::Key)
        {
            if let Some(node_id) = index.key_node_for_segments(path) {
                return Some(node_id);
            }
        }
        if let Some(node_id) = index.value_node_for_segments(path) {
            return Some(node_id);
        }
    }
    find_node_by_path(root, path, prefer_key, store)
}

/// Find a mapping entry (key_id, value_id) in a store-backed mapping node.
fn find_map_entry_in_store(
    parent: NodeId,
    expected_key: &str,
    store: &TreeStore,
) -> Option<(NodeId, NodeId)> {
    let node = store.get(parent)?;
    let mut index = 0;
    while index + 1 < node.content.len() {
        let key_id = node.content[index];
        let value_id = node.content[index + 1];
        let key_node = store.get(key_id)?;
        if key_node.is_map_key && store.value_for(key_id).ok()? == expected_key {
            return Some((key_id, value_id));
        }
        index += 2;
    }
    None
}

// ============================================================================
// ============================================================================

/// Compute the [`PathSpan`] for a given tree path.
///
pub fn compute_path_span(
    store: Option<&TreeStore>,
    cache_key: &str,
    language_name: &str,
    source_input: &str,
    path: &[PathSeg<'_>],
    prefer_key: bool,
) -> PathSpan {
    let entry = match get_current_tree_entry(store, cache_key, language_name, source_input) {
        Some(e) => e,
        None => return PathSpan::EMPTY,
    };
    let root = entry.root;
    let source = &entry.source;

    let store_ref = match store {
        Some(s) => s,
        None => return PathSpan::EMPTY,
    };
    let resolved_id = match find_node_by_path(root, path, prefer_key, store_ref) {
        Some(id) => id,
        None => return PathSpan::EMPTY,
    };
    let resolved = match store_ref.get(resolved_id) {
        Some(n) => n,
        None => return PathSpan::EMPTY,
    };

    if resolved.start_byte == 0
        && resolved.end_byte == 0
        && resolved.line <= 0
        && resolved.column <= 0
    {
        let owned_tree: Option<tree_sitter::Tree>;
        let tree = match &entry.ts_tree {
            Some(tree) => tree,
            None => {
                owned_tree = parse_structured_tree_for_call(language_name, source);
                let Some(tree) = owned_tree.as_ref() else {
                    return PathSpan::EMPTY;
                };
                tree
            }
        };
        if let Some(span) = recover_structured_path_span(
            tree.root_node(),
            language_name,
            source.as_bytes(),
            path,
            prefer_key,
        ) {
            return span;
        }
    }
    let (row, column) = if resolved.line <= 0 && resolved.column <= 0 {
        let index = LineIndex::build(source);
        let lc = index.offset_to_line_column(resolved.start_byte as usize);
        (lc.line as i32, lc.column as i32)
    } else {
        (
            std::cmp::max(resolved.line - 1, 0),
            std::cmp::max(resolved.column - 1, 0),
        )
    };

    PathSpan {
        start_byte: i32::try_from(resolved.start_byte).unwrap_or(-1),
        end_byte: i32::try_from(resolved.end_byte).unwrap_or(-1),
        row,
        column,
    }
}

/// Bulk/path-indexed resolver for snapshot path spans.
///
/// The resolver is bound to one snapshot tree/model pair. It lazily builds the
/// expensive pieces needed for repeated lookups:
/// - a `LineIndex` for byte → line/column recovery
/// - an optional structured fallback tree
/// - a whole-tree path index for node-id based bulk recovery

#[derive(Debug)]
pub struct PathSpanResolver<'a> {
    store: &'a TreeStore,
    root: NodeId,
    tree: Option<&'a tree_sitter::Tree>,
    language_name: &'a str,
    source: &'a str,
    valid: bool,
    line_index: Option<LineIndex>,
    structured_tree: Option<Option<tree_sitter::Tree>>,
    path_index: Option<TreePathIndex>,
    path_index_ref: Option<&'a crate::tree::TreePathIndex>,
}

impl<'a> PathSpanResolver<'a> {
    pub fn new(
        store: &'a TreeStore,
        root: NodeId,
        ts_tree: Option<&'a tree_sitter::Tree>,
        diagnostics_raw: &[u32],
        language_name: &'a str,
        source: &'a str,
    ) -> Self {
        let valid = diagnostics_raw.is_empty();
        Self {
            store,
            root,
            tree: ts_tree,
            language_name,
            source,
            valid,
            line_index: None,
            structured_tree: None,
            path_index: None,
            path_index_ref: None,
        }
    }

    pub fn resolve_nodes<I>(&mut self, nodes: I) -> Vec<(NodeId, PathSpan)>
    where
        I: IntoIterator<Item = (NodeId, bool)>,
    {
        nodes
            .into_iter()
            .map(|(node_id, prefer_key)| (node_id, self.resolve_node(node_id, prefer_key)))
            .collect()
    }

    pub fn resolve_path(&mut self, path: &[PathSeg<'_>], prefer_key: bool) -> PathSpan {
        if !self.valid {
            return PathSpan::EMPTY;
        }
        let resolved_id = match self.resolve_node_id_for_path(path, prefer_key) {
            Some(node_id) => node_id,
            None => return PathSpan::EMPTY,
        };
        self.resolve_node_inner(resolved_id, Some(path), prefer_key)
    }

    pub fn resolve_node(&mut self, node_id: NodeId, prefer_key: bool) -> PathSpan {
        if !self.valid {
            return PathSpan::EMPTY;
        }
        self.resolve_node_inner(node_id, None, prefer_key)
    }
    pub fn with_tree_path_index(mut self, index: &'a crate::tree::TreePathIndex) -> Self {
        self.path_index_ref = Some(index);
        self
    }
    fn resolve_node_id_for_path(
        &mut self,
        path: &[PathSeg<'_>],
        prefer_key: bool,
    ) -> Option<NodeId> {
        if let Some(index) = self.path_index_ref {
            if prefer_key
                && path
                    .last()
                    .is_some_and(|segment| segment.tag == PathSegTag::Key)
            {
                if let Some(node_id) = index.key_node_for_segments(path) {
                    return Some(node_id);
                }
            }
            if let Some(node_id) = index.value_node_for_segments(path) {
                return Some(node_id);
            }
        }
        if let Some(index) = self.path_index.as_ref() {
            if prefer_key
                && path
                    .last()
                    .is_some_and(|segment| segment.tag == PathSegTag::Key)
            {
                if let Some(node_id) = index.key_node_for_segments(path) {
                    return Some(node_id);
                }
            }
            if let Some(node_id) = index.value_node_for_segments(path) {
                return Some(node_id);
            }
        }
        find_node_by_path(self.root, path, prefer_key, self.store)
    }

    fn resolve_node_inner(
        &mut self,
        node_id: NodeId,
        path: Option<&[PathSeg<'_>]>,
        prefer_key: bool,
    ) -> PathSpan {
        let resolved = match self.store.get(node_id) {
            Some(node) => node,
            None => return PathSpan::EMPTY,
        };

        if resolved.start_byte == 0
            && resolved.end_byte == 0
            && resolved.line <= 0
            && resolved.column <= 0
        {
            if self.path_index.is_none() {
                self.path_index = Some(TreePathIndex::build(self.store, self.root));
            }
            let owned_path_buffer = if let Some(segments) = path {
                segments
                    .iter()
                    .map(|seg| match seg.tag {
                        PathSegTag::Key => OwnedPathSeg::Key(seg.key.to_owned()),
                        PathSegTag::Index => OwnedPathSeg::Index(seg.index),
                    })
                    .collect()
            } else {
                let Some(owned) = self
                    .path_index
                    .as_ref()
                    .expect("path index initialized above")
                    .owned_path_for_node(node_id)
                else {
                    return PathSpan::EMPTY;
                };
                owned.to_vec()
            };
            let path_buffer = borrowed_tree_path(&owned_path_buffer);
            let language_name = self.language_name;
            let source_bytes = self.source.as_bytes();
            let Some(tree) = self.ensure_structured_tree() else {
                return PathSpan::EMPTY;
            };
            if let Some(span) = recover_structured_path_span(
                tree.root_node(),
                language_name,
                source_bytes,
                &path_buffer,
                prefer_key,
            ) {
                return span;
            }
        }

        let (row, column) = if resolved.line <= 0 && resolved.column <= 0 {
            let line_index = self
                .line_index
                .get_or_insert_with(|| LineIndex::build(self.source));
            let lc = line_index.offset_to_line_column(resolved.start_byte as usize);
            (lc.line as i32, lc.column as i32)
        } else {
            (
                std::cmp::max(resolved.line - 1, 0),
                std::cmp::max(resolved.column - 1, 0),
            )
        };

        PathSpan {
            start_byte: i32::try_from(resolved.start_byte).unwrap_or(-1),
            end_byte: i32::try_from(resolved.end_byte).unwrap_or(-1),
            row,
            column,
        }
    }

    fn ensure_structured_tree(&mut self) -> Option<&tree_sitter::Tree> {
        if let Some(tree) = self.tree {
            if !tree_has_error(tree) {
                return Some(tree);
            }
        }
        if self.structured_tree.is_none() {
            self.structured_tree = Some(parse_structured_tree_for_call(
                self.language_name,
                self.source,
            ));
        }
        self.structured_tree.as_ref()?.as_ref()
    }
}
/// Resolve one snapshot path to a source span.
///
/// Complexity contract: a single call should stay bounded to one path walk,
/// plus at most one fallback structured parse or one cached `LineIndex`
/// construction when span metadata is missing. Bulk callers SHOULD reuse a
/// `PathSpanResolver` so the path index and structured fallback are built once.
pub fn compute_path_span_for_document(
    store: &TreeStore,
    root: NodeId,
    ts_tree: Option<&tree_sitter::Tree>,
    diagnostics_raw: &[u32],
    language_name: &str,
    source: &str,
    path: &[PathSeg<'_>],
    prefer_key: bool,
) -> PathSpan {
    let mut resolver =
        PathSpanResolver::new(store, root, ts_tree, diagnostics_raw, language_name, source);
    resolver.resolve_path(path, prefer_key)
}
/// Resolve one snapshot path to a source span, optionally using a pre-built
/// [`crate::tree::TreePathIndex`] to avoid building a fresh index or scanning
/// siblings linearly.
pub fn compute_path_span_for_document_with_index(
    store: &TreeStore,
    root: NodeId,
    ts_tree: Option<&tree_sitter::Tree>,
    diagnostics_raw: &[u32],
    language_name: &str,
    source: &str,
    path: &[PathSeg<'_>],
    prefer_key: bool,
    index: Option<&crate::tree::TreePathIndex>,
) -> PathSpan {
    let resolver =
        PathSpanResolver::new(store, root, ts_tree, diagnostics_raw, language_name, source);
    let mut resolver = match index {
        Some(index) => resolver.with_tree_path_index(index),
        None => resolver,
    };
    resolver.resolve_path(path, prefer_key)
}
