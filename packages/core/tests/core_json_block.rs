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

#[test]
fn json_block_ndjson_pass02_finds_each_line() {
    // pass02.1.ndjson: every line is a valid JSON array
    let source = include_str!("../../../test/fixtures/ndjson/pass02.1.ndjson");

    // Line 0: [true]
    let span = find_json_block_at_position("json", source, 0, 3);
    assert!(span.found, "row 0 should find [true]");
    assert_eq!(
        &source[span.start_byte as usize..span.end_byte as usize],
        "[true]"
    );

    // Line 1: ["Gilbert", "2013", 24, true]
    let span = find_json_block_at_position("json", source, 1, 10);
    assert!(span.found, "row 1 should find its array");
    assert_eq!(
        &source[span.start_byte as usize..span.end_byte as usize],
        r#"["Gilbert", "2013", 24, true]"#
    );

    // Last non-empty line (row 4): ["Deloise", "2012A", 19, true]
    let span = find_json_block_at_position("json", source, 4, 10);
    assert!(span.found, "row 4 should find its array");
    assert_eq!(
        &source[span.start_byte as usize..span.end_byte as usize],
        r#"["Deloise", "2012A", 19, true]"#
    );
}

#[test]
fn json_block_ndjson_pass01_multiline_object_and_null() {
    // pass01.0.ndjson: first object spans 6 lines, then null, then single-line objects
    let source = include_str!("../../../test/fixtures/ndjson/pass01.0.ndjson");

    // Cursor inside the multiline {…} at row 1 → finds the whole block (rows 0..=5)
    let span = find_json_block_at_position("json", source, 1, 8);
    assert!(
        span.found,
        "multiline object at row 1 col 8 should be found"
    );
    let block = &source[span.start_byte as usize..span.end_byte as usize];
    // Must be a JSON object binding the 6-line span
    assert!(block.starts_with('{'), "block must start with '{{'");
    assert!(block.ends_with('}'), "block must end with '}}'");
    assert_eq!(span.start_row, 0, "block starts at row 0");
    assert_eq!(span.end_row, 5, "block ends at row 5");
    assert_eq!(span.start_column, 0, "block starts at column 0");

    // null row (row 6) → no container found
    let span = find_json_block_at_position("json", source, 6, 0);
    assert!(!span.found, "null is not a JSON container");

    // Single-line object at row 7
    let span = find_json_block_at_position("json", source, 7, 10);
    assert!(span.found, "single-line object at row 7 should be found");
    assert_eq!(span.start_row, 7);
    assert_eq!(span.end_row, 7);

    // Last non-empty line (row 9): {"name": "May", "wins": []}
    let span = find_json_block_at_position("json", source, 9, 5);
    assert!(span.found, "last line at row 9 should be found");
    assert_eq!(span.start_row, 9);
    assert_eq!(span.end_row, 9);
}

#[test]
fn json_block_ndjson_fail01_rejects_invalid_line() {
    // fail01.0.ndjson: line 1 is invalid (starts with [ but closes with })
    let source = include_str!("../../../test/fixtures/ndjson/fail01.0.ndjson");

    // Row 0: [true] → valid
    let span = find_json_block_at_position("json", source, 0, 3);
    assert!(span.found, "row 0 [true] should be found");
    assert_eq!(
        &source[span.start_byte as usize..span.end_byte as usize],
        "[true]"
    );

    // Row 1: ["Gilbert", "2013", 24, true} → mismatched bracket
    let span = find_json_block_at_position("json", source, 1, 10);
    assert!(
        !span.found,
        "row 1 has mismatched brackets, should be EMPTY"
    );

    // Row 2: ["Alexa", "2013", 29, true] → valid again
    let span = find_json_block_at_position("json", source, 2, 10);
    assert!(span.found, "row 2 should be found");
    assert_eq!(
        &source[span.start_byte as usize..span.end_byte as usize],
        r#"["Alexa", "2013", 29, true]"#
    );

    // Invalid line does not poison subsequent lines
    let span = find_json_block_at_position("json", source, 4, 5);
    assert!(span.found, "row 4 should still be found");
}

#[test]
fn json_block_large_amazon_ndjson() {
    let source = include_str!("../../../test/fixtures/ndjson/amazon_cellphones.1.ndjson");
    let line_count = source.lines().filter(|l| !l.is_empty()).count();
    assert!(
        line_count > 700,
        "expected a large fixture, got {line_count} lines"
    );

    // Header row (row 0): array of column names
    let span = find_json_block_at_position("json", source, 0, 10);
    assert!(span.found, "header row should be found");
    assert_eq!(span.start_row, 0);
    assert_eq!(span.end_row, 0);

    // A data row near the middle
    let span = find_json_block_at_position("json", source, line_count as u32 / 2, 20);
    assert!(span.found, "middle row should be found");
    assert_eq!(span.start_row, span.end_row, "single-line row");
    assert!(
        source.as_bytes()[span.start_byte as usize] == b'[',
        "found block must start with '['"
    );

    // Last non-empty row
    let last_row = (line_count - 1) as u32;
    let span = find_json_block_at_position("json", source, last_row, 10);
    assert!(span.found, "last row should be found");
    assert_eq!(span.start_row, span.end_row);
}

#[test]
fn json_block_jsonl_file() {
    let source = include_str!("../../../test/fixtures/ndjson/langchain_closed.1.jsonl");
    let line_count = source.lines().filter(|l| !l.is_empty()).count();
    assert!(
        line_count > 100,
        "expected jsonl fixture with >100 lines, got {line_count}"
    );

    // First line: {"title": ..., ...}
    let span = find_json_block_at_position("json", source, 0, 10);
    assert!(span.found, "first jsonl line should be found");
    assert_eq!(span.start_row, 0);
    assert_eq!(span.end_row, 0);
    assert!(
        source.as_bytes()[span.start_byte as usize] == b'{',
        "first line should start with '{{'"
    );

    // A middle line (cursor inside the title field)
    let span = find_json_block_at_position("json", source, line_count as u32 / 2, 20);
    assert!(span.found, "middle jsonl line should be found");
    assert_eq!(span.start_row, span.end_row, "single-line");

    // Last non-empty row
    let last_row = (line_count - 1) as u32;
    let span = find_json_block_at_position("json", source, last_row, 5);
    assert!(span.found, "last jsonl line should be found");
    assert_eq!(span.start_row, span.end_row);
}
