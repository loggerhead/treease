use crate::analysis::line_index::LineIndex;
use crate::analysis::span_index::StructuralSpanIndex;
use crate::tree::{NodeId, TreeNodeKind, TreeStore};

use serde::{Deserialize, Serialize};
use tree_sitter::Point;
use tsify::Tsify;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTextEdit {
    pub start_byte: u32,
    pub old_end_byte: u32,
    pub new_end_byte: u32,
    #[serde(rename = "text", alias = "replacement")]
    pub replacement: String,
}

pub fn edit_delta(edit: &DocumentTextEdit) -> i64 {
    edit.new_end_byte as i64 - edit.old_end_byte as i64
}

pub fn apply_delta(base: u32, delta: i64) -> Option<u32> {
    if delta >= 0 {
        base.checked_add(delta as u32)
    } else {
        base.checked_sub((-delta) as u32)
    }
}

pub fn replaced_byte_count(edit: &DocumentTextEdit) -> usize {
    let old_len = edit.old_end_byte.saturating_sub(edit.start_byte);
    let new_len = edit.new_end_byte.saturating_sub(edit.start_byte);
    old_len.max(new_len) as usize
}
pub fn edit_byte_range(source: &str, edit: &DocumentTextEdit) -> Option<(usize, usize)> {
    let start = edit.start_byte as usize;
    let old_end = edit.old_end_byte as usize;
    if start > old_end || old_end > source.len() {
        return None;
    }
    if !source.is_char_boundary(start) || !source.is_char_boundary(old_end) {
        return None;
    }
    Some((start, old_end))
}

pub fn point_for_offset(line_index: &LineIndex, offset: usize) -> Point {
    let line_column = line_index.offset_to_line_column(offset);
    Point::new(line_column.line as usize, line_column.column as usize)
}

pub fn point_after_replacement(start: Point, replacement: &str) -> Point {
    let mut row = start.row;
    let mut column = start.column;
    for byte in replacement.bytes() {
        if byte == b'\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    Point::new(row, column)
}

pub fn input_edit_for_source(
    source: &str,
    edit: &DocumentTextEdit,
) -> Option<tree_sitter::InputEdit> {
    let (start_byte, old_end_byte) = edit_byte_range(source, edit)?;
    let new_end_byte = edit.new_end_byte as usize;
    if new_end_byte != start_byte.checked_add(edit.replacement.len())? {
        return None;
    }

    let line_index = LineIndex::build(source);
    let start_position = point_for_offset(&line_index, start_byte);
    let old_end_position = point_for_offset(&line_index, old_end_byte);
    let new_end_position = point_after_replacement(start_position, &edit.replacement);

    Some(tree_sitter::InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position,
        old_end_position,
        new_end_position,
    })
}

pub fn apply_edits_to_tree_and_source(
    mut source_text: String,
    edits: &[DocumentTextEdit],
    tree: &mut tree_sitter::Tree,
) -> Option<String> {
    for edit in edits {
        let input_edit = input_edit_for_source(&source_text, edit)?;
        tree.edit(&input_edit);
        source_text = apply_edit_to_source(&source_text, edit);
    }
    Some(source_text)
}

pub fn apply_edit_to_source(source: &str, edit: &DocumentTextEdit) -> String {
    let start = edit.start_byte as usize;
    let old_end = edit.old_end_byte as usize;
    if start > source.len() || old_end > source.len() || start > old_end {
        return source.to_string();
    }

    let mut out = String::with_capacity(source.len() + edit.replacement.len());
    out.push_str(&source[..start]);
    out.push_str(&edit.replacement);
    out.push_str(&source[old_end..]);
    out
}

pub fn find_exact_node_for_edit(
    store: &TreeStore,
    root: NodeId,
    edit: &DocumentTextEdit,
    span_index: Option<&StructuralSpanIndex>,
) -> Option<NodeId> {
    if let Some(found) = span_index
        .and_then(|index| index.find_exact_scalar(edit.start_byte, edit.old_end_byte))
        .and_then(|candidate| match store.get(candidate) {
            Some(node)
                if node.kind == TreeNodeKind::Scalar
                    && node.start_byte == edit.start_byte
                    && node.end_byte == edit.old_end_byte =>
            {
                Some(candidate)
            }
            _ => None,
        })
    {
        return Some(found);
    }
    find_exact_node_for_edit_inner(store, root, edit)
}

pub fn find_affected_node_id_for_edit(
    store: &TreeStore,
    root: NodeId,
    edit: &DocumentTextEdit,
) -> Option<NodeId> {
    find_deepest_node_id_at_offset(store, root, edit.start_byte).or_else(|| {
        if edit.start_byte > 0 && edit.old_end_byte == edit.start_byte {
            find_deepest_node_id_at_offset(store, root, edit.start_byte - 1)
        } else {
            None
        }
    })
}

pub fn find_reparse_boundary_id(
    store: &TreeStore,
    affected: NodeId,
    root: NodeId,
) -> Option<NodeId> {
    if affected == root {
        return None;
    }
    let node = store.get(affected)?;
    match node.kind {
        TreeNodeKind::Mapping | TreeNodeKind::Sequence => Some(affected),
        TreeNodeKind::Scalar => node.parent,
        TreeNodeKind::Alias | TreeNodeKind::Unknown => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StructuralOffsetUpdate {
    pub adjusted: Vec<NodeId>,
}

pub fn collect_subtree_node_ids(store: &TreeStore, root: NodeId) -> Vec<NodeId> {
    let mut ids = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(node) = store.get(id) else {
            continue;
        };
        ids.push(id);
        stack.extend(node.content.iter().rev().copied());
    }
    ids
}

pub fn adjust_tree_store_offsets_from_collecting(
    store: &mut TreeStore,
    id: NodeId,
    pivot: u32,
    delta: i64,
    skip: Option<NodeId>,
) -> StructuralOffsetUpdate {
    let mut update = StructuralOffsetUpdate::default();
    adjust_tree_store_offsets_from_collecting_inner(store, id, pivot, delta, skip, &mut update);
    update
}

fn adjust_tree_store_offsets_from_collecting_inner(
    store: &mut TreeStore,
    id: NodeId,
    pivot: u32,
    delta: i64,
    skip: Option<NodeId>,
    update: &mut StructuralOffsetUpdate,
) {
    let Some(node) = store.get_mut(id) else {
        return;
    };
    if Some(id) == skip {
        return;
    }
    if node.end_byte < pivot {
        return;
    }

    let mut changed = false;
    if node.start_byte >= pivot {
        node.start_byte = apply_delta(node.start_byte, delta).unwrap_or(0);
        changed = true;
    }
    if node.end_byte >= pivot {
        node.end_byte = apply_delta(node.end_byte, delta).unwrap_or(0);
        changed = true;
    }
    let children = node.content.clone();
    if changed {
        update.adjusted.push(id);
    }
    for child in children {
        adjust_tree_store_offsets_from_collecting_inner(store, child, pivot, delta, skip, update);
    }
}
pub fn adjust_tree_store_offsets_from(
    store: &mut TreeStore,
    id: NodeId,
    pivot: u32,
    delta: i64,
    skip: Option<NodeId>,
) {
    let _ = adjust_tree_store_offsets_from_collecting(store, id, pivot, delta, skip);
}

pub fn recompute_tree_store_locations(store: &mut TreeStore, id: NodeId, line_index: &LineIndex) {
    recompute_tree_store_locations_inner(store, id, line_index, None);
}

pub fn recompute_tree_store_locations_with_span_index(
    store: &mut TreeStore,
    id: NodeId,
    line_index: &LineIndex,
    span_index: &mut StructuralSpanIndex,
) {
    recompute_tree_store_locations_inner(store, id, line_index, Some(span_index));
}

fn find_exact_node_for_edit_inner(
    store: &TreeStore,
    id: NodeId,
    edit: &DocumentTextEdit,
) -> Option<NodeId> {
    let node = store.get(id)?;
    for child in &node.content {
        if let Some(found) = find_exact_node_for_edit_inner(store, *child, edit) {
            return Some(found);
        }
    }
    (node.kind == TreeNodeKind::Scalar
        && node.start_byte == edit.start_byte
        && node.end_byte == edit.old_end_byte)
        .then_some(id)
}

fn find_deepest_node_id_at_offset(store: &TreeStore, id: NodeId, offset: u32) -> Option<NodeId> {
    let node = store.get(id)?;
    if offset < node.start_byte || offset >= node.end_byte {
        return None;
    }
    for child in &node.content {
        if let Some(found) = find_deepest_node_id_at_offset(store, *child, offset) {
            return Some(found);
        }
    }
    Some(id)
}

pub fn recompute_tree_store_locations_for(
    store: &mut TreeStore,
    ids: &[NodeId],
    line_index: &LineIndex,
    mut span_index: Option<&mut StructuralSpanIndex>,
) {
    for id in ids {
        let Some(node) = store.get_mut(*id) else {
            continue;
        };
        let position = line_index.offset_to_line_column(node.start_byte as usize);
        node.line = position.line as i32;
        node.column = position.column as i32;
        if let Some(index) = span_index.as_deref_mut() {
            if node.kind == TreeNodeKind::Scalar {
                index.insert_scalar(*id, node.start_byte, node.end_byte);
            }
        }
    }
}
fn recompute_tree_store_locations_inner(
    store: &mut TreeStore,
    id: NodeId,
    line_index: &LineIndex,
    mut span_index: Option<&mut StructuralSpanIndex>,
) {
    let children = {
        let Some(node) = store.get_mut(id) else {
            return;
        };
        let position = line_index.offset_to_line_column(node.start_byte as usize);
        node.line = position.line as i32;
        node.column = position.column as i32;
        if let Some(index) = span_index.as_deref_mut() {
            if node.kind == TreeNodeKind::Scalar {
                index.insert_scalar(id, node.start_byte, node.end_byte);
            }
        }
        node.content.clone()
    };
    for child in children {
        recompute_tree_store_locations_inner(store, child, line_index, span_index.as_deref_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::SemType;
    use crate::tree::TreeNode as CoreTreeNode;

    #[test]
    fn find_exact_node_for_edit_ignores_stale_span_index_entries() {
        let mut store = TreeStore::default();
        let mut root = CoreTreeNode::scalar(SemType::Int, "1");
        root.start_byte = 0;
        root.end_byte = 1;
        let root_id = store.add(root);

        let mut stale = CoreTreeNode::scalar(SemType::Int, "9");
        stale.start_byte = 5;
        stale.end_byte = 6;
        let stale_id = store.add(stale);

        let mut index = StructuralSpanIndex::default();
        index.insert_scalar(stale_id, 0, 1);

        let found = find_exact_node_for_edit(
            &store,
            root_id,
            &DocumentTextEdit {
                start_byte: 0,
                old_end_byte: 1,
                new_end_byte: 1,
                replacement: "2".to_owned(),
            },
            Some(&index),
        )
        .expect("fallback scan should still find the real scalar");

        assert_eq!(found, root_id);
    }

    #[test]
    fn adjust_offsets_collects_only_nodes_at_or_after_pivot() {
        let mut store = TreeStore::default();

        let mut root = CoreTreeNode::default();
        root.kind = TreeNodeKind::Mapping;
        root.start_byte = 0;
        root.end_byte = 30;
        let root_id = store.add(root);

        let mut before = CoreTreeNode::scalar(SemType::Str, "before");
        before.start_byte = 1;
        before.end_byte = 7;
        let before_id = store.add(before);

        let mut after = CoreTreeNode::scalar(SemType::Str, "after");
        after.start_byte = 20;
        after.end_byte = 27;
        let after_id = store.add(after);

        store.get_mut(root_id).unwrap().content = vec![before_id, after_id];

        let update = adjust_tree_store_offsets_from_collecting(&mut store, root_id, 10, 3, None);

        assert!(update.adjusted.contains(&root_id));
        assert!(!update.adjusted.contains(&before_id));
        assert!(update.adjusted.contains(&after_id));
        assert_eq!(store.get(before_id).unwrap().start_byte, 1);
        assert_eq!(store.get(after_id).unwrap().start_byte, 23);
    }

    #[test]
    fn recompute_locations_for_updates_only_requested_nodes_and_span_index() {
        let mut store = TreeStore::default();

        let mut root = CoreTreeNode::default();
        root.kind = TreeNodeKind::Mapping;
        root.start_byte = 0;
        root.end_byte = 18;
        root.line = 99;
        root.column = 99;
        let root_id = store.add(root);

        let mut value = CoreTreeNode::scalar(SemType::Str, "value");
        value.start_byte = 12;
        value.end_byte = 17;
        value.line = 99;
        value.column = 99;
        let value_id = store.add(value);

        store.get_mut(root_id).unwrap().content = vec![value_id];

        let line_index = LineIndex::build("root:\n  k: value\n");
        let mut span_index = StructuralSpanIndex::default();
        recompute_tree_store_locations_for(
            &mut store,
            &[value_id],
            &line_index,
            Some(&mut span_index),
        );

        assert_eq!(store.get(root_id).unwrap().line, 99);
        assert_eq!(store.get(value_id).unwrap().line, 1);
        assert_eq!(store.get(value_id).unwrap().column, 6);
        assert_eq!(span_index.find_exact_scalar(12, 17), Some(value_id));
    }
}
