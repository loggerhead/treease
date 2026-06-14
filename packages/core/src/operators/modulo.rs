use crate::operators::operator_helpers::*;
use crate::operators::*;

// ── Modulo operator (a % b) ───────────────────────────────────────

/// Floating-point modulo: lhs - trunc(lhs / rhs) * rhs
fn fmod(lhs: f64, rhs: f64) -> f64 {
    lhs - (lhs / rhs).trunc() * rhs
}

/// Modulo two scalar values.
pub fn modulo_scalars(
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

    // Integer modulo
    if lhs_tid == Some(SemType::Int) && rhs_tid == Some(SemType::Int) {
        target.kind = NodeKind::Scalar;
        let lhs_parsed = parse_int64(&lhs.value)?;
        let rhs_parsed = parse_int64(&rhs.value)?;
        if rhs_parsed == 0 {
            return Err(CoreError::Eval(EvalError::CannotModuloByZero));
        }
        let remainder = lhs_parsed % rhs_parsed;
        target.tag.clone_from(&lhs.tag);
        target.sem_type = lhs.resolved_sem_type();
        target.value = remainder.to_string();
        return Ok(());
    }

    // Float modulo
    let lhs_is_number = lhs_tid == Some(SemType::Int) || lhs_tid == Some(SemType::Float);
    let rhs_is_number = rhs_tid == Some(SemType::Int) || rhs_tid == Some(SemType::Float);
    if lhs_is_number && rhs_is_number {
        target.kind = NodeKind::Scalar;
        let lhs_num = parse_float64(&lhs.value)?;
        let rhs_num = parse_float64(&rhs.value)?;
        let remainderf = fmod(lhs_num, rhs_num);
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
        target.value = float_to_string(remainderf);
        return Ok(());
    }

    Err(CoreError::Eval(EvalError::CannotModuloTypes))
}

/// Modulo two nodes and return the result.
pub fn modulo_with_nodes(lhs: &TreeNode, rhs: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    if lhs.resolved_sem_type() == Some(SemType::Nil) {
        return Err(CoreError::Eval(EvalError::CannotModuloNull));
    }
    if lhs.kind != NodeKind::Scalar || rhs.kind != NodeKind::Scalar {
        return Err(CoreError::Eval(EvalError::CannotModuloNonScalars));
    }
    let mut target = lhs.copy_without_content()?;
    modulo_scalars(&mut target, lhs, rhs)?;
    Ok(target)
}

// ── Calculation wrapper for cross_function ────────────────────────

fn modulo_calc(
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
    Ok(Some(modulo_with_nodes(l, r)?))
}

/// Main "%" operator entry point.
pub fn modulo_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    cross_function_read_only(d, ctx, expression_node, modulo_calc, false)
}
