use crate::operators::*;

// Evaluate expression node results into a single sequence node.
fn collect_together(
    d: &mut TreeEngine,
    ctx: &Context,
    expression_node: &mut ExpressionNode,
) -> Result<Box<TreeNode>, CoreError> {
    let mut collected = TreeNode::default();
    collected.kind = NodeKind::Sequence;
    collected.sem_type = Some(SemType::Seq);
    collected.tag = SemType::Seq.to_string().into();

    for candidate in &ctx.matching_nodes {
        let child_ctx = ctx.single_readonly_child_context(candidate)?;
        let exp_results = get_matching_nodes(d, &child_ctx, Some(expression_node))?;
        for r in &exp_results.matching_nodes {
            collected.add_child(r)?;
        }
    }
    Ok(Box::new(collected))
}

/// collect operator: aggregate expression results into sequences.
/// Returns a new context with the collected sequence nodes.
pub fn collect_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let rhs_expr = expression_node
        .rhs
        .as_mut()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;

    if ctx.matching_nodes.is_empty() {
        let mut node = TreeNode::default();
        node.kind = NodeKind::Sequence;
        node.sem_type = Some(SemType::Seq);
        node.tag = SemType::Seq.to_string().into();
        node.value = "[]".to_string();
        return ctx.single_child_context(&node);
    }

    // Check if all nodes should be evaluated together.
    let all_together = ctx.matching_nodes.iter().all(|n| n.evaluate_together);

    if all_together {
        let collected = collect_together(d, &ctx, rhs_expr)?;
        return ctx.single_child_context(&collected);
    }

    let mut results: Vec<TreeNode> = Vec::new();
    for candidate in &ctx.matching_nodes {
        let mut collect_candidate =
            (*candidate.create_replacement(NodeKind::Sequence, SemType::Seq.to_string(), "")?)
                .clone();
        let child_ctx = ctx.single_child_context(candidate)?;
        let exp_results = get_matching_nodes(d, &child_ctx, Some(rhs_expr))?;
        for r in &exp_results.matching_nodes {
            collect_candidate.add_child(r)?;
        }
        results.push(collect_candidate);
    }

    ctx.child_context(results)
}
