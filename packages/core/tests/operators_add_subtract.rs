use treease_core::operators::add::add_with_nodes;
use treease_core::operators::subtract::subtract_with_nodes;
use treease_core::operators::{CoreError, EvalError, NodeKind, SemType, TreeNode};

fn string_scalar(value: &str) -> TreeNode {
    TreeNode::scalar(SemType::Str, value)
}

fn int_scalar(value: i64) -> TreeNode {
    TreeNode::scalar(SemType::Int, value.to_string())
}

fn float_scalar(value: &str) -> TreeNode {
    TreeNode::scalar(SemType::Float, value)
}

fn null_scalar() -> TreeNode {
    TreeNode::scalar(SemType::Nil, "")
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

#[test]
fn add_with_nodes_supports_scalars_sequences_maps_and_null_lhs() {
    let string_sum = add_with_nodes(Some(&string_scalar("foo")), Some(&string_scalar("bar")))
        .unwrap()
        .unwrap();
    assert_eq!(string_sum.tag, "!!str");
    assert_eq!(string_sum.value, "foobar");

    let int_sum = add_with_nodes(Some(&int_scalar(3)), Some(&int_scalar(4)))
        .unwrap()
        .unwrap();
    assert_eq!(int_sum.tag, "!!int");
    assert_eq!(int_sum.value, "7");

    let float_sum = add_with_nodes(Some(&int_scalar(3)), Some(&float_scalar("2.5")))
        .unwrap()
        .unwrap();
    assert_eq!(float_sum.tag, "!!float");
    assert_eq!(float_sum.value, "5.5");

    let merged_seq = add_with_nodes(
        Some(&sequence(vec![int_scalar(1), int_scalar(2)])),
        Some(&sequence(vec![int_scalar(3)])),
    )
    .unwrap()
    .unwrap();
    assert_eq!(merged_seq.kind, NodeKind::Sequence);
    assert_eq!(merged_seq.content.len(), 3);
    assert_eq!(merged_seq.content[2].value, "3");

    let unchanged_seq = add_with_nodes(
        Some(&sequence(vec![int_scalar(1), int_scalar(2)])),
        Some(&null_scalar()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(unchanged_seq.content.len(), 2);

    let merged_map = add_with_nodes(
        Some(&mapping(vec![("a", int_scalar(1))])),
        Some(&mapping(vec![("a", int_scalar(2)), ("b", int_scalar(3))])),
    )
    .unwrap()
    .unwrap();
    assert_eq!(merged_map.kind, NodeKind::Mapping);
    assert_eq!(merged_map.content.len(), 4);
    assert_eq!(merged_map.content[0].value, "a");
    assert_eq!(merged_map.content[1].value, "2");
    assert_eq!(merged_map.content[2].value, "b");
    assert_eq!(merged_map.content[3].value, "3");

    let replaced = add_with_nodes(Some(&null_scalar()), Some(&int_scalar(1)))
        .unwrap()
        .unwrap();
    assert_eq!(replaced.tag, "!!int");
    assert_eq!(replaced.value, "1");
}

#[test]
fn subtract_with_nodes_supports_scalars_sequences_and_null_lhs() {
    let int_result = subtract_with_nodes(&int_scalar(10), &int_scalar(3)).unwrap();
    assert_eq!(int_result.value, "7");
    assert_eq!(int_result.tag, "!!int");

    let float_result = subtract_with_nodes(&float_scalar("5.5"), &int_scalar(2)).unwrap();
    assert_eq!(float_result.tag, "!!float");
    assert_eq!(float_result.value, "3.5");

    let seq_result = subtract_with_nodes(
        &sequence(vec![
            string_scalar("a"),
            string_scalar("b"),
            string_scalar("a"),
        ]),
        &sequence(vec![string_scalar("a")]),
    )
    .unwrap();
    assert_eq!(seq_result.kind, NodeKind::Sequence);
    assert_eq!(seq_result.content.len(), 1);
    assert_eq!(seq_result.content[0].value, "b");

    let replaced = subtract_with_nodes(&null_scalar(), &int_scalar(1)).unwrap();
    assert_eq!(replaced.value, "1");
    assert_eq!(replaced.tag, "!!int");
}

#[test]
fn subtract_with_nodes_reports_expected_errors() {
    let string_err = subtract_with_nodes(&string_scalar("cat"), &string_scalar("dog")).unwrap_err();
    assert!(matches!(
        string_err,
        CoreError::Eval(EvalError::StringsCannotBeSubtracted)
    ));

    let map_err =
        subtract_with_nodes(&mapping(vec![("a", int_scalar(1))]), &mapping(vec![])).unwrap_err();
    assert!(matches!(
        map_err,
        CoreError::Eval(EvalError::MapsNotSupportedForSubtraction)
    ));

    let seq_err = subtract_with_nodes(&sequence(vec![int_scalar(1)]), &int_scalar(1)).unwrap_err();
    assert!(matches!(
        seq_err,
        CoreError::Eval(EvalError::CannotSubtractNonSequence)
    ));
}
