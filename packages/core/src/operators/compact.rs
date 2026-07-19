use crate::operators::*;

/// Return whether a JSON value is zero-valued without converting numbers to `f64`.
pub fn is_zero_json_value(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::Bool(value) => !value,
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|number| number == 0.0),
        serde_json::Value::String(value) => value.is_empty(),
        serde_json::Value::Array(value) => value.is_empty(),
        serde_json::Value::Object(value) => value.is_empty(),
    }
}

/// Compact the current JSON container while preserving the original number representation.
pub fn compact_json_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(compact_json_value)
                .filter(|item| !is_zero_json_value(item))
                .collect(),
        ),
        serde_json::Value::Object(entries) => serde_json::Value::Object(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), compact_json_value(value)))
                .filter(|(_, value)| !is_zero_json_value(value))
                .collect(),
        ),
        _ => value.clone(),
    }
}

/// Return whether a node is a zero value in Treease's data model.
pub fn is_zero_node(node: &TreeNode) -> bool {
    match node.kind {
        NodeKind::Sequence | NodeKind::Mapping => node.content.is_empty(),
        NodeKind::Scalar => match node.resolved_sem_type() {
            Some(SemType::Nil) => true,
            Some(SemType::Boolean) => !matches!(
                node.value.to_ascii_lowercase().as_str(),
                "true" | "y" | "yes" | "on" | "1"
            ),
            Some(SemType::Int) | Some(SemType::Float) => {
                node.value.parse::<f64>().is_ok_and(|value| value == 0.0)
            }
            Some(SemType::Str) => node.value.is_empty(),
            _ => false,
        },
        NodeKind::Alias | NodeKind::Unknown => false,
    }
}

fn compact_mapping(original: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut out = (*original.copy_without_content()?).clone();

    for pair in original.content.chunks(2) {
        let [key, value] = pair else {
            break;
        };
        let compacted = compact_tree_node(value)?;
        if !is_zero_node(&compacted) {
            out.add_key_value_child(key, &compacted)?;
        }
    }

    Ok(Box::new(out))
}

fn compact_sequence(original: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut out = (*original.copy_without_content()?).clone();

    for child in &original.content {
        let mut child = compact_tree_node(child)?;
        if is_zero_node(&child) {
            continue;
        }
        child.sequence_index = Some(out.content.len() as i64);
        out.add_child(&child)?;
    }

    Ok(Box::new(out))
}

fn compact_tree_node(original: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
    let mut compacted = match original.kind {
        NodeKind::Mapping => compact_mapping(original)?,
        NodeKind::Sequence => compact_sequence(original)?,
        NodeKind::Scalar | NodeKind::Alias | NodeKind::Unknown => Box::new(original.clone()),
    };
    compacted.leading_content = original.leading_content.clone();
    Ok(compacted)
}

/// Remove zero-valued members from the current mapping or sequence.
pub fn compact_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let results = ctx
        .matching_nodes
        .iter()
        .map(|node| Ok((*compact_tree_node(node)?).clone()))
        .collect::<Result<Vec<_>, CoreError>>()?;

    ctx.child_context(results)
}
