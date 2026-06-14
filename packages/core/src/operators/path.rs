use crate::core::ParsedKey;
use crate::operators::*;

fn is_empty_placeholder(node: &TreeNode) -> bool {
    node.kind == NodeKind::Scalar
        && node.content.is_empty()
        && node.value.is_empty()
        && node.tag.is_empty()
        && matches!(node.resolved_sem_type(), Some(SemType::Nil) | None)
}

fn ensure_mapping_for_setpath(target: &mut TreeNode) {
    if target.kind == NodeKind::Mapping {
        return;
    }

    let preserve_empty_tag = is_empty_placeholder(target);
    target.kind = NodeKind::Mapping;
    target.value.clear();
    target.content.clear();
    if preserve_empty_tag {
        target.sem_type = None;
        target.tag.clear();
    } else {
        target.sem_type = Some(SemType::Map);
        target.tag = SemType::Map.to_string().into();
    }
}

fn ensure_sequence_for_setpath(target: &mut TreeNode) {
    if target.kind == NodeKind::Sequence {
        return;
    }

    let preserve_empty_tag = is_empty_placeholder(target);
    target.kind = NodeKind::Sequence;
    target.value.clear();
    target.content.clear();
    if preserve_empty_tag {
        target.sem_type = None;
        target.tag.clear();
    } else {
        target.sem_type = Some(SemType::Seq);
        target.tag = SemType::Seq.to_string().into();
    }
}

fn find_or_create_map_value(target: &mut TreeNode, key: &str) -> Result<usize, CoreError> {
    let mut i = 0;
    while i + 1 < target.content.len() {
        if target.content[i].is_map_key && target.content[i].value == key {
            return Ok(i + 1);
        }
        i += 2;
    }

    let mut key_node = TreeNode::scalar(SemType::Str, key);
    key_node.is_map_key = true;
    let value_node = TreeNode::default();
    target.add_key_value_child(&key_node, &value_node)?;
    Ok(target.content.len() - 1)
}

fn set_path_value(
    target: &mut TreeNode,
    path: &[ParsedKey],
    assigned: &TreeNode,
) -> Result<(), CoreError> {
    if path.is_empty() {
        return update_from(target, assigned);
    }

    match &path[0] {
        ParsedKey::Str(key) => {
            ensure_mapping_for_setpath(target);
            let value_index = find_or_create_map_value(target, key)?;
            set_path_value(&mut target.content[value_index], &path[1..], assigned)
        }
        ParsedKey::Int(index) => {
            let index: usize = (*index)
                .try_into()
                .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
            ensure_sequence_for_setpath(target);
            while target.content.len() <= index {
                target.add_child(&TreeNode::default())?;
            }
            set_path_value(&mut target.content[index], &path[1..], assigned)
        }
    }
}

fn reindex_tree(
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
                    reindex_tree(store, key_node, Some(node_id), None, None, document)
                };

                {
                    let value_node = &mut node.content[i + 1];
                    value_node.is_map_key = false;
                    reindex_tree(
                        store,
                        value_node,
                        Some(node_id),
                        Some(key_id),
                        None,
                        document,
                    );
                }

                i += 2;
            }
        }
        NodeKind::Sequence => {
            for (index, child) in node.content.iter_mut().enumerate() {
                reindex_tree(
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

fn reindex_context_store(d: &mut TreeEngine, ctx: &mut Context) {
    d.store = TreeStore::new();
    for (index, node) in ctx.matching_nodes.iter_mut().enumerate() {
        reindex_tree(&mut d.store, node, None, None, None, index as u32);
    }
}

/// Walk up the parent chain to build the path from root to this node.
pub fn get_path(node: &TreeNode, store: &TreeStore) -> Result<Vec<ParsedKey>, CoreError> {
    let mut path = Vec::new();
    let mut cur: Option<&TreeNode> = Some(node);
    loop {
        let n = match cur {
            Some(n) => n,
            None => break,
        };
        // Determine the key for n based on its position under its parent.
        // If n has a key node, use its value. Otherwise check if it has
        // a sequence_index for array children. Finally fall back to empty.
        if let Some(key_id) = n.key {
            let key_node = store.get(key_id);
            let key_val = &key_node.value;
            // Try parsing as integer
            if let Ok(idx) = key_val.parse::<i64>() {
                path.push(ParsedKey::Int(idx));
            } else {
                path.push(ParsedKey::Str(key_val.clone()));
            }
        } else if n.sequence_index.unwrap_or(0) > 0
            || n.parent
                .map_or(false, |id| store.get(id).kind == NodeKind::Sequence)
        {
            // For sequence children, use the sequence_index
            path.push(ParsedKey::Int(n.sequence_index.unwrap_or(0)));
        } else {
            // For root or unknown position
            if n.parent.is_some() {
                path.push(ParsedKey::Str(String::new()));
            }
        }
        // Move to parent via store
        cur = n.parent.map(|id| store.get(id));
    }
    // Reverse so root is first, leaf is last
    path.reverse();
    Ok(path)
}

/// Create a path tree node from a ParsedKey.
fn create_path_node_for(key: &ParsedKey) -> Result<Box<TreeNode>, CoreError> {
    match key {
        ParsedKey::Str(s) => create_string_scalar_node(s),
        ParsedKey::Int(i) => create_scalar_node_i64(*i),
    }
}

/// Parse a TreeNode (expected to be a sequence) into an array of ParsedKey.
pub fn get_path_array_from_node(node: &TreeNode) -> Result<Vec<ParsedKey>, CoreError> {
    if node.kind != NodeKind::Sequence {
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }

    let mut out = Vec::with_capacity(node.content.len());
    for child_node in &node.content {
        let tid = child_node.resolved_sem_type();
        if tid == Some(SemType::Str) {
            out.push(ParsedKey::Str(child_node.value.clone()));
        } else if tid == Some(SemType::Int) {
            let number: i64 = child_node
                .value
                .parse()
                .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
            out.push(ParsedKey::Int(number));
        } else {
            return Err(CoreError::Parse(ParseError::InvalidSyntax));
        }
    }
    Ok(out)
}

/// Get the path of the current node(s) as a sequence of string/int scalars.
pub fn get_path_operator(
    ctx: Context,
    d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();

    for candidate in &ctx.matching_nodes {
        let path = get_path(candidate, &d.store)?;
        let mut path_list =
            candidate.create_replacement(NodeKind::Sequence, SemType::Seq.to_string(), "")?;

        for el in &path {
            let path_node = create_path_node_for(el)?;
            path_list.add_child(&path_node)?;
        }
        results.push((*path_list).clone());
    }

    ctx.child_context(results)
}

/// Set a value at the specified path (setpath operator).
pub fn set_path_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let rhs = expression_node.rhs.as_mut().ok_or_else(|| {
        // Diagnostics would be set here in full implementation
        CoreError::Eval(EvalError::MissingRhs)
    })?;

    if rhs.operation.operation_type.id != BLOCK_OP_TYPE.id {
        if let Some(ref d2) = ctx.diagnostics {
            d2.set_messagef(
                "eval",
                &format!(
                    "SETPATH must be given a block (;), got {} instead",
                    rhs.operation.operation_type.name()
                ),
            )?;
        }
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }

    // Get the path from LHS of the block
    let ro = ctx.read_only_clone()?;
    let lhs_path_ctx = get_matching_nodes(d, &ro, rhs.lhs.as_deref_mut())?;
    if lhs_path_ctx.matching_nodes.len() != 1 {
        if let Some(ref d2) = ctx.diagnostics {
            d2.set_messagef(
                "eval",
                &format!(
                    "SETPATH: expected single path but found {} results instead",
                    lhs_path_ctx.matching_nodes.len()
                ),
            )?;
        }
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }

    let lhs_value = &lhs_path_ctx.matching_nodes[0];
    let lhs_path = get_path_array_from_node(lhs_value)?;

    let mut updated = ctx.clone();

    for (index, candidate) in ctx.matching_nodes.iter().enumerate() {
        let ro_sc = ctx.single_readonly_child_context(candidate)?;
        let target_value_ctx = get_matching_nodes(d, &ro_sc, rhs.rhs.as_deref_mut())?;

        if target_value_ctx.matching_nodes.len() != 1 {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!(
                        "SETPATH: expected single value on RHS but found {}",
                        target_value_ctx.matching_nodes.len()
                    ),
                )?;
            }
            return Err(CoreError::Parse(ParseError::InvalidSyntax));
        }

        let target_value = target_value_ctx.matching_nodes[0].clone();
        set_path_value(&mut updated.matching_nodes[index], &lhs_path, &target_value)?;
    }

    reindex_context_store(d, &mut updated);
    Ok(updated)
}
