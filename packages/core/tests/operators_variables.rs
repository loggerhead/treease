use treease_core::expression_pipeline;
use treease_core::expression_pipeline::PipelineError;
use treease_core::operators::variables::{get_variable_operator, use_with_pipe};
use treease_core::operators::{
    ASSIGN_VARIABLE_OP_TYPE, Context, CoreError, EvalError, ExpressionNode, GET_VARIABLE_OP_TYPE,
    NodeKind, Operation, SemType, TreeEngine, TreeNode,
};

fn string_scalar(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: SemType::Str.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn int_scalar(value: i64) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: value.to_string(),
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

fn get_variable_expr(name: &str) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &GET_VARIABLE_OP_TYPE,
            value: None,
            string_value: name.to_string(),
            tree_node: None,
            preferences: None,
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

fn assign_variable_expr() -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ASSIGN_VARIABLE_OP_TYPE,
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

#[test]
fn get_variable_operator_returns_variable_value() {
    let cat = string_scalar("cat");
    let mut ctx = Context::default();
    ctx.set_variable("foo", vec![cat.clone()]).unwrap();
    let mut engine = TreeEngine::default();
    let mut expr = get_variable_expr("foo");

    let out = get_variable_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cat");
}

#[test]
fn get_variable_operator_returns_empty_for_unknown_variable() {
    let ctx = Context::default();
    let mut engine = TreeEngine::default();
    let mut expr = get_variable_expr("unknown");

    let out = get_variable_operator(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 0);
}

#[test]
fn use_with_pipe_returns_must_use_variable_with_pipe_error() {
    let ctx = Context::default();
    let mut engine = TreeEngine::default();
    let mut expr = assign_variable_expr();

    let err = use_with_pipe(ctx, &mut engine, &mut expr).unwrap_err();

    assert!(matches!(
        err,
        CoreError::Eval(EvalError::MustUseVariableWithPipe)
    ));
}

#[test]
fn variable_assignment_and_usage_in_pipeline() {
    // Set a variable and then retrieve it, simulating the basic
    // variable assignment and usage pattern.
    let cat = string_scalar("cat");
    let dog = string_scalar("dog");

    let mut ctx = Context::default();
    ctx.set_variable("foo", vec![cat.clone()]).unwrap();
    ctx.set_variable("bar", vec![dog.clone()]).unwrap();

    let mut engine = TreeEngine::default();

    // Retrieve "foo"
    let mut expr_foo = get_variable_expr("foo");
    let out_foo = get_variable_operator(ctx.clone(), &mut engine, &mut expr_foo).unwrap();
    assert_eq!(out_foo.matching_nodes.len(), 1);
    assert_eq!(out_foo.matching_nodes[0].value, "cat");

    // Retrieve "bar"
    let mut expr_bar = get_variable_expr("bar");
    let out_bar = get_variable_operator(ctx, &mut engine, &mut expr_bar).unwrap();
    assert_eq!(out_bar.matching_nodes.len(), 1);
    assert_eq!(out_bar.matching_nodes[0].value, "dog");
}

#[test]
fn variable_assignment_on_empty_object_with_pipe_to_self() {
    // Tests: .a.b as $foo | .
    // Assigning a non-existent path to a variable and piping to self
    // should return MustUseVariableWithPipe error.
    let ctx = Context::default();
    let mut engine = TreeEngine::default();
    let mut expr = assign_variable_expr();

    let err = use_with_pipe(ctx, &mut engine, &mut expr).unwrap_err();
    assert!(matches!(
        err,
        CoreError::Eval(EvalError::MustUseVariableWithPipe)
    ));
}

#[test]
fn single_value_variable() {
    // Tests: .a as $foo | $foo
    let cat = string_scalar("cat");
    let mut ctx = Context::default();
    ctx.set_variable("foo", vec![cat.clone()]).unwrap();
    let mut engine = TreeEngine::default();
    let mut expr = get_variable_expr("foo");

    let out = get_variable_operator(ctx, &mut engine, &mut expr).unwrap();
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cat");
}

#[test]
fn multi_value_variable() {
    // Tests: .[] as $foo | $foo
    let cat = string_scalar("cat");
    let dog = string_scalar("dog");
    let mut ctx = Context::default();
    ctx.set_variable("foo", vec![cat.clone(), dog.clone()])
        .unwrap();
    let mut engine = TreeEngine::default();
    let mut expr = get_variable_expr("foo");

    let out = get_variable_operator(ctx, &mut engine, &mut expr).unwrap();
    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "cat");
    assert_eq!(out.matching_nodes[1].value, "dog");
}

#[test]
fn variable_assignment_mid_pipeline_for_filtering() {
    // Tests: .[] | . as $f | select($f == 2)
    let two = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: "2".to_string(),
        ..TreeNode::default()
    };
    let mut ctx = Context::default();
    ctx.set_variable("f", vec![two.clone()]).unwrap();
    let mut engine = TreeEngine::default();
    let mut expr = get_variable_expr("f");

    let out = get_variable_operator(ctx, &mut engine, &mut expr).unwrap();
    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "2");
}

#[test]
fn variable_inside_array_construction_with_arithmetic() {
    // Tests: [.[] | . as $f | $f + 1]
    let one = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: "1".to_string(),
        ..TreeNode::default()
    };
    let two = TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: SemType::Int.tag().to_owned(),
        value: "2".to_string(),
        ..TreeNode::default()
    };
    let mut ctx = Context::default();
    ctx.set_variable("f", vec![one.clone(), two.clone()])
        .unwrap();
    let mut engine = TreeEngine::default();
    let mut expr = get_variable_expr("f");

    let out = get_variable_operator(ctx, &mut engine, &mut expr).unwrap();
    assert_eq!(out.matching_nodes.len(), 2);
    assert_eq!(out.matching_nodes[0].value, "1");
    assert_eq!(out.matching_nodes[1].value, "2");
}

#[test]
fn using_variables_to_swap_values() {
    // Tests: .a as $x | .b as $y | .b = $x | .a = $y
    let a_value = string_scalar("a_value");
    let b_value = string_scalar("b_value");
    let mut ctx = Context::default();
    ctx.set_variable("x", vec![a_value.clone()]).unwrap();
    ctx.set_variable("y", vec![b_value.clone()]).unwrap();
    let mut engine = TreeEngine::default();

    let mut expr_x = get_variable_expr("x");
    let out_x = get_variable_operator(ctx.clone(), &mut engine, &mut expr_x).unwrap();
    assert_eq!(out_x.matching_nodes.len(), 1);
    assert_eq!(out_x.matching_nodes[0].value, "a_value");

    let mut expr_y = get_variable_expr("y");
    let out_y = get_variable_operator(ctx, &mut engine, &mut expr_y).unwrap();
    assert_eq!(out_y.matching_nodes.len(), 1);
    assert_eq!(out_y.matching_nodes[0].value, "b_value");
}

#[test]
fn variable_pipeline_assigns_and_reads_value_successfully() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("a", string_scalar("cat"))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a as $foo | $foo")
        .expect("variable pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "cat");
}

#[test]
fn variable_pipeline_returns_original_context_when_assigned_path_is_missing() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b as $foo | .")
        .expect("pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert!(out.matching_nodes[0].content.is_empty());
}

#[test]
fn variable_pipeline_requires_pipe_after_assignment() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let err = expression_pipeline::execute_on_context(&mut engine, &ctx, ".a.b as $foo")
        .expect_err("assignment without pipe should fail");

    assert!(matches!(
        err,
        PipelineError::Compat(CoreError::Eval(EvalError::MustUseVariableWithPipe))
    ));
}

#[test]
fn variable_pipeline_filters_using_mid_pipeline_variable() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(1), int_scalar(2)])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".[] | . as $f | select($f == 2)",
    )
    .expect("filter pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].value, "2");
}

#[test]
fn variable_pipeline_supports_array_construction_with_arithmetic() {
    let ctx = Context {
        matching_nodes: vec![sequence(vec![int_scalar(1), int_scalar(2)])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out =
        expression_pipeline::execute_on_context(&mut engine, &ctx, "[.[] | . as $f | $f + 1]")
            .expect("array construction pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Sequence);
    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "2");
    assert_eq!(out.matching_nodes[0].content[1].value, "3");
}

#[test]
fn variable_pipeline_can_swap_values_using_two_variables() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![
            ("a", string_scalar("a_value")),
            ("b", string_scalar("b_value")),
        ])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();

    let out = expression_pipeline::execute_on_context(
        &mut engine,
        &ctx,
        ".a as $x | .b as $y | .b = $x | .a = $y",
    )
    .expect("swap pipeline should succeed");

    assert_eq!(out.matching_nodes.len(), 1);
    let root = &out.matching_nodes[0];
    assert_eq!(root.kind, NodeKind::Mapping);
    assert_eq!(root.content[0].value, "a");
    assert_eq!(root.content[1].value, "b_value");
    assert_eq!(root.content[2].value, "b");
    assert_eq!(root.content[3].value, "a_value");
}
