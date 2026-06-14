use crate::core::{CoreError, NodeId, SemType, TreeNode, TreeNodeKind, TreeStore};

use super::encoder_javascript::{is_js_identifier, is_safe_integer_literal};
use super::formats_helpers::write_quoted_string;
use super::formats_helpers::{resolve_alias_for_encode, write_indent};
use super::preferences::FormatPreferences;
use super::{escape_json_string, node, scalar_json_text};

#[cfg(test)]
thread_local! {
    static NODE_ID_LOOKUP_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn reset_node_id_lookup_count() {
    NODE_ID_LOOKUP_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
fn node_id_lookup_count() -> usize {
    NODE_ID_LOOKUP_COUNT.with(|count| count.get())
}

trait SmartLayoutSyntax {
    fn null_literal() -> &'static str;
    fn inline_scalar_len(node: &TreeNode) -> Result<usize, CoreError>;
    fn write_scalar(out: &mut String, node: &TreeNode) -> Result<(), CoreError>;
    fn key_display_len(key: &TreeNode) -> usize;
    fn key_printed_len(key: &TreeNode) -> usize;
    fn write_key(out: &mut String, key: &TreeNode) -> Result<(), CoreError>;
}

struct JsonSyntax;

impl SmartLayoutSyntax for JsonSyntax {
    fn null_literal() -> &'static str {
        "null"
    }

    fn inline_scalar_len(node: &TreeNode) -> Result<usize, CoreError> {
        Ok(scalar_json_text(node)?.len())
    }

    fn write_scalar(out: &mut String, node: &TreeNode) -> Result<(), CoreError> {
        out.push_str(&scalar_json_text(node)?);
        Ok(())
    }

    fn key_display_len(key: &TreeNode) -> usize {
        escape_json_string(&key.value).len().saturating_sub(2)
    }

    fn key_printed_len(key: &TreeNode) -> usize {
        escape_json_string(&key.value).len()
    }

    fn write_key(out: &mut String, key: &TreeNode) -> Result<(), CoreError> {
        out.push_str(&escape_json_string(&key.value));
        Ok(())
    }
}

struct PythonSyntax;

impl SmartLayoutSyntax for PythonSyntax {
    fn null_literal() -> &'static str {
        "None"
    }

    fn inline_scalar_len(node: &TreeNode) -> Result<usize, CoreError> {
        Ok(match node.resolved_sem_type() {
            Some(SemType::Nil) => 4,
            Some(SemType::Boolean) => {
                if is_truthy_bool_literal(&node.value) {
                    4
                } else {
                    5
                }
            }
            Some(SemType::Int | SemType::Float) => node.value.len(),
            _ => quoted_len(&node.value, '\''),
        })
    }

    fn write_scalar(out: &mut String, node: &TreeNode) -> Result<(), CoreError> {
        match node.resolved_sem_type() {
            Some(SemType::Nil) => out.push_str("None"),
            Some(SemType::Boolean) => out.push_str(if is_truthy_bool_literal(&node.value) {
                "True"
            } else {
                "False"
            }),
            Some(SemType::Int | SemType::Float) => out.push_str(&node.value),
            None => out.push_str("None"),
            _ => write_quoted_string(out, &node.value, '\''),
        }
        Ok(())
    }

    fn key_display_len(key: &TreeNode) -> usize {
        match key.resolved_sem_type() {
            Some(SemType::Nil) => 4,
            Some(SemType::Boolean) => {
                if is_truthy_bool_literal(&key.value) {
                    4
                } else {
                    5
                }
            }
            Some(SemType::Int | SemType::Float) => key.value.len(),
            _ => quoted_len(&key.value, '\''),
        }
    }

    fn key_printed_len(key: &TreeNode) -> usize {
        Self::key_display_len(key)
    }

    fn write_key(out: &mut String, key: &TreeNode) -> Result<(), CoreError> {
        Self::write_scalar(out, key)
    }
}

struct JavascriptSyntax;

impl SmartLayoutSyntax for JavascriptSyntax {
    fn null_literal() -> &'static str {
        "null"
    }

    fn inline_scalar_len(node: &TreeNode) -> Result<usize, CoreError> {
        Ok(match node.resolved_sem_type() {
            Some(SemType::Nil) => 4,
            Some(SemType::Boolean) => {
                if is_truthy_bool_literal(&node.value) {
                    4
                } else {
                    5
                }
            }
            Some(SemType::Int) => {
                if is_safe_integer_literal(&node.value) {
                    node.value.len()
                } else {
                    quoted_len(&node.value, '\'')
                }
            }
            Some(SemType::Float) => node.value.len(),
            _ => quoted_len(&node.value, '\''),
        })
    }

    fn write_scalar(out: &mut String, node: &TreeNode) -> Result<(), CoreError> {
        match node.resolved_sem_type() {
            Some(SemType::Nil) => out.push_str("null"),
            Some(SemType::Boolean) => out.push_str(if is_truthy_bool_literal(&node.value) {
                "true"
            } else {
                "false"
            }),
            Some(SemType::Int) => {
                if is_safe_integer_literal(&node.value) {
                    out.push_str(&node.value);
                } else {
                    write_quoted_string(out, &node.value, '\'');
                }
            }
            Some(SemType::Float) => out.push_str(&node.value),
            None => out.push_str("null"),
            _ => write_quoted_string(out, &node.value, '\''),
        }
        Ok(())
    }

    fn key_display_len(key: &TreeNode) -> usize {
        if is_js_identifier(&key.value) {
            key.value.len()
        } else {
            quoted_len(&key.value, '\'')
        }
    }

    fn key_printed_len(key: &TreeNode) -> usize {
        Self::key_display_len(key)
    }

    fn write_key(out: &mut String, key: &TreeNode) -> Result<(), CoreError> {
        if is_js_identifier(&key.value) {
            out.push_str(&key.value);
        } else {
            write_quoted_string(out, &key.value, '\'');
        }
        Ok(())
    }
}

pub(crate) fn encode_json_node_smart(
    store: &TreeStore,
    node_id: NodeId,
    prefs: &FormatPreferences,
    depth: usize,
    out: &mut String,
) -> Result<(), CoreError> {
    encode_node_smart::<JsonSyntax>(store, node_id, prefs, depth, out)
}

pub(crate) fn encode_python_node_smart(
    store: &TreeStore,
    node_id: NodeId,
    prefs: &FormatPreferences,
    depth: usize,
    out: &mut String,
) -> Result<(), CoreError> {
    encode_node_smart::<PythonSyntax>(store, node_id, prefs, depth, out)
}

pub(crate) fn encode_javascript_node_smart(
    store: &TreeStore,
    node_id: NodeId,
    prefs: &FormatPreferences,
    depth: usize,
    out: &mut String,
) -> Result<(), CoreError> {
    encode_node_smart::<JavascriptSyntax>(store, node_id, prefs, depth, out)
}

fn encode_node_smart<S: SmartLayoutSyntax>(
    store: &TreeStore,
    node_id: NodeId,
    prefs: &FormatPreferences,
    depth: usize,
    out: &mut String,
) -> Result<(), CoreError> {
    let Some(resolved_id) = resolve_alias_for_encode(store, node_id)? else {
        out.push_str(S::null_literal());
        return Ok(());
    };
    let current = node(store, resolved_id)?;
    match current.kind {
        TreeNodeKind::Scalar => S::write_scalar(out, current)?,
        TreeNodeKind::Alias | TreeNodeKind::Unknown => out.push_str(S::null_literal()),
        TreeNodeKind::Sequence => {
            encode_sequence::<S>(store, resolved_id, current, prefs, depth, out)?
        }
        TreeNodeKind::Mapping => {
            encode_mapping::<S>(store, resolved_id, current, prefs, depth, out)?
        }
    }
    Ok(())
}
fn encode_sequence<S: SmartLayoutSyntax>(
    store: &TreeStore,
    node_id: NodeId,
    current: &TreeNode,
    prefs: &FormatPreferences,
    depth: usize,
    out: &mut String,
) -> Result<(), CoreError> {
    let count = current.content.len();
    if count == 0 {
        out.push_str("[]");
        return Ok(());
    }

    let can_inline = can_inline_node::<S>(store, node_id, prefs, depth)?
        && !(prefs.max_array_inline_items > 0 && count > prefs.max_array_inline_items as usize);
    if can_inline {
        write_inline_node::<S>(store, current, out)?;
        return Ok(());
    }

    if prefs.align_object_arrays {
        if let Some(info) = aligned_object_array_info::<S>(store, &current.content, prefs)? {
            let mut max_line = 0;
            for item_id in &current.content {
                let Some(resolved_id) = resolve_alias_for_encode(store, *item_id)? else {
                    continue;
                };
                let item = node(store, resolved_id)?;
                let line_len =
                    aligned_object_line_len::<S>(store, item, info.pair_count, info.max_key_len)?;
                max_line = max_line.max(line_len);
            }
            if max_line + depth * prefs.indent.max(0) as usize
                <= prefs.max_line_length.max(0) as usize
            {
                out.push('[');
                out.push('\n');
                for (index, item_id) in current.content.iter().enumerate() {
                    let Some(resolved_id) = resolve_alias_for_encode(store, *item_id)? else {
                        continue;
                    };
                    let item = node(store, resolved_id)?;
                    write_indent(out, depth + 1, prefs.indent);
                    write_aligned_object::<S>(store, item, info.pair_count, info.max_key_len, out)?;
                    if index + 1 < count {
                        out.push(',');
                    }
                    out.push('\n');
                }
                write_indent(out, depth, prefs.indent);
                out.push(']');
                return Ok(());
            }
        }
    }

    let all_scalar = current
        .content
        .iter()
        .all(|child| is_scalar_node(store, *child).unwrap_or(false));
    if all_scalar && prefs.max_array_inline_items > 1 {
        out.push('[');
        out.push('\n');
        write_indent(out, depth + 1, prefs.indent);
        let mut line_len = (depth + 1) * prefs.indent.max(0) as usize;
        let mut items_in_line = 0usize;
        for (index, child_id) in current.content.iter().enumerate() {
            let child_len = inline_node_len::<S>(store, *child_id)?;
            if index == 0 {
                write_inline_node_id::<S>(store, *child_id, out)?;
                line_len += child_len;
                items_in_line = 1;
                continue;
            }

            let need_break = items_in_line >= prefs.max_array_inline_items as usize
                || line_len + 2 + child_len > prefs.max_line_length.max(0) as usize;
            if need_break {
                out.push(',');
                out.push('\n');
                write_indent(out, depth + 1, prefs.indent);
                line_len = (depth + 1) * prefs.indent.max(0) as usize;
                write_inline_node_id::<S>(store, *child_id, out)?;
                line_len += child_len;
                items_in_line = 1;
            } else {
                out.push_str(", ");
                write_inline_node_id::<S>(store, *child_id, out)?;
                line_len += 2 + child_len;
                items_in_line += 1;
            }
        }
        out.push('\n');
        write_indent(out, depth, prefs.indent);
        out.push(']');
        return Ok(());
    }

    out.push('[');
    out.push('\n');
    for (index, child_id) in current.content.iter().enumerate() {
        write_indent(out, depth + 1, prefs.indent);
        encode_node_smart::<S>(store, *child_id, prefs, depth + 1, out)?;
        if index + 1 < count {
            out.push(',');
        }
        out.push('\n');
    }
    write_indent(out, depth, prefs.indent);
    out.push(']');
    Ok(())
}

fn encode_mapping<S: SmartLayoutSyntax>(
    store: &TreeStore,
    node_id: NodeId,
    current: &TreeNode,
    prefs: &FormatPreferences,
    depth: usize,
    out: &mut String,
) -> Result<(), CoreError> {
    let pair_count = current.content.len() / 2;
    if pair_count == 0 {
        out.push_str("{}");
        return Ok(());
    }

    if can_inline_node::<S>(store, node_id, prefs, depth)? {
        write_inline_node::<S>(store, current, out)?;
        return Ok(());
    }

    out.push('{');
    out.push('\n');
    for (pair_index, pair) in current.content.chunks_exact(2).enumerate() {
        let key = node(store, pair[0])?;
        write_indent(out, depth + 1, prefs.indent);
        S::write_key(out, key)?;
        out.push(':');
        out.push(' ');
        encode_node_smart::<S>(store, pair[1], prefs, depth + 1, out)?;
        if pair_index + 1 < pair_count {
            out.push(',');
        }
        out.push('\n');
    }
    write_indent(out, depth, prefs.indent);
    out.push('}');
    Ok(())
}

fn inline_complexity_exceeds<S: SmartLayoutSyntax>(
    store: &TreeStore,
    node_id: NodeId,
    max: usize,
    count: &mut usize,
) -> Result<bool, CoreError> {
    let Some(resolved_id) = resolve_alias_for_encode(store, node_id)? else {
        return Ok(false);
    };
    let resolved = node(store, resolved_id)?;
    match resolved.kind {
        TreeNodeKind::Scalar | TreeNodeKind::Alias | TreeNodeKind::Unknown => Ok(false),
        TreeNodeKind::Sequence => {
            *count += 1;
            if *count > max {
                return Ok(true);
            }
            for child in &resolved.content {
                if inline_complexity_exceeds::<S>(store, *child, max, count)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        TreeNodeKind::Mapping => {
            *count += 1;
            if *count > max {
                return Ok(true);
            }
            for pair in resolved.content.chunks_exact(2) {
                if inline_complexity_exceeds::<S>(store, pair[1], max, count)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

fn inline_node_len<S: SmartLayoutSyntax>(
    store: &TreeStore,
    node_id: NodeId,
) -> Result<usize, CoreError> {
    let Some(resolved_id) = resolve_alias_for_encode(store, node_id)? else {
        return Ok(S::null_literal().len());
    };
    let resolved = node(store, resolved_id)?;
    match resolved.kind {
        TreeNodeKind::Scalar => S::inline_scalar_len(resolved),
        TreeNodeKind::Alias | TreeNodeKind::Unknown => Ok(S::null_literal().len()),
        TreeNodeKind::Sequence => {
            if resolved.content.is_empty() {
                return Ok(2);
            }
            let mut total = 2;
            for (index, child) in resolved.content.iter().enumerate() {
                total += inline_node_len::<S>(store, *child)?;
                if index + 1 < resolved.content.len() {
                    total += 2;
                }
            }
            Ok(total)
        }
        TreeNodeKind::Mapping => {
            let pair_count = resolved.content.len() / 2;
            if pair_count == 0 {
                return Ok(2);
            }
            let mut total = 2;
            for (index, pair) in resolved.content.chunks_exact(2).enumerate() {
                let key = node(store, pair[0])?;
                total += S::key_printed_len(key);
                total += 2;
                total += inline_node_len::<S>(store, pair[1])?;
                if index + 1 < pair_count {
                    total += 2;
                }
            }
            Ok(total)
        }
    }
}

fn write_inline_node_id<S: SmartLayoutSyntax>(
    store: &TreeStore,
    node_id: NodeId,
    out: &mut String,
) -> Result<(), CoreError> {
    let Some(resolved_id) = resolve_alias_for_encode(store, node_id)? else {
        out.push_str(S::null_literal());
        return Ok(());
    };
    let resolved = node(store, resolved_id)?;
    write_inline_node::<S>(store, resolved, out)
}

fn write_inline_node<S: SmartLayoutSyntax>(
    store: &TreeStore,
    resolved: &TreeNode,
    out: &mut String,
) -> Result<(), CoreError> {
    match resolved.kind {
        TreeNodeKind::Scalar => S::write_scalar(out, resolved),
        TreeNodeKind::Alias | TreeNodeKind::Unknown => {
            out.push_str(S::null_literal());
            Ok(())
        }
        TreeNodeKind::Sequence => {
            out.push('[');
            for (index, child) in resolved.content.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_inline_node_id::<S>(store, *child, out)?;
            }
            out.push(']');
            Ok(())
        }
        TreeNodeKind::Mapping => {
            out.push('{');
            for (index, pair) in resolved.content.chunks_exact(2).enumerate() {
                let key = node(store, pair[0])?;
                if index > 0 {
                    out.push_str(", ");
                }
                S::write_key(out, key)?;
                out.push(':');
                out.push(' ');
                write_inline_node_id::<S>(store, pair[1], out)?;
            }
            out.push('}');
            Ok(())
        }
    }
}

fn can_inline_node<S: SmartLayoutSyntax>(
    store: &TreeStore,
    node_id: NodeId,
    prefs: &FormatPreferences,
    depth: usize,
) -> Result<bool, CoreError> {
    let mut count = 0usize;
    if inline_complexity_exceeds::<S>(
        store,
        node_id,
        prefs.max_inline_complexity.max(0) as usize,
        &mut count,
    )? {
        return Ok(false);
    }
    let len = inline_node_len::<S>(store, node_id)?;
    Ok(len + depth * prefs.indent.max(0) as usize <= prefs.max_line_length.max(0) as usize)
}

struct AlignedObjectInfo {
    pair_count: usize,
    max_key_len: usize,
}

fn aligned_object_array_info<S: SmartLayoutSyntax>(
    store: &TreeStore,
    items: &[NodeId],
    prefs: &FormatPreferences,
) -> Result<Option<AlignedObjectInfo>, CoreError> {
    if items.len() < 2 {
        return Ok(None);
    }
    let Some(first_id) = resolve_alias_for_encode(store, items[0])? else {
        return Ok(None);
    };
    let first = node(store, first_id)?;
    if first.kind != TreeNodeKind::Mapping {
        return Ok(None);
    }

    let pair_count = first.content.len() / 2;
    let mut max_key_len = 0usize;
    for pair in first.content.chunks_exact(2) {
        let key = node(store, pair[0])?;
        max_key_len = max_key_len.max(S::key_display_len(key));
    }

    for item_id in items {
        let Some(resolved_id) = resolve_alias_for_encode(store, *item_id)? else {
            return Ok(None);
        };
        let current = node(store, resolved_id)?;
        if current.kind != TreeNodeKind::Mapping || current.content.len() / 2 != pair_count {
            return Ok(None);
        }
        for (index, pair) in current.content.chunks_exact(2).enumerate() {
            let expected_pair = &first.content[index * 2..index * 2 + 2];
            let expected_key = node(store, expected_pair[0])?;
            let key = node(store, pair[0])?;
            if key.value != expected_key.value
                || key.resolved_sem_type() != expected_key.resolved_sem_type()
            {
                return Ok(None);
            }
            max_key_len = max_key_len.max(S::key_display_len(key));
            if !can_inline_node::<S>(store, pair[1], prefs, 0)? {
                return Ok(None);
            }
        }
    }

    Ok(Some(AlignedObjectInfo {
        pair_count,
        max_key_len,
    }))
}

fn aligned_value_spacing(
    store: &TreeStore,
    val_node_id: NodeId,
    key_padding: usize,
) -> Result<usize, CoreError> {
    let mut spacing = key_padding + 1;
    let Some(resolved_id) = resolve_alias_for_encode(store, val_node_id)? else {
        return Ok(spacing);
    };
    let val_node = node(store, resolved_id)?;
    if matches!(
        val_node.resolved_sem_type(),
        Some(SemType::Int | SemType::Float)
    ) {
        let len = val_node.value.len();
        if len > 1 && spacing > 1 {
            spacing -= (spacing - 1).min(len - 1);
        }
    }
    Ok(spacing)
}

fn aligned_object_line_len<S: SmartLayoutSyntax>(
    store: &TreeStore,
    mapping: &TreeNode,
    pair_count: usize,
    max_key_len: usize,
) -> Result<usize, CoreError> {
    let mut total = 2usize;
    for index in 0..pair_count {
        if index > 0 {
            total += 2;
        }
        let key = node(store, mapping.content[index * 2])?;
        let key_display_len = S::key_display_len(key);
        let key_printed_len = S::key_printed_len(key);
        let padding = max_key_len.saturating_sub(key_display_len);
        total += key_printed_len
            + 1
            + aligned_value_spacing(store, mapping.content[index * 2 + 1], padding)?;
        total += inline_node_len::<S>(store, mapping.content[index * 2 + 1])?;
    }
    Ok(total)
}

fn write_aligned_object<S: SmartLayoutSyntax>(
    store: &TreeStore,
    mapping: &TreeNode,
    pair_count: usize,
    max_key_len: usize,
    out: &mut String,
) -> Result<(), CoreError> {
    out.push('{');
    for index in 0..pair_count {
        if index > 0 {
            out.push_str(", ");
        }
        let key = node(store, mapping.content[index * 2])?;
        let padding = max_key_len.saturating_sub(S::key_display_len(key));
        S::write_key(out, key)?;
        out.push(':');
        for _ in 0..aligned_value_spacing(store, mapping.content[index * 2 + 1], padding)? {
            out.push(' ');
        }
        write_inline_node_id::<S>(store, mapping.content[index * 2 + 1], out)?;
    }
    out.push('}');
    Ok(())
}

fn is_scalar_node(store: &TreeStore, node_id: NodeId) -> Result<bool, CoreError> {
    let Some(resolved_id) = resolve_alias_for_encode(store, node_id)? else {
        return Ok(true);
    };
    Ok(node(store, resolved_id)?.kind == TreeNodeKind::Scalar)
}

fn is_truthy_bool_literal(value: &str) -> bool {
    value.eq_ignore_ascii_case("true")
        || value.eq_ignore_ascii_case("y")
        || value.eq_ignore_ascii_case("yes")
        || value.eq_ignore_ascii_case("on")
        || value == "1"
}

fn quoted_len(value: &str, quote: char) -> usize {
    let mut out = String::new();
    write_quoted_string(&mut out, value, quote);
    out.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_smart_layout_does_not_scan_store_to_recover_node_ids() {
        let mut store = TreeStore::new();
        let root = store.add(TreeNode {
            kind: TreeNodeKind::Sequence,
            tag: "!!seq".to_owned(),
            ..TreeNode::default()
        });
        for index in 0..64 {
            let mapping = store.add(TreeNode {
                kind: TreeNodeKind::Mapping,
                tag: "!!map".to_owned(),
                ..TreeNode::default()
            });
            store
                .add_key_value_child(
                    mapping,
                    TreeNode::scalar(SemType::Str, "id"),
                    TreeNode::scalar(SemType::Int, &index.to_string()),
                )
                .expect("mapping accepts id");
            store
                .add_child(root, store.get(mapping).expect("mapping exists").clone())
                .expect("sequence accepts mapping");
        }

        let mut prefs = FormatPreferences::base();
        prefs.indent = 2;
        prefs.smart = true;
        prefs.max_line_length = 120;
        prefs.max_inline_complexity = 2;
        prefs.max_array_inline_items = 6;
        prefs.align_object_arrays = true;

        reset_node_id_lookup_count();
        let mut out = String::new();
        encode_json_node_smart(&store, root, &prefs, 0, &mut out).expect("smart encode");

        assert_eq!(
            node_id_lookup_count(),
            0,
            "smart layout should carry NodeId through recursion instead of scanning the store"
        );
    }
}
