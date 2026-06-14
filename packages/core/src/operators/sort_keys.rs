use crate::operators::*;

struct Entry {
    key: TreeNode,
    value: TreeNode,
}

fn nodes_structurally_eq(a: &TreeNode, b: &TreeNode) -> bool {
    if a.kind != b.kind
        || a.tag != b.tag
        || a.value != b.value
        || a.is_map_key != b.is_map_key
        || a.sequence_index != b.sequence_index
        || a.document != b.document
        || a.file_index != b.file_index
        || a.filename != b.filename
        || a.content.len() != b.content.len()
    {
        return false;
    }

    a.content
        .iter()
        .zip(&b.content)
        .all(|(lhs, rhs)| nodes_structurally_eq(lhs, rhs))
}

fn write_back_updated_node(
    haystack: &mut [TreeNode],
    needle: &TreeNode,
    replacement: &TreeNode,
) -> bool {
    for node in haystack.iter_mut() {
        if nodes_structurally_eq(node, needle) {
            *node = replacement.clone();
            return true;
        }
        if write_back_updated_node(&mut node.content, needle, replacement) {
            return true;
        }
    }
    false
}

/// Sort the keys of a mapping node in-place by lexicographic order.
fn sort_keys(node: &mut TreeNode) {
    let pair_count = node.content.len() / 2;
    if pair_count < 2 {
        return;
    }

    // Collect key-value pairs
    let mut entries: Vec<Entry> = Vec::with_capacity(pair_count);
    let mut i: usize = 0;
    while i + 1 < node.content.len() {
        entries.push(Entry {
            key: node.content[i].clone(),
            value: node.content[i + 1].clone(),
        });
        i += 2;
    }

    // Sort by key value (lexicographic)
    entries.sort_by(|a, b| a.key.value.as_bytes().cmp(b.key.value.as_bytes()));

    // Rebuild content from sorted entries
    let mut sorted_items = Vec::with_capacity(node.content.len());
    for entry in &entries {
        sorted_items.push(entry.key.clone());
        sorted_items.push(entry.value.clone());
    }
    node.content = sorted_items;
}

/// Sort the keys of mapping nodes in-place (sort_keys operator).
pub fn sort_keys_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut updated_ctx = ctx.clone();

    for candidate in &ctx.matching_nodes {
        let rhs = get_matching_nodes(
            d,
            &ctx.single_readonly_child_context(candidate)?,
            expression_node.rhs.as_deref_mut(),
        )?;
        for n in rhs.matching_nodes {
            if n.kind == NodeKind::Mapping {
                let mut sorted = n.clone();
                sort_keys(&mut sorted);
                let _ = write_back_updated_node(&mut updated_ctx.matching_nodes, &n, &sorted);
            }
        }
    }

    Ok(updated_ctx)
}
