use crate::core::TreeNode;

use super::quote_json_string;
use super::scalar::{ScalarGraphValueEditPlanner, ScalarGraphValueEditRules};

pub(super) static PLANNER: ScalarGraphValueEditPlanner<PythonGraphValueEditRules> =
    ScalarGraphValueEditPlanner::new(PythonGraphValueEditRules);

pub(super) struct PythonGraphValueEditRules;

impl ScalarGraphValueEditRules for PythonGraphValueEditRules {
    fn format_key(&self, key: &str) -> Option<String> {
        Some(format_python_key(key))
    }

    fn format_value(
        &self,
        value: &serde_json::Value,
        _target: Option<&TreeNode>,
    ) -> Option<String> {
        Some(format_python_value(value))
    }
}

fn format_python_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "None".to_owned(),
        serde_json::Value::Bool(value) => if *value { "True" } else { "False" }.to_owned(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => quote_python_string(value),
        serde_json::Value::Array(items) => {
            let values = items.iter().map(format_python_value).collect::<Vec<_>>();
            format!("[{}]", values.join(", "))
        }
        serde_json::Value::Object(entries) => {
            let values = entries
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}: {}",
                        quote_python_string(key),
                        format_python_value(value)
                    )
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", values.join(", "))
        }
    }
}

fn format_python_key(value: &str) -> String {
    if value.contains('\'') || value.contains('\\') {
        return quote_json_string(value);
    }
    quote_python_string(value)
}

fn quote_python_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    out.push('\'');
    out
}
