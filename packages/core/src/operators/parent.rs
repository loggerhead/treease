use crate::operators::*;

/// Count the total number of parent levels for a node.
fn count_parents(node: &TreeNode, store: &TreeStore) -> i32 {
    let mut count = 0;
    let mut current_id = node.parent;
    while let Some(id) = current_id {
        count += 1;
        current_id = store.get(id).parent;
    }
    count
}

fn levels_to_go_up(candidate: &TreeNode, level: i32, store: &TreeStore) -> i32 {
    if level >= 0 {
        return level;
    }
    let total = count_parents(candidate, store);
    let computed = total + level + 1;
    if computed < 0 { 0 } else { computed }
}

fn get_parent_at_level(
    candidate: &TreeNode,
    levels_to_go_up: i32,
    store: &TreeStore,
) -> Option<TreeNode> {
    if levels_to_go_up <= 0 {
        return candidate.parent.map(|id| store.get(id).clone());
    }

    let mut current_id = candidate.parent;
    let mut current_level = 1;
    while current_level < levels_to_go_up {
        let id = current_id?;
        current_id = store.get(id).parent;
        current_level += 1;
    }
    current_id.map(|id| store.get(id).clone())
}

/// Get the parent node at a specific level (default level 1 = immediate parent).
pub fn get_parent_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let level = match expression_node.operation.preferences.as_deref() {
        Some(OperationPreference::Parent(p)) => p.level,
        _ => ParentOpPreferences::default().level,
    };

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let levels = levels_to_go_up(candidate, level, &d.store);
        if let Some(p) = get_parent_at_level(candidate, levels, &d.store) {
            results.push(p);
        }
    }
    ctx.child_context(results)
}

/// Get all ancestor nodes as a sequence.
pub fn get_parents_operator(
    ctx: Context,
    d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let mut parents_list = TreeNode::default();
        parents_list.kind = NodeKind::Sequence;
        parents_list.sem_type = Some(SemType::Seq);
        parents_list.tag = SemType::Seq.to_string().into();

        let mut current_id = candidate.parent;
        while let Some(id) = current_id {
            let parent_node = d.store.get(id).clone();
            parents_list.add_child(&parent_node)?;
            current_id = d.store.get(id).parent;
        }
        results.push(parents_list);
    }
    ctx.child_context(results)
}
