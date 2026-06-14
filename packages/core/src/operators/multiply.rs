use crate::operators::operator_helpers::*;
use crate::operators::*;

// ── Multiplication operator (a * b) ───────────────────────────────

/// Repeat a string `count` times.
fn repeat(s: &str, count: usize) -> String {
    if count == 0 || s.is_empty() {
        return String::new();
    }
    s.repeat(count)
}

/// Create a scalar target initialized from a base node.
fn init_scalar_target_from(base: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut target = base.copy_without_content()?;
    target.kind = NodeKind::Scalar;
    Ok(target)
}

/// Multiply two scalar values: numeric multiplication or string repetition.
pub fn multiply_scalars(lhs: &TreeNode, rhs: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut lhs_tag = lhs.tag.clone();
    let rhs_tag = rhs.guess_tag_from_custom_type();
    let mut lhs_is_custom = false;
    if !SemType::has_tag_prefix(&lhs_tag) {
        lhs_tag = lhs.guess_tag_from_custom_type();
        lhs_is_custom = true;
    }

    let lhs_sem_type = SemType::from_string(&lhs_tag);
    let rhs_sem_type = SemType::from_string(&rhs_tag);

    // Integer multiplication — preserve hex/octal/binary format from lhs
    if lhs_sem_type == Some(SemType::Int) && rhs_sem_type == Some(SemType::Int) {
        let mut target = init_scalar_target_from(lhs)?;
        target.sem_type = lhs.resolved_sem_type();
        target.tag.clone_from(&lhs.tag);
        let lhs_parsed = parse_int64_with_fmt(&lhs.value)?;
        let rhs_parsed = parse_int64(&rhs.value)?;
        let result = lhs_parsed.value * rhs_parsed;
        target.value = format_int64_with_fmt(&lhs_parsed.fmt, result)?;
        return Ok(target);
    }

    // Float multiplication
    let lhs_is_number = lhs_sem_type == Some(SemType::Int) || lhs_sem_type == Some(SemType::Float);
    let rhs_is_number = rhs_sem_type == Some(SemType::Int) || rhs_sem_type == Some(SemType::Float);
    if lhs_is_number && rhs_is_number {
        let mut target = init_scalar_target_from(lhs)?;
        if lhs_is_custom {
            target.sem_type = None;
            target.tag.clone_from(&lhs.tag);
        } else {
            target.sem_type = Some(SemType::Float);
            target.tag = SemType::Float.to_string().into();
        }
        let lhs_num = parse_float64(&lhs.value)?;
        let rhs_num = parse_float64(&rhs.value)?;
        target.value = float_to_string(lhs_num * rhs_num);
        return Ok(target);
    }

    // String * Int (or Int * String): repeat string
    let str_int = match (lhs_sem_type, rhs_sem_type) {
        (Some(SemType::Str), Some(SemType::Int)) => true,
        (Some(SemType::Int), Some(SemType::Str)) => true,
        _ => false,
    };
    if str_int {
        let (string_node, int_node) = if lhs_sem_type == Some(SemType::Str) {
            (lhs, rhs)
        } else {
            (rhs, lhs)
        };
        let count_i32: i32 = int_node
            .value
            .parse()
            .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
        if count_i32 < 0 {
            return Err(CoreError::Eval(EvalError::NegativeRepeat));
        }
        if count_i32 > 10_000_000 {
            return Err(CoreError::Eval(EvalError::RepeatTooLarge));
        }
        let count = count_i32 as usize;
        let mut target = init_scalar_target_from(lhs)?;
        target.sem_type = string_node.resolved_sem_type();
        target.tag.clone_from(&string_node.tag);
        target.value = repeat(&string_node.value, count);
        return Ok(target);
    }

    Err(CoreError::Eval(EvalError::CannotMultiplyTypes))
}

/// Merge two mappings: shallow merge, RHS keys override LHS keys.
fn merge_mappings(lhs: &TreeNode, rhs: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut out = Box::new(TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.to_string().into(),
        ..Default::default()
    });
    out.encode_separate = lhs.encode_separate || rhs.encode_separate;
    if !rhs.leading_content.is_empty() {
        out.leading_content.clone_from(&rhs.leading_content);
    } else {
        out.leading_content.clone_from(&lhs.leading_content);
    }
    if !rhs.head_comment.is_empty() {
        out.head_comment.clone_from(&rhs.head_comment);
    } else {
        out.head_comment.clone_from(&lhs.head_comment);
    }
    if !rhs.line_comment.is_empty() {
        out.line_comment.clone_from(&rhs.line_comment);
    } else {
        out.line_comment.clone_from(&lhs.line_comment);
    }
    if !rhs.foot_comment.is_empty() {
        out.foot_comment.clone_from(&rhs.foot_comment);
    } else {
        out.foot_comment.clone_from(&lhs.foot_comment);
    }

    // Copy LHS keys/values
    if lhs.kind == NodeKind::Mapping {
        let mut li = 0;
        while li + 1 < lhs.content.len() {
            let k = lhs.content[li].copy()?;
            let v = lhs.content[li + 1].copy()?;
            out.add_key_value_child(&k, &v)?;
            li += 2;
        }
    }

    // Merge RHS: overwrite existing keys, append new ones
    let mut ri = 0;
    while ri + 1 < rhs.content.len() {
        let rk = &rhs.content[ri];
        let rv = &rhs.content[ri + 1];

        let idx_opt = find_map_key_index(&out, &rk.value);
        if let Some(idx) = idx_opt {
            let mut new_v = rv.copy()?;
            new_v.parent = None; // Phase B: register in TreeStore
            new_v.key = None; // Phase B: register in TreeStore
            out.content[idx + 1] = (*new_v).clone();
        } else {
            let k = rk.copy()?;
            let v = rv.copy()?;
            out.add_key_value_child(&k, &v)?;
        }
        ri += 2;
    }

    Ok(out)
}

/// Find the index of a key in a mapping node's content (returns even index).
fn find_map_key_index(map: &TreeNode, key: &str) -> Option<usize> {
    if map.kind != NodeKind::Mapping {
        return None;
    }
    let mut i = 0;
    while i + 1 < map.content.len() {
        let k = &map.content[i];
        if k.kind == NodeKind::Scalar && k.sem_type == Some(SemType::Str) && k.value == key {
            return Some(i);
        }
        i += 2;
    }
    None
}

/// Merge two sequences: concatenate RHS after LHS.
fn merge_sequences(lhs: &TreeNode, rhs: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut out = Box::new(TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.to_string().into(),
        ..Default::default()
    });

    if lhs.kind == NodeKind::Sequence {
        for child in &lhs.content {
            out.add_child(&*child.copy()?)?;
        }
    }
    for child in &rhs.content {
        out.add_child(&*child.copy()?)?;
    }
    Ok(out)
}

/// Multiply two nodes and return the result.
pub fn multiply_with_nodes(lhs: &TreeNode, rhs: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    // Null times anything = copy of itself
    if rhs.resolved_sem_type() == Some(SemType::Nil) {
        return lhs.copy();
    }

    let lhs_is_null = lhs.resolved_sem_type() == Some(SemType::Nil);

    // Map * Map = merge mappings
    if (lhs.kind == NodeKind::Mapping && rhs.kind == NodeKind::Mapping)
        || (lhs_is_null && rhs.kind == NodeKind::Mapping)
    {
        return merge_mappings(lhs, rhs);
    }
    // Seq * Seq = merge sequences
    if (lhs.kind == NodeKind::Sequence && rhs.kind == NodeKind::Sequence)
        || (lhs_is_null && rhs.kind == NodeKind::Sequence)
    {
        return merge_sequences(lhs, rhs);
    }

    multiply_scalars(lhs, rhs)
}

// ── Calculation wrapper for cross_function ────────────────────────

fn multiply_calc(
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
    Ok(Some(multiply_with_nodes(l, r)?))
}

// ── Compound assignment ───────────────────────────────────────────

fn create_multiply_op(lhs: &mut ExpressionNode, rhs: &ExpressionNode) -> Box<ExpressionNode> {
    Box::new(ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &MULTIPLY_OP_TYPE,
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

/// Execute "*=" compound assignment.
pub fn multiply_assign_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    compound_assign_function(d, ctx, expression_node, create_multiply_op)
}

/// Main "*" operator entry point.
pub fn multiply_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    cross_function(d, ctx, expression_node, multiply_calc, false)
}
