use crate::operators::operator_helpers::*;
use crate::operators::*;

// ── Addition operator (a + b) ─────────────────────────────────────

/// Concatenate two string slices into a new String.
fn concat(lhs: &str, rhs: &str) -> String {
    let mut out = String::with_capacity(lhs.len() + rhs.len());
    out.push_str(lhs);
    out.push_str(rhs);
    out
}

/// Merge two mappings: RHS keys overwrite LHS keys, new keys are appended.
fn add_maps(target: &mut TreeNode, lhs: &TreeNode, rhs: &TreeNode) -> Result<(), CoreError> {
    target.content.clear();
    target.add_children(&lhs.content)?;

    let mut i = 0;
    while i + 1 < rhs.content.len() {
        let key = &rhs.content[i];
        let value = &rhs.content[i + 1];
        let idx_in_lhs = find_key_in_map(target, key);
        if idx_in_lhs < 0 {
            target.add_key_value_child(key, value)?;
        } else {
            let idx = idx_in_lhs as usize;
            let old_value = &target.content[idx + 1];
            let new_value = old_value.copy_as_replacement(value)?;
            target.content[idx + 1] = (*new_value).clone();
        }
        i += 2;
    }
    target.kind = NodeKind::Mapping;
    target.sem_type = lhs.resolved_sem_type();
    target.tag.clone_from(&lhs.tag);
    Ok(())
}

/// Merge two sequences: append RHS elements to LHS.
fn add_sequences(target: &mut TreeNode, lhs: &TreeNode, rhs: &TreeNode) -> Result<(), CoreError> {
    target.kind = NodeKind::Sequence;
    target.sem_type = lhs.resolved_sem_type();
    target.tag.clone_from(&lhs.tag);

    target.add_children(&lhs.content)?;

    if rhs.resolved_sem_type() == Some(SemType::Nil) {
        return Ok(());
    }
    if rhs.kind == NodeKind::Sequence {
        target.add_children(&rhs.content)?;
        return Ok(());
    }
    let rhs_copy = rhs.copy()?;
    target.add_child(&rhs_copy)?;
    Ok(())
}

/// Add two scalars: numeric addition or string concatenation.
fn add_scalars(target: &mut TreeNode, lhs: &TreeNode, rhs: &TreeNode) -> Result<(), CoreError> {
    let mut lhs_tag = lhs.tag.clone();
    let rhs_tag = rhs.guess_tag_from_custom_type();
    let mut lhs_is_custom = false;
    if !SemType::has_tag_prefix(&lhs_tag) {
        lhs_tag = lhs.guess_tag_from_custom_type();
        lhs_is_custom = true;
    }

    let lhs_sem_type = SemType::from_string(&lhs_tag);
    let rhs_sem_type = SemType::from_string(&rhs_tag);

    // String concatenation (lhs is string)
    if lhs_sem_type == Some(SemType::Str) {
        target.sem_type = lhs.resolved_sem_type();
        target.tag.clone_from(&lhs.tag);
        if rhs_sem_type == Some(SemType::Nil) {
            target.value.clone_from(&lhs.value);
        } else {
            target.value = concat(&lhs.value, &rhs.value);
        }
        return Ok(());
    }
    // String concatenation (rhs is string)
    if rhs_sem_type == Some(SemType::Str) {
        target.sem_type = rhs.resolved_sem_type();
        target.tag.clone_from(&rhs.tag);
        target.value = concat(&lhs.value, &rhs.value);
        return Ok(());
    }

    // Integer addition — preserve hex/octal/binary format from lhs
    if lhs_sem_type == Some(SemType::Int) && rhs_sem_type == Some(SemType::Int) {
        let lhs_parsed = parse_int64_with_fmt(&lhs.value)?;
        let rhs_parsed = parse_int64(&rhs.value)?;
        let sum = lhs_parsed.value + rhs_parsed;
        target.sem_type = lhs.resolved_sem_type();
        target.tag.clone_from(&lhs.tag);
        target.value = format_int64_with_fmt(&lhs_parsed.fmt, sum)?;
        return Ok(());
    }

    // Float addition
    let lhs_is_number = lhs_sem_type == Some(SemType::Int) || lhs_sem_type == Some(SemType::Float);
    let rhs_is_number = rhs_sem_type == Some(SemType::Int) || rhs_sem_type == Some(SemType::Float);
    if lhs_is_number && rhs_is_number {
        let lhs_num = parse_float64(&lhs.value)?;
        let rhs_num = parse_float64(&rhs.value)?;
        let sumf = lhs_num + rhs_num;
        if lhs_is_custom {
            target.sem_type = None;
            target.tag.clone_from(&lhs.tag);
        } else {
            target.sem_type = Some(SemType::Float);
            target.tag = SemType::Float.to_string().into();
        }
        target.value = float_to_string(sumf);
        return Ok(());
    }
    Err(CoreError::Eval(EvalError::CannotAddTypes))
}

/// Add two nodes and return the result. Called via cross_function.
pub fn add_with_nodes(
    lhs: Option<&TreeNode>,
    rhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    match (lhs, rhs) {
        (None, None) => Ok(None),
        (None, Some(r)) => Ok(Some(r.copy()?)),
        (Some(l), None) => Ok(Some(l.copy()?)),
        (Some(l), Some(r)) => {
            if l.sem_type == Some(SemType::Nil) {
                return Ok(Some(l.copy_as_replacement(r)?));
            }

            // Scalar + Map: wrap scalar as {"value": scalar} then merge
            if l.kind == NodeKind::Scalar && r.kind == NodeKind::Mapping {
                let mut tmp = TreeNode {
                    kind: NodeKind::Mapping,
                    sem_type: Some(SemType::Map),
                    tag: SemType::Map.to_string().into(),
                    ..Default::default()
                };

                let key_node = create_string_scalar_node("value")?;
                tmp.add_key_value_child(&key_node, l)?;

                let mut target = tmp.copy_without_content()?;
                add_maps(&mut target, &tmp, r)?;
                return Ok(Some(target));
            }

            let mut target = l.copy_without_content()?;
            match l.kind {
                NodeKind::Mapping => {
                    if r.kind != NodeKind::Mapping {
                        return Err(CoreError::Eval(EvalError::CannotAddNonMapToMap));
                    }
                    add_maps(&mut target, l, r)?;
                }
                NodeKind::Sequence => {
                    add_sequences(&mut target, l, r)?;
                }
                NodeKind::Scalar => {
                    if r.kind != NodeKind::Scalar {
                        return Err(CoreError::Eval(EvalError::CannotAddNonScalarToScalar));
                    }
                    target.kind = NodeKind::Scalar;
                    add_scalars(&mut target, l, r)?;
                }
                _ => return Err(CoreError::Eval(EvalError::UnsupportedFlat)),
            }
            Ok(Some(target))
        }
    }
}

// ── Calculation wrapper for cross_function ────────────────────────

/// Cross-function calculation for addition.
pub fn add_calc(
    _d: &mut TreeEngine,
    _ctx: Context,
    lhs: Option<&TreeNode>,
    rhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError> {
    add_with_nodes(lhs, rhs)
}

// ── Compound assignment ───────────────────────────────────────────

fn create_add_op(lhs: &mut ExpressionNode, rhs: &ExpressionNode) -> Box<ExpressionNode> {
    Box::new(ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ADD_OP_TYPE,
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

/// Execute "+=" compound assignment.
pub fn add_assign_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    compound_assign_function(d, ctx, expression_node, create_add_op)
}

/// Main "+" operator entry point.
pub fn add_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let calc_when_empty = !ctx.matching_nodes.is_empty();
    cross_function_read_only(d, ctx, expression_node, add_calc, calc_when_empty)
}
