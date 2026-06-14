use crate::core::{NodeId, SemType, TreeNodeKind, TreeStore};
use crate::formats::DecodedDocument;

/// Append a single byte as two lowercase hex digits.
fn append_hex_byte(out: &mut String, value: u8) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    out.push(DIGITS[(value >> 4) as usize] as char);
    out.push(DIGITS[(value & 0x0f) as usize] as char);
}

/// JSON-escape a string value and append it (with surrounding double quotes)
/// to `out`.
///
/// Handles: `"`, `\`, `\n`, `\r`, `\t`, `\b`, `\f`, and control characters
/// below 0x20 (encoded as `\u00XX`).
pub fn append_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            _ if (ch as u32) < 0x20 => {
                out.push_str("\\u00");
                append_hex_byte(out, ch as u8);
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
}

/// Write a scalar [`TreeNode`](crate::core::TreeNode) as a JSON value into
/// `out`.
///
/// Dispatches on the node's resolved semantic type:
/// - `Nil` writes `null`
/// - `Boolean` writes `true`/`false` when parseable, otherwise a JSON string
/// - `Int` writes the parsed integer, falling back to a JSON string
/// - `Float` writes the parsed float, falling back to a JSON string
/// - Everything else is written as a JSON string
pub fn write_scalar_json(_store: &TreeStore, node: &crate::core::TreeNode, out: &mut String) {
    let sem = node.resolved_sem_type().unwrap_or(SemType::Str);
    match sem {
        SemType::Nil => out.push_str("null"),
        SemType::Boolean => match crate::core::core_helpers::parse_bool(&node.value) {
            Some(true) => out.push_str("true"),
            Some(false) => out.push_str("false"),
            None => append_json_string(out, &node.value),
        },
        SemType::Int => match node.value.parse::<i64>() {
            Ok(parsed) => out.push_str(&parsed.to_string()),
            Err(_) => append_json_string(out, &node.value),
        },
        SemType::Float => match node.value.parse::<f64>() {
            Ok(parsed) => out.push_str(&parsed.to_string()),
            Err(_) => append_json_string(out, &node.value),
        },
        _ => append_json_string(out, &node.value),
    }
}

/// Recursively write a [`TreeNode`](crate::core::TreeNode) (identified by
/// `id` in `store`) as JSON into `out`.
///
/// - Sequences are written as `[...]`
/// - Mappings are written as `{...}` with JSON-string keys
/// - Scalars, aliases, and unknown nodes delegate to
///   [`write_scalar_json`]
///
/// Returns `None` when a node cannot be found in the store.
pub fn write_tree_node_json(store: &TreeStore, id: NodeId, out: &mut String) -> Option<()> {
    let node = store.get(id)?;
    match node.kind {
        TreeNodeKind::Sequence => {
            out.push('[');
            for (i, child) in node.content.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_tree_node_json(store, *child, out)?;
            }
            out.push(']');
        }
        TreeNodeKind::Mapping => {
            out.push('{');
            let mut first = true;
            for pair in node.content.chunks_exact(2) {
                let key_node = store.get(pair[0])?;
                if !first {
                    out.push(',');
                }
                first = false;
                append_json_string(out, &key_node.value);
                out.push(':');
                write_tree_node_json(store, pair[1], out)?;
            }
            out.push('}');
        }
        TreeNodeKind::Alias | TreeNodeKind::Scalar | TreeNodeKind::Unknown => {
            write_scalar_json(store, node, out);
        }
    }
    Some(())
}

/// Top-level entry point: encode a tree rooted at `root` in `store` as a
/// JSON string.
///
/// Returns `None` when the root node cannot be found.
pub fn encode_value_json(store: &TreeStore, root: NodeId) -> Option<String> {
    let mut out = String::new();
    write_tree_node_json(store, root, &mut out)?;
    Some(out)
}

/// Encode a [`DecodedDocument`]'s root as a normalised value JSON string.
///
/// This is the primary public API used by the WASM layer. It delegates to
/// [`encode_value_json`] with the document's store and root.
pub fn encode_document_value_json(document: &DecodedDocument) -> Option<String> {
    encode_value_json(&document.store, document.root)
}
