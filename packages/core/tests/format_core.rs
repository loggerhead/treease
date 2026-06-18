use treease_core::core::{
    CoreError, FormatError, find_node_by_path, format_from_string, format_string_from_filename,
    path_seg_key,
};
use treease_core::formats::encoder_json::format_json_document_with_spans;
use treease_core::formats::preferences::FormatPreferences;
use treease_core::stream::{DecodeOptions, decode_to_document_with_options};

#[test]
fn format_from_string_supports_zig_aliases_and_errors() {
    assert_eq!(format_from_string("yaml").unwrap().formal_name, "yaml");
    assert_eq!(format_from_string("y").unwrap().formal_name, "yaml");
    assert_eq!(format_from_string("yml").unwrap().formal_name, "yaml");
    assert_eq!(format_from_string("j").unwrap().formal_name, "json");
    assert_eq!(format_from_string("c").unwrap().formal_name, "csv");
    assert!(matches!(
        format_from_string("doc"),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));
    assert!(matches!(
        format_from_string(""),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));
}

#[test]
fn format_string_from_filename_matches_zig_rules() {
    assert_eq!(format_string_from_filename("test.Yaml"), "yaml");
    assert_eq!(format_string_from_filename("test.index.Yaml"), "yaml");
    assert_eq!(format_string_from_filename("test"), "json");
    assert_eq!(format_string_from_filename("test.json"), "json");
    assert_eq!(format_string_from_filename("TEST.JSON"), "json");
    assert_eq!(format_string_from_filename("test.json/foo"), "json");
    assert_eq!(format_string_from_filename(""), "json");
    assert_eq!(format_string_from_filename("a.yml"), "yaml");
    assert_eq!(format_string_from_filename("a.yaml"), "yaml");
    assert_eq!(format_string_from_filename("a.json"), "json");
    assert_eq!(format_string_from_filename("a.py"), "python");
    assert_eq!(format_string_from_filename("a.js"), "javascript");
    assert_eq!(format_string_from_filename("a.mjs"), "javascript");
    assert_eq!(format_string_from_filename("a.cjs"), "javascript");
    assert_eq!(format_string_from_filename("a.abc"), "abc");
    assert_eq!(format_string_from_filename("a.AbC"), "AbC");
}

#[test]
fn json_format_with_spans_records_nested_value_span() {
    let decoded = decode_to_document_with_options(
        "json",
        r#"{"nested":"{\"inner\":42}"}"#,
        DecodeOptions {
            nest_json: true,
            emit_path: false,
        },
    )
    .expect("nested JSON should decode");

    let mut prefs = FormatPreferences::base();
    prefs.indent = 2;
    prefs.smart = true;

    let formatted = format_json_document_with_spans(&decoded, &prefs).expect("format with spans");
    assert!(formatted.text.contains("\"nested\""));

    // Nest expansion is enabled, so the nested JSON is expanded inline.
    assert!(formatted.text.contains("\"inner\":"));
    // The nested node is a map in the tree (expanded from the original string).
    let nested_str = find_node_by_path(
        decoded.root,
        &[path_seg_key("nested")],
        false,
        &decoded.store,
    )
    .expect("nested node exists");
    let span = formatted
        .spans
        .iter()
        .find(|span| span.node_id == nested_str)
        .expect("nested span recorded");
    assert!(formatted.text[span.start_byte as usize..].starts_with("{"));
}

#[test]
fn json_format_with_spans_handles_repeated_scalars_in_order() {
    let decoded = decode_to_document_with_options(
        "json",
        r#"{"a":1,"b":1,"c":1}"#,
        DecodeOptions {
            nest_json: false,
            emit_path: false,
        },
    )
    .expect("JSON should decode");

    let mut prefs = FormatPreferences::base();
    prefs.indent = 2;
    prefs.smart = false;

    let formatted = format_json_document_with_spans(&decoded, &prefs).expect("format with spans");
    let scalar_spans: Vec<_> = formatted
        .spans
        .iter()
        .filter(|span| &formatted.text[span.start_byte as usize..span.end_byte as usize] == "1")
        .collect();

    assert_eq!(scalar_spans.len(), 3);
    assert!(scalar_spans[0].start_byte < scalar_spans[1].start_byte);
    assert!(scalar_spans[1].start_byte < scalar_spans[2].start_byte);
}
