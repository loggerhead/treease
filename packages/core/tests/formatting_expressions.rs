use std::io::Cursor;

use treease_core::core::{
    CodecService, CompactTag, CoreError, EvalError, NodeId, SemType, TreeNode, TreeNodeKind,
    TreeStore, io_adapters::reader_from_pointer,
};
use treease_core::evaluator::{AllAtOnceEvaluator, ReaderInput, Value};
use treease_core::parser::parse_expression;

fn missing_tree_node() -> CoreError {
    CoreError::Eval(EvalError::MissingTreeNode)
}

fn attach_child(store: &mut TreeStore, parent: NodeId, child: NodeId) -> Result<(), CoreError> {
    let sequence_index = {
        let parent_node = store.get(parent).ok_or_else(missing_tree_node)?;
        if parent_node.kind == TreeNodeKind::Sequence {
            Some(parent_node.content.len() as i64)
        } else {
            None
        }
    };

    let child_node = store.get_mut(child).ok_or_else(missing_tree_node)?;
    child_node.parent = Some(parent);
    child_node.is_map_key = false;
    child_node.set_sequence_index(sequence_index.map(|index| index as u32));

    store
        .get_mut(parent)
        .ok_or_else(missing_tree_node)?
        .content
        .push(child);
    Ok(())
}

fn attach_map_entry(
    store: &mut TreeStore,
    parent: NodeId,
    key: NodeId,
    value: NodeId,
) -> Result<(), CoreError> {
    let key_node = store.get_mut(key).ok_or_else(missing_tree_node)?;
    key_node.parent = Some(parent);
    key_node.is_map_key = true;
    key_node.set_sequence_index(None);

    let value_node = store.get_mut(value).ok_or_else(missing_tree_node)?;
    value_node.parent = Some(parent);
    value_node.is_map_key = false;
    value_node.set_key(Some(key));
    value_node.set_sequence_index(None);

    let parent_node = store.get_mut(parent).ok_or_else(missing_tree_node)?;
    parent_node.content.push(key);
    parent_node.content.push(value);
    Ok(())
}

fn add_value(store: &mut TreeStore, value: &Value) -> Result<NodeId, CoreError> {
    match value {
        Value::Null => Ok(store.add(TreeNode::scalar(SemType::Nil, ""))),
        Value::Bool(value) => Ok(store.add(TreeNode::scalar(SemType::Boolean, value.to_string()))),
        Value::Number(value) if value.fract() == 0.0 => {
            Ok(store.add(TreeNode::scalar(SemType::Int, format!("{value:.0}"))))
        }
        Value::Number(value) => Ok(store.add(TreeNode::scalar(SemType::Float, value.to_string()))),
        Value::String(value) => Ok(store.add(TreeNode {
            kind: TreeNodeKind::Scalar,
            value: value.clone().into(),
            ..TreeNode::default()
        })),
        Value::Array(values) => {
            let parent = store.add(TreeNode {
                kind: TreeNodeKind::Sequence,
                sem_type: Some(SemType::Seq),
                tag: CompactTag::from_sem_type(SemType::Seq),
                ..TreeNode::default()
            });
            for value in values {
                let child = add_value(store, value)?;
                attach_child(store, parent, child)?;
            }
            Ok(parent)
        }
        Value::Object(values) => {
            let parent = store.add(TreeNode {
                kind: TreeNodeKind::Mapping,
                sem_type: Some(SemType::Map),
                tag: CompactTag::from_sem_type(SemType::Map),
                ..TreeNode::default()
            });
            for (key, value) in values {
                let key_id = store.add(TreeNode::scalar(SemType::Str, key.clone()));
                let value_id = add_value(store, value)?;
                attach_map_entry(store, parent, key_id, value_id)?;
            }
            Ok(parent)
        }
    }
}

fn evaluate_to_yaml(input: &str, expression_source: &str) -> String {
    let _expression = parse_expression(expression_source)
        .expect("parse should succeed")
        .expect("tree should exist");
    let mut reader = Cursor::new(input.as_bytes());
    let _inputs = [ReaderInput::new(
        "in.yaml",
        reader_from_pointer(&mut reader),
    )];
    let decoded = CodecService::new()
        .decode("yaml", input)
        .expect("yaml decode should succeed");
    let results = AllAtOnceEvaluator::new()
        .evaluate_nodes(&decoded.store, expression_source, &[decoded.root])
        .expect("formatting expression should evaluate");

    assert_eq!(results.len(), 1);
    let mut store = TreeStore::new();
    let root = add_value(&mut store, &results[0]).expect("value should convert to tree");
    CodecService::new()
        .encode_to_string("yaml", &store, root)
        .expect("yaml encoding should succeed")
}

#[test]
fn formatting_expression_handles_expression_files_and_comments() {
    let output = evaluate_to_yaml(
        "a:\n  b: old",
        "#! core\n\n# This is an expression that updates the map\n# for several great reasons outlined here.\n\n.a.b = \"new\" # line comment here\n| .a.c = \"frog\"\n\n# Now good things will happen.\n",
    );

    assert_eq!(output, "a:\n  b: new\n  c: frog\n");
}

#[test]
fn formatting_expression_ignores_shebang_flags_for_parser_semantics() {
    let output = evaluate_to_yaml(
        "a:\n  b: old",
        "#! core -oj\n\n# This is an expression that updates the map\n# for several great reasons outlined here.\n\n.a.b = \"new\" # line comment here\n| .a.c = \"frog\"\n\n# Now good things will happen.\n",
    );

    assert_eq!(output, "a:\n  b: new\n  c: frog\n");
}

#[test]
fn formatting_expression_ignores_commented_out_pipeline_steps() {
    let output = evaluate_to_yaml(
        "a:\n  b: old",
        "#! core\n# This is an expression that updates the map\n# for several great reasons outlined here.\n\n.a.b = \"new\" # line comment here\n# | .a.c = \"frog\"\n\n# Now good things will happen.\n",
    );

    assert_eq!(output, "a:\n  b: new\n");
}
