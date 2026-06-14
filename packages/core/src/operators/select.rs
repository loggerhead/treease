use crate::operators::operator_helpers::*;
use crate::operators::*;

// ── Select operator ───────────────────────────────────────────────

/// Check if any node in a slice is truthy.
fn any_truthy(nodes: &[TreeNode]) -> bool {
    for n in nodes {
        if is_truthy_node(Some(n)) {
            return true;
        }
    }
    false
}

/// Select operator: keep only nodes whose RHS expression evaluates to truthy.
pub fn select_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();

    let rhs_expr = expression_node
        .rhs
        .as_deref_mut()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;

    for candidate in &ctx.matching_nodes {
        let rhs = get_matching_nodes(
            d,
            &ctx.single_readonly_child_context(candidate)?,
            Some(rhs_expr),
        )?;
        if any_truthy(&rhs.matching_nodes) {
            results.push(candidate.clone());
        }
    }
    ctx.child_context(results)
}
