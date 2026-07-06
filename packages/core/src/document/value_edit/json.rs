use crate::tree::TreeNode;

use super::edit_value_to_plain_json;
use super::scalar::{ScalarGraphValueEditPlanner, ScalarGraphValueEditRules};

pub(super) static PLANNER: ScalarGraphValueEditPlanner<JsonGraphValueEditRules> =
    ScalarGraphValueEditPlanner::new(JsonGraphValueEditRules);

pub(super) struct JsonGraphValueEditRules;

impl ScalarGraphValueEditRules for JsonGraphValueEditRules {
    fn format_subtree_value(&self, value: &serde_json::Value) -> Option<String> {
        Some(serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned()))
    }

    fn format_key(&self, key: &str) -> Option<String> {
        serde_json::to_string(key).ok()
    }

    fn format_value(
        &self,
        value: &serde_json::Value,
        _target: Option<&TreeNode>,
    ) -> Option<String> {
        Some(
            serde_json::to_string(&edit_value_to_plain_json(value))
                .unwrap_or_else(|_| "null".to_owned()),
        )
    }
}
