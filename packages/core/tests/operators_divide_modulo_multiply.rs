use treease_core::operators::divide::{divide_scalars, divide_with_nodes};
use treease_core::operators::modulo::modulo_with_nodes;
use treease_core::operators::multiply::multiply_with_nodes;
use treease_core::operators::{CoreError, EvalError, NodeKind, SemType, TreeNode};

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

fn float_scalar(value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Float),
        tag: SemType::Float.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn custom_scalar(tag: &str, value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: None,
        tag: tag.to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn null_scalar() -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(SemType::Nil),
        tag: SemType::Nil.tag().to_owned(),
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

#[test]
fn divide_with_nodes_supports_split_numbers_custom_tags_and_errors() {
    let split = divide_with_nodes(&string_scalar("cat_meow"), &string_scalar("_")).unwrap();
    assert_eq!(split.kind, NodeKind::Sequence);
    assert_eq!(split.tag, "!!seq");
    assert_eq!(split.content.len(), 2);
    assert_eq!(split.content[0].value, "cat");
    assert_eq!(split.content[1].value, "meow");

    let exact_quotient = divide_with_nodes(&int_scalar(12), &int_scalar(2)).unwrap();
    assert_eq!(exact_quotient.kind, NodeKind::Scalar);
    assert_eq!(exact_quotient.tag, "!!int");
    assert_eq!(exact_quotient.value, "6");

    let fractional_quotient = divide_with_nodes(&int_scalar(12), &int_scalar(5)).unwrap();
    assert_eq!(fractional_quotient.tag, "!!float");
    assert_eq!(fractional_quotient.value, "2.4");

    let quotient = divide_with_nodes(&int_scalar(12), &float_scalar("2.5")).unwrap();
    assert_eq!(quotient.kind, NodeKind::Scalar);
    assert_eq!(quotient.tag, "!!float");
    assert_eq!(quotient.value, "4.8");

    let custom_split = divide_with_nodes(
        &custom_scalar("!horse", "cat_meow"),
        &custom_scalar("!goat", "_"),
    )
    .unwrap();
    assert_eq!(custom_split.kind, NodeKind::Sequence);
    assert_eq!(custom_split.tag, "!!seq");
    assert_eq!(custom_split.content.len(), 2);

    let err = divide_with_nodes(&sequence(vec![int_scalar(1)]), &int_scalar(2)).unwrap_err();
    assert!(matches!(
        err,
        CoreError::Eval(EvalError::CannotDivideNonScalars)
    ));
}

#[test]
fn divide_scalars_handles_zero_and_type_mismatch() {
    let lhs = float_scalar("1");
    let rhs = float_scalar("0");
    let mut pos_target = lhs.copy_without_content().unwrap();
    divide_scalars(&mut pos_target, &lhs, &rhs).unwrap();
    assert_eq!(pos_target.tag, "!!float");
    assert_eq!(pos_target.value, "+Inf");

    let neg_lhs = float_scalar("-1");
    let mut neg_target = neg_lhs.copy_without_content().unwrap();
    divide_scalars(&mut neg_target, &neg_lhs, &rhs).unwrap();
    assert_eq!(neg_target.value, "-Inf");

    let mut type_target = int_scalar(123).copy_without_content().unwrap();
    let err = divide_scalars(&mut type_target, &int_scalar(123), &string_scalar("2")).unwrap_err();
    assert!(matches!(err, CoreError::Eval(EvalError::CannotDivideTypes)));
}

#[test]
fn modulo_with_nodes_supports_integer_and_float_modulo() {
    let int_mod = modulo_with_nodes(&int_scalar(10), &int_scalar(3)).unwrap();
    assert_eq!(int_mod.value, "1");
    assert_eq!(int_mod.tag, "!!int");

    let float_mod = modulo_with_nodes(&float_scalar("5.5"), &float_scalar("2")).unwrap();
    assert_eq!(float_mod.value, "1.5");
    assert_eq!(float_mod.tag, "!!float");

    let custom_mod = modulo_with_nodes(
        &custom_scalar("!horse", "5.5"),
        &custom_scalar("!goat", "2"),
    )
    .unwrap();
    assert_eq!(custom_mod.value, "1.5");
    assert_eq!(custom_mod.tag, "!horse");
}

#[test]
fn modulo_with_nodes_reports_expected_errors() {
    let zero_err = modulo_with_nodes(&int_scalar(1), &int_scalar(0)).unwrap_err();
    assert!(matches!(
        zero_err,
        CoreError::Eval(EvalError::CannotModuloByZero)
    ));

    let non_scalar_err =
        modulo_with_nodes(&sequence(vec![int_scalar(1)]), &int_scalar(1)).unwrap_err();
    assert!(matches!(
        non_scalar_err,
        CoreError::Eval(EvalError::CannotModuloNonScalars)
    ));

    let null_err = modulo_with_nodes(&null_scalar(), &int_scalar(1)).unwrap_err();
    assert!(matches!(
        null_err,
        CoreError::Eval(EvalError::CannotModuloNull)
    ));
}

#[test]
fn multiply_with_nodes_supports_numbers_strings_sequences_and_maps() {
    let int_product = multiply_with_nodes(&int_scalar(2), &int_scalar(3)).unwrap();
    assert_eq!(int_product.tag, "!!int");
    assert_eq!(int_product.value, "6");

    let float_product = multiply_with_nodes(&float_scalar("2.5"), &int_scalar(2)).unwrap();
    assert_eq!(float_product.tag, "!!float");
    assert_eq!(float_product.value, "5");

    let custom_product = multiply_with_nodes(
        &custom_scalar("!horse", "2.5"),
        &custom_scalar("!goat", "2"),
    )
    .unwrap();
    assert_eq!(custom_product.tag, "!horse");
    assert_eq!(custom_product.value, "5");

    let repeated = multiply_with_nodes(&string_scalar("ab"), &int_scalar(3)).unwrap();
    assert_eq!(repeated.tag, "!!str");
    assert_eq!(repeated.value, "ababab");

    let merged_seq = multiply_with_nodes(
        &sequence(vec![int_scalar(1), int_scalar(2)]),
        &sequence(vec![int_scalar(3)]),
    )
    .unwrap();
    assert_eq!(merged_seq.kind, NodeKind::Sequence);
    assert_eq!(merged_seq.content.len(), 3);
    assert_eq!(merged_seq.content[2].value, "3");

    let merged_map = multiply_with_nodes(
        &mapping(vec![("a", int_scalar(1))]),
        &mapping(vec![("a", int_scalar(2)), ("b", int_scalar(3))]),
    )
    .unwrap();
    assert_eq!(merged_map.kind, NodeKind::Mapping);
    assert_eq!(merged_map.content.len(), 4);
    assert_eq!(merged_map.content[1].value, "2");
    assert_eq!(merged_map.content[3].value, "3");
}

#[test]
fn multiply_with_nodes_reports_repeat_and_type_errors() {
    let negative = multiply_with_nodes(&string_scalar("ab"), &int_scalar(-1)).unwrap_err();
    assert!(matches!(
        negative,
        CoreError::Eval(EvalError::NegativeRepeat)
    ));

    let type_err = multiply_with_nodes(&string_scalar("ab"), &mapping(vec![("a", int_scalar(1))]))
        .unwrap_err();
    assert!(matches!(
        type_err,
        CoreError::Eval(EvalError::CannotMultiplyTypes)
    ));
}
