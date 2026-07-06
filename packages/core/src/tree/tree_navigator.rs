use std::cell::OnceCell;

use crate::operators::{
    ASSIGN_OP_TYPE, AssignPreferences, Context, CoreError, EvalError, ExpressionNode,
    MULTIPLY_ASSIGN_OP_TYPE, Operation, OperationPreference, OperatorRegistry,
    SELF_REFERENCE_OP_TYPE, SHORT_PIPE_OP_TYPE, TRAVERSE_PATH_OP_TYPE, TraversePreferences,
    TreeNode, TreeStore, VALUE_OP_TYPE, init_registry,
};

pub struct TreeEngine {
    pub store: TreeStore,
}

impl std::fmt::Debug for TreeEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeEngine").finish()
    }
}

impl TreeEngine {
    pub fn new() -> Self {
        Self {
            store: TreeStore::new(),
        }
    }

    pub fn with_store(store: TreeStore) -> Self {
        Self { store }
    }

    pub fn deeply_assign(
        &mut self,
        ctx: Context,
        path: &[TreeNode],
        rhs: TreeNode,
    ) -> Result<(), CoreError> {
        let rhs_is_mapping = rhs.kind == crate::operators::NodeKind::Mapping;
        let operation_type = if rhs_is_mapping {
            &MULTIPLY_ASSIGN_OP_TYPE
        } else {
            &ASSIGN_OP_TYPE
        };

        let lhs = create_traversal_tree(path, TraversePreferences::default(), false)?;
        let rhs_expr = Box::new(ExpressionNode {
            operation: Box::new(Operation {
                operation_type: &VALUE_OP_TYPE,
                value: None,
                string_value: rhs.value.clone(),
                tree_node: Some(Box::new(rhs)),
                preferences: None,
                update_assign: false,
            }),
            lhs: None,
            rhs: None,
        });

        let mut root = ExpressionNode {
            operation: Box::new(Operation {
                operation_type,
                value: None,
                string_value: String::new(),
                tree_node: None,
                preferences: Some(Box::new(OperationPreference::Assign(
                    AssignPreferences::default(),
                ))),
                update_assign: !rhs_is_mapping,
            }),
            lhs: Some(lhs),
            rhs: Some(rhs_expr),
        };

        let _ = get_matching_nodes(self, &ctx, Some(&mut root))?;
        Ok(())
    }
}

impl Default for TreeEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn with_operator_registry<R>(f: impl FnOnce(&OperatorRegistry) -> R) -> R {
    thread_local! {
        static REGISTRY: OnceCell<OperatorRegistry> = const { OnceCell::new() };
    }
    REGISTRY.with(|registry| {
        let registry = registry.get_or_init(|| {
            let mut registry = OperatorRegistry::new();
            init_registry(&mut registry).expect("compat operator registry should initialize");
            registry
        });
        f(registry)
    })
}

// Centralized here until the runtime dispatcher is ported out of operators.
pub fn get_matching_nodes(
    d: &mut TreeEngine,
    ctx: &Context,
    expression_node: Option<&mut ExpressionNode>,
) -> Result<Context, CoreError> {
    match expression_node {
        None => Ok(ctx.clone()),
        Some(node) => {
            let handler = with_operator_registry(|registry| {
                registry.get_handler(node.operation.operation_type).copied()
            })
            .ok_or_else(|| {
                CoreError::Eval(EvalError::UnknownOperator {
                    op: format!("{:?}", node.operation.operation_type),
                })
            })?;
            handler(ctx.clone(), d, node)
        }
    }
}

pub fn create_traversal_tree(
    indices: &[TreeNode],
    prefs: TraversePreferences,
    target_key: bool,
) -> Result<Box<ExpressionNode>, CoreError> {
    if indices.is_empty() {
        return Ok(Box::new(ExpressionNode {
            operation: Box::new(Operation {
                operation_type: &SELF_REFERENCE_OP_TYPE,
                value: None,
                string_value: String::new(),
                tree_node: None,
                preferences: None,
                update_assign: false,
            }),
            lhs: None,
            rhs: None,
        }));
    }

    if indices.len() == 1 {
        let mut last_prefs = prefs;
        if target_key {
            last_prefs.include_map_keys = true;
            last_prefs.dont_include_map_values = true;
        }
        return Ok(Box::new(ExpressionNode {
            operation: Box::new(Operation {
                operation_type: &TRAVERSE_PATH_OP_TYPE,
                value: None,
                string_value: indices[0].value.clone(),
                tree_node: None,
                preferences: Some(Box::new(OperationPreference::Traverse(last_prefs))),
                update_assign: false,
            }),
            lhs: None,
            rhs: None,
        }));
    }

    Ok(Box::new(ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &SHORT_PIPE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(create_traversal_tree(&indices[..1], prefs.clone(), false)?),
        rhs: Some(create_traversal_tree(&indices[1..], prefs, target_key)?),
    }))
}
