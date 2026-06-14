use crate::operators::operator_helpers::*;
use crate::operators::*;

// ── Filter operator ───────────────────────────────────────────────

/// Collect a slice of tree nodes into a new sequence tree node.
fn collect_as_sequence(nodes: &[TreeNode]) -> Result<Box<TreeNode>, CoreError> {
    let mut collected = Box::new(TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.to_string().into(),
        ..Default::default()
    });
    for n in nodes {
        collected.add_child(n)?;
    }
    if collected.content.is_empty() {
        collected.value = "[]".to_string();
    }
    Ok(collected)
}

/// Splat: flatten each matching node's children into a new context.
/// When a node is a sequence, its children become the matching nodes.
/// When a node is a mapping, only its values become matching nodes
/// with default `TraversePreferences{ include_map_keys: false }`).
fn splat_children(ctx: &Context) -> Result<Context, CoreError> {
    let mut matching_nodes = Vec::new();
    for candidate in &ctx.matching_nodes {
        if candidate.can_visit_values() {
            match candidate.kind {
                NodeKind::Sequence => {
                    for child in &candidate.content {
                        matching_nodes.push(child.clone());
                    }
                }
                NodeKind::Mapping => {
                    // Only include values (odd indices), not keys (even indices)
                    let mut i = 1usize;
                    while i < candidate.content.len() {
                        matching_nodes.push(candidate.content[i].clone());
                        i += 2;
                    }
                }
                _ => {
                    matching_nodes.push(candidate.clone());
                }
            }
        } else {
            matching_nodes.push(candidate.clone());
        }
    }
    ctx.child_context(matching_nodes)
}

/// Any node truthy check.
fn any_truthy(nodes: &[TreeNode]) -> bool {
    for n in nodes {
        if is_truthy_node(Some(n)) {
            return true;
        }
    }
    false
}

/// Select helper: keep only nodes whose RHS expression evaluates to truthy.
fn select_nodes(
    ctx: &Context,
    d: &mut TreeEngine,
    rhs_expr: &mut ExpressionNode,
) -> Result<Vec<TreeNode>, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let rhs = get_matching_nodes(
            d,
            &ctx.single_readonly_child_context(candidate)?,
            Some(rhs_expr),
        )?;
        if any_truthy(&rhs.matching_nodes) {
            results.push(candidate.clone());
        }
    }
    Ok(results)
}

/// Filter operator: for each matching node, splat children, select truthy ones, collect.
pub fn filter_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();

    for candidate in &ctx.matching_nodes {
        let children_ctx = ctx.single_child_context(candidate)?;
        let splatted = splat_children(&children_ctx)?;

        let rhs_expr = expression_node
            .rhs
            .as_deref_mut()
            .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
        let filtered = select_nodes(&splatted, d, rhs_expr)?;

        let collected = collect_as_sequence(&filtered)?;
        results.push((*collected).clone());
    }

    ctx.child_context(results)
}
