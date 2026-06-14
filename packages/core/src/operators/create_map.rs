use crate::operators::operator_helpers::*;
use crate::operators::*;

/// Convert a list of tree nodes into a sequence node.
fn list_to_node_seq(list: &[TreeNode]) -> Result<Box<TreeNode>, CoreError> {
    let mut node = TreeNode::default();
    node.kind = NodeKind::Sequence;
    node.sem_type = Some(SemType::Seq);
    node.tag = SemType::Seq.to_string().into();
    for entry_candidate in list {
        node.add_child(entry_candidate)?;
    }
    Ok(Box::new(node))
}

/// Build a single key-value pair node from LHS (key) and RHS (value).
fn build_single_pair(
    _d: &mut TreeEngine,
    _ctx: Context,
    lhs_opt: Option<&TreeNode>,
    rhs_opt: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    let lhs = match lhs_opt {
        Some(l) => l,
        None => return Ok(None),
    };
    let rhs = match rhs_opt {
        Some(r) => r,
        None => return Ok(None),
    };

    let mut node = TreeNode::default();
    node.kind = NodeKind::Mapping;
    node.sem_type = Some(SemType::Map);
    node.tag = SemType::Map.to_string().into();
    node.add_key_value_child(lhs, rhs)?;
    Ok(Some(Box::new(node)))
}

/// Build a sequence of mapping pairs for a given matching node (or None).
fn sequence_for(
    d: &mut TreeEngine,
    ctx: &Context,
    matching_node: Option<&TreeNode>,
    expression_node: &mut ExpressionNode,
) -> Result<Box<TreeNode>, CoreError> {
    let mut matches: Vec<TreeNode> = Vec::new();
    if let Some(m) = matching_node {
        matches.push(m.clone());
    }

    let child_ctx = ctx.child_context(matches)?;
    let map_pairs = cross_function(d, child_ctx, expression_node, build_single_pair, false)?;

    let mut inner_list = list_to_node_seq(&map_pairs.matching_nodes)?;
    if let Some(source) = matching_node {
        inner_list.document = source.document;
        inner_list.filename = source.filename.clone();
        inner_list.file_index = source.file_index;
    }
    Ok(inner_list)
}

/// create_map operator: build a map from a sequence of items and
/// return a sequence of result pairs.
pub fn create_map_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut sequences: Vec<TreeNode> = Vec::new();

    if !ctx.matching_nodes.is_empty() {
        for matching_node in &ctx.matching_nodes {
            let sequence_node = sequence_for(d, &ctx, Some(matching_node), expression_node)?;
            sequences.push((*sequence_node).clone());
        }
    } else {
        let sequence_node = sequence_for(d, &ctx, None, expression_node)?;
        sequences.push((*sequence_node).clone());
    }

    let node = list_to_node_seq(&sequences)?;
    ctx.single_child_context(&node)
}
