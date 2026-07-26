use std::collections::BTreeMap;

use treease_core::evaluator::{Numeric, Value};
use treease_core::expression_pipeline;
use treease_core::operators::compact::compact_operator;
use treease_core::operators::{
    COMPACT_OP_TYPE, Context, ExpressionNode, NodeKind, Operation, SemType, TreeEngine, TreeNode,
};

fn scalar(sem_type: SemType, value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(sem_type),
        tag: sem_type.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn empty_container(kind: NodeKind, sem_type: SemType) -> TreeNode {
    TreeNode {
        kind,
        sem_type: Some(sem_type),
        tag: sem_type.tag().to_owned(),
        ..TreeNode::default()
    }
}

fn compact_expression() -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &COMPACT_OP_TYPE,
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
fn compact_operator_removes_zero_mapping_values_recursively() {
    let mut nested = empty_container(NodeKind::Mapping, SemType::Map);
    nested.content.push(scalar(SemType::Str, "nested"));
    nested.content.push(scalar(SemType::Int, "0"));

    let node = TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.tag().to_owned(),
        content: vec![
            scalar(SemType::Str, "zero"),
            scalar(SemType::Int, "0"),
            scalar(SemType::Str, "nested"),
            nested,
            scalar(SemType::Str, "kept"),
            scalar(SemType::Float, "1.5"),
        ],
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![node],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expression = compact_expression();
    let out = compact_operator(ctx, &mut engine, &mut expression).unwrap();

    assert_eq!(out.matching_nodes[0].content.len(), 2);
    assert_eq!(out.matching_nodes[0].content[0].value, "kept");
}

#[test]
fn compact_operator_removes_zero_sequence_values_and_reindexes_items() {
    let node = TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: SemType::Seq.tag().to_owned(),
        content: vec![
            scalar(SemType::Nil, ""),
            scalar(SemType::Boolean, "false"),
            scalar(SemType::Int, "0"),
            scalar(SemType::Float, "0.0"),
            scalar(SemType::Str, ""),
            scalar(SemType::Str, "kept"),
            empty_container(NodeKind::Sequence, SemType::Seq),
            empty_container(NodeKind::Mapping, SemType::Map),
            scalar(SemType::Float, "-2.5"),
        ],
        ..TreeNode::default()
    };
    let ctx = Context {
        matching_nodes: vec![node],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expression = compact_expression();
    let out = compact_operator(ctx, &mut engine, &mut expression).unwrap();
    let sequence = &out.matching_nodes[0];

    assert_eq!(sequence.content.len(), 2);
    assert_eq!(sequence.content[0].value, "kept");
    assert_eq!(sequence.content[0].sequence_index, Some(0));
    assert_eq!(sequence.content[1].value, "-2.5");
    assert_eq!(sequence.content[1].sequence_index, Some(1));
}

#[test]
fn compact_operator_keeps_non_container_inputs_unchanged() {
    let node = scalar(SemType::Int, "0");
    let ctx = Context {
        matching_nodes: vec![node.clone()],
        ..Context::default()
    };

    let mut engine = TreeEngine::default();
    let mut expression = compact_expression();
    let out = compact_operator(ctx, &mut engine, &mut expression).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, node.kind);
    assert_eq!(out.matching_nodes[0].sem_type, node.sem_type);
    assert_eq!(out.matching_nodes[0].value, node.value);
}

#[test]
fn compact_expression_is_registered_and_evaluates_mappings_and_arrays() {
    let input = Value::Object(BTreeMap::from([
        ("null".to_owned(), Value::Null),
        ("false".to_owned(), Value::Bool(false)),
        ("empty_array".to_owned(), Value::Array(Vec::new())),
        ("empty_object".to_owned(), Value::Object(BTreeMap::new())),
        ("zero".to_owned(), Value::Number(Numeric::Float(0.0))),
        ("empty_string".to_owned(), Value::String(String::new())),
        (
            "items".to_owned(),
            Value::Array(vec![
                Value::Null,
                Value::Bool(false),
                Value::String(String::new()),
                Value::Number(Numeric::Float(2.0)),
            ]),
        ),
    ]));

    let output = expression_pipeline::evaluate(&input, "compact").unwrap();
    let expected = Value::Object(BTreeMap::from([(
        "items".to_owned(),
        Value::Array(vec![Value::Number(Numeric::Float(2.0))]),
    )]));

    assert_eq!(output, expected);
}
