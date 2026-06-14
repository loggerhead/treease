use crate::operators::*;

/// Build a single entry map {key: ..., value: ...} from a key and value node.
fn entry_map_for(key: &TreeNode, value: &TreeNode) -> Result<TreeNode, CoreError> {
    let key_key_node = *create_string_scalar_node("key")?;
    let value_key_node = *create_string_scalar_node("value")?;
    let mut candidate = TreeNode::default();
    candidate.kind = NodeKind::Mapping;
    candidate.sem_type = Some(SemType::Map);
    candidate.tag = SemType::Map.to_string().into();
    candidate.add_key_value_child(&key_key_node, key)?;
    candidate.add_key_value_child(&value_key_node, value)?;
    Ok(candidate)
}

/// Convert a mapping into an array of {key, value} entry maps.
fn to_entries_from_map(tree_node: &TreeNode) -> Result<TreeNode, CoreError> {
    let mut sequence = *tree_node
        .create_replacement_with_comments(NodeKind::Sequence, SemType::Seq.to_string())?;
    let mut i: usize = 0;
    while i + 1 < tree_node.content.len() {
        let key = &tree_node.content[i];
        let value = &tree_node.content[i + 1];
        let entry = entry_map_for(key, value)?;
        sequence.add_child(&entry)?;
        i += 2;
    }
    Ok(sequence)
}

/// Convert a sequence into an array of {key: <index>, value: <element>} entry maps.
fn to_entries_from_seq(tree_node: &TreeNode) -> Result<TreeNode, CoreError> {
    let mut sequence = *tree_node
        .create_replacement_with_comments(NodeKind::Sequence, SemType::Seq.to_string())?;
    for (idx, value) in tree_node.content.iter().enumerate() {
        let key = *create_scalar_node_i64(idx as i64)?;
        let entry = entry_map_for(&key, value)?;
        sequence.add_child(&entry)?;
    }
    Ok(sequence)
}

/// Parse a single entry map {key: ..., value: ...} back into (key, value) pair.
fn parse_entry(tree_node: &TreeNode) -> Result<(TreeNode, TreeNode), CoreError> {
    if tree_node.kind != NodeKind::Mapping {
        return Err(CoreError::Eval(EvalError::ExpectedMap));
    }

    let mut key_count: usize = 0;
    let mut value_count: usize = 0;
    let mut found_key: Option<TreeNode> = None;
    let mut found_value: Option<TreeNode> = None;

    let mut i: usize = 0;
    while i + 1 < tree_node.content.len() {
        let k = &tree_node.content[i];
        let v = &tree_node.content[i + 1];
        if k.value == "key" {
            key_count += 1;
            found_key = Some(v.clone());
        }
        if k.value == "value" {
            value_count += 1;
            found_value = Some(v.clone());
        }
        i += 2;
    }

    if key_count != 1 || value_count != 1 {
        return Err(CoreError::Parse(ParseError::InvalidSyntax));
    }
    let key = found_key.ok_or(CoreError::Parse(ParseError::InvalidSyntax))?;
    let value = found_value.ok_or(CoreError::Parse(ParseError::InvalidSyntax))?;
    Ok((key, value))
}

/// Convert an array of entry maps back into a single mapping.
fn from_entries(tree_node: &TreeNode) -> Result<TreeNode, CoreError> {
    let mut node = *tree_node.copy_without_content()?;
    for entry in &tree_node.content {
        let (key, value) = parse_entry(entry)?;
        node.add_key_value_child(&key, &value)?;
    }
    node.kind = NodeKind::Mapping;
    node.sem_type = Some(SemType::Map);
    node.tag = SemType::Map.to_string().into();
    Ok(node)
}

/// Convert maps/arrays into an array of entry maps (to_entries operator).
pub fn to_entries_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        match candidate.kind {
            NodeKind::Mapping => {
                results.push(to_entries_from_map(candidate)?);
            }
            NodeKind::Sequence => {
                results.push(to_entries_from_seq(candidate)?);
            }
            _ => {
                if candidate.sem_type != Some(SemType::Nil) {
                    return Err(CoreError::Eval(EvalError::NoKeys));
                }
            }
        }
    }
    ctx.child_context(results)
}

/// Convert an array of entry maps back into a mapping (from_entries operator).
pub fn from_entries_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        if candidate.kind != NodeKind::Sequence {
            return Err(CoreError::Eval(EvalError::FromEntriesOnlyRunsAgainstArrays));
        }
        results.push(from_entries(candidate)?);
    }
    ctx.child_context(results)
}

/// Collect RHS expression results across matching nodes into a single sequence.
fn collect_together(
    d: &mut TreeEngine,
    ctx: &Context,
    expression_node: &mut ExpressionNode,
) -> Result<TreeNode, CoreError> {
    let mut collected = TreeNode::default();
    collected.kind = NodeKind::Sequence;
    collected.sem_type = Some(SemType::Seq);
    collected.tag = SemType::Seq.to_string().into();

    for candidate in &ctx.matching_nodes {
        let exp_results = get_matching_nodes(
            d,
            &ctx.single_readonly_child_context(candidate)?,
            expression_node.rhs.as_deref_mut(),
        )?;
        for r in &exp_results.matching_nodes {
            collected.add_child(r)?;
        }
    }
    Ok(collected)
}

/// Apply RHS expression to entries and merge back into a map (with_entries operator).
pub fn with_entries_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    expression_node
        .rhs
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
    let mut self_expr = create_traversal_tree(&[], TraversePreferences::default(), false)?;

    // 1. Convert to entries
    let to_entries = to_entries_operator(ctx.read_only_clone()?, d, expression_node)?;
    let mut results = Vec::new();

    for entries_list in &to_entries.matching_nodes {
        // 2. Splat entries list
        let splatted = splat(
            ctx.single_child_context(entries_list)?,
            TraversePreferences::default(),
        )?;

        // 3. Execute RHS expression on each entry
        let mut new_results = Vec::new();
        for item in &splatted.matching_nodes {
            let inner_ctx = splatted.single_child_context(item)?;
            let r = get_matching_nodes(d, &inner_ctx, expression_node.rhs.as_deref_mut())?;
            new_results.extend(r.matching_nodes.clone());
        }

        // 4. Collect results and restore original comments
        let collected_ctx = splatted.child_context(new_results)?;
        let mut collected = collect_together(d, &collected_ctx, &mut *self_expr)?;
        collected.leading_content = entries_list.leading_content.clone();
        collected.head_comment = entries_list.head_comment.clone();
        collected.foot_comment = entries_list.foot_comment.clone();

        // 5. Convert back to Map
        let single_ctx = ctx.single_child_context(&collected)?;
        let from_entries = from_entries_operator(single_ctx, d, expression_node)?;
        results.extend(from_entries.matching_nodes.clone());
    }

    ctx.child_context(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(value: &str) -> TreeNode {
        TreeNode {
            value: value.to_string(),
            ..TreeNode::default()
        }
    }

    #[test]
    fn parse_entry_should_return_error_when_key_is_missing() {
        let mut entry = TreeNode {
            kind: NodeKind::Mapping,
            sem_type: Some(SemType::Map),
            tag: SemType::Map.to_string().into(),
            ..TreeNode::default()
        };
        let value_key = scalar("value");
        let value = scalar("actual");
        assert!(entry.add_key_value_child(&value_key, &value).is_ok());

        assert!(matches!(
            parse_entry(&entry),
            Err(CoreError::Parse(ParseError::InvalidSyntax))
        ));
    }

    #[test]
    fn parse_entry_should_return_error_when_value_is_missing() {
        let mut entry = TreeNode {
            kind: NodeKind::Mapping,
            sem_type: Some(SemType::Map),
            tag: SemType::Map.to_string().into(),
            ..TreeNode::default()
        };
        let key_key = scalar("key");
        let key = scalar("actual");
        assert!(entry.add_key_value_child(&key_key, &key).is_ok());

        assert!(matches!(
            parse_entry(&entry),
            Err(CoreError::Parse(ParseError::InvalidSyntax))
        ));
    }
}
