use treease_core::core::{Context, NodeId, RegistryHandle};

#[test]
fn child_context_clones_variables_without_mutating_parent() {
    let mut original = Context::empty(RegistryHandle::default());
    original.dont_auto_create = true;
    original.set_variable("dog", vec![NodeId(1)]);

    let mut clone = original.child_context(vec![NodeId(2)]).unwrap();
    assert_eq!(clone.get_variable("dog"), Some(&vec![NodeId(1)]));

    clone.variables.get_mut("dog").unwrap().push(NodeId(3));

    assert_eq!(original.get_variable("dog"), Some(&vec![NodeId(1)]));
    assert_eq!(clone.get_variable("dog"), Some(&vec![NodeId(1), NodeId(3)]));
    assert_eq!(clone.matching_nodes, vec![NodeId(2)]);
}

#[test]
fn child_context_keeps_empty_variables_when_parent_has_none() {
    let original = Context::empty(RegistryHandle::default());
    let clone = original.child_context(vec![NodeId(4)]).unwrap();

    assert!(clone.variables.is_empty());
    assert_eq!(clone.matching_nodes, vec![NodeId(4)]);
}

#[test]
fn single_child_variants_keep_one_matching_node() {
    let readonly = Context::empty(RegistryHandle::default())
        .single_readonly_child_context(NodeId(5))
        .unwrap();
    assert!(readonly.dont_auto_create);
    assert_eq!(readonly.matching_nodes, vec![NodeId(5)]);

    let mut writable_parent = Context::empty(RegistryHandle::default());
    writable_parent.dont_auto_create = true;
    let child = writable_parent.single_child_context(NodeId(6)).unwrap();
    assert!(child.dont_auto_create);
    assert_eq!(child.matching_nodes, vec![NodeId(6)]);
}

#[test]
fn get_variable_and_set_variable_round_trip() {
    let mut ctx = Context::empty(RegistryHandle::default());
    assert_eq!(ctx.get_variable("missing"), None);

    ctx.set_variable("test", vec![NodeId(7)]);
    ctx.set_variable("other", vec![NodeId(8)]);

    assert_eq!(ctx.get_variable("test"), Some(&vec![NodeId(7)]));
    assert_eq!(ctx.get_variable("other"), Some(&vec![NodeId(8)]));
    assert_eq!(ctx.get_variable("missing"), None);
}

#[test]
fn to_string_value_returns_empty_string() {
    let mut ctx = Context::empty(RegistryHandle::default());
    ctx.append_matching_node(NodeId(10));

    assert_eq!(ctx.to_string_value().unwrap(), "");
}

#[test]
fn clone_and_deep_clone_preserve_values_independently() {
    let mut original = Context::empty(RegistryHandle::default());
    original.dont_auto_create = true;
    original.append_matching_node(NodeId(12));
    original.set_variable("test", vec![NodeId(13)]);

    let shallow = original.clone();
    assert_eq!(shallow.dont_auto_create, original.dont_auto_create);
    assert_eq!(shallow.matching_nodes, vec![NodeId(12)]);
    assert_eq!(shallow.get_variable("test"), Some(&vec![NodeId(13)]));

    let mut deep = original.deep_clone().unwrap();
    assert_eq!(deep.dont_auto_create, original.dont_auto_create);
    assert_eq!(deep.matching_nodes, vec![NodeId(12)]);
    assert_eq!(deep.get_variable("test"), Some(&vec![NodeId(13)]));

    original.append_matching_node(NodeId(14));
    deep.set_variable("test", vec![NodeId(15)]);

    assert_eq!(original.matching_nodes, vec![NodeId(12), NodeId(14)]);
    assert_eq!(shallow.matching_nodes, vec![NodeId(12)]);
    assert_eq!(deep.matching_nodes, vec![NodeId(12)]);
    assert_eq!(original.get_variable("test"), Some(&vec![NodeId(13)]));
    assert_eq!(deep.get_variable("test"), Some(&vec![NodeId(15)]));
}

#[test]
fn readonly_and_writable_clone_override_auto_create_flag() {
    let mut original = Context::empty(RegistryHandle::default());
    original.dont_auto_create = false;
    original.append_matching_node(NodeId(16));

    let readonly = original.read_only_clone().unwrap();
    assert!(readonly.dont_auto_create);
    assert_eq!(readonly.matching_nodes, vec![NodeId(16)]);

    original.dont_auto_create = true;
    let writable = original.writable_clone().unwrap();
    assert!(!writable.dont_auto_create);
    assert_eq!(writable.matching_nodes, vec![NodeId(16)]);
}
