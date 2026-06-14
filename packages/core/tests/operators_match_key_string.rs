use treease_core::operators::operator_helpers::match_key;

#[test]
fn match_key_should_follow_zig_wildcard_cases() {
    let a100 = "a".repeat(100);
    let cases = [
        ("", "", true),
        ("<<", "<<", true),
        ("", "x", false),
        ("x", "", false),
        ("abc", "abc", true),
        ("abc", "*", true),
        ("abc", "*c", true),
        ("abc", "*b", false),
        ("abc", "a*", true),
        ("abc", "b*", false),
        ("a", "a*", true),
        ("a", "*a", true),
        ("axbxcxdxe", "a*b*c*d*e*", true),
        ("axbxcxdxexxx", "a*b*c*d*e*", true),
        ("abxbbxdbxebxczzx", "a*b?c*x", true),
        ("abxbbxdbxebxczzy", "a*b?c*x", false),
        (&a100, "a*a*a*a*b", false),
        ("xxx", "*x", true),
    ];

    for (name, pattern, expected) in cases {
        assert_eq!(
            match_key(name, pattern),
            expected,
            "name={name} pattern={pattern}"
        );
    }
}
