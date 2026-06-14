use std::collections::HashSet;

use crate::operators::*;

/// unique operator: deduplicate array elements (equivalent to unique_by(.)).
pub fn unique(
    ctx: Context,
    d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    // Build unique_by expression: unique_by(.)
    let self_op = Box::new(Operation {
        operation_type: &SELF_REFERENCE_OP_TYPE,
        value: None,
        string_value: String::new(),
        tree_node: None,
        preferences: None,
        update_assign: false,
    });
    let self_exp = Box::new(ExpressionNode {
        operation: self_op,
        lhs: None,
        rhs: None,
    });

    let unique_by_op = Box::new(Operation {
        operation_type: &UNIQUE_BY_OP_TYPE,
        value: None,
        string_value: String::new(),
        tree_node: None,
        preferences: None,
        update_assign: false,
    });
    let mut unique_by_exp = Box::new(ExpressionNode {
        operation: unique_by_op,
        lhs: None,
        rhs: Some(self_exp),
    });

    unique_by(ctx, d, &mut unique_by_exp)
}

/// unique_by operator: deduplicate array elements by expression result.
pub fn unique_by(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results: Vec<TreeNode> = Vec::new();

    let rhs_expr = expression_node
        .rhs
        .as_ref()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;

    for candidate in &ctx.matching_nodes {
        if candidate.kind != NodeKind::Sequence {
            return Err(CoreError::Eval(EvalError::UniqueOnlySupportsArrays));
        }

        // Use hash sets to track seen keys.
        let mut seen_scalars: HashSet<String> = HashSet::new();
        let mut seen_complex: Vec<TreeNode> = Vec::new();

        let mut result_node = (*candidate
            .create_replacement_with_comments(NodeKind::Sequence, SemType::Seq.to_string())?)
        .clone();

        for child in &candidate.content {
            // Evaluate the child through the RHS expression.
            let child_ctx = ctx.single_readonly_child_context(child)?;
            let mut rhs_expr = (*rhs_expr).clone();
            let rhs_ctx = get_matching_nodes(d, &child_ctx, Some(&mut rhs_expr))?;

            // Get the unique key from the RHS result.
            let key_info = get_unique_key(&rhs_ctx);
            if is_already_seen(&key_info, &mut seen_scalars, &mut seen_complex) {
                continue;
            }
            result_node.add_child(child)?;
        }
        results.push(result_node);
    }

    ctx.child_context(results)
}

enum UniqueKey {
    Scalar(String),
    Complex(TreeNode),
}

fn get_unique_key(rhs: &Context) -> UniqueKey {
    if rhs.matching_nodes.is_empty() {
        return UniqueKey::Scalar("null".to_string());
    }
    let key_candidate = &rhs.matching_nodes[0];
    if key_candidate.kind == NodeKind::Scalar {
        UniqueKey::Scalar(key_candidate.value.clone())
    } else {
        UniqueKey::Complex(key_candidate.clone())
    }
}

fn is_already_seen(
    key: &UniqueKey,
    seen_scalars: &mut HashSet<String>,
    seen_complex: &mut Vec<TreeNode>,
) -> bool {
    match key {
        UniqueKey::Scalar(s) => {
            if seen_scalars.contains(s) {
                return true;
            }
            seen_scalars.insert(s.clone());
            false
        }
        UniqueKey::Complex(n) => {
            for prev in seen_complex.iter() {
                if crate::core::recursive_node_compare(prev, n) {
                    return true;
                }
            }
            seen_complex.push(n.clone());
            false
        }
    }
}
