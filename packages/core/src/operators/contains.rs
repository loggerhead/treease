use crate::operators::operator_helpers::*;
use crate::operators::*;

/// Check whether lhs contains rhs (object/array/scalar semantics).
pub fn contains_value(lhs: &TreeNode, rhs: &TreeNode) -> Result<bool, CoreError> {
    match lhs.kind {
        NodeKind::Mapping => contains_object(lhs, rhs),
        NodeKind::Sequence => contains_array(lhs, rhs),
        NodeKind::Scalar => {
            if rhs.kind != NodeKind::Scalar || lhs.tag != rhs.tag {
                return Ok(false);
            }
            if lhs.sem_type == Some(SemType::Nil) {
                return Ok(true);
            }
            Ok(contains_scalars(lhs, rhs))
        }
        _ => Err(CoreError::Eval(EvalError::UnsupportedFlat)),
    }
}

fn contains_scalars(lhs: &TreeNode, rhs: &TreeNode) -> bool {
    if lhs.sem_type == Some(SemType::Str) {
        return lhs.value.contains(&rhs.value);
    }
    // For other scalars: exact equality
    lhs.value == rhs.value
}

fn contains_array_element(array: &TreeNode, item: &TreeNode) -> Result<bool, CoreError> {
    for child in &array.content {
        if contains_value(child, item)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn contains_array(lhs: &TreeNode, rhs: &TreeNode) -> Result<bool, CoreError> {
    if rhs.kind != NodeKind::Sequence {
        return contains_array_element(lhs, rhs);
    }
    for child in &rhs.content {
        if !contains_array_element(lhs, child)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn contains_object(lhs: &TreeNode, rhs: &TreeNode) -> Result<bool, CoreError> {
    if rhs.kind != NodeKind::Mapping {
        return Ok(false);
    }
    let contents = &rhs.content;
    let mut i = 0;
    while i + 1 < contents.len() {
        let rhs_key = &contents[i];
        let rhs_value = &contents[i + 1];

        // Find matching key in lhs
        let lhs_key_index = find_key_in_map(lhs, rhs_key);
        if lhs_key_index < 0 || (lhs_key_index as usize) % 2 != 0 {
            return Ok(false);
        }

        let idx = lhs_key_index as usize;
        if idx + 1 >= lhs.content.len() {
            return Ok(false);
        }
        let lhs_value = &lhs.content[idx + 1];
        if !contains_value(lhs_value, rhs_value)? {
            return Ok(false);
        }
        i += 2;
    }
    Ok(true)
}

/// Cross-function: check containment for a pair of nodes.
pub fn contains_with_nodes(
    _d: &mut TreeEngine,
    ctx: Context,
    lhs_opt: Option<&TreeNode>,
    rhs_opt: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    let lhs = match lhs_opt {
        Some(n) => n,
        None => return Ok(None),
    };
    let rhs = match rhs_opt {
        Some(n) => n,
        None => return Ok(None),
    };

    if lhs.kind != rhs.kind {
        if let Some(ref d2) = ctx.diagnostics {
            let msg = format!("{} cannot check contained in {}", rhs.tag, lhs.tag);
            d2.set_messagef("eval", &msg)?;
        }
        return Err(CoreError::OperatorMessage {
            op: "contains".to_string(),
            message: format!("kind mismatch: {} vs {}", lhs.tag, rhs.tag),
        });
    }

    let ok = contains_value(lhs, rhs)?;
    create_boolean_candidate(lhs, ok).map(|b| Some(b))
}

/// contains operator: check if lhs contains rhs.
pub fn contains_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let ro = ctx.read_only_clone()?;
    cross_function(d, ro, expression_node, contains_with_nodes, false)
}
