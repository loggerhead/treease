use crate::core::core_helpers;
use crate::operators::*;

// ── updateFrom: copy contents from one node to another ──────────

pub fn update_from(dst: &mut TreeNode, src: &TreeNode) -> Result<(), CoreError> {
    if std::ptr::eq(dst, src) {
        return Ok(());
    }

    let parent = dst.parent;
    let key = dst.key;
    let is_map_key = dst.is_map_key;
    let sequence_index = dst.sequence_index;
    let leading_content = dst.leading_content.clone();
    let document = dst.document;
    let filename = dst.filename.clone();
    let line = dst.line;
    let column = dst.column;
    let file_index = dst.file_index;

    dst.content.clear();
    dst.kind = src.kind;
    dst.sequence_closed = src.sequence_closed;
    dst.add_children(&src.content)?;
    dst.value = src.value.clone();
    dst.tag = src.tag.clone();
    dst.sem_type = src.resolved_sem_type();
    dst.alias = src.alias;
    if !src.anchor.is_empty() {
        dst.anchor = src.anchor.clone();
    }
    dst.encode_separate = src.encode_separate;
    if !src.foot_comment.is_empty() {
        dst.foot_comment = src.foot_comment.clone();
    }
    if !src.head_comment.is_empty() {
        dst.head_comment = src.head_comment.clone();
    }
    if !src.line_comment.is_empty() {
        dst.line_comment = src.line_comment.clone();
    }
    dst.parent = parent;
    dst.key = key;
    dst.is_map_key = is_map_key;
    dst.sequence_index = sequence_index;
    dst.leading_content = leading_content;
    dst.document = document;
    dst.filename = filename;
    dst.line = line;
    dst.column = column;
    dst.file_index = file_index;
    Ok(())
}

/// Check whether two nodes represent the "same" candidate identity.
///
/// matched LHS candidate in place. In Phase A we only have owned clones, so
/// we approximate that identity with the stable metadata that is preserved
/// across operator traversal plus a few structural fast-reject checks.
fn nodes_structurally_eq(a: &TreeNode, b: &TreeNode) -> bool {
    if a.kind != b.kind {
        return false;
    }
    if a.tag != b.tag {
        return false;
    }
    if a.value != b.value {
        return false;
    }
    if a.is_map_key != b.is_map_key {
        return false;
    }
    if a.sequence_index != b.sequence_index {
        return false;
    }
    if a.parent.is_some() && b.parent.is_some() && a.parent != b.parent {
        return false;
    }
    if a.key.is_some() && b.key.is_some() && a.key != b.key {
        return false;
    }
    if a.start_byte != b.start_byte || a.end_byte != b.end_byte {
        return false;
    }
    if a.document != b.document || a.file_index != b.file_index {
        return false;
    }
    if a.line != b.line || a.column != b.column {
        return false;
    }
    if a.filename != b.filename {
        return false;
    }
    // Fast-reject: different child counts mean different nodes.
    if a.content.len() != b.content.len() {
        return false;
    }
    a.content
        .iter()
        .zip(&b.content)
        .all(|(a_child, b_child)| nodes_structurally_eq(a_child, b_child))
}

/// Recursively search `haystack` for a node structurally equal to
/// `needle` and apply `update_from` with `src`. Returns `true` if a
/// match was found and updated in-place.
fn find_and_update_in_place(
    haystack: &mut Vec<TreeNode>,
    needle: &TreeNode,
    src: &TreeNode,
) -> Result<bool, CoreError> {
    for node in haystack.iter_mut() {
        if nodes_structurally_eq(node, needle) {
            update_from(node, src)?;
            return Ok(true);
        }
        if find_and_update_in_place(&mut node.content, needle, src)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// compoundAssignFunction: execute compound assignment (e.g. +=, -=).
///
/// held by `lhs.matching_nodes`, mutating the tree in-place. In Phase A
/// we achieve the same by recursively searching `ctx.matching_nodes` for
/// the structural counterpart of each LHS candidate and updating it
/// in-place — no clone-and-writeback indirection.
pub fn compound_assign_function(
    d: &mut TreeEngine,
    mut ctx: Context,
    expression_node: &mut ExpressionNode,
    calculation: CompoundCalculation,
) -> Result<Context, CoreError> {
    let lhs = get_matching_nodes(d, &ctx, expression_node.lhs.as_deref_mut())?;

    for candidate in &lhs.matching_nodes {
        let clone = candidate.copy()?;
        let value_op = create_value_operation(clone)?;
        let mut value_copy_exp = Box::new(ExpressionNode {
            operation: value_op,
            lhs: None,
            rhs: None,
        });

        let rhs = expression_node
            .rhs
            .as_ref()
            .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
        let mut calc_exp = calculation(&mut value_copy_exp, rhs);
        let calc_ctx = get_matching_nodes(d, &ctx, Some(&mut calc_exp))?;
        if calc_ctx.matching_nodes.is_empty() {
            continue;
        }
        // Direct in-place mutation via recursive structural search —
        let _ = find_and_update_in_place(
            &mut ctx.matching_nodes,
            candidate,
            &calc_ctx.matching_nodes[0],
        )?;
    }
    Ok(ctx)
}

/// emptyOperator: clear matching nodes.
pub fn empty_operator(
    mut ctx: Context,
    _d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    ctx.matching_nodes = Vec::new();
    Ok(ctx)
}

/// identityOperator: return context unchanged.
pub fn identity_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    Ok(ctx)
}

/// isTruthyNode: check if a node is semantically truthy.
pub fn is_truthy_node(node: Option<&TreeNode>) -> bool {
    match node {
        None => false,
        Some(n) => {
            if n.sem_type == Some(SemType::Nil) {
                return false;
            }
            if n.kind == NodeKind::Scalar && n.sem_type == Some(SemType::Boolean) {
                let v = n.value.to_lowercase();
                return v == "y" || v == "yes" || v == "on" || v == "true";
            }
            true
        }
    }
}

/// createBooleanCandidate: create a boolean tree node, preserving
/// key relationships from the owner node.
pub fn create_boolean_candidate(owner: &TreeNode, value: bool) -> Result<Box<TreeNode>, CoreError> {
    let val_string = if value { "true" } else { "false" };
    let mut n =
        (*owner.create_replacement(NodeKind::Scalar, SemType::Boolean.to_string(), val_string)?)
            .clone();
    if owner.is_map_key {
        n.is_map_key = false;
        // Phase B: set to owner's NodeId when TreeStore is available.
        n.key = None;
    }
    Ok(Box::new(n))
}

// ── Cross-function infrastructure ────────────────────────────────

fn results_for_rhs(
    d: &mut TreeEngine,
    ctx: &Context,
    lhs_candidate: Option<&TreeNode>,
    prefs: &CrossFunctionPreferences,
    rhs_exp: &mut ExpressionNode,
    results: &mut Vec<TreeNode>,
) -> Result<(), CoreError> {
    if let Some(ref f) = prefs.lhs_result_value {
        let r = f(ctx.clone(), lhs_candidate)?;
        if let Some(node) = r {
            results.push((*node).clone());
            return Ok(());
        }
    }

    let rhs = get_matching_nodes(d, ctx, Some(rhs_exp))?;

    if prefs.calc_when_empty && rhs.matching_nodes.is_empty() {
        let result_candidate = (prefs.calculation)(d, ctx.clone(), lhs_candidate, None)?;
        if let Some(r) = result_candidate {
            results.push((*r).clone());
        }
        return Ok(());
    }

    for rhs_candidate in &rhs.matching_nodes {
        let result_candidate =
            (prefs.calculation)(d, ctx.clone(), lhs_candidate, Some(rhs_candidate))?;
        if let Some(r) = result_candidate {
            results.push((*r).clone());
        }
    }
    Ok(())
}

fn do_cross_func(
    d: &mut TreeEngine,
    ctx: &Context,
    expression_node: &mut ExpressionNode,
    prefs: &CrossFunctionPreferences,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();

    let lhs = get_matching_nodes(d, ctx, expression_node.lhs.as_deref_mut())?;

    if prefs.calc_when_empty && !ctx.matching_nodes.is_empty() && lhs.matching_nodes.is_empty() {
        let rhs = expression_node
            .rhs
            .as_deref_mut()
            .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
        results_for_rhs(d, ctx, None, prefs, rhs, &mut results)?;
    }

    for lhs_candidate in &lhs.matching_nodes {
        let rhs = expression_node
            .rhs
            .as_deref_mut()
            .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
        results_for_rhs(d, ctx, Some(lhs_candidate), prefs, rhs, &mut results)?;
    }
    ctx.child_context(results)
}

/// crossFunction: evaluate LHS × RHS combinations with a calculation.
pub fn cross_function(
    d: &mut TreeEngine,
    ctx: Context,
    expression_node: &mut ExpressionNode,
    calculation: CrossFunctionCalculation,
    calc_when_empty: bool,
) -> Result<Context, CoreError> {
    let prefs = CrossFunctionPreferences {
        calc_when_empty,
        lhs_result_value: None,
        calculation,
    };
    cross_function_with_prefs(d, ctx, expression_node, prefs)
}

/// crossFunctionReadOnly: read-only variant of cross_function.
pub fn cross_function_read_only(
    d: &mut TreeEngine,
    ctx: Context,
    expression_node: &mut ExpressionNode,
    calculation: CrossFunctionCalculation,
    calc_when_empty: bool,
) -> Result<Context, CoreError> {
    let ro = ctx.read_only_clone()?;
    cross_function(d, ro, expression_node, calculation, calc_when_empty)
}

/// crossFunctionWithPrefs: cross-function with full preferences.
pub fn cross_function_with_prefs(
    d: &mut TreeEngine,
    ctx: Context,
    expression_node: &mut ExpressionNode,
    prefs: CrossFunctionPreferences,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();

    let mut evaluate_all_together = true;
    for n in &ctx.matching_nodes {
        evaluate_all_together = evaluate_all_together && n.evaluate_together;
        if !evaluate_all_together {
            break;
        }
    }

    if evaluate_all_together {
        return do_cross_func(d, &ctx, expression_node, &prefs);
    }

    for n in &ctx.matching_nodes {
        let inner = do_cross_func(d, &ctx.single_child_context(n)?, expression_node, &prefs)?;
        for m in &inner.matching_nodes {
            results.push(m.clone());
        }
    }

    ctx.child_context(results)
}

// ── Parsing helpers for string→number ────────────────────────────

/// Parse an integer string, preserving the format (hex/octal/binary/decimal).
/// `core_helpers.parseInt64` which returns `{ fmt, value }`.
pub fn parse_int64_with_fmt(s: &str) -> Result<core_helpers::ParseInt64Result, CoreError> {
    core_helpers::parse_int64(s).map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))
}

/// Parse an integer string, discarding the format (always decimal output).
/// Kept for backward compatibility with callers that don't need format preservation.
pub fn parse_int64(s: &str) -> Result<i64, CoreError> {
    core_helpers::parse_int64(s)
        .map(|parsed| parsed.value)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))
}

pub fn parse_float64(s: &str) -> Result<f64, CoreError> {
    s.parse::<f64>()
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))
}

/// Format an i64 using the given format string (e.g. "%v", "0x%X", "0o%o", "0b%b").
pub fn format_int64_with_fmt(fmt_str: &str, value: i64) -> Result<String, CoreError> {
    core_helpers::format_int64(fmt_str, value)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))
}

pub fn float_to_string(value: f64) -> String {
    core_helpers::float_to_string(value)
}

// ── Key matching ─────────────────────────────────────────────────

pub fn match_key(key_value: &str, wanted_key: &str) -> bool {
    core_helpers::match_key(key_value, wanted_key)
}

// ── splat: expand sequence/mapping nodes into individual elements ──

/// Expand matching nodes: if a node is a Sequence or Mapping, replace it
/// with its content children; otherwise keep the node as-is.
///
/// Takes `Context` by value so that non-collection leaf nodes can be
/// pointer sharing semantics as closely as Phase A allows.
pub fn splat(mut ctx: Context, prefs: TraversePreferences) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for node in ctx.matching_nodes.drain(..) {
        match node.kind {
            NodeKind::Sequence => {
                for child in &node.content {
                    results.push(child.clone());
                }
            }
            NodeKind::Mapping => {
                let mut i: usize = 0;
                while i + 1 < node.content.len() {
                    if prefs.include_map_keys {
                        results.push(node.content[i].clone());
                    }
                    if !prefs.dont_include_map_values {
                        results.push(node.content[i + 1].clone());
                    }
                    i += 2;
                }
            }
            _ => {
                // Move the node directly — no clone needed for scalars/aliases.
                results.push(node);
            }
        }
    }
    ctx.matching_nodes = results;
    Ok(ctx)
}

pub fn find_key_in_map(map_node: &TreeNode, key_node: &TreeNode) -> i32 {
    core_helpers::find_key_in_map(map_node, key_node)
}
