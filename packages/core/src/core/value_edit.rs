use super::{NodeId, TreeStore, lang_from_name};

/// Errors that can occur during value editing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueEditError {
    /// Core error from decode or encode.
    Core(crate::core::CoreError),
    /// Path or node not found during value edit.
    PathNotFound,
    /// Unsupported language for encoding.
    UnsupportedLanguage(String),
}

impl std::fmt::Display for ValueEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueEditError::Core(e) => write!(f, "{e}"),
            ValueEditError::PathNotFound => write!(f, "path not found in document"),
            ValueEditError::UnsupportedLanguage(l) => {
                write!(f, "unsupported language for value edit: {l}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentPathSeg {
    pub tag: i32,
    pub key: String,
    pub index: i32,
}

impl DocumentPathSeg {
    pub const KEY_TAG: i32 = 0;
    pub const INDEX_TAG: i32 = 1;

    fn is_index(&self) -> bool {
        self.tag == Self::INDEX_TAG
    }
}
pub fn apply_value_edit_text(
    language: &str,
    text: &str,
    path: &[DocumentPathSeg],
    prefer_key: bool,
    value_json: &str,
) -> Result<String, ValueEditError> {
    let mut decoded = crate::stream::decode_to_document_with_options(
        language,
        text,
        crate::stream::DecodeOptions {
            nest_json: false,
            emit_path: false,
        },
    )
    .map_err(ValueEditError::Core)?;
    let value_decoded = crate::stream::decode_to_document_with_options(
        "json",
        value_json,
        crate::stream::DecodeOptions {
            nest_json: false,
            emit_path: false,
        },
    )
    .map_err(ValueEditError::Core)?;

    if prefer_key {
        let target = node_for_path_with_prefer_key(&decoded.store, decoded.root, path, true)
            .ok_or(ValueEditError::PathNotFound)?;
        sync_value_between_stores(
            &value_decoded.store,
            value_decoded.root,
            &mut decoded.store,
            target,
        )?;
    } else if path.is_empty() {
        let new_root = clone_subtree(
            &value_decoded.store,
            value_decoded.root,
            &mut decoded.store,
            None,
        )?;
        decoded.root = new_root;
    } else {
        let target = node_for_path(&decoded.store, decoded.root, path)
            .ok_or(ValueEditError::PathNotFound)?;
        overwrite_node(
            &mut decoded.store,
            target,
            &value_decoded.store,
            value_decoded.root,
        )?;
    }

    encode_document(language, &decoded.store, decoded.root).map_err(ValueEditError::Core)
}

fn canonical_language_name(language: &str) -> Option<&'static str> {
    lang_from_name(language).map(|spec| spec.name)
}

fn encode_document(
    language: &str,
    store: &TreeStore,
    root: NodeId,
) -> Result<String, crate::core::CoreError> {
    use crate::formats::{encoder_json, encoder_toml, encoder_yaml};
    let normalized = canonical_language_name(language).unwrap_or("json");
    match normalized {
        "json" => encoder_json::encode_json(store, root),
        "yaml" => encoder_yaml::encode_yaml(store, root),
        "toml" => encoder_toml::encode_toml(store, root),
        other => Err(crate::core::CoreError::Io(format!(
            "unsupported language for value edit: {other}"
        ))),
    }
}
fn clone_subtree(
    src_store: &TreeStore,
    src_id: NodeId,
    dst_store: &mut TreeStore,
    parent: Option<NodeId>,
) -> Result<NodeId, ValueEditError> {
    let src = src_store.get(src_id).ok_or_else(|| {
        ValueEditError::Core(crate::core::CoreError::Io("source node missing".into()))
    })?;
    let mut cloned = src.clone();
    cloned.parent = parent;
    cloned.is_map_key = false;
    cloned.set_sequence_index(None);
    cloned.value = crate::core::NodeValueRef::Missing;
    let old_content = std::mem::take(&mut cloned.content);
    let new_id = dst_store.add(cloned);
    sync_value_between_stores(src_store, src_id, dst_store, new_id)?;
    for child_id in old_content {
        let child_new = clone_subtree(src_store, child_id, dst_store, Some(new_id))?;
        dst_store
            .get_mut(new_id)
            .ok_or_else(|| {
                ValueEditError::Core(crate::core::CoreError::Io("node missing after add".into()))
            })?
            .content
            .push(child_new);
    }
    Ok(new_id)
}

fn overwrite_node(
    dst_store: &mut TreeStore,
    target: NodeId,
    src_store: &TreeStore,
    src_id: NodeId,
) -> Result<(), ValueEditError> {
    let src = src_store.get(src_id).ok_or_else(|| {
        ValueEditError::Core(crate::core::CoreError::Io("source node missing".into()))
    })?;
    {
        let target_node = dst_store.get_mut(target).ok_or_else(|| {
            ValueEditError::Core(crate::core::CoreError::Io("target node missing".into()))
        })?;
        target_node.kind = src.kind;
        target_node.sem_type = src.sem_type;
        target_node.tag = src.tag.clone();
        target_node.value = crate::core::NodeValueRef::Missing;
        target_node.content.clear();
    }
    sync_value_between_stores(src_store, src_id, dst_store, target)?;
    for child_id in &src_store
        .get(src_id)
        .ok_or_else(|| {
            ValueEditError::Core(crate::core::CoreError::Io("source node missing".into()))
        })?
        .content
    {
        let child_new = clone_subtree(src_store, *child_id, dst_store, Some(target))?;
        dst_store
            .get_mut(target)
            .ok_or_else(|| {
                ValueEditError::Core(crate::core::CoreError::Io(
                    "node missing after overwrite".into(),
                ))
            })?
            .content
            .push(child_new);
    }
    Ok(())
}

fn sync_value_between_stores(
    src_store: &TreeStore,
    src_id: NodeId,
    dst_store: &mut TreeStore,
    dst_id: NodeId,
) -> Result<(), ValueEditError> {
    match src_store
        .value_ref_for(src_id)
        .map_err(ValueEditError::Core)?
    {
        Some(_) => {
            let value = src_store.value_for(src_id).map_err(ValueEditError::Core)?;
            dst_store
                .set_value(dst_id, value)
                .map_err(ValueEditError::Core)?;
        }
        None => {
            dst_store
                .remove_value(dst_id)
                .map_err(ValueEditError::Core)?;
        }
    }
    Ok(())
}

fn node_for_path_with_prefer_key(
    store: &TreeStore,
    root: NodeId,
    path: &[DocumentPathSeg],
    prefer_key: bool,
) -> Option<NodeId> {
    let mut current = root;
    for (index, segment) in path.iter().enumerate() {
        let node = store.get(current)?;
        current = if segment.is_index() {
            let sequence_index = usize::try_from(segment.index).ok()?;
            *node.content.get(sequence_index)?
        } else {
            node.content.chunks_exact(2).find_map(|pair| {
                let key = store.get(pair[0])?;
                if key.is_map_key
                    && store
                        .value_for(pair[0])
                        .is_ok_and(|value| value == segment.key)
                {
                    if prefer_key && index + 1 == path.len() {
                        Some(pair[0])
                    } else {
                        Some(pair[1])
                    }
                } else {
                    None
                }
            })?
        };
    }
    Some(current)
}

fn node_for_path(store: &TreeStore, root: NodeId, path: &[DocumentPathSeg]) -> Option<NodeId> {
    node_for_path_with_prefer_key(store, root, path, false)
}
