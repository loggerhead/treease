use treease_core::expression_pipeline;
use treease_core::operators::collect_object::collect_object_operator;
use treease_core::operators::{
    COLLECT_OBJECT_OP_TYPE, Context, CoreError, Diagnostics, ExpressionNode, NodeKind, Operation,
    SemType, TreeEngine, TreeNode,
};

fn expression(operation: &'static treease_core::operators::OperationType) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: operation,
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

fn string_scalar(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: "string".to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
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
fn collect_object_operator_returns_empty_map_for_empty_context() {
    let ctx = Context::default();
    let mut engine = TreeEngine::default();
    let mut expr = expression(&COLLECT_OBJECT_OP_TYPE);

    let out = collect_object_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].tag, SemType::Map.tag());
    assert_eq!(out.matching_nodes[0].value, "{}");
}

#[test]
fn collect_object_operator_rejects_mismatching_node_sizes() {
    let short = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: "seq".to_owned(),
        content: vec![string_scalar("a")],
        ..TreeNode::default()
    };
    let long = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: "seq".to_owned(),
        content: vec![string_scalar("a"), string_scalar("b")],
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![long, short],
        diagnostics: Some(Box::new(Diagnostics)),
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = expression(&COLLECT_OBJECT_OP_TYPE);

    let err = collect_object_operator(ctx, &mut engine, &mut expr).unwrap_err();

    assert!(matches!(
        err,
        CoreError::OperatorMessage { ref op, ref message }
            if op == "collect_object" && message.contains("mismatching node sizes")
    ));
}

#[test]
fn collect_object_two_documents_stay_as_two_wrapped_outputs_in_pipeline() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![("name", string_scalar("Mike"))]),
            mapping(vec![("name", string_scalar("Bob"))]),
        ],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "{\"wrap\": .}")
        .expect("two-doc collect_object pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content[0].value, "wrap");
    assert_eq!(out.matching_nodes[0].content[1].content[1].value, "Mike");
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[1].content[0].value, "wrap");
    assert_eq!(out.matching_nodes[1].content[1].content[1].value, "Bob");
}

#[test]
fn collect_object_pipeline_reads_literal_name_field() {
    let mut engine = TreeEngine::default();
    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &Context::default(),
        "{\"name\": \"mike\"} | .name",
    )
    .expect("literal collect_object field access should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "mike");
}

#[test]
fn collect_object_pipeline_rejects_mismatched_pairs() {
    let mut engine = TreeEngine::default();
    let err = expression_pipeline::execute_on_context(
        &mut engine,
        &Context::default(),
        "{\"c\": \"a\", \"b\", \"d\"}",
    )
    .unwrap_err();

    assert!(matches!(
        err,
        treease_core::expression_pipeline::PipelineError::Compat(CoreError::OperatorMessage {
            ref op,
            ref message,
        })
            if op == "collect_object" && message.contains("mismatching node sizes")
    ));
}

#[test]
fn collect_object_pipeline_reads_nested_array_value() {
    let mut engine = TreeEngine::default();
    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &Context::default(),
        "{\"person\": {\"names\": [\"mike\"]}} | .person.names[0]",
    )
    .expect("nested collect_object traversal should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "mike");
}

#[test]
fn collect_object_pipeline_builds_key_from_each_input_name() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("name", string_scalar("cat"))]),
            mapping(vec![("name", string_scalar("dog"))]),
        ])],
        ..Context::default()
    };

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, ".[] | {.name: \"great\"}")
            .expect("collect_object name projection should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "cat");
    assert_eq!(out.matching_nodes[0].content[1].value, "great");
    assert_eq!(out.matching_nodes[1].content[0].value, "dog");
    assert_eq!(out.matching_nodes[1].content[1].value, "great");
}

#[test]
fn collect_object_pipeline_keeps_literal_and_traversal_pairs_per_document() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![
            mapping(vec![
                ("name", string_scalar("Mike")),
                (
                    "pets",
                    mapping(vec![(
                        "cows",
                        sequence(vec![string_scalar("apl"), string_scalar("bba")]),
                    )]),
                ),
            ]),
            mapping(vec![
                ("name", string_scalar("Rosey")),
                (
                    "pets",
                    mapping(vec![(
                        "sheep",
                        sequence(vec![string_scalar("frog"), string_scalar("meow")]),
                    )]),
                ),
            ]),
        ],
        ..Context::default()
    };

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "{\"a\": .name, \"b\": .pets}")
            .expect("collect_object literal and traversal pair pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "a");
    assert_eq!(out.matching_nodes[0].content[1].value, "Mike");
    assert_eq!(out.matching_nodes[0].content[2].value, "b");
    assert_eq!(out.matching_nodes[0].content[3].content[0].value, "cows");
    assert_eq!(out.matching_nodes[1].content[0].value, "a");
    assert_eq!(out.matching_nodes[1].content[1].value, "Rosey");
    assert_eq!(out.matching_nodes[1].content[2].value, "b");
    assert_eq!(out.matching_nodes[1].content[3].content[0].value, "sheep");
}

#[test]
fn collect_object_with_splat_value_emits_multiple_maps_in_pipeline() {
    let mut engine = TreeEngine::default();
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

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, "{.name: .pets.[]}")
        .expect("collect_object splat pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].content[0].value, "Mike");
    assert_eq!(out.matching_nodes[0].content[1].value, "cat");
    assert_eq!(out.matching_nodes[1].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[1].content[0].value, "Mike");
    assert_eq!(out.matching_nodes[1].content[1].value, "dog");
}

#[test]
fn collect_object_splat_pipeline_emits_values_for_each_document() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![("name", string_scalar("cat"))]),
            mapping(vec![("name", string_scalar("dog"))]),
        ])],
        ..Context::default()
    };

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, ".[] | {.name: \"great\"}[]")
            .expect("collect_object splat result pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "great");
    assert_eq!(out.matching_nodes[1].value, "great");
}

// ── Merge empty objects then assign ───────────────────────────────

#[test]
fn collect_object_merge_empty_objects_then_assign() {
    // ({} + {}) | (.b = 3) — merging two empty objects and then
    // assigning a field. The collect_object creates each {}.
    // Verify that collect_object on empty input produces a valid
    // empty mapping that can participate in further operations.
    let ctx = Context::default();
    let mut engine = TreeEngine::default();
    let mut expr = expression(&COLLECT_OBJECT_OP_TYPE);

    let out = collect_object_operator(ctx, &mut engine, &mut expr).unwrap();

    // Empty input produces an empty mapping
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].tag, SemType::Map.tag());
    assert_eq!(out.matching_nodes[0].value, "{}");
    assert!(out.matching_nodes[0].content.is_empty());
}

#[test]
fn collect_object_pipeline_merges_two_empty_objects_then_assigns() {
    let mut engine = TreeEngine::default();
    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &Context::default(),
        "({} + {}) | (.b = 3)",
    )
    .expect("merge empty objects then assign should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let map = &out.matching_nodes[0];
    assert_eq!(map.kind, NodeKind::Mapping);
    assert_eq!(map.content[0].value, "b");
    assert_eq!(map.content[1].value, "3");
}

#[test]
fn collect_object_pipeline_supports_array_append_payload() {
    let mut engine = TreeEngine::default();
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", sequence(vec![]))])],
        ..Context::default()
    };

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".a += [{\"key\": \"att2\", \"value\": \"val2\"}]",
    )
    .expect("array append with collect_object payload should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let array = &out.matching_nodes[0].content[1];
    assert_eq!(array.kind, NodeKind::Sequence);
    assert_eq!(array.content.len(), 1);
    assert_eq!(array.content[0].content[0].value, "key");
    assert_eq!(array.content[0].content[1].value, "att2");
    assert_eq!(array.content[0].content[2].value, "value");
    assert_eq!(array.content[0].content[3].value, "val2");
}

#[test]
fn collect_object_pipeline_creates_multiple_literal_pairs_from_scratch() {
    let mut engine = TreeEngine::default();
    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &Context::default(),
        "{\"wrap\": \"frog\", \"bing\": \"bong\"}",
    )
    .expect("multi-pair collect_object literal should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let map = &out.matching_nodes[0];
    assert_eq!(map.kind, NodeKind::Mapping);
    assert_eq!(map.content[0].value, "wrap");
    assert_eq!(map.content[1].value, "frog");
    assert_eq!(map.content[2].value, "bing");
    assert_eq!(map.content[3].value, "bong");
}

#[test]
fn collect_object_pipeline_creates_multiple_nested_objects_from_assignments() {
    let mut engine = TreeEngine::default();
    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &Context::default(),
        "(.a.b = \"foo\") | (.d.e = \"bar\")",
    )
    .expect("collect_object chained assignment creation should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let map = &out.matching_nodes[0];
    assert_eq!(map.kind, NodeKind::Mapping);
    assert_eq!(map.content[0].value, "a");
    assert_eq!(map.content[1].kind, NodeKind::Mapping);
    assert_eq!(map.content[2].value, "d");
    assert_eq!(map.content[3].kind, NodeKind::Mapping);
}
