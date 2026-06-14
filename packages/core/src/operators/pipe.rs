use crate::operators::*;

/// Pass the left-hand result into the right-hand expression (pipe operator).
pub fn pipe_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    if let Some(ref lhs_node) = expression_node.lhs {
        if lhs_node.operation.operation_type.id == ASSIGN_VARIABLE_OP_TYPE.id {
            return variables::variable_loop(d, ctx, expression_node);
        }
    }

    let lhs_ctx = get_matching_nodes(d, &ctx, expression_node.lhs.as_deref_mut())?;
    let rhs_context = ctx.child_context(lhs_ctx.matching_nodes.clone())?;
    let rhs_ctx = get_matching_nodes(d, &rhs_context, expression_node.rhs.as_deref_mut())?;
    ctx.child_context(rhs_ctx.matching_nodes)
}
