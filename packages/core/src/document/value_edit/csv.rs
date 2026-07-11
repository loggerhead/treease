use crate::tree::TreeNode;

use super::edit_value_as_scalar_string;
use super::scalar::{ScalarGraphValueEditPlanner, ScalarGraphValueEditRules};

static PLANNER: ScalarGraphValueEditPlanner<CsvGraphValueEditRules> =
    ScalarGraphValueEditPlanner::new(CsvGraphValueEditRules);

pub(super) struct CsvGraphValueEditRules;

pub(crate) fn planner() -> &'static dyn super::GraphValueEditPlanner {
    &PLANNER
}

impl ScalarGraphValueEditRules for CsvGraphValueEditRules {
    fn supports_key_edit(&self) -> bool {
        true
    }
    fn format_key(&self, key: &str) -> Option<String> {
        Some(format_csv_field(key))
    }
    fn format_value(
        &self,
        value: &serde_json::Value,
        _target: Option<&TreeNode>,
    ) -> Option<String> {
        Some(format_csv_field(&edit_value_as_scalar_string(value)))
    }
}

fn format_csv_field(value: &str) -> String {
    if !value
        .chars()
        .any(|ch| matches!(ch, ',' | '"' | '\n' | '\r'))
    {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
    out
}
