use crate::operators::*;

/// Return the node kind (map, seq, scalar, alias) as a string.
pub fn get_kind_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let kind_text = match candidate.kind {
            NodeKind::Mapping => "map",
            NodeKind::Sequence => "seq",
            NodeKind::Scalar => "scalar",
            NodeKind::Alias => "alias",
            NodeKind::Unknown => "unknown",
        };
        let result =
            candidate.create_replacement(NodeKind::Scalar, SemType::Str.to_string(), kind_text)?;
        results.push((*result).clone());
    }
    ctx.child_context(results)
}
