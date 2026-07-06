use std::io::Write;

use crate::errors::{CoreError, EvalError};
use crate::language::SemType;
use crate::tree::{CompactTag, NodeId, TreeNode, TreeNodeKind, TreeStore};

pub trait Encode {
    fn encode(
        &self,
        store: &TreeStore,
        node: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError>;

    fn encode_evaluated_value(
        &self,
        _value: &crate::evaluator::Value,
        _writer: &mut dyn Write,
    ) -> Result<bool, CoreError> {
        Ok(false)
    }

    fn encode_to_string(&self, store: &TreeStore, node: NodeId) -> Result<String, CoreError> {
        let mut bytes = Vec::new();
        self.encode(store, node, &mut bytes)?;
        String::from_utf8(bytes)
            .map_err(|_| CoreError::System(crate::errors::SystemError::InvalidUtf8))
    }
}

pub(crate) fn print_yaml_document_separator(
    out: &mut String,
    line_ending: &str,
    print_doc_separators: bool,
) {
    if print_doc_separators {
        out.push_str("---");
        out.push_str(line_ending);
    }
}

pub(crate) fn print_yaml_leading_content(
    out: &mut String,
    content: &str,
    print_doc_separators: bool,
) {
    let mut last_line_had_newline = false;
    for segment in content.split_inclusive('\n') {
        let (line_body, line_ending) = if let Some(body) = segment.strip_suffix("\r\n") {
            last_line_had_newline = true;
            (body, "\r\n")
        } else if let Some(body) = segment.strip_suffix('\n') {
            last_line_had_newline = true;
            (body, "\n")
        } else {
            last_line_had_newline = false;
            (segment, "")
        };

        if line_body.contains("$DocSeparator$") {
            print_yaml_document_separator(
                out,
                if line_ending.is_empty() {
                    "\n"
                } else {
                    line_ending
                },
                print_doc_separators,
            );
            continue;
        }

        let trimmed_left = line_body.trim_start_matches([' ', '\t']);
        let is_comment_line = trimmed_left.starts_with('#');
        let is_directive_line = trimmed_left.starts_with('%');
        if !line_body.is_empty() && !is_comment_line && !is_directive_line {
            out.push_str("# ");
        }
        out.push_str(line_body);
        out.push_str(line_ending);
    }

    if !last_line_had_newline && !content.is_empty() {
        out.push('\n');
    }
}

pub(crate) fn missing_tree_node() -> CoreError {
    CoreError::Eval(EvalError::MissingTreeNode)
}

/// Recursively forces mapping even-index child nodes' sem_type/tag to `.str`.
/// Walks the tree depth-first; for mapping nodes, sets every even-indexed
/// child's sem_type to `Str` and tag to `!!str`.
pub fn map_keys_to_strings(store: &mut TreeStore, node_id: NodeId) -> Result<(), CoreError> {
    let (is_mapping, children): (bool, Vec<NodeId>) = {
        let node = store.get(node_id).ok_or_else(missing_tree_node)?;
        (node.kind == TreeNodeKind::Mapping, node.content.clone())
    };
    if is_mapping {
        for (i, child_id) in children.iter().copied().enumerate() {
            if i % 2 == 0 {
                if let Some(child) = store.get_mut(child_id) {
                    child.sem_type = Some(SemType::Str);
                    child.tag = CompactTag::from_sem_type(SemType::Str);
                }
            }
        }
    }
    for child_id in children {
        map_keys_to_strings(store, child_id)?;
    }
    Ok(())
}

pub(crate) fn node(store: &TreeStore, id: NodeId) -> Result<&TreeNode, CoreError> {
    store.get(id).ok_or_else(missing_tree_node)
}

pub(crate) fn scalar(sem_type: SemType, value: impl Into<String>) -> TreeNode {
    TreeNode::scalar(sem_type, value)
}

pub(crate) fn add_sequence(store: &mut TreeStore) -> NodeId {
    store.add(TreeNode {
        kind: TreeNodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: CompactTag::from_sem_type(SemType::Seq),
        ..TreeNode::default()
    })
}

pub(crate) fn add_mapping(store: &mut TreeStore) -> NodeId {
    store.add(TreeNode {
        kind: TreeNodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: CompactTag::from_sem_type(SemType::Map),
        ..TreeNode::default()
    })
}

pub(crate) fn add_scalar(
    store: &mut TreeStore,
    sem_type: SemType,
    value: impl Into<String>,
) -> NodeId {
    store.add(scalar(sem_type, value))
}

pub(crate) fn append_child(
    store: &mut TreeStore,
    parent: NodeId,
    child: NodeId,
) -> Result<(), CoreError> {
    let sequence_index = {
        let parent_node = node(store, parent)?;
        if parent_node.kind == TreeNodeKind::Sequence {
            Some(parent_node.content.len() as u32)
        } else {
            None
        }
    };
    let child_node = store.get_mut(child).ok_or_else(missing_tree_node)?;
    child_node.parent = Some(parent);
    child_node.set_sequence_index(sequence_index);
    store
        .get_mut(parent)
        .ok_or_else(missing_tree_node)?
        .content
        .push(child);
    Ok(())
}

pub(crate) fn append_key_value(
    store: &mut TreeStore,
    parent: NodeId,
    key: impl Into<String>,
    value: NodeId,
) -> Result<NodeId, CoreError> {
    let key_id = store.add(TreeNode {
        kind: TreeNodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: CompactTag::from_sem_type(SemType::Str),
        value: key.into().into(),
        parent: Some(parent),
        is_map_key: true,
        ..TreeNode::default()
    });
    let value_node = store.get_mut(value).ok_or_else(missing_tree_node)?;
    value_node.parent = Some(parent);
    value_node.set_key(Some(key_id));
    let parent_node = store.get_mut(parent).ok_or_else(missing_tree_node)?;
    parent_node.content.push(key_id);
    parent_node.content.push(value);
    Ok(key_id)
}

pub(crate) fn append_existing_key_value(
    store: &mut TreeStore,
    parent: NodeId,
    key_id: NodeId,
    value_id: NodeId,
) -> Result<(), CoreError> {
    {
        let key_node = store.get_mut(key_id).ok_or_else(missing_tree_node)?;
        key_node.parent = Some(parent);
        key_node.is_map_key = true;
        key_node.set_sequence_index(None);
    }
    {
        let value_node = store.get_mut(value_id).ok_or_else(missing_tree_node)?;
        value_node.parent = Some(parent);
        value_node.is_map_key = false;
        value_node.set_key(Some(key_id));
        value_node.set_sequence_index(None);
    }
    let parent_node = store.get_mut(parent).ok_or_else(missing_tree_node)?;
    parent_node.content.push(key_id);
    parent_node.content.push(value_id);
    Ok(())
}

pub(crate) fn escape_json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}

pub(crate) fn is_truthy_literal(value: &str) -> bool {
    value.eq_ignore_ascii_case("true")
        || value.eq_ignore_ascii_case("y")
        || value.eq_ignore_ascii_case("yes")
        || value.eq_ignore_ascii_case("on")
        || value == "1"
}
