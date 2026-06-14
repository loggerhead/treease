use treease_core::operators::equals::is_equals_with_nodes;
use treease_core::operators::{NodeKind, SemType, TreeNode};

fn scalar(sem_type: SemType, value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(sem_type),
        tag: sem_type.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn int_scalar(value: i64) -> TreeNode {
    scalar(SemType::Int, &value.to_string())
}

fn string_scalar(value: &str) -> TreeNode {
    scalar(SemType::Str, value)
}

fn null_scalar() -> TreeNode {
    scalar(SemType::Nil, "null")
}

#[test]
fn equals_operator_matches_null_numbers_and_strings() {
    assert_eq!(
        is_equals_with_nodes(false, Some(&null_scalar()), Some(&null_scalar()))
            .unwrap()
            .value,
        "true"
    );
    assert_eq!(
        is_equals_with_nodes(false, Some(&int_scalar(3)), Some(&int_scalar(3)))
            .unwrap()
            .value,
        "true"
    );
    assert_eq!(
        is_equals_with_nodes(
            false,
            Some(&string_scalar("meow")),
            Some(&string_scalar("meow"))
        )
        .unwrap()
        .value,
        "true"
    );
}

#[test]
fn equals_nil_nil_returns_true_and_negate_flips_to_false() {
    // Zig: nil == nil
    assert_eq!(
        is_equals_with_nodes(false, None, None).unwrap().value,
        "true"
    );
    // Zig: nil != nil (negate=true) → false
    assert_eq!(
        is_equals_with_nodes(true, None, None).unwrap().value,
        "false"
    );
}

#[test]
fn equals_nil_rhs_null_returns_true() {
    // Zig: nil vs "!!null" node → true
    assert_eq!(
        is_equals_with_nodes(false, None, Some(&null_scalar()))
            .unwrap()
            .value,
        "true"
    );
}

#[test]
fn equals_wildcard_match_star_suffix() {
    // Zig: "cat" == "*at" → true
    assert_eq!(
        is_equals_with_nodes(
            false,
            Some(&string_scalar("cat")),
            Some(&string_scalar("*at"))
        )
        .unwrap()
        .value,
        "true"
    );
    // Zig: "dog" == "*at" → false
    assert_eq!(
        is_equals_with_nodes(
            false,
            Some(&string_scalar("dog")),
            Some(&string_scalar("*at"))
        )
        .unwrap()
        .value,
        "false"
    );
}

#[test]
fn not_equals_operator_flips_false_and_true_cases() {
    assert_eq!(
        is_equals_with_nodes(true, Some(&int_scalar(3)), Some(&int_scalar(32)))
            .unwrap()
            .value,
        "true"
    );
    assert_eq!(
        is_equals_with_nodes(
            true,
            Some(&string_scalar("cat")),
            Some(&string_scalar("cat"))
        )
        .unwrap()
        .value,
        "false"
    );
}
