use treease_core::operators::contains::contains_value;
use treease_core::operators::{NodeKind, SemType, TreeNode};

fn string_scalar(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Str),
        tag: "string".to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn int_scalar(value: i64) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Int),
        tag: "integer".to_owned(),
        value: value.to_string(),
        ..TreeNode::default()
    }
}

fn null_scalar() -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Nil),
        tag: "null".to_owned(),
        ..TreeNode::default()
    }
}

fn sequence(items: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        kind: NodeKind::Sequence,
        sem_type: Some(SemType::Seq),
        tag: "array".to_owned(),
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
        tag: "object".to_owned(),
        content,
        ..TreeNode::default()
    }
}

#[test]
fn contains_handles_null_integers_and_strings() {
    assert!(contains_value(&null_scalar(), &null_scalar()).unwrap());
    assert!(contains_value(&int_scalar(3), &int_scalar(3)).unwrap());
    assert!(!contains_value(&int_scalar(3), &int_scalar(32)).unwrap());
    assert!(contains_value(&string_scalar("foobar"), &string_scalar("bar")).unwrap());
    assert!(contains_value(&string_scalar("meow"), &string_scalar("meow")).unwrap());
}

#[test]
fn contains_checks_array_subset_and_missing_values() {
    let lhs = sequence(vec![
        string_scalar("foobar"),
        string_scalar("foobaz"),
        string_scalar("blarp"),
    ]);
    let rhs = sequence(vec![string_scalar("baz"), string_scalar("bar")]);
    let missing = sequence(vec![string_scalar("camel")]);

    assert!(contains_value(&lhs, &rhs).unwrap());
    assert!(!contains_value(&lhs, &missing).unwrap());
}

#[test]
fn contains_checks_nested_object_membership() {
    let lhs = mapping(vec![
        ("foo", int_scalar(12)),
        (
            "bar",
            sequence(vec![
                int_scalar(1),
                int_scalar(2),
                mapping(vec![("barp", int_scalar(12)), ("blip", int_scalar(13))]),
            ]),
        ),
    ]);
    let rhs = mapping(vec![(
        "bar",
        sequence(vec![mapping(vec![("barp", int_scalar(12))])]),
    )]);
    let not_included = mapping(vec![
        ("foo", int_scalar(12)),
        (
            "bar",
            sequence(vec![mapping(vec![("barp", int_scalar(15))])]),
        ),
    ]);

    assert!(contains_value(&lhs, &rhs).unwrap());
    assert!(!contains_value(&lhs, &not_included).unwrap());
}
