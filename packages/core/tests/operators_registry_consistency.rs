use treease_core::operators::{
    OpFlags, OperatorRegistry, PIPE_OP_TYPE, SELF_REFERENCE_OP_TYPE, VALUE_OP_TYPE, append_ops,
    get_registered_format, init_registry,
};

#[test]
fn registry_consistency_append_ops_exposes_core_symbols() {
    let mut entries = Vec::new();

    append_ops(&mut entries, &OpFlags::default());

    assert!(entries.iter().any(|entry| entry.id == PIPE_OP_TYPE.id));
    assert!(
        entries
            .iter()
            .any(|entry| entry.id == SELF_REFERENCE_OP_TYPE.id)
    );
    assert!(entries.iter().any(|entry| entry.id == VALUE_OP_TYPE.id));
}

#[test]
fn registry_consistency_init_registry_registers_handlers_and_formats() {
    let mut registry = OperatorRegistry::new();

    init_registry(&mut registry).unwrap();

    assert!(registry.get_handler(&PIPE_OP_TYPE).is_some());
    assert!(registry.get_handler(&SELF_REFERENCE_OP_TYPE).is_some());
    assert!(registry.get_handler(&VALUE_OP_TYPE).is_some());
    assert!(get_registered_format("json").is_some());
    assert!(get_registered_format("yaml").is_some());
}
