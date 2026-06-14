use crate::operators::operator_helpers::*;
use crate::operators::*;

/// Check if each candidate node is a map key.
pub fn is_key_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let result = create_boolean_candidate(candidate, candidate.is_map_key)?;
        results.push((*result).clone());
    }
    ctx.child_context(results)
}

/// Get the key node for each candidate (only meaningful for map value nodes).
pub fn get_key_operator(
    ctx: Context,
    d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        if let Some(key_id) = candidate.key {
            let key_node = d.store.get(key_id);
            results.push(key_node.clone());
        } else if !candidate.is_map_key {
            if let Some(index) = candidate.sequence_index {
                results.push(*create_scalar_node_i64(index)?);
            }
        }
    }
    ctx.child_context(results)
}

fn new_sequence_node() -> Result<Box<TreeNode>, CoreError> {
    let mut seq = Box::new(TreeNode::default());
    seq.kind = NodeKind::Sequence;
    seq.sem_type = Some(SemType::Seq);
    seq.tag = SemType::Seq.to_string().into();
    Ok(seq)
}

fn get_map_keys(node: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut seq = new_sequence_node()?;
    let contents = &node.content;
    let mut i = 0;
    while i + 1 < contents.len() {
        seq.add_child(&contents[i])?;
        i += 2;
    }
    Ok(seq)
}

fn get_indices(node: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut seq = new_sequence_node()?;
    for (idx, _) in node.content.iter().enumerate() {
        let child = create_scalar_node_i64(idx as i64)?;
        seq.add_child(&child)?;
    }
    Ok(seq)
}

/// Get all keys (for maps) or indices (for sequences).
pub fn keys_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let target = match candidate.kind {
            NodeKind::Mapping => get_map_keys(candidate)?,
            NodeKind::Sequence => get_indices(candidate)?,
            _ => return Err(CoreError::Eval(EvalError::KeysOnlyWorksForMapsAndArrays)),
        };
        results.push((*target).clone());
    }
    ctx.child_context(results)
}
