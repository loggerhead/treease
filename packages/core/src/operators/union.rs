use crate::operators::*;

fn nodes_structurally_eq(a: &TreeNode, b: &TreeNode) -> bool {
    if a.kind != b.kind
        || a.tag != b.tag
        || a.value != b.value
        || a.is_map_key != b.is_map_key
        || a.sequence_index != b.sequence_index
        || a.document != b.document
        || a.file_index != b.file_index
        || a.filename != b.filename
        || a.content.len() != b.content.len()
    {
        return false;
    }

    a.content
        .iter()
        .zip(&b.content)
        .all(|(lhs, rhs)| nodes_structurally_eq(lhs, rhs))
}

fn expressions_share_input(expr: Option<&ExpressionNode>) -> bool {
    match expr {
        None => true,
        Some(node) => node.operation.operation_type.id == SELF_REFERENCE_OP_TYPE.id,
    }
}

/// union operator: combine LHS and RHS matching nodes into a single set.
/// Simple dedup: if both sides refer to the same list (by pointer/length),
/// only include one copy.
pub fn union_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let lhs = get_matching_nodes(d, &ctx, expression_node.lhs.as_deref_mut())?;
    let rhs = get_matching_nodes(d, &ctx, expression_node.rhs.as_deref_mut())?;

    let mut results: Vec<TreeNode> = Vec::new();
    results.extend(lhs.matching_nodes.iter().cloned());

    // we can check if both contexts have the same length and content references.
    let same_pointer_list = std::ptr::eq(lhs.matching_nodes.as_ptr(), rhs.matching_nodes.as_ptr())
        && lhs.matching_nodes.len() == rhs.matching_nodes.len();
    let same_passthrough_list = expressions_share_input(expression_node.lhs.as_deref())
        && expressions_share_input(expression_node.rhs.as_deref())
        && lhs.matching_nodes.len() == rhs.matching_nodes.len()
        && lhs
            .matching_nodes
            .iter()
            .zip(&rhs.matching_nodes)
            .all(|(lhs_node, rhs_node)| nodes_structurally_eq(lhs_node, rhs_node));
    let same_list = same_pointer_list || same_passthrough_list;

    if !same_list {
        results.extend(rhs.matching_nodes.iter().cloned());
    }

    lhs.child_context(results)
}
