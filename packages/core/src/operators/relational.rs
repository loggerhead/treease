use crate::operators::operator_helpers::*;
use crate::operators::traverse_path::splat as traverse_splat;
use crate::operators::*;

// Check whether all nodes should be evaluated together.
fn should_evaluate_all_together(nodes: &[TreeNode]) -> bool {
    for n in nodes {
        if !n.evaluate_together {
            return false;
        }
    }
    true
}

// Compare two scalar nodes for relational comparison.
// Supports integer, float, string, and null comparisons.
fn compare_relational_scalars(
    ctx: &Context,
    prefs: &RelationalPref,
    lhs: &TreeNode,
    rhs: &TreeNode,
) -> Result<bool, CoreError> {
    let lhs_tag = lhs.guess_tag_from_custom_type();
    let rhs_tag = rhs.guess_tag_from_custom_type();
    let lhs_sem_type = SemType::from_string(&lhs_tag);
    let rhs_sem_type = SemType::from_string(&rhs_tag);

    // Both are integers
    if let (Some(lhs_st), Some(rhs_st)) = (lhs_sem_type, rhs_sem_type) {
        if lhs_st == SemType::Int && rhs_st == SemType::Int {
            let lhs_num = parse_int64(&lhs.value)?;
            let rhs_num = parse_int64(&rhs.value)?;
            if prefs.or_equal && lhs_num == rhs_num {
                return Ok(true);
            }
            if prefs.greater {
                return Ok(lhs_num > rhs_num);
            }
            return Ok(lhs_num < rhs_num);
        }
    }

    // Both are numbers (int or float)
    let lhs_number = lhs_sem_type.map_or(false, |t| t == SemType::Int || t == SemType::Float);
    let rhs_number = rhs_sem_type.map_or(false, |t| t == SemType::Int || t == SemType::Float);
    if lhs_number && rhs_number {
        let lhs_num: f64 = parse_float64(&lhs.value)?;
        let rhs_num: f64 = parse_float64(&rhs.value)?;
        if prefs.or_equal && lhs_num == rhs_num {
            return Ok(true);
        }
        if prefs.greater {
            return Ok(lhs_num > rhs_num);
        }
        return Ok(lhs_num < rhs_num);
    }

    // Both are strings
    if let (Some(lhs_st), Some(rhs_st)) = (lhs_sem_type, rhs_sem_type) {
        if lhs_st == SemType::Str && rhs_st == SemType::Str {
            if prefs.or_equal && lhs.value == rhs.value {
                return Ok(true);
            }
            let ord = lhs.value.cmp(&rhs.value);
            if prefs.greater {
                return Ok(ord == std::cmp::Ordering::Greater);
            }
            return Ok(ord == std::cmp::Ordering::Less);
        }
    }

    // Both are null with or_equal
    if let (Some(lhs_st), Some(rhs_st)) = (lhs_sem_type, rhs_sem_type) {
        if lhs_st == SemType::Nil && rhs_st == SemType::Nil && prefs.or_equal {
            return Ok(true);
        }
    }

    // One of them is null
    if lhs_sem_type == Some(SemType::Nil) || rhs_sem_type == Some(SemType::Nil) {
        return Ok(false);
    }

    if let Some(ref d2) = ctx.diagnostics {
        d2.set_messagef(
            "eval",
            &format!("{} not yet supported for comparison", lhs.tag),
        )?;
    }
    Err(CoreError::Eval(EvalError::UnsupportedFlat))
}

// Evaluate relational comparison for two candidate nodes.
fn evaluate_relational_candidates(
    ctx: &Context,
    prefs: &RelationalPref,
    lhs: Option<&TreeNode>,
    rhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    let l = match lhs {
        None => {
            let _r = match rhs {
                None => {
                    return create_boolean_candidate(&TreeNode::default(), prefs.or_equal)
                        .map(Some);
                }
                Some(r) => return create_boolean_candidate(r, false).map(Some),
            };
        }
        Some(l) => l,
    };

    let r = match rhs {
        None => return create_boolean_candidate(l, false).map(Some),
        Some(r) => r,
    };

    match l.kind {
        NodeKind::Mapping => {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef("eval", "maps not yet supported for comparison")?;
            }
            Err(CoreError::Eval(EvalError::UnsupportedFlat))
        }
        NodeKind::Sequence => {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef("eval", "arrays not yet supported for comparison")?;
            }
            Err(CoreError::Eval(EvalError::UnsupportedFlat))
        }
        _ => {
            if r.kind != NodeKind::Scalar {
                let nice = r.get_nice_path()?;
                if let Some(ref d2) = ctx.diagnostics {
                    d2.set_messagef(
                        "eval",
                        &format!("{} ({}) cannot be compared with {}", r.tag, nice, l.tag),
                    )?;
                }
                return Err(CoreError::Eval(EvalError::UnsupportedFlat));
            }
            let bool_v = compare_relational_scalars(ctx, prefs, l, r)?;
            create_boolean_candidate(l, bool_v).map(Some)
        }
    }
}

fn append_relational_results_for_rhs(
    d: &mut TreeEngine,
    ctx: &Context,
    lhs_candidate: Option<&TreeNode>,
    prefs: &RelationalPref,
    rhs_exp: &mut ExpressionNode,
    results: &mut Vec<TreeNode>,
) -> Result<(), CoreError> {
    let rhs = get_matching_nodes(d, ctx, Some(rhs_exp))?;

    if rhs.matching_nodes.is_empty() {
        if let Some(r) = evaluate_relational_candidates(ctx, prefs, lhs_candidate, None)? {
            results.push((*r).clone());
        }
        return Ok(());
    }

    for rhs_candidate in &rhs.matching_nodes {
        if let Some(r) =
            evaluate_relational_candidates(ctx, prefs, lhs_candidate, Some(rhs_candidate))?
        {
            results.push((*r).clone());
        }
    }
    Ok(())
}

// Evaluate relational cross product: compare each LHS match with each RHS match.
fn evaluate_relational_cross_product(
    d: &mut TreeEngine,
    ctx: &Context,
    expression_node: &mut ExpressionNode,
    prefs: &RelationalPref,
) -> Result<Context, CoreError> {
    let mut results: Vec<TreeNode> = Vec::new();

    let rhs_exp = expression_node
        .rhs
        .as_mut()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
    let lhs = get_matching_nodes(d, ctx, expression_node.lhs.as_deref_mut())?;

    if !ctx.matching_nodes.is_empty() && lhs.matching_nodes.is_empty() {
        append_relational_results_for_rhs(d, ctx, None, prefs, rhs_exp, &mut results)?;
    }

    for lhs_candidate in &lhs.matching_nodes {
        append_relational_results_for_rhs(
            d,
            ctx,
            Some(lhs_candidate),
            prefs,
            rhs_exp,
            &mut results,
        )?;
    }
    ctx.child_context(results)
}

/// Equals operator (delegates to equals module).
pub fn equals_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    crate::operators::equals::equals_operator(ctx, d, expression_node)
}

/// Not-equals operator (delegates to equals module).
pub fn not_equals_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    crate::operators::equals::not_equals_operator(ctx, d, expression_node)
}

/// Relational comparison operator: supports <, >, <=, >=.
pub fn relational_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let prefs = get_relational_prefs(expression_node);

    if should_evaluate_all_together(&ctx.matching_nodes) {
        return evaluate_relational_cross_product(d, &ctx, expression_node, &prefs);
    }

    let mut results: Vec<TreeNode> = Vec::new();
    for n in &ctx.matching_nodes {
        let inner = evaluate_relational_cross_product(
            d,
            &ctx.single_child_context(n)?,
            expression_node,
            &prefs,
        )?;
        results.extend(inner.matching_nodes.iter().cloned());
    }
    ctx.child_context(results)
}

/// Extract RelationalPref from the expression node's preferences.
fn get_relational_prefs(expression_node: &ExpressionNode) -> RelationalPref {
    match expression_node.operation.preferences.as_deref() {
        Some(OperationPreference::Relational(p)) => p.clone(),
        _ => RelationalPref::default(),
    }
}

// Find the superlative (min or max) by comparing elements in each sequence.
// Propagates errors instead of swallowing them.
fn superlative_by_comparison(
    ctx: &Context,
    prefs: &RelationalPref,
    store: &TreeStore,
) -> Result<Context, CoreError> {
    let mut results: Vec<TreeNode> = Vec::new();

    for seq_node in &ctx.matching_nodes {
        let splatted = traverse_splat(
            ctx.single_child_context(seq_node)?,
            TraversePreferences::default(),
            store,
        )?;
        if splatted.matching_nodes.is_empty() {
            continue;
        }
        let mut best = splatted.matching_nodes[0].clone();

        for i in 1..splatted.matching_nodes.len() {
            let candidate = &splatted.matching_nodes[i];
            let better = compare_relational_scalars(ctx, prefs, candidate, &best)?;
            if better {
                best = candidate.clone();
            }
        }
        results.push(best);
    }
    ctx.child_context(results)
}

/// min operator: find the minimum value in each sequence.
pub fn min_operator(
    ctx: Context,
    d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let prefs = RelationalPref {
        greater: false,
        or_equal: false,
    };
    superlative_by_comparison(&ctx, &prefs, &d.store)
}

/// max operator: find the maximum value in each sequence.
pub fn max_operator(
    ctx: Context,
    d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let prefs = RelationalPref {
        greater: true,
        or_equal: false,
    };
    superlative_by_comparison(&ctx, &prefs, &d.store)
}
