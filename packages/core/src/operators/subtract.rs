use crate::operators::operator_helpers::*;
use crate::operators::*;

// ── Subtraction operator (a - b) ──────────────────────────────────

/// Recursively compare two tree nodes for structural equality.
fn recursive_node_equal(a: &TreeNode, b: &TreeNode) -> bool {
    if a.kind != b.kind {
        return false;
    }
    if a.sem_type != b.sem_type {
        return false;
    }
    if a.value != b.value {
        return false;
    }
    if a.tag != b.tag {
        return false;
    }
    if a.content.len() != b.content.len() {
        return false;
    }
    for (ca, cb) in a.content.iter().zip(b.content.iter()) {
        if !recursive_node_equal(ca, cb) {
            return false;
        }
    }
    true
}

/// Remove elements from lhs array that also appear in rhs array.
fn subtract_array(lhs: &TreeNode, rhs: &TreeNode) -> Result<Vec<TreeNode>, CoreError> {
    let mut out = Vec::new();
    'outer: for lchild in &lhs.content {
        for rchild in &rhs.content {
            if recursive_node_equal(lchild, rchild) {
                continue 'outer;
            }
        }
        out.push(lchild.clone());
    }
    Ok(out)
}

/// Subtract two scalar values.
fn subtract_scalars(
    target: &mut TreeNode,
    lhs: &TreeNode,
    rhs: &TreeNode,
) -> Result<(), CoreError> {
    let mut lhs_tag = lhs.tag.clone();
    let mut rhs_tag = rhs.tag.clone();
    let mut lhs_is_custom = false;
    if !SemType::has_tag_prefix(&lhs_tag) {
        lhs_tag = lhs.guess_tag_from_custom_type();
        lhs_is_custom = true;
    }
    if !SemType::has_tag_prefix(&rhs_tag) {
        rhs_tag = rhs.guess_tag_from_custom_type();
    }

    let lhs_tid = SemType::from_string(&lhs_tag);
    let rhs_tid = SemType::from_string(&rhs_tag);

    if lhs_tid == Some(SemType::Str) {
        return Err(CoreError::Eval(EvalError::StringsCannotBeSubtracted));
    }

    // Integer subtraction — preserve hex/octal/binary format from lhs
    if lhs_tid == Some(SemType::Int) && rhs_tid == Some(SemType::Int) {
        let lhs_parsed = parse_int64_with_fmt(&lhs.value)?;
        let rhs_parsed = parse_int64(&rhs.value)?;
        let result = lhs_parsed.value - rhs_parsed;
        target.tag.clone_from(&lhs.tag);
        target.sem_type = lhs.resolved_sem_type();
        target.value = format_int64_with_fmt(&lhs_parsed.fmt, result)?;
        return Ok(());
    }

    // Float subtraction
    let lhs_is_number = lhs_tid == Some(SemType::Int) || lhs_tid == Some(SemType::Float);
    let rhs_is_number = rhs_tid == Some(SemType::Int) || rhs_tid == Some(SemType::Float);
    if lhs_is_number && rhs_is_number {
        let lhs_num = parse_float64(&lhs.value)?;
        let rhs_num = parse_float64(&rhs.value)?;
        let resultf = lhs_num - rhs_num;
        target.tag = if lhs_is_custom {
            lhs.tag.clone()
        } else {
            SemType::Float.to_string().into()
        };
        target.sem_type = if lhs_is_custom {
            None
        } else {
            Some(SemType::Float)
        };
        target.value = float_to_string(resultf);
        return Ok(());
    }

    Err(CoreError::Eval(EvalError::CannotSubtractTypes))
}

/// Subtract two nodes and return the result.
pub fn subtract_with_nodes(lhs: &TreeNode, rhs: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    if lhs.resolved_sem_type() == Some(SemType::Nil) {
        return lhs.copy_as_replacement(rhs);
    }

    let mut target = lhs.copy_without_content()?;
    match lhs.kind {
        NodeKind::Mapping => Err(CoreError::Eval(EvalError::MapsNotSupportedForSubtraction)),
        NodeKind::Sequence => {
            if rhs.kind != NodeKind::Sequence {
                return Err(CoreError::Eval(EvalError::CannotSubtractNonSequence));
            }
            target.kind = NodeKind::Sequence;
            target.tag.clone_from(&lhs.tag);
            target.sem_type = lhs.resolved_sem_type();
            let kept = subtract_array(lhs, rhs)?;
            target.add_children(&kept)?;
            Ok(target)
        }
        NodeKind::Scalar => {
            if rhs.kind != NodeKind::Scalar {
                return Err(CoreError::Eval(EvalError::CannotSubtractNonScalar));
            }
            target.kind = NodeKind::Scalar;
            subtract_scalars(&mut target, lhs, rhs)?;
            Ok(target)
        }
        _ => Err(CoreError::Eval(EvalError::UnsupportedFlat)),
    }
}

// ── Calculation wrapper for cross_function ────────────────────────

fn subtract_calc(
    _d: &mut TreeEngine,
    _ctx: Context,
    lhs: Option<&TreeNode>,
    rhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    let l = match lhs {
        Some(n) => n,
        None => return Ok(None),
    };
    let r = match rhs {
        Some(n) => n,
        None => return Ok(None),
    };
    Ok(Some(subtract_with_nodes(l, r)?))
}

// ── Compound assignment ───────────────────────────────────────────

fn create_subtract_op(lhs: &mut ExpressionNode, rhs: &ExpressionNode) -> Box<ExpressionNode> {
    Box::new(ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &SUBTRACT_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(lhs.clone())),
        rhs: Some(Box::new(rhs.clone())),
    })
}

/// Execute "-=" compound assignment.
pub fn subtract_assign_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    compound_assign_function(d, ctx, expression_node, create_subtract_op)
}

/// Main "-" operator entry point.
pub fn subtract_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let ro = ctx.read_only_clone()?;
    cross_function(d, ro, expression_node, subtract_calc, false)
}
