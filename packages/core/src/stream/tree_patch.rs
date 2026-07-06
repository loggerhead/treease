use crate::core::NodeId;

/// Internal tree mutation patch produced by the streaming tree builder
/// as it consumes parser events.  Patches are applied to an append-only
/// `TreeStore` and serve as a replayable log.  They are not exposed in
/// the document protocol.
#[derive(Debug, Clone)]
pub enum TreePatch {
    /// Declares the root `NodeId` and anchors the start of the patch log.
    /// Must be the first structural patch in a job.
    DocumentStarted { root: NodeId },

    /// A new node is inserted into the tree.  Container nodes (Mapping,
    /// Sequence) are not yet sealed; scalar nodes are implicitly sealed
    /// on insertion.
    NodeInserted {
        node_id: NodeId,
        parent: Option<NodeId>,
        key: Option<NodeId>,
        sequence_index: Option<u32>,
        kind: i32,
        sem_type: i32,
        tag: String,
        value: String,
    },

    /// A map key scalar is inserted into the currently-open mapping.
    KeyInserted {
        key_id: NodeId,
        parent: NodeId,
        key_text: String,
        tag: String,
    },

    /// A container node that was previously inserted is now sealed (closed).
    /// For scalars this is implicit at insertion time; implementations may
    /// still emit a `NodeSealed` but must do so at most once.
    NodeSealed { node_id: NodeId },

    /// A parser-level diagnostic that does not affect tree structure.
    DiagnosticAdded {
        message: String,
        line: u32,
        column: u32,
        byte_offset: Option<u32>,
    },

    /// The document root has been sealed and no further structural patches
    /// will be emitted for this job.
    DocumentEnded { root: NodeId },
}

impl TreePatch {
    /// Returns true if this patch carries structural tree state change
    /// (as opposed to diagnostic-only).
    pub fn is_structural(&self) -> bool {
        !matches!(self, TreePatch::DiagnosticAdded { .. })
    }
}

/// A batch of [`TreePatch`] items produced from processing one or more
/// parser events.
#[derive(Debug, Clone, Default)]
pub struct TreePatchBatch {
    pub patches: Vec<TreePatch>,
}

impl TreePatchBatch {
    pub fn new() -> Self {
        Self {
            patches: Vec::new(),
        }
    }

    pub fn push(&mut self, patch: TreePatch) {
        self.patches.push(patch);
    }

    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }

    pub fn len(&self) -> usize {
        self.patches.len()
    }
}

impl Extend<TreePatch> for TreePatchBatch {
    fn extend<T: IntoIterator<Item = TreePatch>>(&mut self, iter: T) {
        self.patches.extend(iter);
    }
}

// ── Debug / validation helpers ──────────────────────────────────────────

/// Checks a patch sequence for invariants that must hold in debug builds.
/// Returns `Ok(())` if all invariants pass, or an `Err` describing the
/// first violation.
pub fn check_patch_sequence(patches: &[TreePatch]) -> Result<(), String> {
    use std::collections::HashSet;

    let mut seen_root: Option<NodeId> = None;
    let mut seen_start = false;
    let mut seen_end = false;
    let mut allocated: HashSet<NodeId> = HashSet::new();
    let mut sealed: HashSet<NodeId> = HashSet::new();
    let mut next_id: usize = 0;
    let mut pending_key: Option<NodeId> = None;
    let mut pending_parent: Option<NodeId> = None;
    let mut open_containers: Vec<NodeId> = Vec::new();

    for patch in patches {
        if seen_end {
            return Err("patch after DocumentEnded".into());
        }
        match patch {
            TreePatch::DocumentStarted { root } => {
                if seen_start {
                    return Err("duplicate DocumentStarted".into());
                }
                seen_start = true;
                seen_root = Some(*root);
            }
            TreePatch::NodeInserted {
                node_id,
                parent,
                key,
                sequence_index: _,
                ..
            } => {
                // NodeId must be monotonically increasing
                if node_id.index() != next_id {
                    return Err(format!(
                        "non-monotonic NodeId: expected {}, got {}",
                        next_id,
                        node_id.index()
                    ));
                }
                next_id += 1;
                allocated.insert(*node_id);

                match parent {
                    Some(parent_id) => {
                        if !allocated.contains(parent_id) {
                            return Err(format!("parent {:?} not yet allocated", parent_id));
                        }
                        if sealed.contains(parent_id) {
                            return Err(format!(
                                "cannot insert child into sealed parent {:?}",
                                parent_id
                            ));
                        }
                        // For sequence children, sequence_index must match
                        // but we can only check approximately here
                    }
                    None => {
                        if seen_root != Some(*node_id) {
                            return Err(format!(
                                "root {:?} does not match DocumentStarted {:?}",
                                node_id, seen_root
                            ));
                        }
                    }
                }

                // If the parent is the current open mapping and we have a
                // pending key, the key must match
                if let Some(pid) = pending_parent {
                    if Some(pid) == *parent && pending_key.is_some() {
                        if *key != pending_key {
                            return Err(format!(
                                "key mismatch: expected {:?}, got {:?}",
                                pending_key, key
                            ));
                        }
                        pending_key = None;
                        pending_parent = None;
                    }
                }
            }
            TreePatch::KeyInserted {
                key_id,
                parent,
                key_text: _,
                ..
            } => {
                if key_id.index() != next_id {
                    return Err(format!(
                        "non-monotonic KeyInserted id: expected {}, got {}",
                        next_id,
                        key_id.index()
                    ));
                }
                next_id += 1;
                allocated.insert(*key_id);
                if !allocated.contains(parent) {
                    return Err(format!("key parent {:?} not yet allocated", parent));
                }
                if sealed.contains(parent) {
                    return Err(format!("cannot insert key into sealed parent {:?}", parent));
                }
                if pending_key.is_some() {
                    return Err("pending key not consumed before new KeyInserted".into());
                }
                pending_key = Some(*key_id);
                pending_parent = Some(*parent);
                // Keys are implicitly sealed
                sealed.insert(*key_id);
            }
            TreePatch::NodeSealed { node_id } => {
                if !allocated.contains(node_id) {
                    return Err(format!("sealed node {:?} not allocated", node_id));
                }
                if sealed.contains(node_id) {
                    return Err(format!("node {:?} sealed more than once", node_id));
                }
                sealed.insert(*node_id);

                // If this was the top of the open stack, pop it
                if open_containers.last() == Some(node_id) {
                    open_containers.pop();
                }
            }
            TreePatch::DiagnosticAdded { .. } => {
                // No structural invariants to check
            }
            TreePatch::DocumentEnded { root } => {
                if seen_root != Some(*root) {
                    return Err(format!(
                        "DocumentEnded root {:?} does not match DocumentStarted {:?}",
                        root, seen_root
                    ));
                }
                seen_end = true;
            }
        }

        // Track open containers
        if let TreePatch::NodeInserted { kind, node_id, .. } = patch {
            if *kind == 0 || *kind == 1 {
                // Sequence or Mapping
                open_containers.push(*node_id);
            }
        }
    }

    if !seen_end {
        return Err("patch sequence missing DocumentEnded".into());
    }

    if pending_key.is_some() {
        return Err("unconsumed pending key at end of sequence".into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nid(id: usize) -> NodeId {
        NodeId::from_index(id)
    }

    #[test]
    fn valid_sequence_passes_checks() {
        let patches = vec![
            TreePatch::DocumentStarted { root: nid(0) },
            TreePatch::NodeInserted {
                node_id: nid(0),
                parent: None,
                key: None,
                sequence_index: None,
                kind: 1,
                sem_type: 0,
                tag: "!!map".into(),
                value: "".into(),
            },
            TreePatch::NodeSealed { node_id: nid(0) },
            TreePatch::DocumentEnded { root: nid(0) },
        ];
        assert!(check_patch_sequence(&patches).is_ok());
    }

    #[test]
    fn duplicate_document_started_fails() {
        let patches = vec![
            TreePatch::DocumentStarted { root: nid(0) },
            TreePatch::DocumentStarted { root: nid(0) },
        ];
        assert!(check_patch_sequence(&patches).is_err());
    }

    #[test]
    fn non_monotonic_node_id_fails() {
        let patches = vec![
            TreePatch::DocumentStarted { root: nid(0) },
            TreePatch::NodeInserted {
                node_id: nid(0),
                parent: None,
                key: None,
                sequence_index: None,
                kind: 0,
                sem_type: 1,
                tag: "!!seq".into(),
                value: "".into(),
            },
            TreePatch::NodeInserted {
                node_id: nid(5), // skipped!
                parent: None,
                key: None,
                sequence_index: None,
                kind: 2,
                sem_type: 2,
                tag: "!!str".into(),
                value: "".into(),
            },
        ];
        assert!(check_patch_sequence(&patches).is_err());
    }

    #[test]
    fn child_of_sealed_parent_fails() {
        let patches = vec![
            TreePatch::DocumentStarted { root: nid(0) },
            TreePatch::NodeInserted {
                node_id: nid(0),
                parent: None,
                key: None,
                sequence_index: None,
                kind: 1,
                sem_type: 0,
                tag: "!!map".into(),
                value: "".into(),
            },
            TreePatch::NodeSealed { node_id: nid(0) },
            TreePatch::NodeInserted {
                node_id: nid(1),
                parent: Some(nid(0)),
                key: None,
                sequence_index: Some(0),
                kind: 2,
                sem_type: 2,
                tag: "!!str".into(),
                value: "x".into(),
            },
            TreePatch::DocumentEnded { root: nid(0) },
        ];
        assert!(check_patch_sequence(&patches).is_err());
    }

    #[test]
    fn double_seal_fails() {
        let patches = vec![
            TreePatch::DocumentStarted { root: nid(0) },
            TreePatch::NodeInserted {
                node_id: nid(0),
                parent: None,
                key: None,
                sequence_index: None,
                kind: 0,
                sem_type: 1,
                tag: "!!seq".into(),
                value: "".into(),
            },
            TreePatch::NodeSealed { node_id: nid(0) },
            TreePatch::NodeSealed { node_id: nid(0) },
            TreePatch::DocumentEnded { root: nid(0) },
        ];
        assert!(check_patch_sequence(&patches).is_err());
    }
}
