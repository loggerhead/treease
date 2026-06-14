use crate::operators::*;

/// Reduce an array by applying a block expression with an accumulator variable
/// (reduce operator).
pub fn reduce_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let lhs = expression_node
        .lhs
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingLhs))?;
    let rhs = expression_node
        .rhs
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;

    if lhs.operation.operation_type.id != ASSIGN_VARIABLE_OP_TYPE.id {
        if let Some(ref d2) = ctx.diagnostics {
            d2.set_messagef(
                "eval",
                &format!(
                    "reduce must be given a variables assignment, got {} instead",
                    lhs.operation.operation_type.name()
                ),
            )?;
        }
        return Err(CoreError::OperatorMessage {
            op: "reduce".to_string(),
            message: "must be given a variable assignment".to_string(),
        });
    }
    if rhs.operation.operation_type.id != BLOCK_OP_TYPE.id {
        if let Some(ref d2) = ctx.diagnostics {
            d2.set_messagef(
                "eval",
                &format!(
                    "reduce must be given a block, got {} instead",
                    rhs.operation.operation_type.name()
                ),
            )?;
        }
        return Err(CoreError::OperatorMessage {
            op: "reduce".to_string(),
            message: "must be given a block".to_string(),
        });
    }

    let array_ctx = match lhs.lhs.clone() {
        Some(mut boxed) => get_matching_nodes(d, &ctx, Some(&mut *boxed))?,
        None => get_matching_nodes(d, &ctx, None)?,
    };

    let var_name_node = lhs
        .rhs
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
    let var_name = &var_name_node.operation.string_value;

    let mut accum = match rhs.lhs.clone() {
        Some(mut boxed) => get_matching_nodes(d, &ctx, Some(&mut *boxed))?,
        None => get_matching_nodes(d, &ctx, None)?,
    };
    let block_exp = rhs
        .rhs
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;

    for candidate in &array_ctx.matching_nodes {
        let list = vec![candidate.clone()];
        accum.set_variable(var_name, list)?;
        let mut block_clone = (*block_exp).clone();
        accum = get_matching_nodes(d, &accum, Some(&mut block_clone))?;
    }

    Ok(accum)
}
