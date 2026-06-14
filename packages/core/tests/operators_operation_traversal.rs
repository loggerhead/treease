use treease_core::core::{
    ParsedKey, build_recursive_descent_expression, build_traversal_expression,
};
use treease_core::expression_pipeline;
use treease_core::operators::pipe::pipe_operator;
use treease_core::operators::traverse_path::traverse_path_operator;
use treease_core::operators::{Context, ExpressionNode, Operation, TreeEngine};
use treease_core::operators::{
    NodeKind, OperationPreference, PIPE_OP_TYPE, SELF_REFERENCE_OP_TYPE, SHORT_PIPE_OP_TYPE,
    SemType, TRAVERSE_PATH_OP_TYPE, TraversePreferences, TreeNode, create_scalar_node_i64,
    create_string_scalar_node, create_traversal_tree, create_value_operation,
};

fn mk_mapping() -> TreeNode {
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.tag().to_owned(),
        ..TreeNode::default()
    }
}

fn mk_str(value: &str) -> TreeNode {
    TreeNode::scalar(SemType::Str, value)
}

fn mk_key(value: &str) -> TreeNode {
    let mut n = mk_str(value);
    n.is_map_key = true;
    n
}

fn add_kv(parent: &mut TreeNode, key: &str, value_node: TreeNode) {
    parent.content.push(mk_key(key));
    parent.content.push(value_node);
}

fn mk_traverse_expr(key: &str) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &TRAVERSE_PATH_OP_TYPE,
            value: None,
            string_value: key.to_owned(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

#[test]
fn traverse_path_treats_merge_key_as_normal_key() {
    let mut foo = mk_mapping();
    add_kv(&mut foo, "a", mk_str("foo_a"));
    add_kv(&mut foo, "thing", mk_str("foo_thing"));

    let alias_to_foo = TreeNode {
        kind: NodeKind::Alias,
        tag: "alias".to_owned(),
        value: "*foo".to_owned(),
        ..TreeNode::default()
    };

    let mut bar = mk_mapping();
    add_kv(&mut bar, "thing", mk_str("bar_thing"));
    add_kv(&mut bar, "<<", alias_to_foo);

    let mut root = mk_mapping();
    add_kv(&mut root, "bar", bar);

    let ctx = Context {
        matching_nodes: vec![root.clone()],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let mut e_bar = mk_traverse_expr("bar");
    let mut e_thing = mk_traverse_expr("thing");
    let mut e_merge_key = mk_traverse_expr("<<");

    let c1 = traverse_path_operator(ctx, &mut engine, &mut e_bar).unwrap();
    let c2 = traverse_path_operator(c1, &mut engine, &mut e_thing).unwrap();
    assert_eq!(c2.matching_nodes.len(), 1);
    assert_eq!(c2.matching_nodes[0].value, "bar_thing");

    let c3 = traverse_path_operator(
        Context {
            matching_nodes: vec![root],
            ..Context::default()
        },
        &mut engine,
        &mut e_bar,
    )
    .unwrap();
    let c4 = traverse_path_operator(c3, &mut engine, &mut e_merge_key).unwrap();
    assert_eq!(c4.matching_nodes.len(), 1);
    assert_eq!(c4.matching_nodes[0].kind, NodeKind::Alias);
}

#[test]
fn traverse_path_optional_traverse_skips_scalar() {
    let scalar = mk_str("cat");
    let ctx = Context {
        matching_nodes: vec![scalar],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let prefs = TraversePreferences {
        optional_traverse: true,
        ..TraversePreferences::default()
    };
    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &TRAVERSE_PATH_OP_TYPE,
            value: None,
            string_value: "[]".to_owned(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Traverse(prefs))),
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    };

    let out = traverse_path_operator(ctx, &mut engine, &mut expr).unwrap();
    assert_eq!(out.matching_nodes.len(), 0);
}

fn path_nodes(path: &[ParsedKey]) -> Vec<treease_core::operators::TreeNode> {
    path.iter()
        .map(|segment| match segment {
            ParsedKey::Str(value) => *create_string_scalar_node(value).unwrap(),
            ParsedKey::Int(value) => *create_scalar_node_i64(*value).unwrap(),
        })
        .collect()
}

#[test]
fn create_value_operation_should_store_candidate_node() {
    let candidate = Box::new(TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: "!!int".to_string(),
        value: "12".to_string(),
        ..TreeNode::default()
    });
    let operation = create_value_operation(candidate).unwrap();

    assert_eq!(operation.operation_type.name(), "value");
    assert!(operation.tree_node.is_some());
    let node = operation.tree_node.as_ref().unwrap();
    assert_eq!(node.tag, "!!int");
    assert_eq!(node.value, "12");
}

#[test]
fn traversal_builder_empty_path_returns_self_operation() {
    let expression = create_traversal_tree(&[], TraversePreferences::default(), false).unwrap();

    assert!(std::ptr::eq(
        expression.operation.operation_type,
        &SELF_REFERENCE_OP_TYPE
    ));
    assert!(expression.lhs.is_none());
    assert!(expression.rhs.is_none());
}

#[test]
fn traversal_builder_single_segment_applies_target_key_preferences() {
    let nodes = path_nodes(&[ParsedKey::Str("a".to_string())]);
    let expression = create_traversal_tree(
        &nodes,
        TraversePreferences {
            dont_follow_alias: true,
            ..TraversePreferences::default()
        },
        true,
    )
    .unwrap();

    assert!(std::ptr::eq(
        expression.operation.operation_type,
        &TRAVERSE_PATH_OP_TYPE
    ));
    match expression.operation.preferences.as_deref() {
        Some(OperationPreference::Traverse(pref)) => {
            assert!(pref.include_map_keys);
            assert!(pref.dont_include_map_values);
            assert!(pref.dont_follow_alias);
        }
        _ => panic!("expected traverse preferences"),
    }
}

#[test]
fn traversal_builder_multi_segment_chains_with_short_pipe() {
    let nodes = path_nodes(&[ParsedKey::Str("a".to_string()), ParsedKey::Int(1)]);
    let expression = create_traversal_tree(&nodes, TraversePreferences::default(), false).unwrap();

    assert!(std::ptr::eq(
        expression.operation.operation_type,
        &SHORT_PIPE_OP_TYPE
    ));
    assert!(expression.lhs.is_some());
    assert!(expression.rhs.is_some());
    assert!(std::ptr::eq(
        expression.lhs.as_ref().unwrap().operation.operation_type,
        &TRAVERSE_PATH_OP_TYPE
    ));
    assert!(std::ptr::eq(
        expression.rhs.as_ref().unwrap().operation.operation_type,
        &TRAVERSE_PATH_OP_TYPE
    ));
}

#[test]
fn traversal_builder_target_key_only_affects_last_segment() {
    let nodes = path_nodes(&[
        ParsedKey::Str("a".to_string()),
        ParsedKey::Str("b".to_string()),
    ]);
    let expression = create_traversal_tree(&nodes, TraversePreferences::default(), true).unwrap();
    let left = expression.lhs.as_ref().unwrap();
    let right = expression.rhs.as_ref().unwrap();

    match left.operation.preferences.as_deref() {
        Some(OperationPreference::Traverse(pref)) => {
            assert!(!pref.include_map_keys);
            assert!(!pref.dont_include_map_values);
        }
        _ => panic!("expected left traverse preferences"),
    }
    match right.operation.preferences.as_deref() {
        Some(OperationPreference::Traverse(pref)) => {
            assert!(pref.include_map_keys);
            assert!(pref.dont_include_map_values);
        }
        _ => panic!("expected right traverse preferences"),
    }
}

#[test]
fn traversal_builder_supports_integer_path_segments() {
    let nodes = path_nodes(&[ParsedKey::Int(2)]);
    let expression = create_traversal_tree(&nodes, TraversePreferences::default(), false).unwrap();

    assert!(std::ptr::eq(
        expression.operation.operation_type,
        &TRAVERSE_PATH_OP_TYPE
    ));
    assert_eq!(expression.operation.string_value, "2");
}

#[test]
fn simple_traversal_builder_matches_zig_short_pipe_shape() {
    let expression = build_traversal_expression(&[
        ParsedKey::Str("a".to_string()),
        ParsedKey::Str("b".to_string()),
    ]);

    assert_eq!(
        expression.operation.operation_type.id,
        treease_core::core::OperationId::ShortPipe
    );
    assert_eq!(expression.operation.string_value, "");
    let lhs = expression.lhs.as_ref().expect("lhs should exist");
    let rhs = expression.rhs.as_ref().expect("rhs should exist");
    assert_eq!(
        lhs.operation.operation_type.id,
        treease_core::core::OperationId::TraversePath
    );
    assert_eq!(lhs.operation.string_value, "a");
    assert!(lhs.lhs.is_none());
    assert!(lhs.rhs.is_none());
    assert_eq!(
        rhs.operation.operation_type.id,
        treease_core::core::OperationId::TraversePath
    );
    assert_eq!(rhs.operation.string_value, "b");
}

#[test]
fn simple_traversal_builder_applies_zig_default_traverse_preferences() {
    let expression = build_traversal_expression(&[ParsedKey::Str("name".to_string())]);

    match expression.operation.preferences.as_deref() {
        Some(treease_core::core::OperationPreferences::Traverse(pref)) => {
            assert!(!pref.include_map_keys);
            assert!(!pref.dont_include_map_values);
            assert!(!pref.dont_follow_alias);
        }
        _ => panic!("expected traverse preferences on traversal expression"),
    }
}

#[test]
fn recursive_descent_builder_matches_zig_pipeline_shape() {
    let expression = build_recursive_descent_expression(&[ParsedKey::Str("name".to_string())]);

    assert_eq!(
        expression.operation.operation_type.id,
        treease_core::core::OperationId::ShortPipe
    );
    assert_eq!(expression.operation.string_value, "");
    let lhs = expression.lhs.as_ref().expect("lhs should exist");
    let rhs = expression.rhs.as_ref().expect("rhs should exist");
    assert_eq!(
        lhs.operation.operation_type.id,
        treease_core::core::OperationId::RecursiveDescent
    );
    assert_eq!(
        rhs.operation.operation_type.id,
        treease_core::core::OperationId::TraversePath
    );
    assert_eq!(rhs.operation.string_value, "name");
}

#[test]
fn recursive_descent_builder_keeps_zig_recursive_preferences() {
    let expression = build_recursive_descent_expression(&[]);

    match expression.operation.preferences.as_deref() {
        Some(treease_core::core::OperationPreferences::RecursiveDescent(pref)) => {
            assert!(pref.traverse_preferences.dont_follow_alias);
            assert!(!pref.traverse_preferences.include_map_keys);
        }
        _ => panic!("expected recursive descent preferences"),
    }

    let piped_expression =
        build_recursive_descent_expression(&[ParsedKey::Str("name".to_string())]);
    let recursive = piped_expression.lhs.as_ref().expect("lhs should exist");
    match recursive.operation.preferences.as_deref() {
        Some(treease_core::core::OperationPreferences::RecursiveDescent(pref)) => {
            assert!(pref.traverse_preferences.dont_follow_alias);
            assert!(!pref.traverse_preferences.include_map_keys);
        }
        _ => panic!("expected recursive descent preferences on recursive node"),
    }
}

#[test]
fn short_pipe_pipeline_executes_nested_traversal_steps() {
    let mut bar = mk_mapping();
    add_kv(&mut bar, "thing", mk_str("bar_thing"));

    let mut root = mk_mapping();
    add_kv(&mut root, "bar", bar);

    let ctx = Context {
        matching_nodes: vec![root],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".bar | .thing")
        .expect("short pipe traversal should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Scalar);
    assert_eq!(out.matching_nodes[0].value, "bar_thing");
}

#[test]
fn pipe_operator_passes_lhs_results_into_rhs_like_zig_pipe_operator() {
    let mut nested = mk_mapping();
    add_kv(&mut nested, "target", mk_str("lhs"));

    let mut root = mk_mapping();
    add_kv(&mut root, "outer", nested);

    let ctx = Context {
        matching_nodes: vec![root],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let mut expr = ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &PIPE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: Some(Box::new(mk_traverse_expr("outer"))),
        rhs: Some(Box::new(mk_traverse_expr("target"))),
    };

    let out = pipe_operator(ctx, &mut engine, &mut expr).expect("pipe operator should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "lhs");
}
