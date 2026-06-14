use crate::operators::operator_helpers::*;
use crate::operators::*;

/// Pick from a map node using keys from the indices list.
fn pick_map(original: &TreeNode, indices: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut out = (*original.copy_without_content()?).clone();

    for key_to_find in &indices.content {
        let idx_in_map = find_key_in_map(original, key_to_find);
        if idx_in_map >= 0 {
            let idx = idx_in_map as usize;
            let cloned_key = original.content[idx].copy()?;
            let cloned_value = original.content[idx + 1].copy()?;
            out.add_key_value_child(&cloned_key, &cloned_value)?;
        }
    }

    Ok(Box::new(out))
}

/// Pick from a sequence node using integer indices from the indices list.
fn pick_sequence(original: &TreeNode, indices: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut out = (*original.copy_without_content()?).clone();

    for idx_node in &indices.content {
        let index_in_array = parse_int64(&idx_node.value)
            .map_err(|_| CoreError::Eval(EvalError::CannotIndexArray))?;
        if index_in_array < 0 {
            continue;
        }
        let idx = index_in_array as usize;
        if idx >= original.content.len() {
            continue;
        }
        let el = &original.content[idx];
        out.add_child(&*el.copy()?)?;
    }

    Ok(Box::new(out))
}

/// Pick elements from a map or sequence based on index/keys.
pub fn pick_with_nodes(node: &TreeNode, indices: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut replacement = match node.kind {
        NodeKind::Mapping => pick_map(node, indices)?,
        NodeKind::Sequence => pick_sequence(node, indices)?,
        _ => return Err(CoreError::Eval(EvalError::CannotPickIndicesFromType)),
    };
    replacement.leading_content = node.leading_content.clone();
    Ok(replacement)
}

/// pick operator: select elements from maps/sequences by key/index.
pub fn pick_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let context_indices = get_matching_nodes(d, &ctx, expression_node.rhs.as_deref_mut())?;

    let indices_to_pick = if !context_indices.matching_nodes.is_empty() {
        context_indices.matching_nodes[0].clone()
    } else {
        TreeNode::default()
    };

    let mut results: Vec<TreeNode> = Vec::new();
    for node in &ctx.matching_nodes {
        let replacement = match pick_with_nodes(node, &indices_to_pick) {
            Ok(r) => r,
            Err(CoreError::Eval(EvalError::CannotPickIndicesFromType)) => {
                let nice = node.get_nice_path()?;
                if let Some(ref d2) = ctx.diagnostics {
                    d2.set_messagef(
                        "eval",
                        &format!("cannot pick indices from type {} ({})", node.tag, nice),
                    )?;
                }
                return Err(CoreError::Eval(EvalError::CannotPickIndicesFromType));
            }
            Err(CoreError::Eval(EvalError::CannotIndexArray)) => {
                if let Some(ref d2) = ctx.diagnostics {
                    d2.set_messagef("eval", "cannot index array with non-integer")?;
                }
                return Err(CoreError::Eval(EvalError::CannotIndexArray));
            }
            Err(e) => return Err(e),
        };
        results.push((*replacement).clone());
    }
    ctx.child_context(results)
}
