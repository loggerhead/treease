use treease_core::core::core_helpers::parse_snippet;
use treease_core::core::{
    CoreError, ParseError, SemType as CoreSemType, TreeNode as CoreTreeNode, TreeStore,
    ensure_seq_index, get_map_entry, recursive_node_compare,
};
use treease_core::operators::{NodeKind, SemType, TreeNode};

fn string_scalar(value: &str) -> TreeNode {
    TreeNode::scalar(SemType::Str, value)
}

fn make_map(entries: &[(&str, &str)]) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        let mut key_node = string_scalar(key);
        key_node.is_map_key = true;
        content.push(key_node);
        content.push(string_scalar(value));
    }
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.tag().to_owned(),
        content,
        ..TreeNode::default()
    }
}

fn mapping(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        let mut key_node = string_scalar(key);
        key_node.is_map_key = true;
        content.push(key_node);
        content.push(value);
    }
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.tag().to_owned(),
        content,
        ..TreeNode::default()
    }
}

fn make_seq(values: &[&str]) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: values.iter().map(|v| string_scalar(v)).collect(),
        ..TreeNode::default()
    }
}

#[test]
fn core_utils_recursive_compare_treats_nil_nodes_as_equal() {
    let left = TreeNode::scalar(SemType::Nil, "");
    let right = TreeNode::scalar(SemType::Nil, "ignored");

    assert!(recursive_node_compare(&left, &right));
}

#[test]
fn core_utils_recursive_compare_ignores_map_entry_order() {
    let left = make_map(&[("a", "1"), ("b", "2")]);
    let right = make_map(&[("b", "2"), ("a", "1")]);

    assert!(recursive_node_compare(&left, &right));
}

#[test]
fn core_utils_recursive_compare_keeps_sequence_order_significant() {
    let left = make_seq(&["1", "2", "3"]);
    let right = make_seq(&["3", "2", "1"]);

    assert!(!recursive_node_compare(&left, &right));
}

#[test]
fn core_utils_map_lookup_and_seq_growth_match_zig_helpers() {
    let mut store = TreeStore::new();
    let root = store.add(CoreTreeNode::scalar(CoreSemType::Str, "root"));
    treease_core::core::ensure_map(&mut store, root).unwrap();
    let _value = treease_core::core::get_or_create_map_value(&mut store, root, "b").unwrap();
    let entry = get_map_entry(&store, root, "b").unwrap().unwrap();

    assert_eq!(store.value_for(entry.key).unwrap(), "b");

    let seq_root = store.add(CoreTreeNode::scalar(CoreSemType::Str, "root"));
    let index = ensure_seq_index(&mut store, seq_root, 2).unwrap();

    assert_eq!(store.get(seq_root).unwrap().content.len(), 3);
    assert_eq!(store.get(index).unwrap().sem_type, Some(CoreSemType::Nil));
}

#[test]
fn core_utils_parse_snippet_equivalents_handle_empty_comment_and_scalars() {
    let empty = parse_snippet("").unwrap();
    assert_eq!(empty.sem_type, Some(SemType::Nil));

    let comment = parse_snippet("# hello").unwrap();
    assert_eq!(comment.line_comment, "# hello");

    let bool_node = parse_snippet("true").unwrap();
    assert_eq!(bool_node.sem_type, Some(SemType::Boolean));

    let int_node = parse_snippet("1").unwrap();
    assert_eq!(int_node.sem_type, Some(SemType::Int));
}

#[test]
fn core_utils_parse_snippet_equivalent_rejects_single_colon() {
    assert!(matches!(
        parse_snippet(":"),
        Err(CoreError::Parse(ParseError::InvalidYaml))
    ));
}

#[test]
fn recursive_node_equal_supports_deep_map_sequence_equality_with_reordered_map_keys() {
    // Build left: {obj: {b: "2", a: "1"}, seq: ["x", "y"]}
    let left = mapping(vec![
        ("obj", make_map(&[("b", "2"), ("a", "1")])),
        ("seq", make_seq(&["x", "y"])),
    ]);

    // Build right: {seq: ["x", "y"], obj: {a: "1", b: "2"}}
    let right = mapping(vec![
        ("seq", make_seq(&["x", "y"])),
        ("obj", make_map(&[("a", "1"), ("b", "2")])),
    ]);

    assert!(recursive_node_compare(&left, &right));
}

#[test]
fn recursive_node_equal_treats_numeric_scalar_and_numeric_like_string_as_different() {
    let left = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: "42".to_string(),
        ..TreeNode::default()
    };
    let right = string_scalar("42");

    assert!(!recursive_node_compare(&left, &right));
}
