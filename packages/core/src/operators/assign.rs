use crate::core::ParsedKey;
use crate::operators::*;

#[derive(Debug, Clone)]
enum AssignPathSegment {
    Static(ParsedKey),
    Dynamic(ExpressionNode),
}

fn get_assign_preferences(expression_node: &ExpressionNode) -> AssignPreferences {
    match expression_node.operation.preferences.as_deref() {
        Some(OperationPreference::Assign(p)) => p.clone(),
        _ => AssignPreferences::default(),
    }
}

/// Update attributes (kind, tag, alias, comments, etc.) from another node,
/// following assignment preference rules.
fn update_attributes_from(n: &mut TreeNode, other: &TreeNode, prefs: &AssignPreferences) {
    if n.kind != other.kind {
        n.content.clear();
        n.value.clear();
    }
    n.kind = other.kind;

    if prefs.clobber_custom_tags || SemType::has_tag_prefix(&n.tag) || n.tag.is_empty() {
        n.tag = other.tag.clone();
        n.sem_type = other.resolved_sem_type();
    }

    n.alias = other.alias.clone();
    if !prefs.dont_overwrite_anchor && !other.anchor.is_empty() {
        n.anchor = other.anchor.clone();
    }
    n.encode_separate = other.encode_separate;

    if !other.foot_comment.is_empty() {
        n.foot_comment = other.foot_comment.clone();
    }
    if !other.head_comment.is_empty() {
        n.head_comment = other.head_comment.clone();
    }
    if !other.line_comment.is_empty() {
        n.line_comment = other.line_comment.clone();
    }
}

/// Update a node's content and attributes from another node.
fn update_from(
    n: &mut TreeNode,
    other: &TreeNode,
    prefs: &AssignPreferences,
) -> Result<(), CoreError> {
    if std::ptr::eq(n, other) {
        return Ok(());
    }

    n.content.clear();
    n.kind = other.kind;
    n.add_children(&other.content)?;
    n.value = other.value.clone();
    update_attributes_from(n, other, prefs);
    Ok(())
}

fn nodes_structurally_eq(a: &TreeNode, b: &TreeNode) -> bool {
    if a.kind != b.kind {
        return false;
    }
    if a.tag != b.tag {
        return false;
    }
    if a.value != b.value {
        return false;
    }
    if a.is_map_key != b.is_map_key {
        return false;
    }
    if a.sequence_index != b.sequence_index {
        return false;
    }
    if a.parent.is_some() && b.parent.is_some() && a.parent != b.parent {
        return false;
    }
    if a.key.is_some() && b.key.is_some() && a.key != b.key {
        return false;
    }
    if a.start_byte != b.start_byte || a.end_byte != b.end_byte {
        return false;
    }
    if a.document != b.document || a.file_index != b.file_index {
        return false;
    }
    if a.line != b.line || a.column != b.column {
        return false;
    }
    if a.filename != b.filename {
        return false;
    }
    if a.content.len() != b.content.len() {
        return false;
    }
    a.content
        .iter()
        .zip(&b.content)
        .all(|(a_child, b_child)| nodes_structurally_eq(a_child, b_child))
}

fn find_and_update_in_place(
    haystack: &mut [TreeNode],
    needle: &TreeNode,
    src: &TreeNode,
    prefs: &AssignPreferences,
) -> Result<bool, CoreError> {
    for node in haystack.iter_mut() {
        if nodes_structurally_eq(node, needle) {
            update_from(node, src, prefs)?;
            return Ok(true);
        }
        if find_and_update_in_place(&mut node.content, needle, src, prefs)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Assign-update operator (=): update matching nodes from RHS results.
pub fn assign_update_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let prefs = get_assign_preferences(expression_node);
    let collected_path = expression_node
        .lhs
        .as_deref()
        .map(collect_assign_path)
        .transpose()?;

    if let Some(path) = collected_path.clone().flatten() {
        let eval_ctx = if expression_node.operation.update_assign {
            get_matching_nodes(d, &ctx, expression_node.lhs.as_deref_mut())?
        } else {
            ctx.clone()
        };
        let resolved_path = resolve_assign_path(d, &eval_ctx, &path)?;
        let rhs_ctx = get_matching_nodes(d, &eval_ctx, expression_node.rhs.as_deref_mut())?;

        let assigned = rhs_ctx.matching_nodes.first().cloned().unwrap_or_else(|| {
            let mut null = TreeNode::scalar(SemType::Nil, "null");
            null.value = "null".to_string();
            null
        });

        let mut updated = ctx.clone();
        for node in &mut updated.matching_nodes {
            assign_path(node, &resolved_path, &assigned, &prefs)?;
        }
        sync_single_root_store(d, &updated);
        return Ok(updated);
    }

    if !expression_node.operation.update_assign {
        let split_probe = expression_node
            .lhs
            .as_deref()
            .map(split_assign_anchor_and_path)
            .transpose()?;
        if let Some((mut anchor_expr, relative_path)) = split_probe.flatten() {
            let anchors = get_matching_nodes(d, &ctx, Some(&mut anchor_expr))?;

            let mut ro = ctx.read_only_clone()?;
            ro.assign_prefs = prefs.clone();
            let rhs = get_matching_nodes(d, &ro, expression_node.rhs.as_deref_mut())?;
            let assigned = rhs.matching_nodes.first().cloned().unwrap_or_else(|| {
                let mut null = TreeNode::scalar(SemType::Nil, "null");
                null.value = "null".to_string();
                null
            });

            let mut updated = ctx.clone();
            let mut i = anchors.matching_nodes.len();
            while i > 0 {
                i -= 1;
                let candidate = &anchors.matching_nodes[i];
                let resolved_path = resolve_assign_path(
                    d,
                    &ctx.single_readonly_child_context(candidate)?,
                    &relative_path,
                )?;
                let mut replacement = candidate.clone();
                assign_path(&mut replacement, &resolved_path, &assigned, &prefs)?;
                let _ = find_and_update_in_place(
                    &mut updated.matching_nodes,
                    candidate,
                    &replacement,
                    &prefs,
                )?;
            }

            sync_single_root_store(d, &updated);
            return Ok(updated);
        }
    }

    let lhs = get_matching_nodes(d, &ctx, expression_node.lhs.as_deref_mut())?;

    if !expression_node.operation.update_assign {
        let mut ro = ctx.read_only_clone()?;
        ro.assign_prefs = prefs.clone();
        let rhs = get_matching_nodes(d, &ro, expression_node.rhs.as_deref_mut())?;
        if rhs.matching_nodes.is_empty() {
            return Ok(ctx);
        }

        let mut updated = ctx.clone();
        let mut i = lhs.matching_nodes.len();
        while i > 0 {
            i -= 1;
            let candidate = &lhs.matching_nodes[i];
            let mut replacement = candidate.clone();
            if !prefs.only_write_null || replacement.resolved_sem_type() == Some(SemType::Nil) {
                for rhs_candidate in &rhs.matching_nodes {
                    update_from(&mut replacement, rhs_candidate, &prefs)?;
                }
            }
            let _ = find_and_update_in_place(
                &mut updated.matching_nodes,
                candidate,
                &replacement,
                &prefs,
            )?;
        }
        sync_single_root_store(d, &updated);
        return Ok(updated);
    }

    let mut updated = ctx.clone();

    // Reverse iterate for safe in-place updates
    let mut i = lhs.matching_nodes.len();
    while i > 0 {
        i -= 1;
        let candidate = &lhs.matching_nodes[i];
        let rhs = get_matching_nodes(
            d,
            &ctx.single_child_context(candidate)?,
            expression_node.rhs.as_deref_mut(),
        )?;
        if !rhs.matching_nodes.is_empty() {
            let _ = find_and_update_in_place(
                &mut updated.matching_nodes,
                candidate,
                &rhs.matching_nodes[0],
                &prefs,
            )?;
        }
    }

    sync_single_root_store(d, &updated);
    Ok(updated)
}

fn collect_assign_path(node: &ExpressionNode) -> Result<Option<Vec<AssignPathSegment>>, CoreError> {
    match node.operation.operation_type.id {
        OperationId::SelfReference => Ok(Some(Vec::new())),
        OperationId::ShortPipe | OperationId::Pipe => {
            let lhs = node
                .lhs
                .as_deref()
                .ok_or(CoreError::Eval(EvalError::MissingLhs))?;
            let rhs = node
                .rhs
                .as_deref()
                .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
            let Some(mut path) = collect_assign_path(lhs)? else {
                return Ok(None);
            };
            let Some(rhs_path) = collect_assign_path(rhs)? else {
                return Ok(None);
            };
            path.extend(rhs_path);
            Ok(Some(path))
        }
        OperationId::TraversePath => {
            let mut path = match node.lhs.as_deref() {
                Some(lhs) => match collect_assign_path(lhs)? {
                    Some(path) => path,
                    None => return Ok(None),
                },
                None => Vec::new(),
            };
            path.push(match node.rhs.as_deref() {
                Some(rhs) => extract_assign_path_segment(rhs)?,
                None => {
                    AssignPathSegment::Static(ParsedKey::Str(node.operation.string_value.clone()))
                }
            });
            Ok(Some(path))
        }
        OperationId::TraverseArray => {
            let lhs = node
                .lhs
                .as_deref()
                .ok_or(CoreError::Eval(EvalError::MissingLhs))?;
            let rhs = node
                .rhs
                .as_deref()
                .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
            let Some(mut path) = collect_assign_path(lhs)? else {
                return Ok(None);
            };
            let Some(segment) = extract_array_assign_path_segment(rhs)? else {
                return Ok(None);
            };
            path.push(segment);
            Ok(Some(path))
        }
        _ => Ok(None),
    }
}

fn split_assign_anchor_and_path(
    node: &ExpressionNode,
) -> Result<Option<(ExpressionNode, Vec<AssignPathSegment>)>, CoreError> {
    match node.operation.operation_type.id {
        OperationId::TraversePath => {
            let lhs = node
                .lhs
                .as_deref()
                .ok_or(CoreError::Eval(EvalError::MissingLhs))?;
            let segment = match node.rhs.as_deref() {
                Some(rhs) => extract_assign_path_segment(rhs)?,
                None => {
                    AssignPathSegment::Static(ParsedKey::Str(node.operation.string_value.clone()))
                }
            };

            if let Some((anchor, mut path)) = split_assign_anchor_and_path(lhs)? {
                path.push(segment);
                return Ok(Some((anchor, path)));
            }

            if collect_assign_path(lhs)?.is_none() {
                return Ok(Some((lhs.clone(), vec![segment])));
            }

            Ok(None)
        }
        OperationId::TraverseArray => {
            let lhs = node
                .lhs
                .as_deref()
                .ok_or(CoreError::Eval(EvalError::MissingLhs))?;
            let rhs = node
                .rhs
                .as_deref()
                .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
            let Some(segment) = extract_array_assign_path_segment(rhs)? else {
                return Ok(None);
            };

            if let Some((anchor, mut path)) = split_assign_anchor_and_path(lhs)? {
                path.push(segment);
                return Ok(Some((anchor, path)));
            }

            if collect_assign_path(lhs)?.is_none() {
                return Ok(Some((lhs.clone(), vec![segment])));
            }

            Ok(None)
        }
        OperationId::ShortPipe | OperationId::Pipe => {
            let lhs = node
                .lhs
                .as_deref()
                .ok_or(CoreError::Eval(EvalError::MissingLhs))?;
            let rhs = node
                .rhs
                .as_deref()
                .ok_or(CoreError::Eval(EvalError::MissingRhs))?;

            if let Some(path) = collect_assign_path(rhs)? {
                return Ok(Some((lhs.clone(), path)));
            }

            if let Some((rhs_anchor, path)) = split_assign_anchor_and_path(rhs)? {
                return Ok(Some((
                    ExpressionNode {
                        operation: node.operation.clone(),
                        lhs: Some(Box::new(lhs.clone())),
                        rhs: Some(Box::new(rhs_anchor)),
                    },
                    path,
                )));
            }

            Ok(None)
        }
        _ => Ok(None),
    }
}

fn extract_assign_path_segment(node: &ExpressionNode) -> Result<AssignPathSegment, CoreError> {
    if let Some(key) = extract_literal_path_key(node)? {
        return Ok(AssignPathSegment::Static(key));
    }
    Ok(AssignPathSegment::Dynamic(node.clone()))
}

fn extract_array_assign_path_segment(
    node: &ExpressionNode,
) -> Result<Option<AssignPathSegment>, CoreError> {
    match node.operation.operation_type.id {
        OperationId::Collect => match node.rhs.as_deref() {
            Some(inner) if inner.operation.operation_type.id == OperationId::Empty => Ok(None),
            Some(inner) => extract_array_assign_path_segment(inner),
            None => Err(CoreError::Eval(EvalError::MissingRhs)),
        },
        _ => Ok(Some(extract_assign_path_segment(node)?)),
    }
}

fn extract_literal_path_key(node: &ExpressionNode) -> Result<Option<ParsedKey>, CoreError> {
    match node.operation.operation_type.id {
        OperationId::Value => parsed_key_from_tree_node(
            node.operation
                .tree_node
                .as_deref()
                .ok_or(CoreError::Parse(ParseError::InvalidSyntax))?,
        )
        .map(Some),
        _ => Ok(None),
    }
}

fn parsed_key_from_tree_node(node: &TreeNode) -> Result<ParsedKey, CoreError> {
    match node.resolved_sem_type() {
        Some(SemType::Int) => node
            .value
            .parse::<i64>()
            .map(ParsedKey::Int)
            .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax)),
        Some(SemType::Str) => Ok(ParsedKey::Str(node.value.clone())),
        _ => Err(CoreError::Parse(ParseError::InvalidSyntax)),
    }
}

fn assign_path(
    target: &mut TreeNode,
    path: &[ParsedKey],
    assigned: &TreeNode,
    prefs: &AssignPreferences,
) -> Result<(), CoreError> {
    if path.is_empty() {
        if !prefs.only_write_null || target.resolved_sem_type() == Some(SemType::Nil) {
            update_from(target, assigned, prefs)?;
        }
        return Ok(());
    }

    match &path[0] {
        ParsedKey::Str(key) => {
            ensure_mapping(target);
            let value_index = find_or_create_map_value(target, key)?;
            assign_path(
                &mut target.content[value_index],
                &path[1..],
                assigned,
                prefs,
            )
        }
        ParsedKey::Int(index) => {
            let index: usize = (*index)
                .try_into()
                .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
            ensure_sequence(target);
            while target.content.len() <= index {
                let null = TreeNode::scalar(SemType::Nil, "null");
                target.add_child(&null)?;
            }
            assign_path(&mut target.content[index], &path[1..], assigned, prefs)
        }
    }
}

fn resolve_assign_path(
    d: &mut TreeEngine,
    ctx: &Context,
    path: &[AssignPathSegment],
) -> Result<Vec<ParsedKey>, CoreError> {
    path.iter()
        .map(|segment| match segment {
            AssignPathSegment::Static(key) => Ok(key.clone()),
            AssignPathSegment::Dynamic(expr) => resolve_dynamic_assign_path_segment(d, ctx, expr),
        })
        .collect()
}

fn resolve_dynamic_assign_path_segment(
    d: &mut TreeEngine,
    ctx: &Context,
    expr: &ExpressionNode,
) -> Result<ParsedKey, CoreError> {
    let mut expr = expr.clone();
    let resolved = get_matching_nodes(d, ctx, Some(&mut expr))?;
    if resolved.matching_nodes.len() != 1 {
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }
    parsed_key_from_tree_node(&resolved.matching_nodes[0])
}

fn ensure_mapping(target: &mut TreeNode) {
    if target.kind == NodeKind::Mapping {
        return;
    }
    target.kind = NodeKind::Mapping;
    target.sem_type = Some(SemType::Map);
    target.tag = SemType::Map.tag().to_string();
    target.value.clear();
    target.content.clear();
}

fn ensure_sequence(target: &mut TreeNode) {
    if target.kind == NodeKind::Sequence {
        return;
    }
    target.kind = NodeKind::Sequence;
    target.sem_type = Some(SemType::Seq);
    target.tag = SemType::Seq.tag().to_string();
    target.value.clear();
    target.content.clear();
}

fn find_or_create_map_value(target: &mut TreeNode, key: &str) -> Result<usize, CoreError> {
    let mut i = 0;
    while i + 1 < target.content.len() {
        if target.content[i].value == key && target.content[i].is_map_key {
            return Ok(i + 1);
        }
        i += 2;
    }

    let mut key_node = TreeNode::scalar(SemType::Str, key);
    key_node.is_map_key = true;
    let value_node = TreeNode::scalar(SemType::Nil, "null");
    target.add_key_value_child(&key_node, &value_node)?;
    Ok(target.content.len() - 1)
}

fn sync_single_root_store(d: &mut TreeEngine, updated: &Context) {
    if d.store.len() == 1 && updated.matching_nodes.len() == 1 {
        let root = d.store.get_mut(crate::operators::NodeId(0));
        *root = updated.matching_nodes[0].clone();
    }
}
