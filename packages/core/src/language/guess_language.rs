use regex_lite::Regex;

use crate::language::Language;

#[derive(Debug, Clone, Copy)]
struct FeatureStats {
    first_char: Option<char>,
    has_brace: bool,
    has_colon: bool,
    has_equal: bool,
    has_syntax_equal: bool,
    has_double_quote: bool,
    has_single_quote: bool,
    has_slash_comment: bool,
    has_indent: bool,
    has_yaml_list: bool,
    has_toml_section: bool,
    has_toml_bare_section: bool,
    has_toml_key_value: bool,
    has_toml_comment_only: bool,
    has_json_scalar: bool,
    has_json_container: bool,
    has_json_simple_array: bool,
    has_yaml_doc_start: bool,
    has_yaml_document_marker: bool,
    has_yaml_directive: bool,
    has_yaml_tag: bool,
    has_yaml_anchor_alias: bool,
    has_yaml_explicit_key: bool,
    has_yaml_block_scalar: bool,
    has_yaml_bare_mapping: bool,
    has_yaml_quoted_mapping_key: bool,
    has_yaml_list_flow_mapping: bool,
    has_yaml_multiline_quoted_scalar: bool,
    has_yaml_single_quoted_scalar: bool,
    has_yaml_flow_collection: bool,
    has_json_quoted_key: bool,
    has_single_quoted_key: bool,
    has_unquoted_key: bool,
    has_trailing_comma: bool,
    has_python_literal: bool,
    has_colon_newline: bool,
}

#[derive(Debug, Clone, Copy)]
struct GuessResult {
    language: Option<Language>,
    score: f32,
}

const MAX_SAMPLE_LENGTH: usize = 1024;
const MIN_SAMPLE_LENGTH: usize = 8;

const FEATURE_HIT_WEIGHT: f32 = 15.0;
const AMBIGUITY_PENALTY_WEIGHT: f32 = 30.0;

pub fn guess_language(input: &str) -> Option<Language> {
    let raw = truncate_input(input)
        .trim_start_matches('\u{feff}')
        .to_string();
    let stats = scan_features(&raw);
    let trimmed_len = raw.trim().chars().count();

    if trimmed_len < MIN_SAMPLE_LENGTH
        && !stats.has_json_scalar
        && !stats.has_json_container
        && !stats.has_yaml_document_marker
        && !stats.has_yaml_directive
        && !stats.has_yaml_tag
        && !stats.has_yaml_anchor_alias
        && !stats.has_yaml_explicit_key
        && !stats.has_yaml_block_scalar
        && !stats.has_yaml_bare_mapping
        && !stats.has_yaml_quoted_mapping_key
        && !stats.has_yaml_list_flow_mapping
        && !stats.has_yaml_multiline_quoted_scalar
        && !stats.has_yaml_single_quoted_scalar
        && !stats.has_yaml_flow_collection
        && !stats.has_yaml_list
        && !stats.has_toml_key_value
        && !stats.has_toml_section
        && !stats.has_toml_comment_only
    {
        return None;
    }

    let mut results: Vec<GuessResult> = pick_candidates(&stats)
        .into_iter()
        .map(|language| GuessResult {
            language: Some(language),
            score: FEATURE_HIT_WEIGHT * score_features(language, &stats)
                - AMBIGUITY_PENALTY_WEIGHT * ambiguity_penalty(language),
        })
        .collect();

    results.sort_by(|left, right| right.score.total_cmp(&left.score));
    results.first().and_then(|best| best.language)
}

fn truncate_input(text: &str) -> String {
    text.chars().take(MAX_SAMPLE_LENGTH).collect()
}

fn find_first_non_whitespace(text: &str) -> Option<char> {
    text.chars().find(|ch| !ch.is_whitespace())
}

fn strip_quoted_content(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in text.chars() {
        if let Some(current_quote) = quote {
            result.push(if matches!(ch, '\n' | '\r') { ch } else { ' ' });
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == current_quote {
                quote = None;
            }
            continue;
        }

        if matches!(ch, '"' | '\'') {
            quote = Some(ch);
            result.push(' ');
            continue;
        }

        result.push(ch);
    }

    result
}

fn trim_bom_and_whitespace(text: &str) -> &str {
    text.trim_start_matches('\u{feff}')
        .trim_matches(|ch: char| ch.is_ascii_whitespace())
}

fn has_match(pattern: &str, text: &str) -> bool {
    Regex::new(pattern)
        .expect("guess language regex should compile")
        .is_match(text)
}

fn scan_features(text: &str) -> FeatureStats {
    let syntax_text = strip_quoted_content(text);
    let trimmed = trim_bom_and_whitespace(text);
    let has_brace = text.contains('{') || text.contains('}');
    let has_colon = text.contains(':');
    let has_equal = text.contains('=');
    let has_syntax_equal = syntax_text.contains('=');
    let has_double_quote = text.contains('"');
    let has_single_quote = text.contains('\'');
    let has_slash_comment = text.contains("//") || text.contains("/*");
    let has_indent = has_match(r"(?m)^[ \t]+", text);
    let has_yaml_list = has_match(r"(?m)^\s*-\s+\S+", text);
    let has_toml_section = has_match(
        r"(?m)^\s*(?:\[\[[^\],]+\]\]|\[[^\],]+\])\s*(?:#.*)?$",
        &syntax_text,
    );
    let has_toml_bare_section = has_match(
        r"(?m)^\s*(?:\[\[\s*[A-Za-z0-9_-][^\],]*\]\]|\[\s*[A-Za-z0-9_-][^\],]*\])\s*(?:#.*)?$",
        text,
    );
    let has_toml_key_value = has_match(
        r#"(?m)^\s*(?:"[^"\r\n]*"|'[^'\r\n]*'|[A-Za-z0-9_-]+)(?:\s*\.\s*(?:"[^"\r\n]*"|'[^'\r\n]*'|[A-Za-z0-9_-]+))*\s*="#,
        text,
    );
    let has_toml_comment_only = !trimmed.is_empty() && has_match(r"^(?:\s*#.*(?:\r?\n|$))+$", text);
    let has_json_scalar = has_match(
        r#"^(?:-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|"(?:\\.|[^"\\\r\n])*")$"#,
        trimmed,
    );
    let has_json_container = has_match(r"^(?:\{|\[)", trimmed);
    let has_json_simple_array = has_match(
        r#"(?s)^\[\s*(?:(?:"(?:\\.|[^"\\])*"|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|\{\}|\[\])\s*,\s*)*(?:"(?:\\.|[^"\\])*"|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|\{\}|\[\])?\s*\]$"#,
        trimmed,
    );
    let has_yaml_doc_start = has_match(r"(?m)^\s*---\s*$", text);
    let has_yaml_document_marker = has_match(r"(?m)^\s*(?:---|\.\.\.)(?:\s|$)", text);
    let has_yaml_directive = has_match(r"(?m)^\s*%[A-Z][A-Z0-9-]*(?:\s|$)", text);
    let has_yaml_tag = has_match(
        r"(^|\s)!(?:![A-Za-z0-9_-]+|[A-Za-z][A-Za-z0-9_-]*|<[^>\r\n]+>|\s+\S|$)",
        text,
    );
    let has_yaml_anchor_alias = has_match(r"(^|\s)[&*][A-Za-z0-9_-]+", text);
    let has_yaml_explicit_key = has_match(r"(?m)^\s*[?:](?:\s|:|$)|^\s*\[\s*\?", text);
    let has_yaml_block_scalar = has_match(
        r"(?:^|\n)\s*(?:---\s*)?(?:[A-Za-z0-9_-][^:\n]*:\s*)?[|>][+-]?\d?(?:\s|$)",
        text,
    );
    let has_yaml_bare_mapping = has_match(r#"(?m)^[^\s#"'{\[][^:\n]*:\s+\S"#, text);
    let has_yaml_quoted_mapping_key =
        has_match(r#"(?m)^\s*(?:"(?:\\.|[^"\\])*"|'(?:[^']|'')*')\s*:"#, text);
    let has_yaml_list_flow_mapping = has_match(r"(?m)^\s*-\s*\{[^}\r\n]*:\S", text);
    let has_yaml_multiline_quoted_scalar =
        has_match(r#"(?s)^["'][\s\S]*\n[\s\S]*["']\s*$"#, trimmed);
    let has_yaml_single_quoted_scalar = has_match(r"^'(?:[^']|'')*'\s*$", trimmed);
    let has_yaml_flow_collection = has_match(r"^(?:\{|\[)", trimmed)
        && (has_match(r#"[\[{,]\s*[A-Za-z_][^"'\n,\[\]{}]*?(?:[:,])"#, trimmed)
            || has_match(r"\{\s*\?", trimmed)
            || has_match(r":\s*(?:,|\})", trimmed));
    let has_json_quoted_key = has_match(r#"[\{,]\s*"[^"\n]+"\s*:"#, text);
    let has_single_quoted_key = has_match(r"[\{,]\s*'[^'\n]+'\s*:", text);
    let has_unquoted_key = has_match(r"[\{,]\s*[$A-Z_a-z][\w$]*\s*:", &syntax_text);
    let has_trailing_comma = has_match(r",\s*[\}\]]", text);
    let has_python_literal = has_match(r"\b(True|False|None)\b", text);
    let has_colon_newline = has_match(r":\s*(?:\r?\n|$)", text);

    FeatureStats {
        first_char: find_first_non_whitespace(text),
        has_brace,
        has_colon,
        has_equal,
        has_syntax_equal,
        has_double_quote,
        has_single_quote,
        has_slash_comment,
        has_indent,
        has_yaml_list,
        has_toml_section,
        has_toml_bare_section,
        has_toml_key_value,
        has_toml_comment_only,
        has_json_scalar,
        has_json_container,
        has_json_simple_array,
        has_yaml_doc_start,
        has_yaml_document_marker,
        has_yaml_directive,
        has_yaml_tag,
        has_yaml_anchor_alias,
        has_yaml_explicit_key,
        has_yaml_block_scalar,
        has_yaml_bare_mapping,
        has_yaml_quoted_mapping_key,
        has_yaml_list_flow_mapping,
        has_yaml_multiline_quoted_scalar,
        has_yaml_single_quoted_scalar,
        has_yaml_flow_collection,
        has_json_quoted_key,
        has_single_quoted_key,
        has_unquoted_key,
        has_trailing_comma,
        has_python_literal,
        has_colon_newline,
    }
}

fn pick_candidates(stats: &FeatureStats) -> Vec<Language> {
    if stats.has_toml_section && stats.has_toml_key_value && stats.first_char != Some('<') {
        return vec![Language::Toml];
    }
    if stats.first_char == Some('[') && stats.has_json_quoted_key && !stats.has_yaml_document_marker
    {
        return vec![Language::Json];
    }
    if stats.first_char == Some('{') && stats.has_python_literal {
        return vec![Language::Python];
    }
    if stats.has_yaml_document_marker {
        return vec![Language::Yaml];
    }
    if stats.first_char == Some('{')
        && stats.has_double_quote
        && !stats.has_unquoted_key
        && !stats.has_single_quoted_key
        && !stats.has_trailing_comma
    {
        return vec![Language::Json];
    }

    let has_yaml_specific_syntax = stats.has_yaml_document_marker
        || stats.has_yaml_directive
        || stats.has_yaml_tag
        || stats.has_yaml_anchor_alias
        || stats.has_yaml_explicit_key
        || stats.has_yaml_block_scalar
        || stats.has_yaml_bare_mapping
        || stats.has_yaml_quoted_mapping_key
        || stats.has_yaml_list_flow_mapping
        || stats.has_yaml_multiline_quoted_scalar
        || stats.has_yaml_single_quoted_scalar
        || stats.has_yaml_flow_collection;
    let is_object_like = stats.has_brace
        || stats.first_char == Some('{')
        || stats.first_char == Some('[')
        || stats.has_json_scalar
        || stats.has_json_container;
    let is_mapping_like = stats.has_colon
        && (stats.has_indent
            || stats.has_yaml_list
            || stats.has_yaml_doc_start
            || stats.has_colon_newline);

    let mut candidates = Vec::new();
    let mut push_unique = |language: Language| {
        if !candidates.contains(&language) {
            candidates.push(language);
        }
    };

    if (stats.has_toml_key_value || stats.has_toml_section || stats.has_toml_comment_only)
        && stats.first_char != Some('<')
    {
        push_unique(Language::Toml);
    }

    if stats.has_yaml_list || is_mapping_like || has_yaml_specific_syntax {
        push_unique(Language::Yaml);
    }

    if is_object_like || is_mapping_like {
        push_unique(Language::Json);
    }

    if stats.first_char == Some('{') && (stats.has_unquoted_key || stats.has_trailing_comma) {
        push_unique(Language::Javascript);
    }

    if stats.first_char == Some('{') && (stats.has_single_quoted_key || stats.has_python_literal) {
        push_unique(Language::Python);
    }

    if candidates.is_empty() {
        vec![Language::Json, Language::Yaml, Language::Toml]
    } else {
        candidates
    }
}

fn ambiguity_penalty(language: Language) -> f32 {
    match language {
        Language::Json => 0.2,
        Language::Yaml => 1.2,
        Language::Toml => 0.5,
        Language::Python => 1.5,
        Language::Javascript => 2.0,
        Language::Csv | Language::None => 0.0,
    }
}

fn score_features(language: Language, stats: &FeatureStats) -> f32 {
    match language {
        Language::Json => {
            let mut score = 0.0;
            if stats.has_json_scalar {
                score += 4.0;
            }
            if stats.has_json_container {
                score += 4.0;
            }
            if stats.first_char == Some('[') {
                score += 3.0;
            }
            if stats.first_char == Some('{') && stats.has_json_quoted_key {
                score += 3.0;
            }
            if stats.first_char == Some('{')
                && stats.has_colon
                && stats.has_double_quote
                && !stats.has_unquoted_key
            {
                score += 4.0;
            }
            if stats.has_toml_bare_section && !stats.has_json_simple_array {
                score -= 8.0;
            }
            if stats.has_json_quoted_key {
                score += 2.0;
            }
            if stats.has_double_quote && !stats.has_single_quote {
                score += 1.0;
            }
            if stats.has_syntax_equal {
                score -= 2.0;
            }
            if stats.has_yaml_list_flow_mapping {
                score -= 4.0;
            }
            if stats.has_unquoted_key {
                score -= 2.0;
            }
            if stats.has_single_quoted_key {
                score -= 2.0;
            }
            if stats.has_python_literal {
                score -= 2.0;
            }
            if stats.has_trailing_comma {
                score -= 1.0;
            }
            score
        }
        Language::Javascript => {
            let mut score = 0.0;
            if stats.has_unquoted_key {
                score += 3.0;
            }
            if stats.has_unquoted_key && !stats.has_json_quoted_key && !stats.has_single_quoted_key
            {
                score += 3.0;
            }
            if stats.has_trailing_comma {
                score += 1.0;
            }
            if stats.has_slash_comment {
                score += 1.0;
            }
            if stats.has_json_quoted_key {
                score -= 2.0;
            }
            if stats.has_python_literal {
                score -= 1.0;
            }
            score
        }
        Language::Python => {
            let mut score = 0.0;
            if stats.has_python_literal {
                score += 3.0;
            }
            if stats.has_single_quoted_key {
                score += 2.0;
            }
            if stats.has_json_quoted_key {
                score -= 2.0;
            }
            if stats.has_unquoted_key {
                score -= 1.0;
            }
            if stats.has_slash_comment {
                score -= 2.0;
            }
            score
        }
        Language::Yaml => {
            let mut score = 0.0;
            if stats.has_yaml_document_marker {
                score += 4.0;
            }
            if stats.has_yaml_directive {
                score += 5.0;
            }
            if stats.has_yaml_tag {
                score += 5.0;
            }
            if stats.has_yaml_anchor_alias {
                score += 5.0;
            }
            if stats.has_yaml_explicit_key {
                score += 4.0;
            }
            if stats.has_yaml_block_scalar {
                score += 4.0;
            }
            if stats.has_yaml_bare_mapping {
                score += 7.0;
            }
            if stats.has_yaml_quoted_mapping_key {
                score += 7.0;
            }
            if stats.has_yaml_list_flow_mapping {
                score += 6.0;
            }
            if stats.has_yaml_multiline_quoted_scalar {
                score += 5.0;
            }
            if stats.has_yaml_single_quoted_scalar {
                score += 5.0;
            }
            if stats.has_yaml_flow_collection {
                score += 5.0;
            }
            if stats.has_yaml_list {
                score += 6.0;
            }
            if stats.has_colon_newline {
                score += 2.0;
            }
            if stats.has_indent {
                score += 1.0;
            }
            if stats.has_brace && !stats.has_yaml_tag && !stats.has_yaml_bare_mapping {
                score -= 1.0;
            }
            if stats.has_equal {
                score -= 1.0;
            }
            if stats.first_char == Some('<') {
                score -= 3.0;
            }
            score
        }
        Language::Toml => {
            let mut score = 0.0;
            if stats.has_toml_section && !stats.has_json_simple_array {
                score += 6.0;
            }
            if stats.has_toml_key_value {
                score += 6.0;
            }
            if stats.has_toml_comment_only {
                score += 4.0;
            }
            if stats.has_syntax_equal {
                score += 1.0;
            }
            if stats.first_char == Some('[') && !stats.has_toml_section {
                score -= 3.0;
            }
            if stats.has_brace && !stats.has_toml_key_value {
                score -= 1.0;
            }
            if stats.has_colon_newline {
                score -= 1.0;
            }
            if stats.has_single_quoted_key {
                score -= 3.0;
            }
            if stats.has_python_literal {
                score -= 3.0;
            }
            score
        }
        Language::Csv | Language::None => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::guess_language;
    use crate::language::Language;

    #[test]
    fn guess_language_short_input_stays_null_without_signal() {
        assert_eq!(guess_language("abc"), None);
        assert_eq!(guess_language("   "), None);
    }

    #[test]
    fn guess_language_detects_json_yaml_toml() {
        assert_eq!(
            guess_language(r#"{"name": "alice", "age": 30}"#),
            Some(Language::Json)
        );
        assert_eq!(
            guess_language("name: alice\nage: 30\nitems:\n  - one\n  - two"),
            Some(Language::Yaml)
        );
        assert_eq!(
            guess_language("[database]\nhost = \"localhost\"\nport = 5432"),
            Some(Language::Toml)
        );
    }

    #[test]
    fn guess_language_detects_python_and_javascript() {
        assert_eq!(
            guess_language("{'name': 'alice', 'active': True, 'value': None}"),
            Some(Language::Python)
        );
        assert_eq!(
            guess_language(r#"{ name: "alice", age: 30, }"#),
            Some(Language::Javascript)
        );
    }
}
