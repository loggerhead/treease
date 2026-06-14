use treease_core::expression_pipeline;
use treease_core::operators::create_map::create_map_operator;
use treease_core::operators::{
    CREATE_MAP_OP_TYPE, Context, ExpressionNode, NodeKind, Operation, SELF_REFERENCE_OP_TYPE,
    SemType, TraversePreferences, TreeEngine, TreeNode, create_traversal_tree,
    create_value_operation,
};

fn string_scalar(value: &str) -> TreeNode {
    TreeNode::scalar(SemType::Str, value)
}

fn create_map_expression(lhs: TreeNode, rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &CREATE_MAP_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(ExpressionNode {
            operation: create_value_operation(Box::new(lhs)).unwrap(),
            lhs: None,
            rhs: None,
        })),
        rhs: Some(Box::new(rhs)),
    }
}

fn self_expression() -> ExpressionNode {
    ExpressionNode {
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
    }
}

fn mapping(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        content.push(string_scalar(key));
        content.push(value);
    }
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(treease_core::operators::SemType::Map),
        tag: treease_core::operators::SemType::Map.tag().to_owned(),
        content,
        ..TreeNode::default()
    }
}

#[test]
fn create_map_operator_builds_pair_from_literal_key_and_value() {
    let mut engine = TreeEngine::default();
    let mut expr = create_map_expression(
        string_scalar("frog"),
        ExpressionNode {
            operation: create_value_operation(Box::new(string_scalar("jumps"))).unwrap(),
            lhs: None,
            rhs: None,
        },
    );

    let out = create_map_operator(Context::default(), &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let outer_seq = &out.matching_nodes[0];
    assert_eq!(outer_seq.kind, NodeKind::Sequence);
    assert_eq!(outer_seq.content.len(), 1);
    let pair = pair(doc_bucket(outer_seq, 0), 0);
    assert_eq!(pair.kind, NodeKind::Mapping);
    assert_eq!(pair.content[0].value, "frog");
    assert_eq!(pair.content[1].value, "jumps");
}

#[test]
fn create_map_operator_wraps_current_node_under_literal_key() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("name", string_scalar("Mike"))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = create_map_expression(string_scalar("wrap"), self_expression());

    let out = create_map_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let outer_seq = &out.matching_nodes[0];
    assert_eq!(outer_seq.kind, NodeKind::Sequence);
    assert_eq!(outer_seq.content.len(), 1);
    let pair = pair(doc_bucket(outer_seq, 0), 0);
    assert_eq!(pair.kind, NodeKind::Mapping);
    assert_eq!(pair.content[0].value, "wrap");
    assert_eq!(pair.content[1].kind, NodeKind::Mapping);
    assert_eq!(pair.content[1].content[0].value, "name");
    assert_eq!(pair.content[1].content[1].value, "Mike");
}

// ── Additional helpers ────────────────────────────────────────────

fn create_map_expression_with_lhs_expr(lhs: ExpressionNode, rhs: ExpressionNode) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &CREATE_MAP_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(lhs)),
        rhs: Some(Box::new(rhs)),
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

fn doc_bucket(outer_seq: &TreeNode, doc_idx: usize) -> &TreeNode {
    &outer_seq.content[doc_idx]
}

fn pair(bucket: &TreeNode, pair_idx: usize) -> &TreeNode {
    &bucket.content[pair_idx]
}

// ── Traversal key + splat value ───────────────────────────────────

#[test]
fn create_map_with_traversal_key_and_splat_value() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("name", string_scalar("Alice")),
            (
                "pets",
                sequence(vec![string_scalar("cat"), string_scalar("dog")]),
            ),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".name: .pets.[]")
        .expect("create_map traversal+splat pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let outer_seq = &out.matching_nodes[0];
    assert_eq!(outer_seq.kind, NodeKind::Sequence);
    let bucket = doc_bucket(outer_seq, 0);
    assert_eq!(outer_seq.content.len(), 1);
    assert_eq!(bucket.content.len(), 2);
    assert_eq!(pair(bucket, 0).content[0].value, "Alice");
    assert_eq!(pair(bucket, 0).content[1].value, "cat");
    assert_eq!(pair(bucket, 1).content[0].value, "Alice");
    assert_eq!(pair(bucket, 1).content[1].value, "dog");
}

// ── Multiple create-map expressions ───────────────────────────────

#[test]
fn create_map_multiple_expressions_with_splat() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("name", string_scalar("Alice")),
            ("pets", sequence(vec![string_scalar("cat")])),
            ("food", sequence(vec![string_scalar("pizza")])),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".name: .pets.[], \"f\":.food.[]",
    )
    .expect("create_map multi-expression splat pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[0].value,
        "Alice"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[1].value,
        "cat"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[0].value,
        "f"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[1].value,
        "pizza"
    );
}

// ── Multiple create-map across multiple documents ─────────────────

#[test]
fn create_map_across_multiple_documents() {
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![("name", string_scalar("Alice"))]),
            mapping(vec![("name", string_scalar("Bob"))]),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // "wrap": . across two documents
    let mut expr = create_map_expression(string_scalar("wrap"), self_expression());

    let out = create_map_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let outer_seq = &out.matching_nodes[0];
    assert_eq!(outer_seq.kind, NodeKind::Sequence);
    assert_eq!(outer_seq.content.len(), 2);
    // First document wrapped
    assert_eq!(pair(doc_bucket(outer_seq, 0), 0).content[0].value, "wrap");
    assert_eq!(
        pair(doc_bucket(outer_seq, 0), 0).content[1].content[0].value,
        "name"
    );
    assert_eq!(
        pair(doc_bucket(outer_seq, 0), 0).content[1].content[1].value,
        "Alice"
    );
    // Second document wrapped
    assert_eq!(pair(doc_bucket(outer_seq, 1), 0).content[0].value, "wrap");
    assert_eq!(
        pair(doc_bucket(outer_seq, 1), 0).content[1].content[0].value,
        "name"
    );
    assert_eq!(
        pair(doc_bucket(outer_seq, 1), 0).content[1].content[1].value,
        "Bob"
    );
}

// ── Literal keys + traversal values ───────────────────────────────

#[test]
fn create_map_with_literal_keys_and_traversal_values() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("name", string_scalar("Alice")),
            ("pets", sequence(vec![string_scalar("cat")])),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // "a":.name
    let val_trav = create_traversal_tree(
        &[string_scalar("name")],
        TraversePreferences::default(),
        false,
    )
    .unwrap();
    let mut expr = create_map_expression(string_scalar("a"), *val_trav);

    let out = create_map_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let outer_seq = &out.matching_nodes[0];
    assert_eq!(outer_seq.content.len(), 1);
    let bucket = doc_bucket(outer_seq, 0);
    assert_eq!(bucket.content.len(), 1);
    assert_eq!(pair(bucket, 0).content[0].value, "a");
    assert_eq!(pair(bucket, 0).content[1].value, "Alice");
}

// ── Wrap + literal across documents ───────────────────────────────

#[test]
fn create_map_wrap_and_literal_across_documents() {
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![("name", string_scalar("Alice"))]),
            mapping(vec![("name", string_scalar("Bob"))]),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // "wrap": . across two documents
    let mut expr = create_map_expression(string_scalar("wrap"), self_expression());

    let out = create_map_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let outer_seq = &out.matching_nodes[0];
    assert_eq!(outer_seq.content.len(), 2);
    // Both documents wrapped
    assert_eq!(pair(doc_bucket(outer_seq, 0), 0).content[0].value, "wrap");
    assert_eq!(pair(doc_bucket(outer_seq, 1), 0).content[0].value, "wrap");
}

#[test]
fn create_map_block_expression_returns_both_sequences() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("name", string_scalar("Mike")),
            (
                "pets",
                sequence(vec![string_scalar("cat"), string_scalar("dog")]),
            ),
            (
                "food",
                sequence(vec![string_scalar("hotdog"), string_scalar("burger")]),
            ),
        ])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".name: .pets.[], \"f\":.food.[]",
    )
    .expect("create_map block pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 1);
    assert_eq!(doc_bucket(&out.matching_nodes[0], 0).content.len(), 2);
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[0].value,
        "Mike"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[1].value,
        "cat"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 1).content[1].value,
        "dog"
    );
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[1].content.len(), 1);
    assert_eq!(doc_bucket(&out.matching_nodes[1], 0).content.len(), 2);
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[0].value,
        "f"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[1].value,
        "hotdog"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 1).content[1].value,
        "burger"
    );
}

#[test]
fn create_map_block_expression_keeps_multi_doc_outputs_separate() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![
                ("name", string_scalar("Mike")),
                (
                    "pets",
                    sequence(vec![string_scalar("cat"), string_scalar("dog")]),
                ),
                (
                    "food",
                    sequence(vec![string_scalar("hotdog"), string_scalar("burger")]),
                ),
            ]),
            mapping(vec![
                ("name", string_scalar("Fred")),
                ("pets", sequence(vec![string_scalar("mouse")])),
                (
                    "food",
                    sequence(vec![
                        string_scalar("pizza"),
                        string_scalar("onion"),
                        string_scalar("apple"),
                    ]),
                ),
            ]),
        ],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".name: .pets.[], \"f\":.food.[]",
    )
    .expect("multi-doc create_map block pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Sequence);
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[0].value,
        "Mike"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 1), 0).content[0].value,
        "Fred"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[1].value,
        "hotdog"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 1), 0).content[1].value,
        "pizza"
    );
}

#[test]
fn create_map_wrap_and_name_combo_returns_two_outputs() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![("name", string_scalar("Mike"))]),
            mapping(vec![("name", string_scalar("Bob"))]),
        ],
        ..Context::default()
    };

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "\"wrap\": ., .name: \"great\"")
            .expect("wrap and name combo pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[0].value,
        "wrap"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[1].content[1].value,
        "Mike"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 1), 0).content[1].content[1].value,
        "Bob"
    );
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Sequence);
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[0].value,
        "Mike"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[1].value,
        "great"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 1), 0).content[0].value,
        "Bob"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 1), 0).content[1].value,
        "great"
    );
}

#[test]
fn create_map_pipeline_supports_traversing_created_key() {
    let mut engine = TreeEngine::default();
    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &Context::default(),
        "(\"frog\": \"jumps\") | .[0][0] | .frog",
    )
    .expect("create_map traversal pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "jumps");
}

// ── Sets key properly with path verification ──────────────────────

#[test]
fn create_map_sets_is_map_key_on_literal_pair() {
    // ("frog": "jumps") | .[0][0] | .frog
    // Verifies that after create_map, the key has is_map_key=true
    // and the value has is_map_key=false, so path resolution works.
    let mut engine = TreeEngine::default();
    let mut expr = create_map_expression(
        string_scalar("frog"),
        ExpressionNode {
            operation: create_value_operation(Box::new(string_scalar("jumps"))).unwrap(),
            lhs: None,
            rhs: None,
        },
    );

    let out = create_map_operator(Context::default(), &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let outer_seq = &out.matching_nodes[0];
    assert_eq!(outer_seq.kind, NodeKind::Sequence);
    assert_eq!(outer_seq.content.len(), 1);
    let pair = pair(doc_bucket(outer_seq, 0), 0);
    assert_eq!(pair.kind, NodeKind::Mapping);
    // Key is marked as map key for path resolution
    assert!(pair.content[0].is_map_key);
    assert_eq!(pair.content[0].value, "frog");
    // Value is not a map key
    assert!(!pair.content[1].is_map_key);
    assert_eq!(pair.content[1].value, "jumps");
}

// ── Sets key properly on map (collect_object syntax) ──────────────

#[test]
fn collect_object_sets_key_properly_on_map() {
    let mut engine = TreeEngine::default();
    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &Context::default(),
        "{\"frog\": \"jumps\"} | .frog",
    )
    .expect("collect_object map traversal should succeed");
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "jumps");
}

// ── Traversal key + splat value with path verification ────────────

#[test]
fn create_map_traversal_key_splat_value_with_is_map_key() {
    // (.name: .pets.[]) | .[0][0] | ..
    // Verifies that after create_map with traversal key and splat value,
    // the key and value nodes have correct is_map_key flags.
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("name", string_scalar("Mike")),
            (
                "pets",
                sequence(vec![string_scalar("cat"), string_scalar("dog")]),
            ),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".name: .pets.[]")
        .expect("create_map traversal+splat key flag pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let outer_seq = &out.matching_nodes[0];
    assert_eq!(outer_seq.kind, NodeKind::Sequence);
    let bucket = doc_bucket(outer_seq, 0);
    assert_eq!(outer_seq.content.len(), 1);
    assert_eq!(bucket.content.len(), 2);
    // First pair: Mike -> cat
    assert_eq!(pair(bucket, 0).content[0].value, "Mike");
    assert!(pair(bucket, 0).content[0].is_map_key);
    assert_eq!(pair(bucket, 0).content[1].value, "cat");
    assert!(!pair(bucket, 0).content[1].is_map_key);
    // Second pair: Mike -> dog
    assert_eq!(pair(bucket, 1).content[0].value, "Mike");
    assert!(pair(bucket, 1).content[0].is_map_key);
    assert_eq!(pair(bucket, 1).content[1].value, "dog");
    assert!(!pair(bucket, 1).content[1].is_map_key);
}

// ── Check path of nested child ────────────────────────────────────

#[test]
fn create_map_literal_key_with_nested_traversal_value() {
    // ("b":.pets) | .[0][0] | .b.cows
    // Input has pets with nested structure; create_map wraps it under "b".
    let ctx = Context {
        matching_nodes: vec![mapping(vec![(
            "pets",
            mapping(vec![("cows", string_scalar("value"))]),
        )])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // "b":.pets
    let val_trav = create_traversal_tree(
        &[string_scalar("pets")],
        TraversePreferences::default(),
        false,
    )
    .unwrap();
    let mut expr = create_map_expression(string_scalar("b"), *val_trav);

    let out = create_map_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let outer_seq = &out.matching_nodes[0];
    assert_eq!(outer_seq.kind, NodeKind::Sequence);
    assert_eq!(outer_seq.content.len(), 1);
    let pair = pair(doc_bucket(outer_seq, 0), 0);
    assert_eq!(pair.kind, NodeKind::Mapping);
    // Key "b" is marked as map key
    assert_eq!(pair.content[0].value, "b");
    assert!(pair.content[0].is_map_key);
    // Value is the nested pets map {cows: value}
    assert_eq!(pair.content[1].kind, NodeKind::Mapping);
    assert!(!pair.content[1].is_map_key);
    assert_eq!(pair.content[1].content[0].value, "cows");
    assert_eq!(pair.content[1].content[1].value, "value");
}

// ── Traversal key: traversal value (no splat) ─────────────────────

#[test]
fn create_map_with_traversal_key_and_traversal_value_no_splat() {
    // .name: .age
    // Both key and value come from traversals, no splat on either side.
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("name", string_scalar("Mike")),
            ("age", TreeNode::scalar(SemType::Int, "32")),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    // .name: .age
    let key_trav = create_traversal_tree(
        &[string_scalar("name")],
        TraversePreferences::default(),
        false,
    )
    .unwrap();
    let val_trav = create_traversal_tree(
        &[string_scalar("age")],
        TraversePreferences::default(),
        false, // no splat
    )
    .unwrap();
    let mut expr = create_map_expression_with_lhs_expr(*key_trav, *val_trav);

    let out = create_map_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let outer_seq = &out.matching_nodes[0];
    assert_eq!(outer_seq.kind, NodeKind::Sequence);
    assert_eq!(outer_seq.content.len(), 1);
    let pair = pair(doc_bucket(outer_seq, 0), 0);
    assert_eq!(pair.kind, NodeKind::Mapping);
    assert_eq!(pair.content[0].value, "Mike");
    assert!(pair.content[0].is_map_key);
    assert_eq!(pair.content[1].value, "32");
    assert!(!pair.content[1].is_map_key);
}

// ── Multiple create-map expressions with splat ────────────────────

#[test]
fn create_map_multiple_expressions_with_splat_full() {
    // .name: .pets.[], "f":.food.[]
    // Two create-map expressions in a block, each with splat values.
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("name", string_scalar("Mike")),
            (
                "pets",
                sequence(vec![string_scalar("cat"), string_scalar("dog")]),
            ),
            (
                "food",
                sequence(vec![string_scalar("hotdog"), string_scalar("burger")]),
            ),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".name: .pets.[], \"f\":.food.[]",
    )
    .expect("create_map multi-expression full pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[0].value,
        "Mike"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[1].value,
        "cat"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 1).content[1].value,
        "dog"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[0].value,
        "f"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[1].value,
        "hotdog"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 1).content[1].value,
        "burger"
    );
}

// ── Multiple create-map across documents ──────────────────────────

#[test]
fn create_map_multiple_expressions_across_documents() {
    // .name: .pets.[], "f":.food.[] across 2 documents
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![
                ("name", string_scalar("Mike")),
                (
                    "pets",
                    sequence(vec![string_scalar("cat"), string_scalar("dog")]),
                ),
                (
                    "food",
                    sequence(vec![string_scalar("hotdog"), string_scalar("burger")]),
                ),
            ]),
            mapping(vec![
                ("name", string_scalar("Fred")),
                ("pets", sequence(vec![string_scalar("mouse")])),
                (
                    "food",
                    sequence(vec![
                        string_scalar("pizza"),
                        string_scalar("onion"),
                        string_scalar("apple"),
                    ]),
                ),
            ]),
        ],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".name: .pets.[], \"f\":.food.[]",
    )
    .expect("create_map multi-expression multi-doc pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[0].value,
        "Mike"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 0), 0).content[1].value,
        "cat"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 1), 0).content[0].value,
        "Fred"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[0], 1), 0).content[1].value,
        "mouse"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[0].value,
        "f"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 0), 0).content[1].value,
        "hotdog"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 1), 0).content[0].value,
        "f"
    );
    assert_eq!(
        pair(doc_bucket(&out.matching_nodes[1], 1), 0).content[1].value,
        "pizza"
    );
}
