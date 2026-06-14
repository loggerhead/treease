use crate::operators::operator_helpers::*;
use crate::operators::*;

/// Check if a map has a specific key, or an array has a specific index.
fn map_has_key(node: &TreeNode, wanted_key: &str) -> bool {
    let contents = &node.content;
    let mut i = 0;
    while i + 1 < contents.len() {
        if contents[i].value == wanted_key {
            return true;
        }
        i += 2;
    }
    false
}

fn array_has_index(
    node: &TreeNode,
    wanted_key: &str,
    wanted_tag: Option<SemType>,
) -> Result<bool, CoreError> {
    if wanted_tag != Some(SemType::Int) {
        return Ok(false);
    }
    let number: i64 = wanted_key
        .parse()
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    Ok((node.content.len() as i64) > number)
}

/// has operator: check if map contains a key or array contains an index.
pub fn has_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let ro = ctx.read_only_clone()?;
    let rhs_ctx = get_matching_nodes(d, &ro, expression_node.rhs.as_deref_mut())?;

    let wanted = rhs_ctx.matching_nodes.first();
    let wanted_key = wanted.map_or("null", |w| w.value.as_str());
    let wanted_tag = wanted.and_then(|w| w.resolved_sem_type());

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let has_key = match candidate.kind {
            NodeKind::Mapping => map_has_key(candidate, wanted_key),
            NodeKind::Sequence => array_has_index(candidate, wanted_key, wanted_tag)?,
            _ => false,
        };
        let result = create_boolean_candidate(candidate, has_key)?;
        results.push((*result).clone());
    }
    ctx.child_context(results)
}
