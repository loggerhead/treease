use treease_core::core::{
    FormatDefinition, OperationId, OperationType, Registry, RegistryFormatPreferences,
    RegistryOwner, SemType, create_value_operation, to_handle,
};

#[test]
fn registry_handle_roundtrip_and_format_lookup_work() {
    let mut registry = Registry::init();
    registry.formats.register_format(FormatDefinition {
        name: "yaml".to_owned(),
        encoder_symbol: Some("encode_yaml".to_owned()),
        decoder_symbol: Some("decode_yaml".to_owned()),
        encoder_prefs_symbol: None,
        decoder_prefs_symbol: None,
    });

    let handle = to_handle(registry);
    let shared = treease_core::core::registry::from_handle(&handle);
    let guard = shared.borrow();
    let prefs = RegistryFormatPreferences::default();

    assert_eq!(guard.get_encoder("yaml", &prefs), Some("encode_yaml"));
    assert_eq!(guard.get_decoder("yaml", &prefs), Some("decode_yaml"));
    assert_eq!(guard.get_decoder("unknown", &prefs), None);
}

#[test]
fn registry_initializes_codec_service() {
    let registry = Registry::init();

    assert!(registry.codecs.get_decoder("json").is_ok());
    assert!(registry.codecs.get_encoder("json", 2).is_ok());
}

#[test]
fn registry_owner_supports_owned_and_borrowed_handles() {
    let owned = RegistryOwner::init_owned();
    assert!(owned.owns);

    let borrowed = RegistryOwner::init_borrowed(owned.handle());
    assert!(!borrowed.owns);

    let shared = treease_core::core::registry::from_handle(&borrowed.ptr());
    assert!(shared.try_borrow().is_ok());
}

#[test]
fn operator_registry_registers_builtin_and_custom_handlers() {
    let owner = RegistryOwner::init_owned();
    let shared = treease_core::core::registry::from_handle(&owner.ptr());
    let mut guard = shared.borrow_mut();

    let length = OperationType::new(OperationId::Length, 0, 0);
    guard
        .operators
        .register_operator_symbol(length.clone(), "length_handler");
    guard
        .operators
        .register_custom("custom_test", 1, 10, "custom_handler");

    let custom = OperationType::custom("custom_test", 1, 10);
    assert_eq!(
        guard.operators.get_entry(&length).unwrap().handler_symbol,
        "length_handler"
    );
    assert_eq!(
        guard.operators.get_entry(&custom).unwrap().handler_symbol,
        "custom_handler"
    );
}

#[test]
fn operator_registry_returns_null_for_empty_custom_name() {
    let owner = RegistryOwner::init_owned();
    let shared = treease_core::core::registry::from_handle(&owner.ptr());
    let guard = shared.borrow();

    let custom = OperationType::custom("", 0, 0);
    assert!(guard.operators.get_handler(&custom).is_none());
}

#[test]
fn operator_registry_allows_handler_override() {
    let owner = RegistryOwner::init_owned();
    let shared = treease_core::core::registry::from_handle(&owner.ptr());
    let mut guard = shared.borrow_mut();

    let length = OperationType::new(OperationId::Length, 0, 0);
    guard
        .operators
        .register_operator_symbol(length.clone(), "first_handler");
    guard
        .operators
        .register_operator_symbol(length.clone(), "second_handler");

    let entry = guard.operators.get_entry(&length).unwrap();
    assert_eq!(entry.handler_symbol, "second_handler");
}

#[test]
fn operation_type_name_matches_zig_special_cases() {
    assert_eq!(OperationType::new(OperationId::Or, 2, 20).name(), "or");
    assert_eq!(OperationType::new(OperationId::And, 2, 20).name(), "and");
    assert_eq!(
        OperationType::new(OperationId::Union, 2, 10).name(),
        "union"
    );
    assert_eq!(
        OperationType::custom("custom_test", 0, 0).name(),
        "custom_test"
    );
}

#[test]
fn create_value_operation_builds_inferred_scalar_node() {
    let op = create_value_operation("42".to_string());

    assert_eq!(op.operation_type.id, OperationId::Value);
    let node = op
        .tree_node
        .expect("value operation should carry a tree node");
    assert_eq!(node.sem_type, Some(SemType::Int));
    assert_eq!(
        node.get_value_rep_with("42").unwrap(),
        treease_core::core::ValueRep::Int(42)
    );
}

#[test]
fn create_value_operation_infers_null_literal_like_zig_typed_values() {
    let op = create_value_operation("null".to_string());

    assert_eq!(op.operation_type.id, OperationId::Value);
    let node = op
        .tree_node
        .expect("value operation should carry a tree node");
    assert_eq!(node.sem_type, Some(SemType::Nil));
    assert_eq!(
        node.get_value_rep_with("null").unwrap(),
        treease_core::core::ValueRep::Nil
    );
}
