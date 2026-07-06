use super::tree_node::{NodeId, ParsedKey, TreeNode, TreeNodeKind};
use super::tree_store::TreeStore;
use crate::errors::{CoreError, EvalError, ParseError};
use crate::language::SemType;

pub type PathElement = ParsedKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapEntry {
    pub key: NodeId,
    pub value: NodeId,
    pub key_index: usize,
}

pub fn create_scalar_node(sem_type: SemType, value: impl Into<String>) -> TreeNode {
    TreeNode::scalar(sem_type, value)
}

pub fn get_map_entry(
    store: &TreeStore,
    map: NodeId,
    wanted_key: &str,
) -> Result<Option<MapEntry>, CoreError> {
    let Some(map_node) = store.get(map) else {
        return Err(CoreError::Eval(EvalError::MissingTreeNode));
    };
    if map_node.kind != TreeNodeKind::Mapping {
        return Ok(None);
    }

    for (key_index, pair) in map_node.content.chunks_exact(2).enumerate() {
        let key_id = pair[0];
        let value_id = pair[1];
        if store.value_for(key_id).is_ok_and(|key| key == wanted_key) {
            return Ok(Some(MapEntry {
                key: key_id,
                value: value_id,
                key_index: key_index * 2,
            }));
        }
    }
    Ok(None)
}

pub fn ensure_map(store: &mut TreeStore, node: NodeId) -> Result<(), CoreError> {
    let Some(node) = store.get_mut(node) else {
        return Err(CoreError::Eval(EvalError::MissingTreeNode));
    };
    if node.kind == TreeNodeKind::Mapping {
        return Ok(());
    }
    node.kind = TreeNodeKind::Mapping;
    node.set_sem_type(SemType::Map);
    node.content.clear();
    node.value = Default::default();
    Ok(())
}

pub fn ensure_seq(store: &mut TreeStore, node: NodeId) -> Result<(), CoreError> {
    let Some(node) = store.get_mut(node) else {
        return Err(CoreError::Eval(EvalError::MissingTreeNode));
    };
    if node.kind == TreeNodeKind::Sequence {
        return Ok(());
    }
    node.kind = TreeNodeKind::Sequence;
    node.set_sem_type(SemType::Seq);
    node.content.clear();
    node.value = Default::default();
    Ok(())
}

pub fn ensure_seq_index(
    store: &mut TreeStore,
    seq: NodeId,
    index: i64,
) -> Result<NodeId, CoreError> {
    if index < 0 {
        return Err(ParseError::NegativeIndex.into());
    }
    ensure_seq(store, seq)?;
    let wanted = index as usize;
    while store
        .get(seq)
        .ok_or(CoreError::Eval(EvalError::MissingTreeNode))?
        .content
        .len()
        <= wanted
    {
        let null_node = create_scalar_node(SemType::Nil, "");
        store.add_child(seq, null_node)?;
    }
    store
        .get(seq)
        .and_then(|node| node.content.get(wanted).copied())
        .ok_or(CoreError::Eval(EvalError::MissingTreeNode))
}

pub fn get_or_create_map_value(
    store: &mut TreeStore,
    map: NodeId,
    key: &str,
) -> Result<NodeId, CoreError> {
    ensure_map(store, map)?;
    if let Some(entry) = get_map_entry(store, map, key)? {
        return Ok(entry.value);
    }
    let key_node = create_scalar_node(SemType::Str, key);
    let null_node = create_scalar_node(SemType::Nil, "");
    let (_, value_id) = store.add_key_value_child(map, key_node, null_node)?;
    Ok(value_id)
}
