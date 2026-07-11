#[cfg(not(feature = "lite"))]
pub(crate) mod csv;
pub(crate) mod javascript;
pub(crate) mod json;
#[cfg(not(feature = "lite"))]
pub(crate) mod python;
mod scalar;
#[cfg(not(feature = "lite"))]
pub(crate) mod toml;
#[cfg(not(feature = "lite"))]
pub(crate) mod yaml;

use crate::formats::DecodedDocument;
use crate::tree::DocumentTextEdit;
use crate::wasm_types::{PathSeg, PathSpan};

use super::protocol::{
    GraphPathSeg, GraphValueEditFallbackReason, GraphValueEditPlan, GraphValueEditPlanMode,
    GraphValueEditRequest,
};
use super::snapshot::AnalysisBundle;

pub(crate) struct GraphValueEditContext<'a> {
    pub analysis: &'a AnalysisBundle,
    pub document: &'a DecodedDocument,
    pub request: &'a GraphValueEditRequest,
    pub path_index: Option<&'a crate::tree::TreePathIndex>,
}

pub(crate) trait GraphValueEditPlanner: Sync {
    fn plan(&self, ctx: GraphValueEditContext<'_>) -> GraphValueEditPlan;
}

pub(crate) fn plan_graph_value_edit(
    analysis: &AnalysisBundle,
    document: &DecodedDocument,
    request: &GraphValueEditRequest,
    path_index: Option<&crate::tree::TreePathIndex>,
) -> GraphValueEditPlan {
    let context = GraphValueEditContext {
        analysis,
        document,
        request,
        path_index,
    };
    crate::language::capability::plan_graph_value_edit_with_capability(&analysis.language, context)
        .unwrap_or_else(|_| {
            graph_value_edit_fallback(GraphValueEditFallbackReason::UnsupportedLanguage)
        })
}

pub(crate) fn graph_value_edit_fallback(
    reason: GraphValueEditFallbackReason,
) -> GraphValueEditPlan {
    GraphValueEditPlan {
        mode: GraphValueEditPlanMode::Replace,
        edits: Vec::new(),
        reason: Some(reason),
    }
}

pub(super) fn graph_value_edit_edits(edits: Vec<DocumentTextEdit>) -> GraphValueEditPlan {
    GraphValueEditPlan {
        mode: GraphValueEditPlanMode::Edits,
        edits,
        reason: None,
    }
}

pub(super) fn request_path_segments(path: &[GraphPathSeg]) -> Vec<PathSeg<'_>> {
    path.iter()
        .map(|segment| {
            if segment.tag == 1 {
                crate::tree::path_seg_index(segment.index)
            } else {
                crate::tree::path_seg_key(&segment.key)
            }
        })
        .collect()
}

pub(super) fn span_to_edit(span: PathSpan, replacement: String) -> Option<DocumentTextEdit> {
    if span.start_byte < 0 || span.end_byte < span.start_byte {
        return None;
    }
    let start_byte = u32::try_from(span.start_byte).ok()?;
    let old_end_byte = u32::try_from(span.end_byte).ok()?;
    let new_end_byte = start_byte.checked_add(replacement.len() as u32)?;
    Some(DocumentTextEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        replacement,
    })
}

pub(super) fn edit_value_as_scalar_string(value: &serde_json::Value) -> String {
    match edit_value_to_plain_json(value) {
        serde_json::Value::String(value) => value,
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::Bool(value) => {
            if value {
                "true".to_owned()
            } else {
                "false".to_owned()
            }
        }
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub(super) fn edit_value_to_plain_json(value: &serde_json::Value) -> serde_json::Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    if !(object.contains_key("kind") && object.contains_key("children")) {
        return value.clone();
    }
    let kind = object
        .get("kind")
        .and_then(|value| value.as_i64())
        .unwrap_or(2);
    match kind {
        0 => serde_json::Value::Array(
            object
                .get("children")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .map(edit_value_to_plain_json)
                .collect(),
        ),
        1 => {
            let mut out = serde_json::Map::new();
            let children = object
                .get("children")
                .and_then(|value| value.as_array())
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let mut index = 0usize;
            while index + 1 < children.len() {
                let key = edit_value_as_scalar_string(&children[index]);
                out.insert(key, edit_value_to_plain_json(&children[index + 1]));
                index += 2;
            }
            serde_json::Value::Object(out)
        }
        2 | 3 => {
            let raw = object
                .get("value")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            match object
                .get("semType")
                .and_then(|value| value.as_i64())
                .unwrap_or(2)
            {
                3 => raw
                    .parse::<i64>()
                    .map(serde_json::Value::from)
                    .unwrap_or_else(|_| serde_json::Value::String(raw.to_owned())),
                4 => raw
                    .parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                    .map(serde_json::Value::Number)
                    .unwrap_or_else(|| serde_json::Value::String(raw.to_owned())),
                5 => serde_json::Value::Bool(matches!(
                    raw.trim().to_ascii_lowercase().as_str(),
                    "true" | "yes" | "y"
                )),
                6 => serde_json::Value::Null,
                _ => serde_json::Value::String(raw.to_owned()),
            }
        }
        _ => object
            .get("value")
            .and_then(|value| value.as_str())
            .map(|value| serde_json::Value::String(value.to_owned()))
            .unwrap_or_else(|| value.clone()),
    }
}

pub(super) fn quote_json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}
