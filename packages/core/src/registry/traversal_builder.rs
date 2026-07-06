use super::{ExpressionNode, Operation, OperationId, OperationPreferences};
use crate::registry::operation_prefs::{RecursiveDescentPreferences, TraversePreferences};
use crate::tree::tree_node::ParsedKey;

pub fn build_traversal_expression(path: &[ParsedKey]) -> ExpressionNode {
    if path.is_empty() {
        return ExpressionNode::leaf(Operation::new(OperationId::SelfRef, "self", 0, u32::MAX));
    }

    let mut segments = path.iter();
    let first = build_traversal_segment(segments.next().expect("path is not empty"));
    segments.fold(first, |lhs, segment| {
        ExpressionNode::binary(
            Operation::binary(OperationId::ShortPipe, "", 45),
            lhs,
            build_traversal_segment(segment),
        )
    })
}

pub fn build_recursive_descent_expression(path: &[ParsedKey]) -> ExpressionNode {
    let mut recursive_op = Operation::new(OperationId::RecursiveDescent, "..", 0, 50);
    recursive_op.preferences = Some(Box::new(OperationPreferences::RecursiveDescent(
        RecursiveDescentPreferences {
            traverse_preferences: TraversePreferences {
                dont_follow_alias: true,
                ..TraversePreferences::default()
            },
            ..RecursiveDescentPreferences::default()
        },
    )));
    let recursive = ExpressionNode::leaf(recursive_op);
    if path.is_empty() {
        return recursive;
    }

    ExpressionNode::binary(
        Operation::binary(OperationId::ShortPipe, "", 45),
        recursive,
        build_traversal_expression(path),
    )
}

fn build_traversal_segment(segment: &ParsedKey) -> ExpressionNode {
    let string_value = match segment {
        ParsedKey::Str(key) => key.clone(),
        ParsedKey::Int(index) => index.to_string(),
    };
    let mut op = Operation::new(OperationId::TraversePath, string_value, 0, 55);
    op.preferences = Some(Box::new(OperationPreferences::Traverse(
        TraversePreferences::default(),
    )));
    ExpressionNode::leaf(op)
}
