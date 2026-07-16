use std::{
    cell::{OnceCell, RefCell},
    collections::HashMap,
};

use crate::{
    core::canonical_format_name,
    operators::registry_tables_formats::{FormatEntry, FormatFlags, append_formats},
    operators::*,
};

/// Unified registry that holds both operator and format registries,
pub struct Registry {
    pub operators: OperatorRegistry,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            operators: OperatorRegistry::new(),
        }
    }

    /// Returns the global singleton Registry, initializing it on first access.
    /// rather than separate global tables for operators and formats.
    pub fn with_global<R>(f: impl FnOnce(&Registry) -> R) -> R {
        thread_local! {
            static REGISTRY: OnceCell<RefCell<Registry>> = const { OnceCell::new() };
        }
        REGISTRY.with(|registry| {
            let registry = registry.get_or_init(|| RefCell::new(Registry::new()));
            f(&registry.borrow())
        })
    }

    /// Initialize both operator and format registries from the built-in tables.
    /// This is the unified entry point that replaces separate init_registry +
    /// ensure_default_formats_registered calls.
    pub fn init(&mut self) -> Result<(), CoreError> {
        init_registry(&mut self.operators)
    }

    /// Look up a registered format by name (canonical or alias).
    pub fn get_format(&self, name: &str) -> Option<FormatEntry> {
        get_registered_format(name)
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Internal format registry (global singleton) ──────────────────

fn with_formats_registry<R>(f: impl FnOnce(&RefCell<HashMap<String, FormatEntry>>) -> R) -> R {
    thread_local! {
        static FORMATS: OnceCell<RefCell<HashMap<String, FormatEntry>>> = const { OnceCell::new() };
    }
    FORMATS.with(|formats| f(formats.get_or_init(|| RefCell::new(HashMap::new()))))
}

fn ensure_default_formats_registered() {
    let is_empty = with_formats_registry(|formats| formats.borrow().is_empty());
    if !is_empty {
        return;
    }

    let mut defaults = Vec::new();
    append_formats(&mut defaults, &FormatFlags::default());

    with_formats_registry(|formats| {
        let mut formats = formats.borrow_mut();
        if formats.is_empty() {
            for entry in defaults {
                formats.insert(entry.name.to_string(), entry);
            }
        }
    });
}

/// Initialize the operator and format registries.
/// In the full implementation, the registry handle lives in Context.codec_registry.
pub fn init_registry(reg: &mut OperatorRegistry) -> Result<(), CoreError> {
    let flags = OpFlags::default();
    let mut op_list = Vec::new();
    append_ops(&mut op_list, &flags);

    for entry in &op_list {
        reg.register_operator(entry.id, entry.handler)?;
    }

    let mut format_list = Vec::new();
    append_formats(&mut format_list, &FormatFlags::default());
    register_format_list(reg, &format_list);
    Ok(())
}
pub fn register_op(
    reg: &mut OperatorRegistry,
    id: OperationId,
    handler: OperatorHandler,
) -> Result<(), CoreError> {
    reg.register_operator(id, handler)
}

pub fn register_op_list(reg: &mut OperatorRegistry, ops: &[OpEntry]) -> Result<(), CoreError> {
    for entry in ops {
        register_op(reg, entry.id, entry.handler)?;
    }
    Ok(())
}

/// Register a format into the registry.
pub fn register_format(_reg: &mut OperatorRegistry, entry: &FormatEntry) {
    with_formats_registry(|formats| {
        formats.borrow_mut().insert(entry.name.to_string(), *entry);
    });
}

/// Register a list of formats into the registry.
pub fn register_format_list(reg: &mut OperatorRegistry, formats: &[FormatEntry]) {
    for entry in formats {
        register_format(reg, entry);
    }
}

pub fn get_registered_format(name: &str) -> Option<FormatEntry> {
    ensure_default_formats_registered();

    let canonical = canonical_format_name(name).ok().unwrap_or(name);
    with_formats_registry(|formats| {
        let formats = formats.borrow();
        formats
            .get(canonical)
            .copied()
            .or_else(|| formats.get(name).copied())
    })
}
