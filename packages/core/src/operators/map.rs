use crate::operators::*;

/// Splat a single node into its children as matching nodes.
fn splat_single(ctx: &Context) -> Result<Context, CoreError> {
    if ctx.matching_nodes.is_empty() {
        return Ok(ctx.clone());
    }
    let node = &ctx.matching_nodes[0];
    match node.kind {
        NodeKind::Sequence => ctx.child_context(node.content.clone()),
        NodeKind::Mapping => {
            let mut values = Vec::new();
            let mut i = 1usize;
            while i < node.content.len() {
                values.push(node.content[i].clone());
                i += 2;
            }
            ctx.child_context(values)
        }
        _ => ctx.single_child_context(node),
    }
}

/// map operator: apply an expression to each element of a sequence or
/// mapping and return a new sequence of results.
pub fn map_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results: Vec<TreeNode> = Vec::new();

    let rhs_expr = expression_node.rhs.as_ref().ok_or(EvalError::MissingRhs)?;

    for candidate in &ctx.matching_nodes {
        let splatted = splat_single(&ctx.single_child_context(candidate)?)?;
        if splatted.matching_nodes.is_empty() {
            results.push((*candidate.copy()?).clone());
            continue;
        }

        let mut rhs_expr = (*rhs_expr).clone();
        let rhs = get_matching_nodes(d, &splatted, Some(&mut rhs_expr))?;
        let mut collected =
            (*candidate.create_replacement(NodeKind::Sequence, SemType::Seq.to_string(), "")?)
                .clone();
        for r in &rhs.matching_nodes {
            collected.add_child(r)?;
        }
        results.push(collected);
    }
    ctx.child_context(results)
}

/// map_values operator: apply an expression to each value in a mapping,
pub fn map_values_operator(
    mut ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let rhs_expr = expression_node.rhs.as_ref().ok_or(EvalError::MissingRhs)?;

    let mut candidate_index = 0;
    while candidate_index < ctx.matching_nodes.len() {
        if ctx.matching_nodes[candidate_index].kind != NodeKind::Mapping {
            candidate_index += 1;
            continue;
        }

        let mut i = 0;
        // Iterate key-value pairs: i is the key, i+1 is the value.
        while i + 1 < ctx.matching_nodes[candidate_index].content.len() {
            let value = ctx.matching_nodes[candidate_index].content[i + 1].clone();
            let mut rhs_expr = (*rhs_expr).clone();
            let rhs = get_matching_nodes(
                d,
                &ctx.single_readonly_child_context(&value)?,
                Some(&mut rhs_expr),
            )?;

            if let Some(replacement) = rhs.matching_nodes.first() {
                // In-place mutation: copy replacement content into the existing
                // value node while preserving parent/key/is_map_key/sequence_index.
                update_from(
                    &mut ctx.matching_nodes[candidate_index].content[i + 1],
                    replacement,
                )?;
            }
            i += 2;
        }
        candidate_index += 1;
    }
    Ok(ctx)
}
