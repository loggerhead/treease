use crate::operators::*;

/// length operator: compute the length of each matching node.
/// - nil scalars -> 0
/// - scalars -> string length
/// - mappings -> number of key-value pairs (content.len() / 2)
/// - sequences -> number of elements
pub fn length_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results: Vec<TreeNode> = Vec::new();

    for candidate in &ctx.matching_nodes {
        let length: usize = match candidate.kind {
            NodeKind::Scalar => {
                if candidate.sem_type == Some(SemType::Nil) {
                    0
                } else {
                    candidate.value.len()
                }
            }
            NodeKind::Mapping => candidate.content.len() / 2,
            NodeKind::Sequence => candidate.content.len(),
            _ => 0,
        };

        let length_str = length.to_string();
        let result = candidate.create_replacement(
            NodeKind::Scalar,
            SemType::Int.to_string(),
            &length_str,
        )?;
        results.push((*result).clone());
    }

    ctx.child_context(results)
}
