use treease_core::operators::unique::{unique, unique_by};
use treease_core::operators::{
    Context, ExpressionNode, NodeKind, Operation, SemType, TraversePreferences, TreeEngine,
    TreeNode, UNIQUE_BY_OP_TYPE, UNIQUE_OP_TYPE, create_traversal_tree, splat,
};

fn unique_expression() -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &UNIQUE_OP_TYPE,
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

fn unique_by_expression(path_segments: Vec<TreeNode>) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &UNIQUE_BY_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: Some(
            create_traversal_tree(&path_segments, TraversePreferences::default(), false).unwrap(),
        ),
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
fn unique_operator_deduplicates_scalars_and_keeps_original_order() {
    let mut seq = sequence(vec![
        int_scalar(2),
        int_scalar(1),
        int_scalar(3),
        int_scalar(2),
    ]);
    seq.leading_content = "# abc\n".to_owned();
    let ctx = Context {
        matching_nodes: vec![seq],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_expression();

    let out = unique(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.leading_content, "# abc\n");
    assert_eq!(seq.content.len(), 3);
    assert_eq!(seq.content[0].value, "2");
    assert_eq!(seq.content[1].value, "1");
    assert_eq!(seq.content[2].value, "3");
}

#[test]
fn unique_by_operator_deduplicates_objects_by_selected_field() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("cat")),
            ]),
            mapping(vec![
                ("name", string_scalar("billy")),
                ("pet", string_scalar("dog")),
            ]),
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("dog")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_by_expression(vec![string_scalar("name")]);

    let out = unique_by(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.content.len(), 2);
    assert_eq!(seq.content[0].content[1].value, "harry");
    assert_eq!(seq.content[1].content[1].value, "billy");
}

#[test]
fn unique_operator_splat() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(2), int_scalar(1), int_scalar(2)])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_expression();

    let out = unique(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.content.len(), 2);
    assert_eq!(seq.content[0].value, "2");
    assert_eq!(seq.content[1].value, "1");
}

#[test]
fn unique_operator_result_can_be_splatted_to_unique_values() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(2), int_scalar(1), int_scalar(2)])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_expression();

    let unique_out = unique(ctx, &mut engine, &mut expr).unwrap();
    let splatted = splat(unique_out, TraversePreferences::default()).unwrap();

    assert_eq!(splatted.matching_nodes.len(), 2);
    assert_eq!(splatted.matching_nodes[0].value, "2");
    assert_eq!(splatted.matching_nodes[1].value, "1");
}

#[test]
fn unique_operator_deduplicates_array_of_objects() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("cat")),
            ]),
            mapping(vec![
                ("name", string_scalar("billy")),
                ("pet", string_scalar("dog")),
            ]),
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("cat")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_expression();

    let out = unique(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.content.len(), 2);
}

#[test]
fn unique_operator_deduplicates_array_of_arrays() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            sequence(vec![string_scalar("cat"), string_scalar("dog")]),
            sequence(vec![string_scalar("cat"), string_scalar("sheep")]),
            sequence(vec![string_scalar("cat"), string_scalar("dog")]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_expression();

    let out = unique(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.content.len(), 2);
}

#[test]
fn unique_by_operator_with_missing_field() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("cat")),
            ]),
            mapping(vec![("pet", string_scalar("fish"))]),
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("dog")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_by_expression(vec![string_scalar("name")]);

    let out = unique_by(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.content.len(), 2);
}

#[test]
fn unique_by_operator_splat() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("cat")),
            ]),
            mapping(vec![("pet", string_scalar("fish"))]),
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("dog")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_by_expression(vec![string_scalar("name")]);

    let out = unique_by(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.content.len(), 2);
}

#[test]
fn unique_by_operator_result_can_be_splatted_like_zig_unique_by_splat() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("cat")),
            ]),
            mapping(vec![("pet", string_scalar("fish"))]),
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("dog")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_by_expression(vec![string_scalar("name")]);

    let unique_out = unique_by(ctx, &mut engine, &mut expr).unwrap();
    let splatted = splat(unique_out, TraversePreferences::default()).unwrap();

    assert_eq!(splatted.matching_nodes.len(), 2);
    assert_eq!(splatted.matching_nodes[0].content[0].value, "name");
    assert_eq!(splatted.matching_nodes[0].content[1].value, "harry");
    assert_eq!(splatted.matching_nodes[1].content[0].value, "pet");
    assert_eq!(splatted.matching_nodes[1].content[1].value, "fish");
}

#[test]
fn unique_by_operator_with_nested_path() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![
            mapping(vec![
                ("name", string_scalar("harry")),
                ("pet", string_scalar("cat")),
            ]),
            mapping(vec![
                ("name", string_scalar("billy")),
                ("pet", string_scalar("dog")),
            ]),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_by_expression(vec![string_scalar("cat"), string_scalar("dog")]);

    let out = unique_by(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.content.len(), 1);
}

#[test]
fn unique_by_operator_preserves_comments() {
    let mut seq = sequence(vec![
        mapping(vec![
            ("name", string_scalar("harry")),
            ("pet", string_scalar("cat")),
        ]),
        mapping(vec![
            ("name", string_scalar("billy")),
            ("pet", string_scalar("dog")),
        ]),
        mapping(vec![
            ("name", string_scalar("harry")),
            ("pet", string_scalar("dog")),
        ]),
    ]);
    seq.leading_content = "# header comment\n".to_owned();
    let ctx = Context {
        matching_nodes: vec![seq],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = unique_by_expression(vec![string_scalar("name")]);

    let out = unique_by(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    let seq = &out.matching_nodes[0];
    assert_eq!(seq.leading_content, "# header comment\n");
    assert_eq!(seq.content.len(), 2);
}
