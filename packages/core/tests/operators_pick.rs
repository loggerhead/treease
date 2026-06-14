use treease_core::expression_pipeline;
use treease_core::operators::pick::{pick_operator, pick_with_nodes};
use treease_core::operators::{
    Context, ExpressionNode, NodeKind, Operation, PICK_OP_TYPE, SemType, TraversePreferences,
    TreeEngine, TreeNode, create_value_operation, splat,
};

fn expression_with_rhs(rhs: TreeNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &PICK_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(rhs)).unwrap(),
            lhs: None,
            rhs: None,
        })),
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

fn sequence(items: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: items,
        ..TreeNode::default()
    }
}

#[test]
fn pick_with_nodes_preserves_pick_order_and_comments_for_maps() {
    let mut node = mapping(vec![
        ("cat", string_scalar("meow")),
        ("dog", string_scalar("bark")),
        ("hamster", string_scalar("squeak")),
    ]);
    node.tag = "!things".to_owned();
    node.leading_content = "# abc\n".to_owned();
    node.head_comment = "head".to_owned();
    node.foot_comment = "foot".to_owned();

    let indices = sequence(vec![
        string_scalar("hamster"),
        string_scalar("cat"),
        string_scalar("goat"),
    ]);

    let out = pick_with_nodes(&node, &indices).unwrap();

    assert_eq!(out.kind, NodeKind::Mapping);
    assert_eq!(out.tag, "!things");
    assert_eq!(out.leading_content, "# abc\n");
    assert_eq!(out.head_comment, "head");
    assert_eq!(out.foot_comment, "foot");
    assert_eq!(out.content.len(), 4);
    assert_eq!(out.content[0].value, "hamster");
    assert_eq!(out.content[1].value, "squeak");
    assert_eq!(out.content[2].value, "cat");
    assert_eq!(out.content[3].value, "meow");
}

#[test]
fn pick_operator_picks_sequence_indices_and_skips_invalid_values() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            string_scalar("cat"),
            string_scalar("leopard"),
            string_scalar("lion"),
        ])],
        ..Context::default()
    };
    let rhs = sequence(vec![
        int_scalar(2),
        int_scalar(0),
        int_scalar(734),
        int_scalar(-5),
    ]);
    let mut engine = TreeEngine::default();
    let mut expr = expression_with_rhs(rhs);

    let out = pick_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.kind, NodeKind::Sequence);
    assert_eq!(seq.content.len(), 2);
    assert_eq!(seq.content[0].value, "lion");
    assert_eq!(seq.content[1].value, "cat");
}

// ── Expression pipeline scenarios ────────────────────────────────

fn map_key_node(value: &str) -> TreeNode {
    let mut n = string_scalar(value);
    n.is_map_key = true;
    n
}

fn mapping_with_keys(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        content.push(map_key_node(key));
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

fn doc_map(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    mapping_with_keys(entries)
}

#[test]
fn pick_expression_picks_keys_from_map() {
    // .myMap |= pick(["hamster", "cat", "goat"])
    let input = doc_map(vec![(
        "myMap",
        mapping(vec![
            ("cat", string_scalar("meow")),
            ("dog", string_scalar("bark")),
            ("thing", string_scalar("hamster")),
            ("hamster", string_scalar("squeak")),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".myMap |= pick([\"hamster\", \"cat\", \"goat\"])",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let root = &out.matching_nodes[0];
    assert_eq!(root.content[0].value, "myMap");
    let my_map = &root.content[1];
    assert_eq!(my_map.kind, NodeKind::Mapping);
    assert_eq!(my_map.content.len(), 4); // hamster: squeak, cat: meow
    assert_eq!(my_map.content[0].value, "hamster");
    assert_eq!(my_map.content[1].value, "squeak");
    assert_eq!(my_map.content[2].value, "cat");
    assert_eq!(my_map.content[3].value, "meow");
}

#[test]
fn pick_expression_picks_keys_from_map_including_all_keys() {
    // .myMap |= pick( (["thing"] + keys) | unique)
    let input = doc_map(vec![(
        "myMap",
        mapping(vec![
            ("cat", string_scalar("meow")),
            ("dog", string_scalar("bark")),
            ("thing", string_scalar("hamster")),
            ("hamster", string_scalar("squeak")),
        ]),
    )]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".myMap |= pick( ([\"thing\"] + keys) | unique)",
    )
    .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let my_map = &out.matching_nodes[0].content[1];
    assert_eq!(my_map.kind, NodeKind::Mapping);
    // Should contain thing, cat, dog, hamster (all keys)
    assert_eq!(my_map.content.len(), 8);
    // thing: hamster (first because it's picked first)
    assert_eq!(my_map.content[0].value, "thing");
    assert_eq!(my_map.content[1].value, "hamster");
}

#[test]
fn pick_expression_splat_after_pick() {
    // pick(["hamster", "cat"]) -- returns mapping with picked keys in pick order.
    // Note: Rust `[]` is the Collect operator, not a splat like in Zig.
    // Here we verify the pick result is a mapping with the right content.
    let input = mapping(vec![
        ("cat", string_scalar("meow")),
        ("dog", string_scalar("bark")),
        ("thing", string_scalar("hamster")),
        ("hamster", string_scalar("squeak")),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "pick([\"hamster\", \"cat\"])")
            .expect("evaluation should succeed");

    // Single mapping node with the picked keys in pick order.
    assert_eq!(out.matching_nodes.len(), 1);
    let picked = &out.matching_nodes[0];
    assert_eq!(picked.kind, NodeKind::Mapping);
    // Content: key(hamster), value(squeak), key(cat), value(meow)
    assert_eq!(picked.content.len(), 4);
    assert_eq!(picked.content[0].value, "hamster"); // key
    assert_eq!(picked.content[1].value, "squeak"); // value
    assert_eq!(picked.content[2].value, "cat"); // key
    assert_eq!(picked.content[3].value, "meow"); // value
}

#[test]
fn pick_expression_splat_returns_picked_values_in_pick_order() {
    let input = mapping(vec![
        ("cat", string_scalar("meow")),
        ("dog", string_scalar("bark")),
        ("hamster", string_scalar("squeak")),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let picked =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "pick([\"hamster\", \"cat\"])")
            .expect("pick pipeline should succeed");
    let splatted = splat(picked, TraversePreferences::default()).expect("splat should succeed");

    assert_eq!(splatted.matching_nodes.len(), 2);
    assert_eq!(splatted.matching_nodes[0].value, "squeak");
    assert_eq!(splatted.matching_nodes[1].value, "meow");
}

#[test]
fn pick_expression_preserves_custom_tag() {
    // .myMap |= pick(["hamster", "cat", "goat"]) with !things tag
    let inner = {
        let mut m = mapping(vec![
            ("cat", string_scalar("meow")),
            ("dog", string_scalar("bark")),
            ("thing", string_scalar("hamster")),
            ("hamster", string_scalar("squeak")),
        ]);
        m.tag = "!things".to_owned();
        m.sem_type = None;
        m
    };
    let input = doc_map(vec![("myMap", inner)]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".myMap |= pick([\"hamster\", \"cat\", \"goat\"])",
    )
    .expect("evaluation should succeed");

    let my_map = &out.matching_nodes[0].content[1];
    assert_eq!(my_map.tag, "!things");
}

#[test]
fn pick_expression_preserves_leading_content() {
    // .myMap |= pick(["hamster", "cat", "goat"]) with leading content
    let inner = {
        let mut m = mapping(vec![
            ("cat", string_scalar("meow")),
            ("dog", string_scalar("bark")),
            ("thing", string_scalar("hamster")),
            ("hamster", string_scalar("squeak")),
        ]);
        m.leading_content = "# abc\n".to_owned();
        m
    };
    let input = {
        let mut m = doc_map(vec![("myMap", inner)]);
        m.foot_comment = "xyz".to_owned();
        m
    };
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".myMap |= pick([\"hamster\", \"cat\", \"goat\"])",
    )
    .expect("evaluation should succeed");

    let my_map = &out.matching_nodes[0].content[1];
    assert!(my_map.leading_content.contains("abc"));
}

#[test]
fn pick_expression_picks_indices_from_array() {
    // pick([2, 0, 734, -5])
    let input = sequence(vec![
        string_scalar("cat"),
        string_scalar("leopard"),
        string_scalar("lion"),
    ]);
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "pick([2, 0, 734, -5])")
        .expect("evaluation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.kind, NodeKind::Sequence);
    assert_eq!(seq.content.len(), 2);
    assert_eq!(seq.content[0].value, "lion");
    assert_eq!(seq.content[1].value, "cat");
}

#[test]
fn pick_expression_picks_indices_from_array_with_leading_content() {
    // pick([2, 0, 734, -5]) with leading content
    let mut input = sequence(vec![
        string_scalar("cat"),
        string_scalar("leopard"),
        string_scalar("lion"),
    ]);
    input.leading_content = "# abc\n".to_owned();
    let ctx = Context {
        matching_nodes: vec![input],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "pick([2, 0, 734, -5])")
        .expect("evaluation should succeed");

    let seq = &out.matching_nodes[0];
    assert!(seq.leading_content.contains("abc"));
}
