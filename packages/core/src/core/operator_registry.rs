use std::collections::HashMap;

use super::expression::{OperationId, OperationType};
use crate::operators::OperatorHandler;

/// A registered operator entry: stores both the type metadata and the
/// callable handler function pointer.
#[derive(Debug, Clone)]
pub struct RegisteredOperator {
    pub operation_type: OperationType,
    pub handler_symbol: String,
    pub handler: Option<OperatorHandler>,
}

impl PartialEq for RegisteredOperator {
    fn eq(&self, other: &Self) -> bool {
        self.operation_type == other.operation_type && self.handler_symbol == other.handler_symbol
    }
}
impl Eq for RegisteredOperator {}

#[derive(Debug, Clone, Default)]
pub struct OperatorRegistry {
    handlers: HashMap<OperationId, RegisteredOperator>,
    custom_handlers: HashMap<String, RegisteredOperator>,
}

impl OperatorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init() -> Self {
        Self::new()
    }

    pub fn deinit(&mut self) {
        self.handlers.clear();
        self.custom_handlers.clear();
    }

    /// Register an operator with both a symbol name and a callable handler.
    pub fn register_operator(
        &mut self,
        operation_type: OperationType,
        handler_symbol: impl Into<String>,
        handler: OperatorHandler,
    ) {
        let registered = RegisteredOperator {
            operation_type: operation_type.clone(),
            handler_symbol: handler_symbol.into(),
            handler: Some(handler),
        };
        self.handlers.insert(operation_type.id, registered);
    }

    /// Register an operator with only a symbol name (no callable handler yet).
    pub fn register_operator_symbol(
        &mut self,
        operation_type: OperationType,
        handler_symbol: impl Into<String>,
    ) {
        let registered = RegisteredOperator {
            operation_type: operation_type.clone(),
            handler_symbol: handler_symbol.into(),
            handler: None,
        };
        self.handlers.insert(operation_type.id, registered);
    }

    pub fn register_custom(
        &mut self,
        name: impl Into<String>,
        num_args: u32,
        precedence: u32,
        handler_symbol: impl Into<String>,
    ) {
        let name = name.into();
        let operation_type = OperationType::custom(name.clone(), num_args, precedence);
        self.custom_handlers.insert(
            name,
            RegisteredOperator {
                operation_type,
                handler_symbol: handler_symbol.into(),
                handler: None,
            },
        );
    }

    /// Register a custom operator with a callable handler.
    pub fn register_custom_handler(
        &mut self,
        name: impl Into<String>,
        num_args: u32,
        precedence: u32,
        handler_symbol: impl Into<String>,
        handler: OperatorHandler,
    ) {
        let name = name.into();
        let operation_type = OperationType::custom(name.clone(), num_args, precedence);
        self.custom_handlers.insert(
            name,
            RegisteredOperator {
                operation_type,
                handler_symbol: handler_symbol.into(),
                handler: Some(handler),
            },
        );
    }

    /// Look up the callable handler for an operation type.
    /// Returns `None` if no handler is registered or if the handler
    /// was registered as a symbol-only entry.
    pub fn get_handler(&self, operation_type: &OperationType) -> Option<&OperatorHandler> {
        if operation_type.id == OperationId::Custom {
            return operation_type
                .custom_name
                .as_deref()
                .and_then(|name| self.custom_handlers.get(name))
                .and_then(|r| r.handler.as_ref());
        }
        self.handlers
            .get(&operation_type.id)
            .and_then(|r| r.handler.as_ref())
    }

    /// Look up the full registered entry (including symbol-only entries).
    pub fn get_entry(&self, operation_type: &OperationType) -> Option<&RegisteredOperator> {
        if operation_type.id == OperationId::Custom {
            return operation_type
                .custom_name
                .as_deref()
                .and_then(|name| self.custom_handlers.get(name));
        }
        self.handlers.get(&operation_type.id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RegisteredOperator> {
        self.handlers.values().chain(self.custom_handlers.values())
    }
}
