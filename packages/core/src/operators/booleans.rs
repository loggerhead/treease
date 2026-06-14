use crate::operators::operator_helpers::*;
use crate::operators::*;

// ── Boolean operators (and, or, not, any, all) ────────────────────

/// Get an owner reference: prefer lhs, then rhs, then a default empty node.
fn get_owner<'a>(
    lhs: Option<&'a TreeNode>,
    rhs: Option<&'a TreeNode>,
    default: &'a TreeNode,
) -> &'a TreeNode {
    lhs.or(rhs).unwrap_or(default)
}

/// Return a boolean candidate representing the truthiness of RHS.
fn return_rhs_truthy(
    _d: &mut TreeEngine,
    _ctx: Context,
    lhs: Option<&TreeNode>,
    rhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    let rhs_bool = is_truthy_node(rhs);
    let default_node = TreeNode::default();
    let owner = get_owner(lhs, rhs, &default_node);
    Ok(Some(create_boolean_candidate(owner, rhs_bool)?))
}

/// LHS result value for OR: if LHS is truthy, return boolean true (short-circuit).
fn return_lhs_when_true(
    _ctx: Context,
    lhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    lhs_when_truthy(lhs, true)
}

/// LHS result value for AND: if LHS is falsy, return boolean false (short-circuit).
fn return_lhs_when_false(
    _ctx: Context,
    lhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    lhs_when_truthy(lhs, false)
}

/// If lhs truthiness matches want_truthy, return a boolean candidate.
fn lhs_when_truthy(
    lhs: Option<&TreeNode>,
    want_truthy: bool,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    let truthy = is_truthy_node(lhs);
    if truthy != want_truthy {
        return Ok(None);
    }
    let owner = match lhs {
        Some(n) => n,
        None => &TreeNode::default(),
    };
    Ok(Some(create_boolean_candidate(owner, truthy)?))
}

/// Find a boolean value in a sequence, optionally evaluating an expression per element.
fn find_boolean(
    want: bool,
    d: &mut TreeEngine,
    ctx: &Context,
    mut expression_node: Option<&mut ExpressionNode>,
    sequence_node: &TreeNode,
) -> Result<bool, CoreError> {
    for node0 in &sequence_node.content {
        if let Some(expr) = expression_node.as_deref_mut() {
            let rhs =
                get_matching_nodes(d, &ctx.single_readonly_child_context(node0)?, Some(expr))?;
            if !rhs.matching_nodes.is_empty()
                && is_truthy_node(Some(&rhs.matching_nodes[0])) == want
            {
                return Ok(true);
            }
        } else if is_truthy_node(Some(node0)) == want {
            return Ok(true);
        }
    }
    Ok(false)
}

/// all operator: true if every element in an array is truthy.
pub fn all_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes.clone() {
        if candidate.kind != NodeKind::Sequence {
            if let Some(ref d2) = ctx.diagnostics {
                let _ = d2.set_messagef(
                    "eval",
                    &format!("all only supports arrays, was {}", candidate.tag),
                );
            }
            return Err(CoreError::Eval(EvalError::UnsupportedFlat));
        }
        let boolean_result = find_boolean(
            false,
            d,
            &ctx,
            expression_node.rhs.as_deref_mut(),
            &candidate,
        )?;
        let result = create_boolean_candidate(&candidate, !boolean_result)?;
        results.push((*result).clone());
    }
    ctx.child_context(results)
}

/// any operator: true if any element in an array is truthy.
pub fn any_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes.clone() {
        if candidate.kind != NodeKind::Sequence {
            if let Some(ref d2) = ctx.diagnostics {
                let _ = d2.set_messagef(
                    "eval",
                    &format!("any only supports arrays, was {}", candidate.tag),
                );
            }
            return Err(CoreError::Eval(EvalError::UnsupportedFlat));
        }
        let boolean_result = find_boolean(
            true,
            d,
            &ctx,
            expression_node.rhs.as_deref_mut(),
            &candidate,
        )?;
        let result = create_boolean_candidate(&candidate, boolean_result)?;
        results.push((*result).clone());
    }
    ctx.child_context(results)
}

/// OR operator: logical or with short-circuit.
pub fn or_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let ro = ctx.read_only_clone()?;
    let prefs = CrossFunctionPreferences {
        calc_when_empty: true,
        calculation: return_rhs_truthy,
        lhs_result_value: Some(return_lhs_when_true),
    };
    cross_function_with_prefs(d, ro, expression_node, prefs)
}

/// AND operator: logical and with short-circuit.
pub fn and_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let ro = ctx.read_only_clone()?;
    let prefs = CrossFunctionPreferences {
        calc_when_empty: true,
        calculation: return_rhs_truthy,
        lhs_result_value: Some(return_lhs_when_false),
    };
    cross_function_with_prefs(d, ro, expression_node, prefs)
}

/// NOT operator: negate truthiness of each matching node.
pub fn not_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let truthy = is_truthy_node(Some(candidate));
        let result = create_boolean_candidate(candidate, !truthy)?;
        results.push((*result).clone());
    }
    ctx.child_context(results)
}
