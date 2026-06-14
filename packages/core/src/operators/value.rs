use crate::operators::*;

/// Produce copies of the current operation's tree node (value operator).
pub fn value_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let n = expression_node
        .operation
        .tree_node
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingTreeNode))?;

    let clone_count = if ctx.matching_nodes.is_empty() {
        1
    } else {
        ctx.matching_nodes.len()
    };
    let mut results = Vec::new();
    for _ in 0..clone_count {
        results.push(*(n.copy()?));
    }

    ctx.child_context(results)
}
