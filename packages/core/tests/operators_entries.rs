use treease_core::core::tree_navigator::create_traversal_tree;
use treease_core::expression_pipeline;
use treease_core::operators::compat::TraversePreferences;
use treease_core::operators::entries::{
    from_entries_operator, to_entries_operator, with_entries_operator,
};
use treease_core::operators::{
    Context, ExpressionNode, FROM_ENTRIES_OP_TYPE, NodeKind, Operation, SemType,
    TO_ENTRIES_OP_TYPE, TreeEngine, TreeNode, WITH_ENTRIES_OP_TYPE,
};

fn expression(operation_type: &'static treease_core::operators::OperationType) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

fn scalar(sem_type: SemType, value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(sem_type),
        tag: sem_type.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn string_scalar(value: &str) -> TreeNode {
    scalar(SemType::Str, value)
}

fn int_scalar(value: i64) -> TreeNode {
    scalar(SemType::Int, &value.to_string())
}

fn mapping(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        content.push(string_scalar(key));
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

fn map_value<'a>(map: &'a TreeNode, key: &str) -> &'a TreeNode {
    let mut i = 0;
    while i + 1 < map.content.len() {
        if map.content[i].value == key {
            return &map.content[i + 1];
        }
        i += 2;
    }
    panic!("missing key {key}");
}

#[test]
fn to_entries_operator_turns_mapping_into_key_value_entry_sequence() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", int_scalar(1)), ("b", int_scalar(2))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&TO_ENTRIES_OP_TYPE);

    let out = to_entries_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let sequence = &out.matching_nodes[0];
    assert_eq!(sequence.kind, NodeKind::Sequence);
    assert_eq!(sequence.content.len(), 2);
    assert_eq!(map_value(&sequence.content[0], "key").value, "a");
    assert_eq!(map_value(&sequence.content[0], "value").value, "1");
    assert_eq!(map_value(&sequence.content[1], "key").value, "b");
    assert_eq!(map_value(&sequence.content[1], "value").value, "2");
}

#[test]
fn from_entries_operator_turns_entry_sequence_back_into_mapping() {
    let entry_sequence = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: vec![
            mapping(vec![("key", string_scalar("x")), ("value", int_scalar(1))]),
            mapping(vec![("key", string_scalar("y")), ("value", int_scalar(2))]),
        ],
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![entry_sequence],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&FROM_ENTRIES_OP_TYPE);

    let out = from_entries_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let map = &out.matching_nodes[0];
    assert_eq!(map.kind, NodeKind::Mapping);
    assert_eq!(map_value(map, "x").value, "1");
    assert_eq!(map_value(map, "y").value, "2");
}

#[test]
fn to_entries_from_sequence_returns_indexed_entries() {
    let seq = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: vec![string_scalar("a"), string_scalar("b")],
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![seq],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&TO_ENTRIES_OP_TYPE);

    let out = to_entries_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
}

#[test]
fn to_entries_null_yields_empty() {
    let null_node = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Nil),
        tag: SemType::Nil.tag().to_owned(),
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![null_node],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&TO_ENTRIES_OP_TYPE);

    let out = to_entries_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 0);
}

#[test]
fn from_entries_roundtrip_preserves_map() {
    let input = mapping(vec![("a", int_scalar(1)), ("b", int_scalar(2))]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut to_expr = expression(&TO_ENTRIES_OP_TYPE);

    let to = to_entries_operator(ctx, &mut engine, &mut to_expr).unwrap();
    let mut from_expr = expression(&FROM_ENTRIES_OP_TYPE);
    let from = from_entries_operator(to, &mut engine, &mut from_expr).unwrap();

    assert_eq!(from.matching_nodes.len(), 1);
    assert_eq!(from.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(map_value(&from.matching_nodes[0], "a").value, "1");
    assert_eq!(map_value(&from.matching_nodes[0], "b").value, "2");
}

#[test]
fn with_entries_applies_rhs_over_entries() {
    // Zig: with_entries with RHS that sets values to "0" → {a:0, b:0}
    // Rust: use a traversal RHS that preserves entries as-is (roundtrip)
    let input = mapping(vec![("a", int_scalar(1)), ("b", int_scalar(2))]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    // Use create_traversal_tree as RHS: pass each entry through unchanged
    let traversal = *create_traversal_tree(&[], TraversePreferences::default(), false).unwrap();
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &WITH_ENTRIES_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(traversal)),
    };

    let result = with_entries_operator(ctx, &mut engine, &mut expr).unwrap();
    // Result should be a mapping with original keys
    assert_eq!(result.matching_nodes.len(), 1);
    let map = &result.matching_nodes[0];
    assert_eq!(map.kind, NodeKind::Mapping);
    assert!(map.content.len() >= 4); // at least 2 key-value pairs
    // Verify key-value pairs
    let mut found: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut i = 0;
    while i + 1 < map.content.len() {
        found.insert(
            map.content[i].value.as_str(),
            map.content[i + 1].value.as_str(),
        );
        i += 2;
    }
    assert_eq!(found.get("a"), Some(&"1"));
    assert_eq!(found.get("b"), Some(&"2"));
}

#[test]
fn with_entries_can_rewrite_each_value_field() {
    let input = mapping(vec![("a", int_scalar(1)), ("b", int_scalar(2))]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let result =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "with_entries(.value = 0)")
            .expect("with_entries value rewrite should succeed");

    assert_eq!(result.matching_nodes.len(), 1);
    let map = &result.matching_nodes[0];
    assert_eq!(map.kind, NodeKind::Mapping);
    assert_eq!(map_value(map, "a").value, "0");
    assert_eq!(map_value(map, "b").value, "0");
}
