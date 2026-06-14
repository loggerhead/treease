use crate::operators::multiply::multiply_with_nodes;
use crate::operators::traverse_path::splat as traverse_splat;
use crate::operators::*;

/// Splat a single node: expand a sequence or mapping node into its children
/// as matching nodes. For non-collection nodes, return the node itself.
/// Uses traverse_path::splat with dont_follow_alias=true and include_map_keys=false
fn splat_node(ctx: &Context, node: &TreeNode, store: &TreeStore) -> Result<Context, CoreError> {
    let child_ctx = ctx.single_child_context(node)?;
    let prefs = TraversePreferences {
        dont_follow_alias: true,
        include_map_keys: false,
        ..TraversePreferences::default()
    };
    traverse_splat(child_ctx, prefs, store)
}

/// Recursively collect slices by multiplying (cartesian product style).
/// Uses the real `multiply::multiply_with_nodes` for deep merge semantics,
fn collect_slice(
    ctx: &Context,
    remaining_matches: &[TreeNode],
    store: &TreeStore,
) -> Result<Context, CoreError> {
    if remaining_matches.is_empty() {
        return Ok(ctx.clone());
    }

    let candidate = &remaining_matches[0];
    let rest = &remaining_matches[1..];

    // Splat the current candidate node.
    let splatted = splat_node(ctx, candidate, store)?;

    // If the context has no matching nodes yet (first call), recurse with splatted.
    if ctx.matching_nodes.is_empty() {
        return collect_slice(&splatted, rest, store);
    }

    // Cartesian product: combine each existing result with each splatted child.
    let mut new_agg: Vec<TreeNode> = Vec::new();
    for agg_candidate in &ctx.matching_nodes {
        for splat_candidate in &splatted.matching_nodes {
            let new_candidate = agg_candidate.copy()?;
            let merged = multiply_with_nodes(&new_candidate, splat_candidate)?;
            new_agg.push((*merged).clone());
        }
    }

    let new_ctx = ctx.child_context(new_agg)?;
    collect_slice(&new_ctx, rest, store)
}

/// collect_object operator: merge multiple matches into objects.
/// Rotates the matrix of key-value pairs from input sequences and
/// then merges each column via the multiply/merge logic.
pub fn collect_object_operator(
    ctx: Context,
    d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut writable_ctx = ctx.clone();
    writable_ctx.dont_auto_create = false;

    if writable_ctx.matching_nodes.is_empty() {
        let mut candidate = TreeNode::default();
        candidate.kind = NodeKind::Mapping;
        candidate.sem_type = Some(SemType::Map);
        candidate.tag = SemType::Map.to_string().into();
        candidate.value = "{}".to_string();
        return writable_ctx.single_child_context(&candidate);
    }

    // Validate all input nodes have the same size.
    let first = &writable_ctx.matching_nodes[0];
    let rotated_len = first.content.len();

    // Rotate the matrix: collect the i-th child of each tree_node into rotated[i].
    let mut rotated: Vec<Vec<TreeNode>> = vec![Vec::new(); rotated_len];

    for tree_node in &writable_ctx.matching_nodes {
        if tree_node.content.len() < rotated_len {
            if let Some(ref d2) = writable_ctx.diagnostics {
                d2.set_message("eval", "CollectObject: mismatching node sizes; are you creating a map with mismatching key value pairs?")?;
            }
            return Err(CoreError::OperatorMessage {
                op: "collect_object".to_string(),
                message: "mismatching node sizes".to_string(),
            });
        }
        for i in 0..rotated_len {
            rotated[i].push(tree_node.content[i].clone());
        }
    }

    // For each column (rotated list), merge the nodes.
    let mut new_object: Vec<TreeNode> = Vec::new();
    for list in &rotated {
        let additions = collect_slice(&writable_ctx.child_context(Vec::new())?, list, &d.store)?;
        for addition in &additions.matching_nodes {
            let mut addition_copy = addition.clone();
            addition_copy.parent = None;
            addition_copy.key = None;
            addition_copy.sequence_index = None;
            new_object.push(addition_copy);
        }
    }

    writable_ctx.child_context(new_object)
}
