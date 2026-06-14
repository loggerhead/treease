use crate::operators::*;

/// Determine the semantic type tag for a converted number string.
fn tag_for_converted(value: &str) -> Result<SemType, CoreError> {
    if value.parse::<i64>().is_ok() {
        return Ok(SemType::Int);
    }
    if value.parse::<f64>().is_ok() {
        return Ok(SemType::Float);
    }
    Err(CoreError::Eval(EvalError::CannotConvertValueToNumber))
}

/// Convert scalar nodes to numeric type (to_number operator).
pub fn to_number_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();

    for candidate in &ctx.matching_nodes {
        if candidate.kind != NodeKind::Scalar {
            return Err(CoreError::Eval(EvalError::CannotConvertNodeToNumber));
        }

        // Already a number — keep as-is
        if candidate.sem_type == Some(SemType::Int) || candidate.sem_type == Some(SemType::Float) {
            results.push(candidate.clone());
            continue;
        }

        let tag = tag_for_converted(&candidate.value)?;
        let result =
            *candidate.create_replacement(NodeKind::Scalar, tag.to_string(), &candidate.value)?;
        results.push(result);
    }

    ctx.child_context(results)
}
