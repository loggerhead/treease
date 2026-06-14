use super::SemType;

pub fn parse_scalar_edit_replacement(
    language: &str,
    replacement: &str,
    is_key: bool,
) -> Option<(SemType, String)> {
    if is_key {
        return parse_key_edit_replacement(language, replacement)
            .map(|value| (SemType::Str, value));
    }

    match language {
        "json" => serde_json::from_str::<serde_json::Value>(replacement)
            .ok()
            .and_then(json_scalar_to_node_value),
        "yaml" => Some(parse_yaml_scalar_replacement(replacement)),
        "toml" => Some(parse_toml_scalar_replacement(replacement)),
        "csv" => Some(parse_inferred_string_scalar(&parse_csv_field_replacement(
            replacement,
        )?)),
        "python" => Some(parse_python_scalar_replacement(replacement)),
        "javascript" => Some(parse_javascript_scalar_replacement(replacement)),
        _ => None,
    }
}

fn parse_key_edit_replacement(language: &str, replacement: &str) -> Option<String> {
    match language {
        "json" => serde_json::from_str::<String>(replacement).ok(),
        "yaml" => Some(parse_yaml_string_replacement(replacement)),
        "toml" => Some(parse_toml_key_replacement(replacement)),
        "csv" => None,
        "python" => Some(parse_python_string_replacement(replacement)),
        "javascript" => Some(parse_javascript_key_replacement(replacement)),
        _ => None,
    }
}

fn parse_yaml_scalar_replacement(replacement: &str) -> (SemType, String) {
    let trimmed = replacement.trim();
    match trimmed {
        "" | "~" | "null" | "Null" | "NULL" => (SemType::Nil, String::new()),
        "true" | "True" | "TRUE" => (SemType::Boolean, "true".to_owned()),
        "false" | "False" | "FALSE" => (SemType::Boolean, "false".to_owned()),
        _ => {
            if let Some(text) = parse_yaml_quoted_string(trimmed) {
                (SemType::Str, text)
            } else {
                parse_inferred_string_scalar(trimmed)
            }
        }
    }
}

fn parse_toml_scalar_replacement(replacement: &str) -> (SemType, String) {
    let trimmed = replacement.trim();
    match trimmed {
        "true" => (SemType::Boolean, "true".to_owned()),
        "false" => (SemType::Boolean, "false".to_owned()),
        _ => {
            if let Ok(value) = serde_json::from_str::<String>(trimmed) {
                return (SemType::Str, value);
            }
            parse_number_scalar(trimmed).unwrap_or_else(|| (SemType::Str, trimmed.to_owned()))
        }
    }
}

fn parse_python_scalar_replacement(replacement: &str) -> (SemType, String) {
    let trimmed = replacement.trim();
    match trimmed {
        "True" => (SemType::Boolean, "true".to_owned()),
        "False" => (SemType::Boolean, "false".to_owned()),
        "None" => (SemType::Nil, String::new()),
        _ => {
            if let Some(value) = parse_quoted_string(trimmed) {
                return (SemType::Str, value);
            }
            parse_number_scalar(trimmed).unwrap_or_else(|| (SemType::Str, trimmed.to_owned()))
        }
    }
}

fn parse_javascript_scalar_replacement(replacement: &str) -> (SemType, String) {
    let trimmed = replacement.trim();
    match trimmed {
        "true" => (SemType::Boolean, "true".to_owned()),
        "false" => (SemType::Boolean, "false".to_owned()),
        "null" => (SemType::Nil, String::new()),
        _ => {
            if let Ok(value) = serde_json::from_str::<String>(trimmed) {
                return (SemType::Str, value);
            }
            if let Some(value) = parse_quoted_string(trimmed) {
                return (SemType::Str, value);
            }
            parse_number_scalar(trimmed).unwrap_or_else(|| (SemType::Str, trimmed.to_owned()))
        }
    }
}

fn parse_inferred_string_scalar(raw: &str) -> (SemType, String) {
    parse_number_scalar(raw).unwrap_or_else(|| {
        let lowered = raw.trim().to_ascii_lowercase();
        match lowered.as_str() {
            "true" | "yes" | "y" => (SemType::Boolean, "true".to_owned()),
            "false" | "no" | "n" => (SemType::Boolean, "false".to_owned()),
            "null" => (SemType::Nil, String::new()),
            _ => (SemType::Str, raw.to_owned()),
        }
    })
}

fn parse_number_scalar(raw: &str) -> Option<(SemType, String)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.parse::<i64>().is_ok() {
        return Some((SemType::Int, trimmed.to_owned()));
    }
    if trimmed.parse::<f64>().is_ok() {
        return Some((SemType::Float, trimmed.to_owned()));
    }
    None
}

fn parse_yaml_string_replacement(replacement: &str) -> String {
    parse_yaml_quoted_string(replacement.trim()).unwrap_or_else(|| replacement.trim().to_owned())
}

fn parse_yaml_quoted_string(trimmed: &str) -> Option<String> {
    let bytes = trimmed.as_bytes();
    if bytes.len() < 2 || bytes.first() != bytes.last() {
        return None;
    }
    match bytes[0] {
        b'\'' => Some(trimmed[1..trimmed.len() - 1].replace("''", "'")),
        b'"' => serde_json::from_str::<String>(trimmed).ok(),
        _ => None,
    }
}

fn parse_toml_key_replacement(replacement: &str) -> String {
    serde_json::from_str::<String>(replacement.trim())
        .unwrap_or_else(|_| replacement.trim().to_owned())
}

fn parse_python_string_replacement(replacement: &str) -> String {
    parse_quoted_string(replacement.trim()).unwrap_or_else(|| replacement.trim().to_owned())
}

fn parse_javascript_key_replacement(replacement: &str) -> String {
    serde_json::from_str::<String>(replacement.trim())
        .ok()
        .or_else(|| parse_quoted_string(replacement.trim()))
        .unwrap_or_else(|| replacement.trim().to_owned())
}

fn parse_quoted_string(trimmed: &str) -> Option<String> {
    let bytes = trimmed.as_bytes();
    if bytes.len() < 2 || bytes.first() != bytes.last() {
        return None;
    }
    let quote = bytes[0];
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }
    let mut out = String::with_capacity(trimmed.len().saturating_sub(2));
    let mut chars = trimmed[1..trimmed.len() - 1].chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some('\'') => out.push('\''),
            Some('"') => out.push('"'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    Some(out)
}

fn parse_csv_field_replacement(replacement: &str) -> Option<String> {
    let bytes = replacement.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes.last() == Some(&b'"') {
        let mut out = String::with_capacity(replacement.len().saturating_sub(2));
        let mut chars = replacement[1..replacement.len() - 1].chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '"' && chars.peek() == Some(&'"') {
                chars.next();
                out.push('"');
            } else if ch == '"' {
                return None;
            } else {
                out.push(ch);
            }
        }
        return Some(out);
    }
    Some(replacement.to_owned())
}

fn json_scalar_to_node_value(value: serde_json::Value) -> Option<(SemType, String)> {
    match value {
        serde_json::Value::Null => Some((SemType::Nil, String::new())),
        serde_json::Value::Bool(value) => Some((
            SemType::Boolean,
            if value { "true" } else { "false" }.to_owned(),
        )),
        serde_json::Value::Number(value) => {
            let text = value.to_string();
            let sem_type = if value.is_i64() || value.is_u64() {
                SemType::Int
            } else {
                SemType::Float
            };
            Some((sem_type, text))
        }
        serde_json::Value::String(value) => Some((SemType::Str, value)),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalar_replacements_without_changing_language_rules() {
        assert_eq!(
            parse_scalar_edit_replacement("toml", "42", false),
            Some((SemType::Int, "42".to_owned()))
        );
        assert_eq!(
            parse_scalar_edit_replacement("python", "None", false),
            Some((SemType::Nil, String::new()))
        );
        assert_eq!(
            parse_scalar_edit_replacement("javascript", "'hi'", false),
            Some((SemType::Str, "hi".to_owned()))
        );
    }

    #[test]
    fn parses_key_replacements_and_rejects_csv_header_edits() {
        assert_eq!(
            parse_scalar_edit_replacement("python", "'a.b'", true),
            Some((SemType::Str, "a.b".to_owned()))
        );
        assert_eq!(parse_scalar_edit_replacement("csv", "\"a,b\"", true), None);
        assert_eq!(
            parse_scalar_edit_replacement("csv", "\"a,b\"", false),
            Some((SemType::Str, "a,b".to_owned()))
        );
    }
}
