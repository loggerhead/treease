use crate::operators::*;

/// Get the string tag of each matching node.
pub fn get_tag_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let result = candidate.create_replacement(
            NodeKind::Scalar,
            SemType::Str.to_string(),
            &candidate.tag,
        )?;
        results.push((*result).clone());
    }
    ctx.child_context(results)
}
