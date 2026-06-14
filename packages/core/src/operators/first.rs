use crate::operators::*;

/// Return the first matching element from children (first operator).
pub fn first_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();

    let rhs_expr_opt = expression_node.rhs.as_deref();

    for candidate in &ctx.matching_nodes {
        let rhs_expr = match rhs_expr_opt {
            Some(expr) => expr,
            None => {
                // No RHS expression — return first child if any
                if !candidate.content.is_empty() {
                    results.push(candidate.content[0].clone());
                }
                continue;
            }
        };

        let children_ctx = ctx.single_child_context(candidate)?;
        let splatted = splat(children_ctx, TraversePreferences::default())?;

        for splat_candidate in &splatted.matching_nodes {
            let splat_ctx = ctx.single_child_context(splat_candidate)?;
            let mut rhs_clone = rhs_expr.clone();
            let rhs = get_matching_nodes(d, &splat_ctx, Some(&mut rhs_clone))?;

            let mut found = false;
            for n in &rhs.matching_nodes {
                if is_truthy_node(Some(n)) {
                    found = true;
                    break;
                }
            }

            if found {
                results.push(splat_candidate.clone());
                break;
            }
        }
    }

    ctx.child_context(results)
}
