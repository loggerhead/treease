use treease_core::core::{JsonBlockSpan, find_json_block_at_position};

fn expect_not_found(source: &str, row: u32, column: u32) {
    assert_eq!(
        find_json_block_at_position("json", source, row, column),
        JsonBlockSpan::EMPTY
    );
}

#[test]
fn json_block_finds_nested_container_in_valid_json() {
    let json = "{\"a\":1,\"b\":[2,3],\"c\":4}";
    let json_span = find_json_block_at_position("json", json, 0, 11);
    assert!(json_span.found);
    assert_eq!(
        &json[json_span.start_byte as usize..json_span.end_byte as usize],
        json
    );
}

#[test]
fn json_block_supports_embedded_json_jsonl_and_invalid_outer_wrappers() {
    let embedded = "INFO start {\"a\":[1,2]} done";
    let embedded_span = find_json_block_at_position("json", embedded, 0, 17);
    assert!(embedded_span.found);
    assert_eq!(
        &embedded[embedded_span.start_byte as usize..embedded_span.end_byte as usize],
        "{\"a\":[1,2]}"
    );

    let jsonl = "{\"a\":1}\n{\"b\":[2,3]}\n{\"c\":4}";
    let jsonl_span = find_json_block_at_position("json", jsonl, 1, 5);
    assert!(jsonl_span.found);
    assert_eq!(
        &jsonl[jsonl_span.start_byte as usize..jsonl_span.end_byte as usize],
        "{\"b\":[2,3]}"
    );

    let invalid_outer = "{\"outer\": { bad {\"ok\":true} }}";
    let inner_span = find_json_block_at_position("json", invalid_outer, 0, 19);
    assert!(inner_span.found);
    assert_eq!(
        &invalid_outer[inner_span.start_byte as usize..inner_span.end_byte as usize],
        "{\"ok\":true}"
    );
}

#[test]
fn json_block_selects_jsonl_row_when_cursor_is_at_row_end_boundary() {
    let lines = [
        r#"{"id":11,"title":"previous"}"#,
        r#"{"id":12,"title":"target","detail":{"source":"cursor row"}}"#,
        r#"{"id":13,"title":"next"}"#,
    ];
    let source = lines.join("\n");
    let target_line = lines[1];
    let span = find_json_block_at_position("json", &source, 1, target_line.len() as u32);

    assert!(span.found);
    assert_eq!(
        &source[span.start_byte as usize..span.end_byte as usize],
        target_line
    );
    assert_eq!(span.start_row, 1);
    assert_eq!(span.end_row, 1);
}

#[test]
fn json_block_ignores_outside_cursor_and_scalar_json() {
    expect_not_found("INFO start {\"a\":1} done", 0, 1);
    expect_not_found("true", 0, 0);
    expect_not_found("123", 0, 0);
    expect_not_found("\"x\"", 0, 0);
}

#[test]
fn json_block_ignores_brackets_inside_strings_and_reports_position() {
    let source = "{\"s\":\"} [ {\",\"v\":[1]}";
    let span = find_json_block_at_position("json", source, 0, 15);
    assert!(span.found);
    assert_eq!(
        &source[span.start_byte as usize..span.end_byte as usize],
        source
    );

    let positioned = "prefix\n  {\"a\": 1}";
    let positioned_span = find_json_block_at_position("json", positioned, 1, 4);
    assert!(positioned_span.found);
    assert_eq!(positioned_span.start_row, 1);
    assert_eq!(positioned_span.start_column, 2);
    assert_eq!(
        &positioned[positioned_span.start_byte as usize..positioned_span.end_byte as usize],
        "{\"a\": 1}"
    );
}

#[test]
fn json_block_rejects_other_languages_and_invalid_json() {
    assert_eq!(
        find_json_block_at_position("yaml", "{\"a\":1}", 0, 1),
        JsonBlockSpan::EMPTY
    );
    assert_eq!(
        find_json_block_at_position("json", "{\"a\":1", 0, 1),
        JsonBlockSpan::EMPTY
    );
}
