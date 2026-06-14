use crate::operators::operator_helpers::*;
use crate::operators::*;

struct SortableNode {
    node: TreeNode,
    compare_context: Context,
}

/// Compare two candidate nodes by semantic type and value.
/// Returns std::cmp::Ordering for use with Rust's sort.
fn compare_values(lhs: &TreeNode, rhs: &TreeNode) -> std::cmp::Ordering {
    let mut lhs_tag = lhs.tag.as_str();
    let mut rhs_tag = rhs.tag.as_str();

    let lhs_tag_owned: String;
    let rhs_tag_owned: String;

    if !SemType::has_tag_prefix(lhs_tag) {
        lhs_tag_owned = lhs.guess_tag_from_custom_type();
        lhs_tag = lhs_tag_owned.as_str();
    }
    if !SemType::has_tag_prefix(rhs_tag) {
        rhs_tag_owned = rhs.guess_tag_from_custom_type();
        rhs_tag = rhs_tag_owned.as_str();
    }

    let lhs_sem_type = SemType::from_string(lhs_tag);
    let rhs_sem_type = SemType::from_string(rhs_tag);
    let lhs_null = lhs_sem_type == Some(SemType::Nil);
    let rhs_null = rhs_sem_type == Some(SemType::Nil);
    if lhs_null && !rhs_null {
        return std::cmp::Ordering::Less;
    }
    if !lhs_null && rhs_null {
        return std::cmp::Ordering::Greater;
    }

    let lhs_bool = lhs_sem_type == Some(SemType::Boolean);
    let rhs_bool = rhs_sem_type == Some(SemType::Boolean);
    if lhs_bool && !rhs_bool {
        return std::cmp::Ordering::Less;
    }
    if !lhs_bool && rhs_bool {
        return std::cmp::Ordering::Greater;
    }
    if lhs_bool && rhs_bool {
        let l = is_truthy_node(Some(lhs));
        let r = is_truthy_node(Some(rhs));
        // true > false, so truthy comes after falsy
        if l == r {
            return std::cmp::Ordering::Equal;
        }
        if l {
            return std::cmp::Ordering::Greater;
        }
        return std::cmp::Ordering::Less;
    }

    if lhs_sem_type == Some(SemType::Int) && rhs_sem_type == Some(SemType::Int) {
        let l: i64 = lhs.value.parse().unwrap_or(0);
        let r: i64 = rhs.value.parse().unwrap_or(0);
        return l.cmp(&r);
    }

    let lhs_number = matches!(lhs_sem_type, Some(SemType::Int) | Some(SemType::Float));
    let rhs_number = matches!(rhs_sem_type, Some(SemType::Int) | Some(SemType::Float));
    if lhs_number && rhs_number {
        let l: f64 = lhs.value.parse().unwrap_or(0.0);
        let r: f64 = rhs.value.parse().unwrap_or(0.0);
        if let Some(ord) = l.partial_cmp(&r) {
            return ord;
        }
        return std::cmp::Ordering::Equal;
    }

    lhs.value.as_bytes().cmp(rhs.value.as_bytes())
}

/// Sort matching nodes by their own natural order (sort operator).
pub fn sort_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let prev_rhs = expression_node.rhs.take();

    let self_expr = create_traversal_tree(&[], TraversePreferences::default(), false)?;
    expression_node.rhs = Some(self_expr);
    let result = sort_by_operator(ctx, d, expression_node);
    expression_node.rhs = prev_rhs;
    result
}

/// Sort matching nodes by evaluating RHS expression per element (sort_by operator).
pub fn sort_by_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();

    for candidate in &ctx.matching_nodes {
        if !candidate.can_visit_values() {
            let nice = candidate.get_nice_path()?;
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!(
                        "node at path [{nice}] is not an array or map (it's a {})",
                        candidate.tag
                    ),
                )?;
            }
            return Err(CoreError::Eval(EvalError::UnsupportedFlat));
        }

        let mut sortable: Vec<SortableNode> = Vec::new();

        match candidate.kind {
            NodeKind::Mapping => {
                let mut i = 1;
                while i < candidate.content.len() {
                    let value_node = &candidate.content[i];
                    let compare_ctx = get_matching_nodes(
                        d,
                        &ctx.single_readonly_child_context(value_node)?,
                        expression_node.rhs.as_deref_mut(),
                    )?;
                    sortable.push(SortableNode {
                        node: value_node.clone(),
                        compare_context: compare_ctx,
                    });
                    i += 2;
                }
            }
            NodeKind::Sequence => {
                for value_node in &candidate.content {
                    let compare_ctx = get_matching_nodes(
                        d,
                        &ctx.single_readonly_child_context(value_node)?,
                        expression_node.rhs.as_deref_mut(),
                    )?;
                    sortable.push(SortableNode {
                        node: value_node.clone(),
                        compare_context: compare_ctx,
                    });
                }
            }
            _ => {}
        }

        // Stable sort using compare_context multi-key comparison
        sortable.sort_by(|a, b| {
            let lhs_ctx = &a.compare_context;
            let rhs_ctx = &b.compare_context;
            let min_len = lhs_ctx
                .matching_nodes
                .len()
                .min(rhs_ctx.matching_nodes.len());
            for k in 0..min_len {
                let l = &lhs_ctx.matching_nodes[k];
                let r = &rhs_ctx.matching_nodes[k];
                match compare_values(l, r) {
                    std::cmp::Ordering::Less => return std::cmp::Ordering::Less,
                    std::cmp::Ordering::Greater => return std::cmp::Ordering::Greater,
                    std::cmp::Ordering::Equal => {}
                }
            }
            lhs_ctx
                .matching_nodes
                .len()
                .cmp(&rhs_ctx.matching_nodes.len())
        });

        let mut sorted_list = *candidate.copy_without_content()?;
        match candidate.kind {
            NodeKind::Mapping => {
                for sn in &sortable {
                    if let Some(key_id) = sn.node.key {
                        let key_node = d.store.get(key_id).clone();
                        let _ = sorted_list.add_key_value_child(&key_node, &sn.node);
                    }
                }
            }
            NodeKind::Sequence => {
                for sn in &sortable {
                    let _ = sorted_list.add_child(&sn.node);
                }
            }
            _ => {}
        }
        results.push(sorted_list);
    }
    ctx.child_context(results)
}
