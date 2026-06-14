use crate::operators::*;

/// Reverse the order of elements in sequence nodes (reverse operator).
pub fn reverse_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        if candidate.kind != NodeKind::Sequence {
            return Err(CoreError::Eval(EvalError::NodeIsNotArray));
        }

        let mut reverse_list = *candidate
            .create_replacement_with_comments(NodeKind::Sequence, SemType::Seq.to_string())?;
        for item in candidate.content.iter().rev() {
            reverse_list.add_child(item)?;
        }
        results.push(reverse_list);
    }
    ctx.child_context(results)
}
