use crate::operators::*;

/// Get the value of a variable.
pub fn get_variable_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let variable_name = &expression_node.operation.string_value;
    let existing = ctx.get_variable(variable_name);
    match existing {
        Some(nodes) => ctx.child_context(nodes.clone()),
        None => ctx.child_context(Vec::new()),
    }
}

/// Error stub: variables must be used with a pipe.
pub fn use_with_pipe(
    ctx: Context,
    _d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    if let Some(ref d) = ctx.diagnostics {
        d.set_messagef(
            "eval",
            "must use variable with a pipe, e.g. `exp as $x | ...`",
        )?;
    }
    Err(CoreError::Eval(EvalError::MustUseVariableWithPipe))
}

/// Error message for MustUseVariableWithPipe.
pub fn error_message(_e: &CoreError) -> Option<&'static str> {
    Some("must use variable with a pipe, e.g. `exp as $x | ...`")
}

fn get_assign_var_prefs(pref: Option<&OperationPreference>) -> AssignVarPreferences {
    match pref {
        Some(OperationPreference::AssignVar(p)) => p.clone(),
        _ => AssignVarPreferences::default(),
    }
}

fn should_evaluate_all_together(nodes: &[TreeNode]) -> bool {
    nodes.iter().all(|n| n.evaluate_together)
}

fn variable_loop_single_child(
    d: &mut TreeEngine,
    ctx: Context,
    original_exp: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let variable_exp = original_exp
        .lhs
        .as_mut()
        .ok_or(CoreError::Eval(EvalError::MissingLhs))?;
    let var_rhs = variable_exp
        .rhs
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;

    if var_rhs.operation.operation_type.id != GET_VARIABLE_OP_TYPE.id {
        return Err(CoreError::Eval(EvalError::InvalidVariableRhs));
    }

    let variable_name = var_rhs.operation.string_value.clone();

    // Evaluate the variable value (LHS of the variable assignment)
    let read_only_ctx = ctx.read_only_clone()?;
    let lhs_ctx = get_matching_nodes(d, &read_only_ctx, variable_exp.lhs.as_deref_mut())?;

    let prefs = get_assign_var_prefs(variable_exp.operation.preferences.as_deref());

    let mut results = Vec::new();

    // For each match, set the variable and evaluate the RHS
    for m in &lhs_ctx.matching_nodes {
        let variable_value = if prefs.is_reference {
            vec![m.clone()]
        } else {
            vec![(*m.copy()?).clone()]
        };

        let mut new_ctx = ctx.child_context(ctx.matching_nodes.clone())?;
        new_ctx.set_variable(&variable_name, variable_value)?;

        let mut rhs_expr = original_exp
            .rhs
            .clone()
            .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
        let rhs_ctx = get_matching_nodes(d, &new_ctx, Some(&mut rhs_expr))?;
        for n in &rhs_ctx.matching_nodes {
            results.push(n.clone());
        }
    }

    // If no matches, evaluate RHS directly with the original context
    if lhs_ctx.matching_nodes.is_empty() {
        let mut rhs_expr = original_exp
            .rhs
            .clone()
            .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
        return get_matching_nodes(d, &ctx, Some(&mut rhs_expr));
    }

    ctx.child_context(results)
}

/// Process variable assignment loop: for each matching node, set variable
/// and evaluate the subsequent expression.
pub fn variable_loop(
    d: &mut TreeEngine,
    ctx: Context,
    original_exp: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    // If all nodes are marked as evaluate_together, process them at once
    if should_evaluate_all_together(&ctx.matching_nodes) {
        return variable_loop_single_child(d, ctx, original_exp);
    }

    // Otherwise process one at a time
    let mut results = Vec::new();
    for n in &ctx.matching_nodes {
        let single = variable_loop_single_child(d, ctx.single_child_context(n)?, original_exp)?;
        for m in &single.matching_nodes {
            results.push(m.clone());
        }
    }
    ctx.child_context(results)
}
