use treease_core::core::core_helpers;

// ── TestParseInt64 ───────────────────────────────────────────────

#[test]
fn test_parse_int64() {
    let scenarios = [
        ("34", 34i64, ""),
        ("10_000", 10000, "10000"),
        ("0x10", 16, ""),
        ("0x10_000", 65536, "0x10000"),
        ("0o10", 8, ""),
        ("0b101", 5, ""),
        ("-9223372036854775808", i64::MIN, ""),
    ];

    for (number_string, expected_parsed, expected_format) in &scenarios {
        let r = core_helpers::parse_int64(number_string).unwrap();
        assert_eq!(
            r.value, *expected_parsed,
            "parse_int64({}) value mismatch",
            number_string
        );

        let expected_fmt: &str = if expected_format.is_empty() {
            number_string
        } else {
            expected_format
        };
        let formatted = core_helpers::format_int64(&r.fmt, r.value).unwrap();
        assert_eq!(
            formatted, expected_fmt,
            "format_int64({}) mismatch",
            number_string
        );
    }
}

// ── TestParseInt ─────────────────────────────────────────────────

#[test]
fn test_parse_int() {
    let scenarios: &[(&str, Option<i32>, bool)] = &[
        ("34", Some(34), false),
        ("10_000", Some(10000), false),
        ("0x10", Some(16), false),
        ("0o10", Some(8), false),
        ("0b10", Some(2), false),
        ("invalid", None, true),
    ];

    for (number_string, expected, expect_error) in scenarios {
        let result = core_helpers::parse_int(number_string);
        if *expect_error {
            assert!(
                result.is_err(),
                "parse_int({}) should have errored",
                number_string
            );
        } else {
            let got = result.unwrap();
            assert_eq!(
                got,
                expected.unwrap(),
                "parse_int({}) mismatch",
                number_string
            );
        }
    }
}
// ── TestProcessEscapeCharacters ──────────────────────────────────

#[test]
fn test_process_escape_characters() {
    let scenarios: &[(&str, &str)] = &[
        ("", ""),
        ("hello", "hello"),
        ("\\\"", "\""),
        ("hello\\\"world", "hello\"world"),
        ("\\n", "\n"),
        ("line1\\nline2", "line1\nline2"),
        ("\\t", "\t"),
        ("hello\\tworld", "hello\tworld"),
        ("\\r", "\r"),
        ("hello\\rworld", "hello\rworld"),
        ("\\f", "\x0c"),
        ("hello\\fworld", "hello\x0cworld"),
        ("\\v", "\x0b"),
        ("hello\\vworld", "hello\x0bworld"),
        ("\\b", "\x08"),
        ("hello\\bworld", "hello\x08world"),
        ("\\a", "\x07"),
        ("hello\\aworld", "hello\x07world"),
        ("\\\"\\n\\t\\r\\f\\v\\b\\a", "\"\n\t\r\x0c\x0b\x08\x07"),
        (
            "multiple\\nlines\\twith\\ttabs",
            "multiple\nlines\twith\ttabs",
        ),
        ("quote\\\"here", "quote\"here"),
        ("\\\\", "\\"),
        ("\\\"test\\\"", "\"test\""),
        ("a\\\\b", "a\\b"),
        ("Hi \\\\(.value)", "Hi \\(.value)"),
    ];

    for (input, expected) in scenarios {
        let got = core_helpers::process_escape_characters(input).unwrap();
        assert_eq!(
            &got, expected,
            "process_escape_characters({:?}) expected {:?} got {:?}",
            input, expected, got
        );
    }
}
