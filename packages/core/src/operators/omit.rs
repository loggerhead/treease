use crate::operators::*;

/// Find the position of an item in an array node using recursive equality.
fn find_in_array(array: &TreeNode, item: &TreeNode) -> i32 {
    for (i, child) in array.content.iter().enumerate() {
        if crate::core::recursive_node_compare(child, item) {
            return i as i32;
        }
    }
    -1
}

/// Omit keys from a map node.
fn omit_map(original: &TreeNode, indices: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut out = (*original.copy_without_content()?).clone();

    let mut i = 0;
    while i + 1 < original.content.len() {
        let key = &original.content[i];
        let value = &original.content[i + 1];
        let pos = find_in_array(indices, key);
        if pos < 0 {
            out.add_key_value_child(key, value)?;
        }
        i += 2;
    }

    Ok(Box::new(out))
}

/// Omit indices from a sequence node.
fn omit_sequence(original: &TreeNode, indices: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut out = (*original.copy_without_content()?).clone();

    for (idx, child) in original.content.iter().enumerate() {
        let key_str = idx.to_string();
        let mut idx_node = TreeNode::default();
        idx_node.kind = NodeKind::Scalar;
        idx_node.sem_type = Some(SemType::Int);
        idx_node.tag = SemType::Int.to_string().into();
        idx_node.value = key_str;
        let pos = find_in_array(indices, &idx_node);
        if pos < 0 {
            out.add_child(child)?;
        }
    }

    Ok(Box::new(out))
}

/// omit operator: exclude specified keys or indices from maps or arrays.
pub fn omit_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let context_indices = get_matching_nodes(d, &ctx, expression_node.rhs.as_deref_mut())?;
    if context_indices.matching_nodes.is_empty() {
        return Ok(ctx);
    }
    let indices_to_omit = &context_indices.matching_nodes[0];
    if indices_to_omit.content.is_empty() {
        return Ok(ctx);
    }

    let mut results: Vec<TreeNode> = Vec::new();
    for node in &ctx.matching_nodes {
        let mut replacement = match node.kind {
            NodeKind::Mapping => omit_map(node, indices_to_omit)?,
            NodeKind::Sequence => omit_sequence(node, indices_to_omit)?,
            _ => return Ok(ctx),
        };
        replacement.leading_content = node.leading_content.clone();
        results.push((*replacement).clone());
    }
    ctx.child_context(results)
}
