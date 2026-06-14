use crate::operators::*;

use crate::expression_pipeline;

fn get_expression_prefs(pref: Option<&OperationPreference>) -> ExpressionOpPreferences {
    match pref {
        Some(OperationPreference::Expression(p)) => p.clone(),
        _ => ExpressionOpPreferences::default(),
    }
}

fn map_pipeline_error(error: expression_pipeline::PipelineError) -> CoreError {
    match error {
        expression_pipeline::PipelineError::Parse(_) => CoreError::Parse(ParseError::InvalidSyntax),
        expression_pipeline::PipelineError::Evaluate(eval) => match eval {
            crate::evaluator::EvaluationError::Core(_) => {
                CoreError::Eval(EvalError::UnsupportedFlat)
            }
            crate::evaluator::EvaluationError::DivisionByZero => {
                CoreError::Eval(EvalError::CannotDivideTypes)
            }
            crate::evaluator::EvaluationError::MissingOperand(_)
            | crate::evaluator::EvaluationError::TypeMismatch { .. } => {
                CoreError::Eval(EvalError::UnsupportedFlat)
            }
            crate::evaluator::EvaluationError::UnsupportedOperation(_) => {
                CoreError::Eval(EvalError::UnsupportedFlat)
            }
        },
        expression_pipeline::PipelineError::Compat(compat) => compat,
    }
}

/// Parse and execute a string expression (expression operator).
///
/// dispatches it through the tree-level evaluator
/// ([`get_matching_nodes`]), preserving node metadata (anchors,
/// comments, map-key flags, etc.) that would be lost by a round-trip
/// through the flat [`Value`] evaluator.
pub fn expression_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let prefs = get_expression_prefs(expression_node.operation.preferences.as_deref());
    let expression = if !prefs.expression.is_empty() {
        prefs.expression
    } else {
        expression_node.operation.string_value.clone()
    };
    if expression.trim().is_empty() {
        return Ok(ctx);
    }

    expression_pipeline::execute_on_context(d, &ctx, &expression).map_err(map_pipeline_error)
}
