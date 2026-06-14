use crate::core::ParsedKey;
use crate::operators::path::{get_path, get_path_array_from_node};
use crate::operators::*;

// ── Internal helpers ───────────────────────────────────────────────

fn assign_document_index(node: &mut TreeNode, document: u32) {
    node.document = document;
    for child in &mut node.content {
        assign_document_index(child, document);
    }
}

fn context_has_links(ctx: &Context) -> bool {
    ctx.matching_nodes.iter().any(tree_has_links)
}

fn tree_has_links(node: &TreeNode) -> bool {
    if node.parent.is_some() || node.key.is_some() {
        return true;
    }
    node.content.iter().any(tree_has_links)
}

fn index_tree(
    store: &mut TreeStore,
    node: &mut TreeNode,
    parent: Option<NodeId>,
    key: Option<NodeId>,
    sequence_index: Option<i64>,
    document: u32,
) -> NodeId {
    node.parent = parent;
    node.key = key;
    node.sequence_index = sequence_index;
    node.document = document;

    let mut store_node = node.clone();
    store_node.content.clear();
    let node_id = store.add(store_node);

    match node.kind {
        NodeKind::Mapping => {
            let mut i = 0;
            while i + 1 < node.content.len() {
                let key_id = {
                    let key_node = &mut node.content[i];
                    key_node.is_map_key = true;
                    index_tree(store, key_node, Some(node_id), None, None, document)
                };

                {
                    let value_node = &mut node.content[i + 1];
                    value_node.is_map_key = false;
                    index_tree(
                        store,
                        value_node,
                        Some(node_id),
                        Some(key_id),
                        None,
                        document,
                    );
                }

                node.content[i + 1].key = Some(key_id);

                i += 2;
            }
        }
        NodeKind::Sequence => {
            for (index, child) in node.content.iter_mut().enumerate() {
                index_tree(
                    store,
                    child,
                    Some(node_id),
                    None,
                    Some(index as i64),
                    document,
                );
            }
        }
        _ => {}
    }

    node_id
}

fn prepare_context_for_delete(d: &mut TreeEngine, ctx: &Context) -> Context {
    let mut working_ctx = ctx.clone();
    for (index, node) in working_ctx.matching_nodes.iter_mut().enumerate() {
        assign_document_index(node, index as u32);
    }

    for (index, node) in working_ctx.matching_nodes.iter_mut().enumerate() {
        index_tree(&mut d.store, node, None, None, None, index as u32);
    }

    working_ctx
}

fn remove_root_from_context(ctx: &mut Context, document: u32) {
    let index = document as usize;
    if index < ctx.matching_nodes.len() {
        ctx.matching_nodes.remove(index);
        for (doc_index, node) in ctx.matching_nodes.iter_mut().enumerate() {
            assign_document_index(node, doc_index as u32);
        }
    }
}

fn child_for_path_mut<'a>(node: &'a mut TreeNode, path: &ParsedKey) -> Option<&'a mut TreeNode> {
    match node.kind {
        NodeKind::Mapping => {
            let mut i = 0;
            while i + 1 < node.content.len() {
                let matches = match path {
                    ParsedKey::Str(key) => node.content[i].value == *key,
                    ParsedKey::Int(index) => node.content[i]
                        .value
                        .parse::<i64>()
                        .map(|value| value == *index)
                        .unwrap_or(false),
                };
                if matches {
                    return node.content.get_mut(i + 1);
                }
                i += 2;
            }
            None
        }
        NodeKind::Sequence => match path {
            ParsedKey::Int(index) if *index >= 0 => node.content.get_mut(*index as usize),
            ParsedKey::Str(index) => index
                .parse::<usize>()
                .ok()
                .and_then(|parsed| node.content.get_mut(parsed)),
            _ => None,
        },
        _ => None,
    }
}

fn delete_from_working_context(root: &mut TreeNode, path: &[ParsedKey]) {
    if path.is_empty() {
        return;
    }
    if path.len() == 1 {
        match root.kind {
            NodeKind::Mapping => delete_from_map(root, &path[0]),
            NodeKind::Sequence => delete_from_array(root, &path[0]),
            _ => {}
        }
        return;
    }

    if let Some(child) = child_for_path_mut(root, &path[0]) {
        delete_from_working_context(child, &path[1..]);
    }
}

/// Delete a key from a mapping node.
fn delete_from_map(node: &mut TreeNode, child_path: &ParsedKey) {
    let mut new_contents = Vec::new();
    let mut i = 0;
    while i + 1 < node.content.len() {
        let key = &node.content[i];
        let value = &node.content[i + 1];

        let should_delete = match child_path {
            ParsedKey::Str(s) => key.value == *s,
            ParsedKey::Int(v) => {
                if let Ok(parsed) = key.value.parse::<i64>() {
                    parsed == *v
                } else {
                    false
                }
            }
        };

        if !should_delete {
            new_contents.push(key.clone());
            new_contents.push(value.clone());
        }
        i += 2;
    }
    node.content = new_contents;
}

/// Delete an index from an array node.
/// After deletion, updates the `sequence_index` and `key.value` of remaining
fn delete_from_array(node: &mut TreeNode, child_path: &ParsedKey) {
    let mut new_contents = Vec::new();
    let mut kept_index: i64 = 0;

    for (index, value) in node.content.iter().enumerate() {
        let should_delete = match child_path {
            ParsedKey::Int(v) => *v == index as i64,
            ParsedKey::Str(s) => {
                if let Ok(parsed) = s.parse::<i64>() {
                    parsed == index as i64
                } else {
                    false
                }
            }
        };

        if !should_delete {
            let mut v = value.clone();
            v.sequence_index = Some(kept_index as i64);
            // Update key value to reflect new index when the array element has a
            // separate key node. Do not rewrite the element's own scalar value.
            if let Some(_key) = v.key {
                // In Phase A with owned TreeNode, key is None; this path is for Phase B
                // when key becomes a NodeId. For now, update is_map_key nodes' values.
            }
            new_contents.push(v);
            kept_index += 1;
        }
    }
    node.content = new_contents;
}

// ── Public operators ───────────────────────────────────────────────

/// Delete operator: remove the current matching nodes from their parent.
pub fn delete_child_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let sync_store_from_original = context_has_links(&ctx);
    let original_rhs = expression_node.rhs.clone();
    let mut working_ctx = prepare_context_for_delete(d, &ctx);
    let ro = working_ctx.read_only_clone()?;
    let nodes_to_delete = get_matching_nodes(d, &ro, expression_node.rhs.as_deref_mut())?;

    let mut i = nodes_to_delete.matching_nodes.len();
    while i > 0 {
        i -= 1;
        let candidate = &nodes_to_delete.matching_nodes[i];

        if candidate.parent.is_none() {
            remove_root_from_context(&mut working_ctx, candidate.document);
            continue;
        }

        let path = get_path(candidate, &d.store)?;
        if path.is_empty() {
            continue;
        }

        if let Some(root) = working_ctx
            .matching_nodes
            .get_mut(candidate.document as usize)
        {
            delete_from_working_context(root, &path);
        }
    }

    if sync_store_from_original {
        if let Some(mut rhs) = original_rhs {
            let original_ro = ctx.read_only_clone()?;
            let nodes_to_delete_in_store = get_matching_nodes(d, &original_ro, Some(&mut rhs))?;

            let mut i = nodes_to_delete_in_store.matching_nodes.len();
            while i > 0 {
                i -= 1;
                let candidate = &nodes_to_delete_in_store.matching_nodes[i];
                let Some(parent_id) = candidate.parent else {
                    continue;
                };
                let path = get_path(candidate, &d.store)?;
                let Some(child_path) = path.last() else {
                    continue;
                };

                let parent = d.store.get_mut(parent_id);
                match parent.kind {
                    NodeKind::Mapping => delete_from_map(parent, child_path),
                    NodeKind::Sequence => delete_from_array(parent, child_path),
                    _ => {}
                }
            }
        }
    }

    Ok(working_ctx)
}

/// del_paths operator: delete nodes by their paths.
pub fn del_paths_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let ro = ctx.read_only_clone()?;
    let path_arrays_context = get_matching_nodes(d, &ro, expression_node.rhs.as_deref_mut())?;

    if path_arrays_context.matching_nodes.len() != 1 {
        if let Some(ref d2) = ctx.diagnostics {
            d2.set_messagef(
                "eval",
                &format!(
                    "DELPATHS: expected single value but found {}",
                    path_arrays_context.matching_nodes.len()
                ),
            )?;
        }
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }

    let path_arrays_node = &path_arrays_context.matching_nodes[0];
    if path_arrays_node.resolved_sem_type() != Some(SemType::Seq) {
        if let Some(ref d2) = ctx.diagnostics {
            d2.set_messagef(
                "eval",
                &format!(
                    "DELPATHS: expected a sequence of sequences, but found {}",
                    path_arrays_node.tag
                ),
            )?;
        }
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }

    let mut updated_context = ctx;
    for (idx, child) in path_arrays_node.content.iter().enumerate() {
        if child.resolved_sem_type() != Some(SemType::Seq) {
            if let Some(ref d2) = updated_context.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!(
                        "DELPATHS: expected entry [{}] to be a sequence, but its a {}. \
                     Note that delpaths takes an array of path arrays, \
                     e.g. [[\"a\", \"b\"]]",
                        idx, child.tag
                    ),
                )?;
            }
            return Err(CoreError::Parse(ParseError::InvalidSyntax));
        }

        let child_path = get_path_array_from_node(child)?;

        // Build traversal tree and a delete expression
        let child_traversal_exp = create_traversal_tree(
            &child_path
                .iter()
                .map(|pk| match pk {
                    ParsedKey::Str(s) => {
                        let mut n = TreeNode::default();
                        n.kind = NodeKind::Scalar;
                        n.value = s.clone();
                        n
                    }
                    ParsedKey::Int(i) => {
                        let mut n = TreeNode::default();
                        n.kind = NodeKind::Scalar;
                        n.value = i.to_string();
                        n
                    }
                })
                .collect::<Vec<_>>(),
            TraversePreferences::default(),
            false,
        )?;

        let delete_child_op = Operation {
            operation_type: &DELETE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        };

        let mut delete_child_op_node = ExpressionNode {
            operation: Box::new(delete_child_op),
            lhs: None,
            rhs: Some(child_traversal_exp),
        };

        updated_context = get_matching_nodes(d, &updated_context, Some(&mut delete_child_op_node))?;
    }

    Ok(updated_context)
}
