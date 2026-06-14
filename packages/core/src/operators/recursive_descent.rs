use crate::operators::traverse_path::splat;
use crate::operators::*;

/// Recursively descend through a node and all its descendants,
/// adding them to the results list.
fn recursive_descent(
    results: &mut Vec<TreeNode>,
    ctx: &Context,
    preferences: &RecursiveDescentPreferences,
    store: &TreeStore,
) -> Result<(), CoreError> {
    for candidate in &ctx.matching_nodes {
        // Add the current node first
        results.push(candidate.clone());

        let can_recurse = candidate.kind != NodeKind::Alias && !candidate.content.is_empty();
        let wants_recurse = preferences.recurse_array || candidate.kind != NodeKind::Sequence;

        if can_recurse && wants_recurse {
            // Expand children and recurse
            let children = splat(
                ctx.single_child_context(candidate)?,
                preferences.traverse_preferences.clone(),
                store,
            )?;
            recursive_descent(results, &children, preferences, store)?;
        }
    }
    Ok(())
}

/// Recursive descent operator (.. syntax): traverse all descendants.
pub fn recursive_descent_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let preferences = match expression_node.operation.preferences.as_deref() {
        Some(OperationPreference::RecursiveDescent(p)) => p.clone(),
        _ => RecursiveDescentPreferences::default(),
    };

    let mut results = Vec::new();
    recursive_descent(&mut results, &ctx, &preferences, &d.store)?;
    ctx.child_context(results)
}
