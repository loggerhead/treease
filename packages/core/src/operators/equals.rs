use crate::operators::operator_helpers::*;
use crate::operators::*;

/// Compare two nodes for equality and produce a boolean node.
/// If `flip` is true the result is negated (not-equals semantics).
pub fn is_equals_with_nodes(
    flip: bool,
    lhs: Option<&TreeNode>,
    rhs: Option<&TreeNode>,
) -> Result<Box<TreeNode>, CoreError> {
    let empty = TreeNode::default();

    let l = match lhs {
        None => {
            let _r = match rhs {
                None => return create_boolean_candidate(&empty, !flip),
                Some(r) => {
                    let mut value = r.resolved_sem_type() == Some(SemType::Nil);
                    if flip {
                        value = !value;
                    }
                    return create_boolean_candidate(r, value);
                }
            };
        }
        Some(l) => l,
    };

    let r = match rhs {
        None => {
            let mut value = l.resolved_sem_type() == Some(SemType::Nil);
            if flip {
                value = !value;
            }
            return create_boolean_candidate(l, value);
        }
        Some(r) => r,
    };

    let mut value = false;
    if l.resolved_sem_type() == Some(SemType::Nil) {
        value = r.resolved_sem_type() == Some(SemType::Nil);
    } else if l.kind == NodeKind::Scalar && r.kind == NodeKind::Scalar {
        value = match_key(&l.value, &r.value);
    }
    if flip {
        value = !value;
    }
    create_boolean_candidate(l, value)
}

fn equals_calc(
    _d: &mut TreeEngine,
    _ctx: Context,
    lhs: Option<&TreeNode>,
    rhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    is_equals_with_nodes(false, lhs, rhs).map(Some)
}

fn not_equals_calc(
    _d: &mut TreeEngine,
    _ctx: Context,
    lhs: Option<&TreeNode>,
    rhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    is_equals_with_nodes(true, lhs, rhs).map(Some)
}

/// Equals operator entry point.
pub fn equals_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    cross_function(d, ctx, expression_node, equals_calc, true)
}

/// Not-equals operator entry point.
pub fn not_equals_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let ro = ctx.read_only_clone()?;
    cross_function(d, ro, expression_node, not_equals_calc, true)
}
