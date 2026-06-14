use treease_core::core::{
    LiteralStyle, SemType, TreeNode, format_buffer_literal, format_json_string, format_literal,
    format_python_string,
};

#[test]
fn literal_format_formats_json_and_python_scalars() {
    assert_eq!(
        format_json_string("a\"\n\t\\\u{0008}\u{000c}"),
        "\"a\\\"\\n\\t\\\\\\b\\f\""
    );
    assert_eq!(
        format_python_string("a'b\\\n\u{0008}\u{000c}"),
        "'a\\'b\\\\\\n\\b\\f'"
    );

    let null_node = TreeNode::scalar(SemType::Nil, "");
    assert_eq!(
        format_literal(&null_node, LiteralStyle::Json).unwrap(),
        "null"
    );
    assert_eq!(
        format_literal(&null_node, LiteralStyle::Python).unwrap(),
        "None"
    );

    let bool_node = TreeNode::scalar(SemType::Boolean, "true");
    assert_eq!(
        format_literal(&bool_node, LiteralStyle::Json).unwrap(),
        "true"
    );
    assert_eq!(
        format_literal(&bool_node, LiteralStyle::Python).unwrap(),
        "True"
    );

    let int_node = TreeNode::scalar(SemType::Int, "12");
    let float_node = TreeNode::scalar(SemType::Float, "3.5");
    assert_eq!(format_literal(&int_node, LiteralStyle::Json).unwrap(), "12");
    assert_eq!(
        format_literal(&float_node, LiteralStyle::Json).unwrap(),
        "3.5"
    );
}

#[test]
fn literal_format_formats_strings_and_buffers() {
    let str_node = TreeNode::scalar(SemType::Str, "cat");
    assert_eq!(
        format_literal(&str_node, LiteralStyle::Json).unwrap(),
        "\"cat\""
    );
    assert_eq!(
        format_literal(&str_node, LiteralStyle::Python).unwrap(),
        "'cat'"
    );
    assert_eq!(
        format_literal(&str_node, LiteralStyle::Buffer).unwrap(),
        "0x636174"
    );

    assert_eq!(format_buffer_literal(b"\x00\xff"), "0x00ff");
}
