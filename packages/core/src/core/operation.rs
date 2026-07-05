use std::cell::OnceCell;

use super::sem_type::SemType;
use super::tree_node::{TreeNode as CoreTreeNode, infer_scalar_tag};
use crate::operators::{
    self, Context, CoreError, ExpressionNode, OperatorRegistry, TreeEngine, init_registry,
};

// Re-export core types for public API compatibility.
pub use super::context::Context as CoreContext;
pub use super::expression::{
    ExpressionNode as CoreExpressionNode, Operation as CoreOperation, OperationId, OperationType,
};
pub use super::operation_defs::*;

pub type OperationPreferences = super::operation_prefs::OperationPreferences;
pub type OperationHandler = operators::OperatorHandler;
pub type OperationNode = CoreOperation;
pub type OperationKind = OperationType;
pub type OperationName = OperationId;

// ── Static operator registry (compat) ────────────────────────────

fn with_operator_registry<R>(f: impl FnOnce(&OperatorRegistry) -> R) -> R {
    thread_local! {
        static REGISTRY: OnceCell<OperatorRegistry> = const { OnceCell::new() };
    }
    REGISTRY.with(|registry| {
        let registry = registry.get_or_init(|| {
            let mut registry = OperatorRegistry::new();
            init_registry(&mut registry).expect("operator registry should initialize");
            registry
        });
        f(registry)
    })
}

// ── get_matching_nodes ───────────────────────────────────────────

/// Execute the operation described by `expression_node` against the current
/// context, dispatching through the operator registry.
///
///   - `expression_node` is null => return ctx unchanged
///   - looks up the handler via `registry.operators.getHandler(operation_type)`
///   - returns `UnknownOperator` if no handler is registered
pub fn get_matching_nodes(
    engine: &mut TreeEngine,
    ctx: &Context,
    expression_node: Option<&mut ExpressionNode>,
) -> Result<Context, CoreError> {
    match expression_node {
        None => Ok(ctx.clone()),
        Some(node) => {
            let handler = if ctx.codec_registry != 0 {
                crate::operators::Registry::with_global(|registry| {
                    registry
                        .operators
                        .get_handler(&node.operation.operation_type)
                        .copied()
                })
                .or_else(|| {
                    with_operator_registry(|registry| {
                        registry
                            .get_handler(&node.operation.operation_type)
                            .copied()
                    })
                })
            } else {
                with_operator_registry(|registry| {
                    registry
                        .get_handler(&node.operation.operation_type)
                        .copied()
                })
            }
            .ok_or(CoreError::Eval(
                crate::operators::EvalError::UnknownOperator {
                    op: node.operation.operation_type.name().to_string(),
                },
            ))?;
            handler(ctx.clone(), engine, node)
        }
    }
}

// ── create_value_operation ───────────────────────────────────────

/// Create an Operation that carries a literal value (the "value" operator).
///
///   - wraps a scalar tree node in a Value-typed Operation
///   - used by the parser/lexer to represent literal values in expression trees
pub fn create_value_operation(string_value: String) -> CoreOperation {
    let trimmed = string_value.trim();
    let sem_type = if matches!(trimmed, "null" | "~") {
        SemType::Nil
    } else {
        let tag = infer_scalar_tag("", trimmed);
        SemType::from_string(tag).unwrap_or(SemType::Str)
    };

    CoreOperation {
        operation_type: VALUE_OP_TYPE,
        value: None,
        string_value: string_value.clone(),
        tree_node: Some(Box::new(CoreTreeNode::scalar(sem_type, string_value))),
        preferences: None,
        update_assign: false,
        token_start: None,
        token_end: None,
    }
}

/// Create a value Operation with an associated tree node.
pub fn create_value_operation_with_node(
    string_value: String,
    tree_node: Box<CoreTreeNode>,
) -> CoreOperation {
    CoreOperation {
        operation_type: VALUE_OP_TYPE,
        value: None,
        string_value,
        tree_node: Some(tree_node),
        preferences: None,
        update_assign: false,
        token_start: None,
        token_end: None,
    }
}
