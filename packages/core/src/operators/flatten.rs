use crate::operators::*;

/// Extract the flatten depth preference from the expression node.
/// Defaults to -1 (unlimited depth) if no preference is set.
fn get_flatten_depth(expression_node: &ExpressionNode) -> i32 {
    match expression_node.operation.preferences.as_deref() {
        Some(OperationPreference::Flatten(p)) => p.depth,
        _ => FlattenPreferences::default().depth,
    }
}

/// Recursively flatten an array node up to the given depth.
fn flatten_node(node: &mut TreeNode, depth: i32) {
    if depth == 0 {
        return;
    }
    if node.kind != NodeKind::Sequence {
        return;
    }

    let mut new_seq: Vec<TreeNode> = Vec::new();

    for child in &node.content {
        if child.kind == NodeKind::Sequence {
            // Recursively flatten the child array with depth-1.
            let mut child_clone = child.clone();
            flatten_node(&mut child_clone, depth - 1);
            // Append the child's contents to the new sequence.
            new_seq.extend(child_clone.content.iter().cloned());
        } else {
            new_seq.push(child.clone());
        }
    }

    // Replace the node's content with the flattened result.
    node.content = new_seq;
}

/// flatten operator: flatten arrays by the specified depth.
pub fn flatten_op(
    ctx: Context,
    _d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let depth = get_flatten_depth(expression_node);
    let mut results = Vec::new();

    for candidate in &ctx.matching_nodes {
        if candidate.kind != NodeKind::Sequence {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_message("eval", "only arrays are supported for flatten")?;
            }
            return Err(CoreError::OperatorMessage {
                op: "flatten".to_string(),
                message: "only arrays are supported for flatten".to_string(),
            });
        }
        let mut candidate = candidate.clone();
        flatten_node(&mut candidate, depth);
        results.push(candidate);
    }

    ctx.child_context(results)
}
