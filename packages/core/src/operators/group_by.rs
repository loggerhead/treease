use crate::operators::*;

use std::collections::HashMap;

/// Group buckets: maps keys to ordered lists of children.
struct GroupBuckets {
    keys: Vec<String>,
    values: HashMap<String, Vec<TreeNode>>,
}

impl GroupBuckets {
    fn new() -> Self {
        Self {
            keys: Vec::new(),
            values: HashMap::new(),
        }
    }

    /// Append a child to the bucket for a given key.
    fn append(&mut self, key: &str, child: &TreeNode) {
        let entry = self.values.entry(key.to_string());
        let is_new = matches!(&entry, std::collections::hash_map::Entry::Vacant(_));
        if is_new {
            self.keys.push(key.to_string());
        }
        entry.or_default().push(child.clone());
    }
}

/// Process candidate children into grouping buckets based on RHS expression.
fn process_into_groups(
    d: &mut TreeEngine,
    ctx: &Context,
    rhs_exp: &mut ExpressionNode,
    node: &TreeNode,
) -> Result<GroupBuckets, CoreError> {
    let mut buckets = GroupBuckets::new();

    for child in &node.content {
        let child_ctx = ctx.single_readonly_child_context(child)?;
        let rhs = get_matching_nodes(d, &child_ctx, Some(rhs_exp))?;
        let key_value = if rhs.matching_nodes.is_empty() {
            "null".to_string()
        } else {
            rhs.matching_nodes[0].value.clone()
        };
        buckets.append(&key_value, child);
    }
    Ok(buckets)
}

/// group_by operator: group array elements by expression result.
pub fn group_by(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results: Vec<TreeNode> = Vec::new();

    let rhs_expr = expression_node
        .rhs
        .as_mut()
        .ok_or(CoreError::Eval(EvalError::MissingRhs))?;

    for candidate in &ctx.matching_nodes {
        if candidate.kind != NodeKind::Sequence {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_message("eval", "only arrays are supported for group by")?;
            }
            return Err(CoreError::OperatorMessage {
                op: "group_by".to_string(),
                message: "only arrays are supported for group by".to_string(),
            });
        }

        let buckets = process_into_groups(d, &ctx, rhs_expr, candidate)?;

        // Build result array: each group becomes a sub-array.
        let mut result_node =
            (*candidate.create_replacement(NodeKind::Sequence, SemType::Seq.to_string(), "")?)
                .clone();

        for k in &buckets.keys {
            let mut group_result = TreeNode::default();
            group_result.kind = NodeKind::Sequence;
            group_result.sem_type = Some(SemType::Seq);
            group_result.tag = SemType::Seq.to_string().into();

            if let Some(list) = buckets.values.get(k) {
                for item in list {
                    group_result.add_child(item)?;
                }
            }
            result_node.add_child(&group_result)?;
        }

        results.push(result_node);
    }

    ctx.child_context(results)
}
