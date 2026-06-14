use std::io::Write;

use crate::core::{
    CoreError, EvalError, NodeId, ParseError, SemType, TreeNode, TreeNodeKind, TreeStore,
};

use super::node;

pub(crate) fn resolve_alias_for_encode(
    store: &TreeStore,
    node_id: NodeId,
) -> Result<Option<NodeId>, CoreError> {
    let mut slow = node_id;
    let mut fast = node_id;

    loop {
        let slow_node = node(store, slow)?;
        if slow_node.kind != crate::core::TreeNodeKind::Alias {
            return Ok(Some(slow));
        }
        let Some(next_slow) = slow_node.alias else {
            return Ok(None);
        };
        slow = next_slow;

        let fast_node = node(store, fast)?;
        if fast_node.kind != crate::core::TreeNodeKind::Alias {
            return Ok(Some(fast));
        }
        let Some(next_fast) = fast_node.alias else {
            return Ok(None);
        };
        fast = next_fast;

        let fast_node = node(store, fast)?;
        if fast_node.kind != crate::core::TreeNodeKind::Alias {
            return Ok(Some(fast));
        }
        let Some(next_fast) = fast_node.alias else {
            return Ok(None);
        };
        fast = next_fast;

        if fast == slow {
            return Ok(None);
        }
    }
}

pub(crate) fn write_indent(out: &mut String, depth: usize, indent: i32) {
    for _ in 0..depth * indent.max(0) as usize {
        out.push(' ');
    }
}

pub(crate) fn write_quoted_string(out: &mut String, value: &str, quote: char) {
    out.push(quote);
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' if quote == '\'' => out.push_str("\\'"),
            '"' if quote == '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            other if other.is_control() => out.push_str(&format!("\\u{:04x}", other as u32)),
            other => out.push(other),
        }
    }
    out.push(quote);
}

pub(crate) fn missing_tree_node_error() -> CoreError {
    CoreError::Eval(EvalError::MissingTreeNode)
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// YAML timestamp tag literal, used by decoders/encoders to recognise
/// timestamp scalars.
pub const TIMESTAMP_TAG: &str = "!!timestamp";

// ---------------------------------------------------------------------------
// ArenaDecoderState
// ---------------------------------------------------------------------------

/// Bump-allocator state for decoders that benefit from arena allocation
/// during a single `decode_str` call.  The arena is reset between calls so
/// that temporary nodes/strings do not leak across documents.
pub struct ArenaDecoderState {
    buf: Vec<u8>,
    pos: usize,
    inited: bool,
}

impl ArenaDecoderState {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            pos: 0,
            inited: false,
        }
    }

    /// Initialise (or re-initialise) the arena.  Previous allocations are
    /// discarded.
    pub fn init(&mut self) {
        self.buf.clear();
        self.pos = 0;
        self.inited = true;
    }

    /// Release all memory held by the arena.
    pub fn release(&mut self) {
        self.buf.clear();
        self.buf.shrink_to_fit();
        self.pos = 0;
        self.inited = false;
    }

    /// Reset the arena for a new decode pass – keeps the backing buffer but
    /// resets the bump pointer.
    pub fn reset(&mut self) {
        self.pos = 0;
        self.inited = true;
    }

    /// Allocate a byte-slice of `layout.size()` bytes with `layout.align()`
    pub fn alloc_bytes(&mut self, size: usize, align: usize) -> &mut [u8] {
        let offset = self.pos.next_multiple_of(align);
        let end = offset
            .checked_add(size)
            .expect("ArenaDecoderState overflow");
        if end > self.buf.len() {
            self.buf.resize(end, 0);
        }
        self.pos = end;
        &mut self.buf[offset..end]
    }

    /// Returns `true` once `init()` has been called at least once.
    pub fn is_inited(&self) -> bool {
        self.inited
    }
}

impl Default for ArenaDecoderState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// String tools
// ---------------------------------------------------------------------------

/// Trim bytes in `cutset` from the left of `s`.
/// Returns a sub-slice view – no allocation.
pub fn trim_left_bytes<'a>(s: &'a [u8], cutset: &[u8]) -> &'a [u8] {
    let mut i = 0;
    while i < s.len() && cutset.contains(&s[i]) {
        i += 1;
    }
    &s[i..]
}

/// Trim bytes in `cutset` from the right of `s`.
/// Returns a sub-slice view – no allocation.
pub fn trim_right_bytes<'a>(s: &'a [u8], cutset: &[u8]) -> &'a [u8] {
    let mut end = s.len();
    while end > 0 && cutset.contains(&s[end - 1]) {
        end -= 1;
    }
    &s[..end]
}

/// Write `s` into `out` surrounded by `quote_char`, escaping special
/// characters (`\\`, `\n`, `\r`, `\t`, backspace, form-feed, the quote
/// character itself, and control chars < 0x20).
pub fn write_escaped_string(out: &mut String, s: &str, quote_char: char) {
    out.push(quote_char);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            _ if ch == quote_char => {
                out.push('\\');
                out.push(quote_char);
            }
            _ if (ch as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", ch as u32));
            }
            _ => out.push(ch),
        }
    }
    out.push(quote_char);
}

/// Return the length of `s` after escaping (including the two surrounding
/// quote characters).  Does not allocate.
pub fn escaped_string_len(s: &str, quote_char: char) -> usize {
    let mut total: usize = 2; // opening + closing quote
    for ch in s.chars() {
        total += match ch {
            '\\' | '\n' | '\r' | '\t' | '\u{08}' | '\u{0c}' => 2,
            _ if ch == quote_char => 2,
            _ if (ch as u32) < 0x20 => 6, // \uXXXX
            _ => 1,
        };
    }
    total
}

// ---------------------------------------------------------------------------
// Node constructors
// ---------------------------------------------------------------------------

/// Allocate a new `TreeNode` with the given `kind` and `sem_type` tag.
pub fn new_node(kind: TreeNodeKind, tag: SemType) -> TreeNode {
    TreeNode {
        kind,
        sem_type: Some(tag),
        tag: tag.to_string(),
        ..TreeNode::default()
    }
}

/// Allocate a new scalar `TreeNode` with the given `tag` and `value`.
pub fn new_scalar(tag: SemType, value: impl Into<String>) -> TreeNode {
    TreeNode {
        kind: TreeNodeKind::Scalar,
        sem_type: Some(tag),
        tag: tag.to_string(),
        value: value.into(),
        ..TreeNode::default()
    }
}

/// Convenience: `new_node(TreeNodeKind::Mapping, SemType::Map)`.
pub fn new_map() -> TreeNode {
    new_node(TreeNodeKind::Mapping, SemType::Map)
}

/// Convenience: `new_node(TreeNodeKind::Sequence, SemType::Seq)`.
pub fn new_seq() -> TreeNode {
    new_node(TreeNodeKind::Sequence, SemType::Seq)
}

/// Append a key/value pair to a mapping node.  Sets `parent`, `is_map_key`,
/// and `key` linkage on the children, then pushes both ids into the map's
/// content list.
pub fn append_map_entry(
    store: &mut TreeStore,
    map_id: NodeId,
    key_node: TreeNode,
    value_node: TreeNode,
) -> Result<(NodeId, NodeId), CoreError> {
    let mut key = key_node;
    key.parent = Some(map_id);
    key.is_map_key = true;

    let mut value = value_node;
    value.parent = Some(map_id);
    value.is_map_key = false;

    let key_id = store.add(key);
    let value_id = store.add(value);

    // Set the back-link from value to its key.
    if let Some(v) = store.get_mut(value_id) {
        v.key = Some(key_id);
    }

    let map = store.get_mut(map_id).ok_or_else(missing_tree_node_error)?;
    map.content.push(key_id);
    map.content.push(value_id);

    Ok((key_id, value_id))
}

/// Append an item to a sequence node.  Sets `parent`, `sequence_index`, and
/// clears `is_map_key` / `key`.
pub fn append_seq_item_with_index_key(
    store: &mut TreeStore,
    seq_id: NodeId,
    item_node: TreeNode,
) -> Result<NodeId, CoreError> {
    let index = {
        let seq = store.get(seq_id).ok_or_else(missing_tree_node_error)?;
        seq.content.len() as i64
    };

    let mut item = item_node;
    item.parent = Some(seq_id);
    item.is_map_key = false;
    item.sequence_index = Some(index);
    item.key = None;

    let item_id = store.add(item);
    let seq = store.get_mut(seq_id).ok_or_else(missing_tree_node_error)?;
    seq.content.push(item_id);

    Ok(item_id)
}

// ---------------------------------------------------------------------------
// Tree-sitter helpers
// ---------------------------------------------------------------------------

/// Set `start_byte` / `end_byte` on a `TreeNode` from a tree-sitter node,
/// offset by `base_offset`.
pub fn set_node_range(node: &mut TreeNode, base_offset: usize, ts_node: tree_sitter::Node) {
    node.start_byte = (base_offset + ts_node.start_byte()) as u32;
    node.end_byte = (base_offset + ts_node.end_byte()) as u32;
}

/// Parse `source` with the given language.  Returns `InvalidSyntax` if the
/// root node contains an error.
pub fn ts_parse_checked(
    language: tree_sitter::Language,
    source: &[u8],
) -> Result<tree_sitter::Tree, CoreError> {
    crate::core::tree_sitter_support::ensure_tree_sitter_runtime();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    let tree = parser
        .parse(source, None)
        .ok_or(CoreError::Parse(ParseError::InvalidSyntax))?;
    if tree.root_node().has_error() {
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }
    Ok(tree)
}

/// Parse `source` with fail-fast behaviour: if a stable prefix already
/// contains a syntax error the function returns early without parsing the
/// remainder.
pub fn ts_parse_checked_fail_fast(
    language: tree_sitter::Language,
    source: &[u8],
) -> Result<tree_sitter::Tree, CoreError> {
    crate::core::tree_sitter_support::ensure_tree_sitter_runtime();
    reject_stable_prefix_syntax_error(language.clone(), source)?;

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    let tree = parser
        .parse(source, None)
        .ok_or(CoreError::Parse(ParseError::InvalidSyntax))?;
    if tree.root_node().has_error() {
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }
    Ok(tree)
}

const FAIL_FAST_PREFIX_LEN: usize = 4096;
const INCOMPLETE_PREFIX_ERROR_MARGIN: usize = 16;

fn reject_stable_prefix_syntax_error(
    language: tree_sitter::Language,
    source: &[u8],
) -> Result<(), CoreError> {
    crate::core::tree_sitter_support::ensure_tree_sitter_runtime();
    let prefix_len = source.len().min(FAIL_FAST_PREFIX_LEN);
    if prefix_len == source.len() {
        return Ok(());
    }

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    let prefix_tree = parser
        .parse(&source[..prefix_len], None)
        .ok_or(CoreError::Parse(ParseError::InvalidSyntax))?;

    let root = prefix_tree.root_node();
    if !root.has_error() {
        return Ok(());
    }
    if let Some(start_byte) = first_error_start_byte(root) {
        if start_byte + INCOMPLETE_PREFIX_ERROR_MARGIN < prefix_len {
            return Err(CoreError::Parse(ParseError::InvalidSyntax));
        }
    }
    Ok(())
}

fn first_error_start_byte(node: tree_sitter::Node) -> Option<usize> {
    if node.is_error() || node.is_missing() {
        return Some(node.start_byte());
    }
    for i in 0..node.child_count() {
        let Some(child) = node.child(i as u32) else {
            continue;
        };
        if false {
            continue;
        }
        if let Some(start_byte) = first_error_start_byte(child) {
            return Some(start_byte);
        }
    }
    None
}

/// Call `f(ctx, child)` for every *named* child of `node`.
pub fn ts_for_each_named_child<E>(
    node: tree_sitter::Node,
    ctx: &mut E,
    f: fn(&mut E, tree_sitter::Node) -> Result<(), CoreError>,
) -> Result<(), CoreError> {
    let n = node.named_child_count();
    for i in 0..n {
        if let Some(child) = node.named_child(i as u32) {
            f(ctx, child)?;
        }
    }
    Ok(())
}

/// From a tree-sitter root node, extract the "value" node, optionally
/// skipping children whose type equals `skip_type` (e.g. `"comment"` for
/// JSON).  If the root is not a `document` node it is returned as-is.
pub fn ts_root_value_node_skip_type<'a>(
    root: tree_sitter::Node<'a>,
    skip_type: &str,
) -> Result<tree_sitter::Node<'a>, CoreError> {
    if root.kind() != "document" {
        return Ok(root);
    }
    let n = root.named_child_count();
    for i in 0..n {
        if let Some(child) = root.named_child(i as u32) {
            if child.kind() == skip_type {
                continue;
            }
            return Ok(child);
        }
    }
    Err(CoreError::Parse(ParseError::InvalidSyntax))
}

/// Map a runtime node-type string to its index in a compile-time list of
/// type names.  Returns `None` when the type is not in the list.
pub fn ts_type_index(types: &[&str], node_type: &str) -> Option<usize> {
    types.iter().position(|&t| t == node_type)
}

/// Find the first *named* child of `node` whose kind matches one of the
/// given `types`.
pub fn ts_find_first_named_child_of_type<'a>(
    node: tree_sitter::Node<'a>,
    types: &[&str],
) -> Option<tree_sitter::Node<'a>> {
    let n = node.named_child_count();
    for i in 0..n {
        let child = node.named_child(i as u32)?;
        if types.contains(&child.kind()) {
            return Some(child);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Adapter factories
// ---------------------------------------------------------------------------

/// Create a `Decode` implementation from a plain function / closure.
///
/// any `Fn(&str) -> Result<DecodedDocument, CoreError>` into a type that
/// implements the `Decode` trait.
pub fn make_decoder<F>(f: F) -> impl super::Decode
where
    F: Fn(&str) -> Result<super::DecodedDocument, CoreError>,
{
    struct FnDecoder<F> {
        f: F,
    }

    impl<F> super::Decode for FnDecoder<F>
    where
        F: Fn(&str) -> Result<super::DecodedDocument, CoreError>,
    {
        fn decode_str(&self, input: &str) -> Result<super::DecodedDocument, CoreError> {
            (self.f)(input)
        }
    }

    FnDecoder { f }
}

/// Create an `Encode` implementation from a plain function / closure.
///
/// any `Fn(&TreeStore, NodeId, &mut dyn Write) -> Result<(), CoreError>`
/// into a type that implements the `Encode` trait.
pub fn make_encoder<F>(f: F) -> impl super::Encode
where
    F: Fn(&TreeStore, NodeId, &mut dyn Write) -> Result<(), CoreError>,
{
    struct FnEncoder<F> {
        f: F,
    }

    impl<F> super::Encode for FnEncoder<F>
    where
        F: Fn(&TreeStore, NodeId, &mut dyn Write) -> Result<(), CoreError>,
    {
        fn encode(
            &self,
            store: &TreeStore,
            node: NodeId,
            writer: &mut dyn Write,
        ) -> Result<(), CoreError> {
            (self.f)(store, node, writer)
        }
    }

    FnEncoder { f }
}
