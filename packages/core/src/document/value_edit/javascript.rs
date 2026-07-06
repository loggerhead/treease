use crate::formats::encoder_javascript::is_js_identifier;
use crate::tree::TreeNode;

use super::quote_json_string;
use super::scalar::{ScalarGraphValueEditPlanner, ScalarGraphValueEditRules};

pub(super) static PLANNER: ScalarGraphValueEditPlanner<JavascriptGraphValueEditRules> =
    ScalarGraphValueEditPlanner::new(JavascriptGraphValueEditRules);

pub(super) struct JavascriptGraphValueEditRules;

impl ScalarGraphValueEditRules for JavascriptGraphValueEditRules {
    fn format_key(&self, key: &str) -> Option<String> {
        Some(format_javascript_key(key))
    }

    fn format_value(
        &self,
        value: &serde_json::Value,
        _target: Option<&TreeNode>,
    ) -> Option<String> {
        Some(format_javascript_value(value))
    }
}

fn format_javascript_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_owned(),
        serde_json::Value::Bool(value) => if *value { "true" } else { "false" }.to_owned(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => quote_json_string(value),
        serde_json::Value::Array(items) => {
            let values = items
                .iter()
                .map(format_javascript_value)
                .collect::<Vec<_>>();
            format!("[{}]", values.join(", "))
        }
        serde_json::Value::Object(entries) => {
            let values = entries
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}: {}",
                        format_javascript_key(key),
                        format_javascript_value(value)
                    )
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", values.join(", "))
        }
    }
}

fn format_javascript_key(key: &str) -> String {
    if is_js_identifier(key) {
        key.to_owned()
    } else {
        quote_json_string(key)
    }
}
