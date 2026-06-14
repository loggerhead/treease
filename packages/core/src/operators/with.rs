use crate::operators::*;

fn nodes_structurally_eq(a: &TreeNode, b: &TreeNode) -> bool {
    if a.kind != b.kind
        || a.tag != b.tag
        || a.value != b.value
        || a.is_map_key != b.is_map_key
        || a.sequence_index != b.sequence_index
        || a.document != b.document
        || a.file_index != b.file_index
        || a.filename != b.filename
        || a.content.len() != b.content.len()
    {
        return false;
    }

    a.content
        .iter()
        .zip(&b.content)
        .all(|(lhs, rhs)| nodes_structurally_eq(lhs, rhs))
}

fn write_back_updated_node(
    haystack: &mut [TreeNode],
    needle: &TreeNode,
    replacement: &TreeNode,
) -> bool {
    for node in haystack.iter_mut() {
        if nodes_structurally_eq(node, needle) {
            *node = replacement.clone();
            return true;
        }
        if write_back_updated_node(&mut node.content, needle, replacement) {
            return true;
        }
    }
    false
}

/// Execute an update expression on the matching context nodes (with operator).
pub fn with_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let rhs = expression_node
        .rhs
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
    if rhs.operation.operation_type.id != BLOCK_OP_TYPE.id {
        if let Some(ref d2) = ctx.diagnostics {
            d2.set_messagef(
                "eval",
                &format!(
                    "with must be given a block (;), got {} instead",
                    rhs.operation.operation_type.name()
                ),
            )?;
        }
        return Err(CoreError::OperatorMessage {
            op: "with".to_string(),
            message: "must be given a block".to_string(),
        });
    }

    let path_exp = rhs
        .lhs
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingLhs))?;
    let update_context = get_matching_nodes(d, &ctx, Some(&mut *(*path_exp).clone()))?;
    let update_exp = rhs
        .rhs
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;

    let mut updated_ctx = ctx.clone();
    for candidate in &update_context.matching_nodes {
        let candidate_result = get_matching_nodes(
            d,
            &update_context.single_child_context(candidate)?,
            Some(&mut *(*update_exp).clone()),
        )?;

        if let Some(replacement) = candidate_result.matching_nodes.first() {
            let _ =
                write_back_updated_node(&mut updated_ctx.matching_nodes, candidate, replacement);
        }
    }

    Ok(updated_ctx)
}
