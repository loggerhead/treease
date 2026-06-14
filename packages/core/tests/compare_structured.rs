use treease_core::compare::compare_texts_structured;

// ── compare_texts_structured ────────────────────────────────────────

#[test]
fn test_structured_equal_json_different_formatting_and_key_order() {
    let left = r#"{"a": 1, "b": 2}"#;
    let right = r#"{
        "b": 2,
        "a": 1
    }"#;

    let result = compare_texts_structured("json", left, right).unwrap();
    assert!(result, "structurally equal JSON should return true");
}

#[test]
fn test_structured_not_equal_json_different_values() {
    let left = r#"{"a": 1, "b": 2}"#;
    let right = r#"{"a": 1, "b": 99}"#;

    let result = compare_texts_structured("json", left, right).unwrap();
    assert!(!result, "structurally different JSON should return false");
}
