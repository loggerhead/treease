use crate::operators::operator_helpers::*;
use crate::operators::*;

// ── OrderedMatches: insert-ordered map from key string to TreeNode ─

struct OrderedMatches {
    keys: Vec<String>,
    values: Vec<(String, TreeNode)>,
}

impl OrderedMatches {
    fn new() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }

    fn put_owned(&mut self, owned_key: String, value: &TreeNode) -> Result<(), CoreError> {
        // Check if key exists
        let found = self.values.iter().any(|(k, _)| *k == owned_key);
        if !found {
            self.keys.push(owned_key.clone());
            self.values.push((owned_key.clone(), value.clone()));
        } else {
            // Update existing entry
            for (k, v) in self.values.iter_mut() {
                if *k == owned_key {
                    *v = value.clone();
                    break;
                }
            }
        }
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

// ── Alias resolution ───────────────────────────────────────────────

fn resolve_alias_for_traverse(
    node: &TreeNode,
    prefs: &TraversePreferences,
    store: &TreeStore,
) -> Option<TreeNode> {
    if node.kind != NodeKind::Alias {
        return Some(node.clone());
    }
    if prefs.dont_follow_alias {
        return None;
    }

    // Floyd's cycle detection using NodeId
    let mut slow: Option<NodeId> = node.alias;
    let mut fast: Option<NodeId> = node.alias;

    loop {
        slow = slow.and_then(|id| store.get(id).alias);
        if slow.map_or(true, |id| store.get(id).kind != NodeKind::Alias) {
            return slow.map(|id| store.get(id).clone());
        }

        let f = match fast.and_then(|id| store.get(id).alias) {
            Some(id) => id,
            None => return None,
        };
        if store.get(f).kind != NodeKind::Alias {
            return Some(store.get(f).clone());
        }
        let f = match store.get(f).alias {
            Some(id) => id,
            None => return None,
        };
        if store.get(f).kind != NodeKind::Alias {
            return Some(store.get(f).clone());
        }
        fast = Some(f);

        // Cycle detected via NodeId equality
        if slow == Some(f) {
            return None;
        }
    }
}

// ── Key matching ───────────────────────────────────────────────────

fn key_matches(key: &TreeNode, wanted_key: &str) -> bool {
    match_key(&key.value, wanted_key)
}

// ── Map traversal ──────────────────────────────────────────────────

fn do_traverse_map(
    new_matches: &mut OrderedMatches,
    node: &TreeNode,
    wanted_key: &str,
    prefs: &TraversePreferences,
    is_splat: bool,
) -> Result<(), CoreError> {
    let contents = &node.content;
    let mut i = 0;
    while i + 1 < contents.len() {
        let key = &contents[i];
        let value = &contents[i + 1];

        if is_splat || key_matches(key, wanted_key) {
            let key_name = key.get_key()?;
            if prefs.include_map_keys {
                new_matches.put_owned(key_name.clone(), key)?;
            }
            if !prefs.dont_include_map_values {
                let value_name = value.get_key()?;
                new_matches.put_owned(value_name, value)?;
            }
        }
        i += 2;
    }
    Ok(())
}

fn traverse_map(
    ctx: &Context,
    matching_node: &mut TreeNode,
    key_node: &TreeNode,
    prefs: &TraversePreferences,
    is_splat: bool,
    store: &TreeStore,
) -> Result<Vec<TreeNode>, CoreError> {
    let mut ordered = OrderedMatches::new();

    do_traverse_map(
        &mut ordered,
        matching_node,
        &key_node.value,
        prefs,
        is_splat,
    )?;

    // Auto-create if not found and allowed
    if !is_splat && !prefs.dont_auto_create && !ctx.dont_auto_create && ordered.is_empty() {
        let mut value_node = matching_node.create_child()?;
        value_node.kind = NodeKind::Scalar;
        value_node.sem_type = Some(SemType::Nil);
        value_node.tag = SemType::Nil.to_string().into();
        value_node.value = "null".to_string();

        matching_node.add_key_value_child(key_node, &value_node)?;

        if prefs.include_map_keys {
            if let Some(key_id) = value_node.key {
                let key_node = store.get(key_id);
                let key_name = key_node.get_key()?;
                ordered.put_owned(key_name, key_node)?;
            }
        }
        if !prefs.dont_include_map_values {
            let value_name = value_node.get_key()?;
            ordered.put_owned(value_name, &value_node)?;
        }
    }

    let mut results = Vec::new();
    for key in &ordered.keys {
        if let Some((_, v)) = ordered.values.iter().find(|(k, _)| k == key) {
            results.push(v.clone());
        }
    }
    Ok(results)
}

// ── Array traversal ────────────────────────────────────────────────

fn traverse_array_with_indices(
    node: &mut TreeNode,
    indices: &[TreeNode],
    prefs: &TraversePreferences,
) -> Result<Vec<TreeNode>, CoreError> {
    let mut new_matches = Vec::new();

    // No indices = splat (all elements)
    if indices.is_empty() {
        for child in &node.content {
            new_matches.push(child.clone());
        }
        return Ok(new_matches);
    }

    for index_node in indices {
        let index = match parse_int64(&index_node.value) {
            Ok(i) => i,
            Err(e) => {
                if prefs.optional_traverse {
                    continue;
                }
                return Err(e);
            }
        };

        let mut index_to_use = index;

        // Auto-fill nulls for out-of-range positive indices
        if index_to_use >= 0 {
            while (index_to_use as usize) >= node.content.len() {
                let null_child = create_scalar_node_null()?;
                node.add_child(&null_child)?;
            }
        }

        // Handle negative indices
        if index_to_use < 0 {
            index_to_use = node.content.len() as i64 + index_to_use;
        }
        if index_to_use < 0 {
            return Err(CoreError::Eval(EvalError::IndexOutOfRange {
                index: index_to_use,
                len: node.content.len(),
            }));
        }

        new_matches.push(node.content[index_to_use as usize].clone());
    }
    Ok(new_matches)
}

fn traverse_map_with_indices(
    ctx: &Context,
    candidate: &mut TreeNode,
    indices: &[TreeNode],
    prefs: &TraversePreferences,
    store: &TreeStore,
) -> Result<Vec<TreeNode>, CoreError> {
    if indices.is_empty() {
        let empty_key = create_string_scalar_node("")?;
        return traverse_map(ctx, candidate, &empty_key, prefs, true, store);
    }

    let mut out = Vec::new();
    for index_node in indices {
        let nodes = traverse_map(ctx, candidate, index_node, prefs, false, store)?;
        out.extend(nodes);
    }
    Ok(out)
}

fn traverse_array_indices(
    ctx: &Context,
    matching_node: &mut TreeNode,
    indices: &[TreeNode],
    prefs: &TraversePreferences,
    store: &TreeStore,
) -> Result<Vec<TreeNode>, CoreError> {
    if matching_node.sem_type == Some(SemType::Nil) {
        matching_node.tag = String::new();
        matching_node.sem_type = None;
        matching_node.kind = NodeKind::Sequence;
        if !indices.is_empty() && indices[0].sem_type != Some(SemType::Int) {
            matching_node.kind = NodeKind::Mapping;
        }
    }

    match matching_node.kind {
        NodeKind::Alias => {
            let resolved = resolve_alias_for_traverse(matching_node, prefs, store);
            match resolved {
                Some(mut r) => traverse_array_indices(ctx, &mut r, indices, prefs, store),
                None => Ok(Vec::new()),
            }
        }
        NodeKind::Sequence => {
            let mut node_clone = matching_node.clone();
            traverse_array_with_indices(&mut node_clone, indices, prefs)
        }
        NodeKind::Mapping => {
            let mut node_clone = matching_node.clone();
            traverse_map_with_indices(ctx, &mut node_clone, indices, prefs, store)
        }
        _ => Ok(Vec::new()),
    }
}

fn traverse_nodes_with_array_indices(
    ctx: &Context,
    indices: &[TreeNode],
    prefs: &TraversePreferences,
    store: &TreeStore,
) -> Result<Context, CoreError> {
    let mut matching_node_map = Vec::new();
    for candidate in &ctx.matching_nodes {
        let mut c = candidate.clone();
        let nodes = traverse_array_indices(ctx, &mut c, indices, prefs, store)?;
        matching_node_map.extend(nodes);
    }
    ctx.child_context(matching_node_map)
}

// ── Single-step traverse ───────────────────────────────────────────

fn traverse(
    ctx: &Context,
    matching_node: &mut TreeNode,
    operation: &Operation,
    store: &TreeStore,
) -> Result<Vec<TreeNode>, CoreError> {
    // Extract TraversePreferences from operation.preferences,
    let prefs = match operation.preferences.as_deref() {
        Some(OperationPreference::Traverse(p)) => p.clone(),
        _ => TraversePreferences::default(),
    };

    // Auto-coerce nil nodes
    if matching_node.sem_type == Some(SemType::Nil)
        && operation.string_value != "[]"
        && !ctx.dont_auto_create
    {
        if operation.string_value.parse::<i64>().is_ok() {
            matching_node.kind = NodeKind::Sequence;
        } else {
            matching_node.kind = NodeKind::Mapping;
        }
        matching_node.tag = String::new();
        matching_node.sem_type = None;
    }

    match matching_node.kind {
        NodeKind::Mapping => {
            let key_node = create_string_scalar_node(&operation.string_value)?;
            traverse_map(ctx, matching_node, &key_node, &prefs, false, store)
        }
        NodeKind::Sequence => {
            let idx_node = create_string_scalar_node(&operation.string_value)?;
            let indices = vec![(*idx_node).clone()];
            traverse_array_with_indices(matching_node, &indices, &prefs)
        }
        NodeKind::Alias => {
            let resolved = resolve_alias_for_traverse(matching_node, &prefs, store);
            match resolved {
                Some(mut r) => traverse(ctx, &mut r, operation, store),
                None => Ok(Vec::new()),
            }
        }
        _ => Ok(Vec::new()),
    }
}

// ── Public operators ───────────────────────────────────────────────

/// Splat: return all children of the current matching nodes.
pub fn splat(
    ctx: Context,
    prefs: TraversePreferences,
    store: &TreeStore,
) -> Result<Context, CoreError> {
    traverse_nodes_with_array_indices(&ctx, &[], &prefs, store)
}

/// Path traversal operator: descend into matching nodes by key/index.
pub fn traverse_path_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut matches = Vec::new();
    for candidate in &ctx.matching_nodes {
        let mut c = candidate.clone();
        let new_nodes = traverse(&ctx, &mut c, &expression_node.operation, &d.store)?;
        matches.extend(new_nodes);
    }
    ctx.child_context(matches)
}

// ── Slice / array traversal ────────────────────────────────────

fn get_slice_number(
    d: &mut TreeEngine,
    ctx: &Context,
    node: &TreeNode,
    expression_node: &mut ExpressionNode,
) -> Result<i64, CoreError> {
    let single_ctx = ctx.single_child_context(node)?;
    let result = get_matching_nodes(d, &single_ctx, Some(expression_node))?;
    if result.matching_nodes.len() != 1 {
        return Err(CoreError::Eval(EvalError::ExpectedSingleNumber));
    }
    result.matching_nodes[0]
        .value
        .parse::<i64>()
        .map_err(|_| CoreError::Eval(EvalError::CannotConvertNodeToNumber))
}

fn slice_array_operator_inner(
    ctx: &Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let lhs_expr = expression_node
        .lhs
        .as_mut()
        .ok_or(CoreError::Eval(EvalError::MissingLhs))?;
    let rhs_expr = expression_node
        .rhs
        .as_mut()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
    let mut results = Vec::new();

    for lhs_node in &ctx.matching_nodes {
        let length_i64 = lhs_node.content.len() as i64;

        let mut start = get_slice_number(d, ctx, lhs_node, lhs_expr)?;
        if start < 0 {
            start = length_i64 + start;
        }
        if start < 0 {
            return Err(CoreError::Eval(EvalError::IndexOutOfRange {
                index: start,
                len: lhs_node.content.len(),
            }));
        }

        let mut end = get_slice_number(d, ctx, lhs_node, rhs_expr)?;
        if end < 0 {
            end = length_i64 + end;
        }
        if end < 0 {
            return Err(CoreError::Eval(EvalError::IndexOutOfRange {
                index: end,
                len: lhs_node.content.len(),
            }));
        }
        if end > length_i64 {
            end = length_i64;
        }

        let mut slice_node = lhs_node.create_replacement(NodeKind::Sequence, &lhs_node.tag, "")?;

        if start < end {
            let children: Vec<&TreeNode> = (start..end)
                .map(|i| &lhs_node.content[i as usize])
                .collect();
            slice_node.add_children(&children.iter().map(|&c| c.clone()).collect::<Vec<_>>())?;
        }

        results.push((*slice_node).clone());
    }

    ctx.child_context(results)
}

/// Array traversal operator: index into arrays, with slice support.
pub fn traverse_array_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    // Check for slice syntax: rhs.rhs is a create_map
    if let Some(ref rhs) = expression_node.rhs {
        if let Some(ref slice) = rhs.rhs {
            if slice.operation.operation_type.id == CREATE_MAP_OP_TYPE.id {
                return slice_array_operator_inner(&ctx, d, &mut (&**slice).clone());
            }
        }
    }

    // Get LHS and RHS contexts
    let lhs_ctx = get_matching_nodes(d, &ctx, expression_node.lhs.as_deref_mut())?;
    let ro = ctx.read_only_clone()?;
    let rhs_ctx = get_matching_nodes(d, &ro, expression_node.rhs.as_deref_mut())?;

    // Extract TraversePreferences from operation.preferences,
    let prefs = match expression_node.operation.preferences.as_deref() {
        Some(OperationPreference::Traverse(p)) => p.clone(),
        _ => TraversePreferences::default(),
    };

    let indices: Vec<TreeNode> = if rhs_ctx.matching_nodes.is_empty() {
        Vec::new()
    } else {
        rhs_ctx.matching_nodes[0].content.clone()
    };

    let result = traverse_nodes_with_array_indices(&lhs_ctx, &indices, &prefs, &d.store)?;
    ctx.child_context(result.matching_nodes)
}
