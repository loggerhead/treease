use treease_core::core::{DiagnosticStage, Diagnostics, ParseErrorInfo};

#[test]
fn diagnostics_set_parse_errorf_sets_stage_and_message() {
    let mut diagnostics = Diagnostics::default();

    diagnostics.set_parse_errorf(
        ParseErrorInfo {
            op_id: Some(12),
            expected_args: Some(2),
            actual_args: Some(1),
            ..ParseErrorInfo::default()
        },
        "bad op",
    );

    assert_eq!(diagnostics.stage, Some(DiagnosticStage::ParseExpression));
    assert_eq!(diagnostics.message, "bad op");
    assert_eq!(diagnostics.parse_info.op_id, Some(12));
    assert_eq!(diagnostics.parse_info.expected_args, Some(2));
    assert_eq!(diagnostics.parse_info.actual_args, Some(1));
}

#[test]
fn diagnostics_set_location_from_offset_populates_line_and_column() {
    let mut diagnostics = Diagnostics::default();

    diagnostics.set_location_from_offset("sample.yml", "a\nbcd\n", 2);

    assert_eq!(diagnostics.location.filename, "sample.yml");
    assert_eq!(diagnostics.location.byte_offset, Some(2));
    assert_eq!(diagnostics.location.line, Some(2));
    assert_eq!(diagnostics.location.column, Some(1));
    assert_eq!(diagnostics.snippet.line_text, "bcd");
}

#[test]
fn diagnostics_set_messagef_formats_message_and_preserves_parse_info() {
    let mut diagnostics = Diagnostics::default();
    diagnostics.set_parse_info(ParseErrorInfo {
        op_id: Some(7),
        ..ParseErrorInfo::default()
    });

    diagnostics.set_messagef(
        DiagnosticStage::Eval,
        "expected {} items",
        format_args!("two"),
    );

    assert_eq!(diagnostics.stage, Some(DiagnosticStage::Eval));
    assert_eq!(diagnostics.message, "expected two items");
    assert_eq!(diagnostics.parse_info.op_id, Some(7));
}

#[test]
fn diagnostics_set_parse_error_and_location_from_slice_match_zig_behavior() {
    let mut diagnostics = Diagnostics::default();
    diagnostics.set_parse_error(
        ParseErrorInfo {
            token_start: Some(2),
            token_end: Some(4),
            ..ParseErrorInfo::default()
        },
        "bad expression",
    );
    diagnostics.set_location_from_offset_in_slice("sample.yml", "root:\n  child\n", 6, 3);

    assert_eq!(diagnostics.stage, Some(DiagnosticStage::ParseExpression));
    assert_eq!(diagnostics.message, "bad expression");
    assert_eq!(diagnostics.location.filename, "sample.yml");
    assert_eq!(diagnostics.location.byte_offset, Some(9));
    assert_eq!(diagnostics.location.line, Some(2));
    assert_eq!(diagnostics.location.column, Some(4));
    assert_eq!(diagnostics.snippet.line_text, "  child");
    assert_eq!(diagnostics.parse_info.token_start, Some(2));
    assert_eq!(diagnostics.parse_info.token_end, Some(4));
}

#[test]
fn diagnostics_set_filename_if_empty_only_backfills_missing_filename() {
    let mut diagnostics = Diagnostics::default();

    diagnostics.set_filename_if_empty("first.yml");
    diagnostics.set_filename_if_empty("second.yml");

    assert_eq!(diagnostics.location.filename, "first.yml");

    diagnostics.set_location_from_offset("sample.yml", "value", 0);
    diagnostics.set_filename_if_empty("third.yml");

    assert_eq!(diagnostics.location.filename, "sample.yml");
}
