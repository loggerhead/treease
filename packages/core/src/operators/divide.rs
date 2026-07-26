use crate::operators::operator_helpers::*;
use crate::operators::*;

// ── Division operator (a / b) ─────────────────────────────────────

/// Split a string by a delimiter into a sequence of string tree nodes.
fn split(value: &str, split_str: &str) -> Vec<TreeNode> {
    if value.is_empty() {
        return Vec::new();
    }

    let mut parts = Vec::new();
    for part in value.split(split_str) {
        let mut n = TreeNode::default();
        n.kind = NodeKind::Scalar;
        n.sem_type = Some(SemType::Str);
        n.tag = SemType::Str.to_string().into();
        n.value = part.to_string();
        parts.push(n);
    }
    parts
}

/// Divide two scalar values: numeric division or string split.
pub fn divide_scalars(
    target: &mut TreeNode,
    lhs: &TreeNode,
    rhs: &TreeNode,
) -> Result<(), CoreError> {
    let mut lhs_tag = lhs.tag.clone();
    let rhs_tag = rhs.guess_tag_from_custom_type();
    let mut lhs_is_custom = false;
    if !SemType::has_tag_prefix(&lhs_tag) {
        lhs_tag = lhs.guess_tag_from_custom_type();
        lhs_is_custom = true;
    }

    let lhs_sem_type = SemType::from_string(&lhs_tag);
    let rhs_sem_type = SemType::from_string(&rhs_tag);
    let lhs_stringish = lhs_sem_type.is_none() || lhs_sem_type == Some(SemType::Str);
    let rhs_stringish = rhs_sem_type.is_none() || rhs_sem_type == Some(SemType::Str);

    // String split: lhs is split by rhs delimiter
    if lhs_stringish && rhs_stringish {
        target.kind = NodeKind::Sequence;
        target.sem_type = Some(SemType::Seq);
        target.tag = SemType::Seq.to_string().into();
        let parts = split(&lhs.value, &rhs.value);
        target.add_children(&parts)?;
        return Ok(());
    }

    // Integer division preserves the left-hand integer representation when it
    // is exact. Division by zero deliberately remains floating-point so the
    // established +Inf/-Inf behavior is retained.
    if lhs_sem_type == Some(SemType::Int) && rhs_sem_type == Some(SemType::Int) {
        let lhs_parsed = parse_int64_with_fmt(&lhs.value)?;
        let rhs_parsed = parse_int64(&rhs.value)?;
        if rhs_parsed != 0 && lhs_parsed.value % rhs_parsed == 0 {
            target.kind = NodeKind::Scalar;
            target.sem_type = lhs.resolved_sem_type();
            target.tag.clone_from(&lhs.tag);
            target.value = format_int64_with_fmt(&lhs_parsed.fmt, lhs_parsed.value / rhs_parsed)?;
            return Ok(());
        }
    }

    // Numeric division
    let lhs_is_number = lhs_sem_type == Some(SemType::Int) || lhs_sem_type == Some(SemType::Float);
    let rhs_is_number = rhs_sem_type == Some(SemType::Int) || rhs_sem_type == Some(SemType::Float);
    if lhs_is_number && rhs_is_number {
        target.kind = NodeKind::Scalar;
        let lhs_num = parse_float64(&lhs.value)?;
        let rhs_num = parse_float64(&rhs.value)?;
        let quotient = lhs_num / rhs_num;
        if lhs_is_custom {
            target.sem_type = None;
            target.tag.clone_from(&lhs.tag);
        } else {
            target.sem_type = Some(SemType::Float);
            target.tag = SemType::Float.to_string().into();
        }
        target.value = float_to_string(quotient);
        return Ok(());
    }

    Err(CoreError::Eval(EvalError::CannotDivideTypes))
}

/// Divide two nodes and return the result.
pub fn divide_with_nodes(lhs: &TreeNode, rhs: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    if lhs.resolved_sem_type() == Some(SemType::Nil) {
        return Err(CoreError::Eval(EvalError::CannotDivideNull));
    }

    let mut target = lhs.copy_without_content()?;
    if lhs.kind != NodeKind::Scalar || rhs.kind != NodeKind::Scalar {
        return Err(CoreError::Eval(EvalError::CannotDivideNonScalars));
    }
    divide_scalars(&mut target, lhs, rhs)?;
    Ok(target)
}

// ── Calculation wrapper for cross_function ────────────────────────

fn divide_calc(
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
    Ok(Some(divide_with_nodes(l, r)?))
}

/// Main "/" operator entry point.
pub fn divide_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    cross_function_read_only(d, ctx, expression_node, divide_calc, false)
}
