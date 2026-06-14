use std::collections::HashMap;

use crate::core::{NodeId, ParsedKey, TreeStore};
use crate::wasm_types::{PathSeg, PathSegTag};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OwnedPathSeg {
    Key(String),
    Index(i32),
}

impl OwnedPathSeg {
    pub fn borrowed(&self) -> PathSeg<'_> {
        match self {
            Self::Key(key) => PathSeg {
                tag: PathSegTag::Key,
                key,
                index: 0,
            },
            Self::Index(index) => PathSeg {
                tag: PathSegTag::Index,
                key: "",
                index: *index,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathLookup<'a> {
    Key(&'a str),
    Index(i32),
}

impl<'a> PathLookup<'a> {
    pub const fn key(key: &'a str) -> Self {
        Self::Key(key)
    }

    pub const fn index(index: i32) -> Self {
        Self::Index(index)
    }
}
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TreePathIndex {
    layer: Arc<TreePathIndexLayer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TreePathIndexLayer {
    Owned(TreePathIndexOwned),
    Overlay(TreePathIndexOverlay),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TreePathIndexOwned {
    node_paths: Vec<Option<Vec<OwnedPathSeg>>>,
    value_nodes_by_path: HashMap<Vec<OwnedPathSeg>, NodeId>,
    key_nodes_by_path: HashMap<Vec<OwnedPathSeg>, NodeId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TreePathIndexPatch {
    node_paths: HashMap<usize, Option<Vec<OwnedPathSeg>>>,
    value_nodes_by_path: HashMap<Vec<OwnedPathSeg>, Option<NodeId>>,
    key_nodes_by_path: HashMap<Vec<OwnedPathSeg>, Option<NodeId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TreePathIndexOverlay {
    base: TreePathIndex,
    patch: TreePathIndexPatch,
}

#[derive(Debug, Clone, Copy)]
pub struct TreePathIndexStructuralUpdate<'a> {
    pub changed_root: NodeId,
    pub removed_node_ids: &'a [NodeId],
}

impl Default for TreePathIndex {
    fn default() -> Self {
        Self {
            layer: Arc::new(TreePathIndexLayer::Owned(TreePathIndexOwned::default())),
        }
    }
}

impl PartialEq for TreePathIndex {
    fn eq(&self, other: &Self) -> bool {
        self.flatten_owned() == other.flatten_owned()
    }
}

impl Eq for TreePathIndex {}

impl TreePathIndex {
    pub fn build(store: &TreeStore, root: NodeId) -> Self {
        let mut owned = TreePathIndexOwned {
            node_paths: vec![None; store.len()],
            value_nodes_by_path: HashMap::new(),
            key_nodes_by_path: HashMap::new(),
        };
        let mut current = Vec::new();
        populate_tree_path_index_owned(store, root, &mut current, &mut owned);
        Self {
            layer: Arc::new(TreePathIndexLayer::Owned(owned)),
        }
    }

    pub fn path(&self, path: &[PathLookup<'_>]) -> Vec<OwnedPathSeg> {
        path.iter()
            .map(|segment| match segment {
                PathLookup::Key(key) => OwnedPathSeg::Key((*key).to_owned()),
                PathLookup::Index(index) => OwnedPathSeg::Index(*index),
            })
            .collect()
    }

    pub fn owned_path_from_segments(path: &[PathSeg<'_>]) -> Vec<OwnedPathSeg> {
        path.iter()
            .map(|segment| match segment.tag {
                PathSegTag::Key => OwnedPathSeg::Key(segment.key.to_owned()),
                PathSegTag::Index => OwnedPathSeg::Index(segment.index),
            })
            .collect()
    }

    pub fn value_node_for_segments(&self, path: &[PathSeg<'_>]) -> Option<NodeId> {
        let owned = Self::owned_path_from_segments(path);
        self.value_node(&owned)
    }

    pub fn key_node_for_segments(&self, path: &[PathSeg<'_>]) -> Option<NodeId> {
        let owned = Self::owned_path_from_segments(path);
        self.key_node(&owned)
    }

    pub fn value_node(&self, path: &[OwnedPathSeg]) -> Option<NodeId> {
        match self.layer.as_ref() {
            TreePathIndexLayer::Owned(owned) => owned.value_nodes_by_path.get(path).copied(),
            TreePathIndexLayer::Overlay(overlay) => overlay
                .patch
                .value_nodes_by_path
                .get(path)
                .copied()
                .flatten()
                .or_else(|| {
                    if overlay.patch.value_nodes_by_path.contains_key(path) {
                        None
                    } else {
                        overlay.base.value_node(path)
                    }
                }),
        }
    }

    pub fn key_node(&self, path: &[OwnedPathSeg]) -> Option<NodeId> {
        match self.layer.as_ref() {
            TreePathIndexLayer::Owned(owned) => owned.key_nodes_by_path.get(path).copied(),
            TreePathIndexLayer::Overlay(overlay) => overlay
                .patch
                .key_nodes_by_path
                .get(path)
                .copied()
                .flatten()
                .or_else(|| {
                    if overlay.patch.key_nodes_by_path.contains_key(path) {
                        None
                    } else {
                        overlay.base.key_node(path)
                    }
                }),
        }
    }

    pub fn path_for_node(&self, node_id: NodeId) -> Option<Vec<PathSeg<'_>>> {
        self.owned_path_for_node(node_id)
            .map(|segments| segments.iter().map(OwnedPathSeg::borrowed).collect())
    }

    pub fn owned_path_for_node(&self, node_id: NodeId) -> Option<&[OwnedPathSeg]> {
        match self.layer.as_ref() {
            TreePathIndexLayer::Owned(owned) => owned.node_paths.get(node_id.0)?.as_deref(),
            TreePathIndexLayer::Overlay(overlay) => {
                match overlay.patch.node_paths.get(&node_id.0) {
                    Some(Some(path)) => Some(path.as_slice()),
                    Some(None) => None,
                    None => overlay.base.owned_path_for_node(node_id),
                }
            }
        }
    }

    pub fn updated_for_structural_edit(
        &self,
        store: &TreeStore,
        update: TreePathIndexStructuralUpdate<'_>,
    ) -> Self {
        let mut patch = TreePathIndexPatch::default();
        let mut changed_prefix = self
            .owned_path_for_node(update.changed_root)
            .map(|path| path.to_vec())
            .unwrap_or_default();

        for node_id in update.removed_node_ids {
            self.remove_node_into_patch(*node_id, &mut patch);
        }
        populate_tree_path_index_patch(store, update.changed_root, &mut changed_prefix, &mut patch);
        Self {
            layer: Arc::new(TreePathIndexLayer::Overlay(TreePathIndexOverlay {
                base: self.clone(),
                patch,
            })),
        }
    }

    fn remove_node_into_patch(&self, node_id: NodeId, patch: &mut TreePathIndexPatch) {
        let Some(path) = self.owned_path_for_node(node_id).map(|path| path.to_vec()) else {
            patch.node_paths.insert(node_id.0, None);
            return;
        };
        patch.node_paths.insert(node_id.0, None);
        if self.value_node(&path) == Some(node_id) {
            patch.value_nodes_by_path.insert(path.clone(), None);
        }
        if self.key_node(&path) == Some(node_id) {
            patch.key_nodes_by_path.insert(path, None);
        }
    }

    fn flatten_owned(&self) -> TreePathIndexOwned {
        match self.layer.as_ref() {
            TreePathIndexLayer::Owned(owned) => owned.clone(),
            TreePathIndexLayer::Overlay(overlay) => {
                let mut owned = overlay.base.flatten_owned();
                for (node_id, path) in &overlay.patch.node_paths {
                    if *node_id >= owned.node_paths.len() {
                        owned.node_paths.resize(*node_id + 1, None);
                    }
                    owned.node_paths[*node_id] = path.clone();
                }
                for (path, node_id) in &overlay.patch.value_nodes_by_path {
                    match node_id {
                        Some(node_id) => {
                            owned.value_nodes_by_path.insert(path.clone(), *node_id);
                        }
                        None => {
                            owned.value_nodes_by_path.remove(path);
                        }
                    }
                }
                for (path, node_id) in &overlay.patch.key_nodes_by_path {
                    match node_id {
                        Some(node_id) => {
                            owned.key_nodes_by_path.insert(path.clone(), *node_id);
                        }
                        None => {
                            owned.key_nodes_by_path.remove(path);
                        }
                    }
                }
                owned
            }
        }
    }
}

fn populate_tree_path_index_owned(
    store: &TreeStore,
    node_id: NodeId,
    current: &mut Vec<OwnedPathSeg>,
    index: &mut TreePathIndexOwned,
) {
    set_owned_node_path(index, node_id, current);
    index.value_nodes_by_path.insert(current.clone(), node_id);

    let Some(node) = store.get(node_id) else {
        return;
    };
    if node.is_map_key {
        index.key_nodes_by_path.insert(current.clone(), node_id);
    }

    let children = node.content.clone();
    for child_id in children {
        let pushed = match node_path_segment_for(store, child_id) {
            Some(segment) => {
                current.push(segment);
                true
            }
            None => false,
        };
        populate_tree_path_index_owned(store, child_id, current, index);
        if pushed {
            current.pop();
        }
    }
}

fn populate_tree_path_index_patch(
    store: &TreeStore,
    node_id: NodeId,
    current: &mut Vec<OwnedPathSeg>,
    patch: &mut TreePathIndexPatch,
) {
    patch.node_paths.insert(node_id.0, Some(current.clone()));
    patch
        .value_nodes_by_path
        .insert(current.clone(), Some(node_id));

    let Some(node) = store.get(node_id) else {
        return;
    };
    if node.is_map_key {
        patch
            .key_nodes_by_path
            .insert(current.clone(), Some(node_id));
    }

    let children = node.content.clone();
    for child_id in children {
        let pushed = match node_path_segment_for(store, child_id) {
            Some(segment) => {
                current.push(segment);
                true
            }
            None => false,
        };
        populate_tree_path_index_patch(store, child_id, current, patch);
        if pushed {
            current.pop();
        }
    }
}

fn set_owned_node_path(index: &mut TreePathIndexOwned, node_id: NodeId, path: &[OwnedPathSeg]) {
    if node_id.0 >= index.node_paths.len() {
        index.node_paths.resize(node_id.0 + 1, None);
    }
    index.node_paths[node_id.0] = Some(path.to_vec());
}

fn node_path_segment_for(store: &TreeStore, node_id: NodeId) -> Option<OwnedPathSeg> {
    match store.parsed_key_for(node_id).ok().flatten()? {
        ParsedKey::Str(key) => Some(OwnedPathSeg::Key(key)),
        ParsedKey::Int(index) => i32::try_from(index).ok().map(OwnedPathSeg::Index),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::codec_service::CodecService;
    use crate::core::{find_node_by_path_with_index, path_seg_key};

    #[test]
    fn structural_update_matches_full_build_for_changed_subtree() {
        let source = r#"{"root":{"a":1,"b":2},"tail":3}"#;
        let decoded = CodecService::new()
            .decode("json", source)
            .expect("json fixture should decode");
        let full = TreePathIndex::build(&decoded.store, decoded.root);
        let root_path = [path_seg_key("root")];
        let changed_root = find_node_by_path_with_index(
            decoded.root,
            &root_path,
            false,
            &decoded.store,
            Some(&full),
        )
        .expect("fixture should contain $.root");

        let updated = full.updated_for_structural_edit(
            &decoded.store,
            TreePathIndexStructuralUpdate {
                changed_root,
                removed_node_ids: &[],
            },
        );
        let rebuilt = TreePathIndex::build(&decoded.store, decoded.root);

        assert_eq!(updated, rebuilt);
        let b_path = [path_seg_key("root"), path_seg_key("b")];
        assert_eq!(
            updated.value_node_for_segments(&b_path),
            rebuilt.value_node_for_segments(&b_path)
        );
    }

    #[test]
    fn structural_update_removes_detached_subtree_paths() {
        let source = r#"{"root":{"a":1,"b":2},"tail":3}"#;
        let mut decoded = CodecService::new()
            .decode("json", source)
            .expect("json fixture should decode");
        let full = TreePathIndex::build(&decoded.store, decoded.root);
        let root_path = [path_seg_key("root")];
        let a_path = [path_seg_key("root"), path_seg_key("a")];
        let b_path = [path_seg_key("root"), path_seg_key("b")];
        let changed_root = find_node_by_path_with_index(
            decoded.root,
            &root_path,
            false,
            &decoded.store,
            Some(&full),
        )
        .expect("fixture should contain $.root");
        let old_a = full
            .value_node_for_segments(&a_path)
            .expect("fixture should contain $.root.a");
        let old_b = full
            .value_node_for_segments(&b_path)
            .expect("fixture should contain $.root.b");

        decoded
            .store
            .get_mut(changed_root)
            .expect("changed root should exist")
            .content
            .clear();

        let updated = full.updated_for_structural_edit(
            &decoded.store,
            TreePathIndexStructuralUpdate {
                changed_root,
                removed_node_ids: &[old_a, old_b],
            },
        );

        assert!(updated.value_node_for_segments(&a_path).is_none());
        assert!(updated.value_node_for_segments(&b_path).is_none());
        assert!(updated.value_node_for_segments(&root_path).is_some());
    }

    #[test]
    fn structural_update_overlay_matches_rebuild_on_touched_paths() {
        let source = r#"{"root":{"a":1,"b":2},"tail":3}"#;
        let decoded = CodecService::new()
            .decode("json", source)
            .expect("json fixture should decode");
        let full = TreePathIndex::build(&decoded.store, decoded.root);
        let root_path = [path_seg_key("root")];
        let b_path = [path_seg_key("root"), path_seg_key("b")];
        let tail_path = [path_seg_key("tail")];
        let changed_root = find_node_by_path_with_index(
            decoded.root,
            &root_path,
            false,
            &decoded.store,
            Some(&full),
        )
        .expect("fixture should contain $.root");

        let updated = full.updated_for_structural_edit(
            &decoded.store,
            TreePathIndexStructuralUpdate {
                changed_root,
                removed_node_ids: &[],
            },
        );
        let rebuilt = TreePathIndex::build(&decoded.store, decoded.root);

        assert_eq!(
            updated.value_node_for_segments(&root_path),
            rebuilt.value_node_for_segments(&root_path)
        );
        assert_eq!(
            updated.value_node_for_segments(&b_path),
            rebuilt.value_node_for_segments(&b_path)
        );
        assert_eq!(
            updated.value_node_for_segments(&tail_path),
            rebuilt.value_node_for_segments(&tail_path)
        );
    }
}
