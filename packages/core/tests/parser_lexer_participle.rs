use treease_core::parser::{ParticipleLexerError, lex_participle};

fn lexemes(input: &str) -> Vec<String> {
    lex_participle(input)
        .unwrap()
        .into_iter()
        .map(|token| token.to_string(false))
        .collect()
}

#[test]
fn lexer_participle_handles_traverse_array_and_slice_defaults() {
    assert_eq!(
        lexemes("to_entries[]"),
        ["to_entries", "traverse_array", "[", "empty", "]"]
    );
    assert_eq!(
        lexemes(".[:3]"),
        [
            "self",
            "traverse_array",
            "[",
            "value",
            "create_map",
            "value",
            "]",
        ]
    );
    assert_eq!(
        lexemes(".[-2:]"),
        [
            "self",
            "traverse_array",
            "[",
            "value",
            "create_map",
            "length",
            "]",
        ]
    );
}

#[test]
fn lexer_participle_handles_dot_path_variants() {
    assert_eq!(lexemes(".a"), ["traverse_path"]);
    assert_eq!(lexemes(".a!="), ["traverse_path", "not_equals"]);
    assert_eq!(
        lexemes(".a.b"),
        ["traverse_path", "short_pipe", "traverse_path"]
    );
    assert_eq!(
        lexemes(".a.b?"),
        ["traverse_path", "short_pipe", "traverse_path"]
    );
    assert_eq!(
        lexemes(".a | .b"),
        ["traverse_path", "pipe", "traverse_path"]
    );
    assert_eq!(
        lexemes(".a,.b"),
        ["traverse_path", "union", "traverse_path"]
    );
}

#[test]
fn lexer_participle_handles_parentheses_and_recursive_descent() {
    assert_eq!(lexemes("(.a)"), ["(", "traverse_path", ")"]);
    assert_eq!(lexemes(".."), ["recursive_descent"]);
    assert_eq!(lexemes("..."), ["recursive_descent"]);
}

#[test]
fn lexer_participle_recognizes_builtin_aliases() {
    assert_eq!(lexemes("flatten(3)"), ["flatten"]);
    assert_eq!(lexemes("flatten"), ["flatten"]);
    assert_eq!(lexemes("from_entries"), ["from_entries"]);
    assert_eq!(lexemes("fromEntries"), ["from_entries"]);
    assert_eq!(lexemes("with_entries"), ["with_entries"]);
    assert_eq!(lexemes("withEntries"), ["with_entries"]);
    assert_eq!(lexemes("split"), ["split"]);
    assert_eq!(lexemes("trim"), ["trim"]);
    assert_eq!(lexemes("to_string"), ["to_string"]);
    assert_eq!(lexemes("tostring"), ["to_string"]);
    assert_eq!(lexemes("upcase"), ["change_case"]);
    assert_eq!(lexemes("downcase"), ["change_case"]);
    assert_eq!(lexemes("to_number"), ["to_number"]);
    assert_eq!(lexemes("tonumber"), ["to_number"]);
    assert_eq!(lexemes("sort_keys"), ["sort_keys"]);
    assert_eq!(lexemes("sortKeys"), ["sort_keys"]);
    assert_eq!(lexemes("contains"), ["contains"]);
    assert_eq!(lexemes("parents"), ["get_parents"]);
    assert_eq!(lexemes("is_key"), ["is_key"]);
    assert_eq!(lexemes("iskey"), ["is_key"]);
    assert_eq!(lexemes("first"), ["first"]);
    assert_eq!(lexemes("parent(-1)"), ["get_parent"]);
}

#[test]
fn lexer_participle_rejects_bare_unknown_identifier() {
    let error = lex_participle("foo").unwrap_err();

    assert_eq!(
        error,
        ParticipleLexerError::UnknownToken {
            offset: 0,
            lexeme: "foo".to_string(),
        }
    );
}
