use crate::tree::TreeNode;
use crate::wasm_types::PathSpan;

use super::scalar::{
    ScalarGraphValueEditPlanner, ScalarGraphValueEditRules,
    normalize_default_scalar_graph_edit_span,
};

pub(super) static PLANNER: ScalarGraphValueEditPlanner<YamlGraphValueEditRules> =
    ScalarGraphValueEditPlanner::new(YamlGraphValueEditRules);

pub(super) struct YamlGraphValueEditRules;

impl ScalarGraphValueEditRules for YamlGraphValueEditRules {
    fn normalize_span(&self, source: &str, prefer_key: bool, span: PathSpan) -> Option<PathSpan> {
        let span = normalize_default_scalar_graph_edit_span(span)?;
        if prefer_key {
            Some(span)
        } else {
            refine_yaml_value_span(source, span)
        }
    }

    fn format_key(&self, key: &str) -> Option<String> {
        Some(quote_yaml_single(key))
    }

    fn format_value(
        &self,
        value: &serde_json::Value,
        _target: Option<&TreeNode>,
    ) -> Option<String> {
        Some(format_yaml_value(value))
    }
}

fn refine_yaml_value_span(source: &str, span: PathSpan) -> Option<PathSpan> {
    let start = usize::try_from(span.start_byte).ok()?;
    let end = usize::try_from(span.end_byte).ok()?;
    if end > source.len() || start >= end {
        return None;
    }
    let slice = &source[start..end];
    let mut cursor = 0usize;

    if slice[cursor..].starts_with("- ") || slice[cursor..].starts_with("-\t") {
        cursor += 1;
        while cursor < slice.len() && slice.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
    }

    loop {
        while cursor < slice.len() && slice.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= slice.len() {
            return None;
        }
        let bytes = slice.as_bytes();
        if bytes[cursor] == b'*' {
            return None;
        }
        if bytes[cursor] == b'&' || bytes[cursor] == b'!' {
            cursor += 1;
            while cursor < slice.len() && !slice.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            continue;
        }
        break;
    }

    let new_start = start + cursor;
    (new_start < end).then_some(PathSpan {
        start_byte: i32::try_from(new_start).ok()?,
        end_byte: span.end_byte,
        row: span.row,
        column: span.column,
    })
}

fn format_yaml_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_owned(),
        serde_json::Value::Bool(value) => if *value { "true" } else { "false" }.to_owned(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => quote_yaml_single(value),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => value.to_string(),
    }
}

fn quote_yaml_single(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}
