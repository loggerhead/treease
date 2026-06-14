use crate::operators::operator_helpers::*;
use crate::operators::*;

// ── Alternative operator (a // b) ─────────────────────────────────

/// Select lhs or rhs based on truthiness: if lhs is truthy, return lhs; otherwise rhs.
pub fn alternative_with_nodes<'a>(
    lhs: Option<&'a TreeNode>,
    rhs: Option<&'a TreeNode>,
) -> Option<&'a TreeNode> {
    let l = match lhs {
        Some(n) => n,
        None => return rhs,
    };
    let r = match rhs {
        Some(n) => n,
        None => return Some(l),
    };
    if is_truthy_node(Some(l)) {
        Some(l)
    } else {
        Some(r)
    }
}

/// LHS result for alternative: if LHS is truthy, short-circuit with LHS itself.
fn alternative_lhs_result_value(
    _ctx: Context,
    lhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    let l = match lhs {
        Some(n) => n,
        None => return Ok(None),
    };
    if !is_truthy_node(Some(l)) {
        return Ok(None);
    }
    Ok(Some(l.copy()?))
}

/// Cross-function calculation for alternative.
pub fn alternative_func(
    _d: &mut TreeEngine,
    _ctx: Context,
    lhs: Option<&TreeNode>,
    rhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    match alternative_with_nodes(lhs, rhs) {
        Some(n) => Ok(Some(n.copy()?)),
        None => Ok(None),
    }
}

/// Alternative operator ("//"): select lhs if truthy, otherwise rhs.
pub fn alternative_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let prefs = CrossFunctionPreferences {
        calc_when_empty: true,
        calculation: alternative_func,
        lhs_result_value: Some(alternative_lhs_result_value),
    };
    cross_function_with_prefs(d, ctx, expression_node, prefs)
}
