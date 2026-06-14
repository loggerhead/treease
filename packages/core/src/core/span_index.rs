use std::collections::HashMap;

use super::{NodeId, TreeNodeKind, TreeStore};

#[derive(Debug, Clone, Default)]
pub struct StructuralSpanIndex {
    scalar_nodes: HashMap<u64, NodeId>,
}

impl StructuralSpanIndex {
    pub fn build(store: &TreeStore, root: NodeId) -> Self {
        let mut index = Self::default();
        index.extend_from_subtree(store, root);
        index
    }

    pub fn find_exact_scalar(&self, start_byte: u32, end_byte: u32) -> Option<NodeId> {
        self.scalar_nodes
            .get(&span_key(start_byte, end_byte))
            .copied()
    }

    pub fn insert_scalar(&mut self, id: NodeId, start_byte: u32, end_byte: u32) {
        self.scalar_nodes.insert(span_key(start_byte, end_byte), id);
    }

    pub fn extend_from_subtree(&mut self, store: &TreeStore, root: NodeId) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = store.get(id) else {
                continue;
            };
            if node.kind == TreeNodeKind::Scalar {
                self.insert_scalar(id, node.start_byte, node.end_byte);
            }
            stack.extend(node.content.iter().rev().copied());
        }
    }
}

fn span_key(start_byte: u32, end_byte: u32) -> u64 {
    ((start_byte as u64) << 32) | end_byte as u64
}
