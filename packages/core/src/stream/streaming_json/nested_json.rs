pub fn trim_ascii_whitespace(value: &str) -> &str {
    value.trim_matches(|ch: char| ch.is_ascii_whitespace())
}

pub fn is_nested_json_candidate(value: &str) -> bool {
    let trimmed = trim_ascii_whitespace(value);
    if trimmed.len() < 2 || trimmed.len() > 2_097_152 {
        return false;
    }

    matches!(
        (trimmed.as_bytes().first(), trimmed.as_bytes().last()),
        (Some(b'{'), Some(b'}')) | (Some(b'['), Some(b']'))
    )
}
