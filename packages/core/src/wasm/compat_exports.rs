use crate::compare::{Diff, DiffType, compare_text, diff_texts_structured};
use crate::core::{
    Language, NodeId, TreeStore,
    document_analysis::{DocumentAnalysisDemand, analyze_document_internal_with_demand},
    find_json_block_at_position,
};
use crate::wasm_types::CommonFormatOptions;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
// ── Chunk size helpers ────────────────────────────────────────────────────

/// Select an optimal chunk size based on total input bytes.
/// Mirrors `crate::stream::chunk_size::select_chunk_size`.
#[wasm_bindgen]
pub fn select_chunk_size(total_bytes: usize) -> usize {
    crate::stream::chunk_size::select_chunk_size(total_bytes)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChunkSizeConfig {
    default_chunk_size: usize,
    large_file_threshold: usize,
    large_file_chunk_size: usize,
    huge_file_threshold: usize,
    huge_file_chunk_size: usize,
}

fn to_json_compatible_js_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    // Keep serde_json::Value outputs as plain JS objects/arrays for browser consumers.
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen]
pub fn get_chunk_size_config() -> JsValue {
    let config = ChunkSizeConfig {
        default_chunk_size: crate::stream::chunk_size::DEFAULT_CHUNK_SIZE,
        large_file_threshold: crate::stream::chunk_size::LARGE_FILE_THRESHOLD,
        large_file_chunk_size: crate::stream::chunk_size::LARGE_FILE_CHUNK_SIZE,
        huge_file_threshold: crate::stream::chunk_size::HUGE_FILE_THRESHOLD,
        huge_file_chunk_size: crate::stream::chunk_size::HUGE_FILE_CHUNK_SIZE,
    };
    serde_wasm_bindgen::to_value(&config).unwrap_or(JsValue::UNDEFINED)
}

#[wasm_bindgen]
pub fn guess_language_wasm(text: String) -> Option<String> {
    crate::language::guess_language(&text)
        .and_then(Language::as_name)
        .map(str::to_string)
}

// ── Tool function input/output types ─────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParseValueToTreeInput {
    language: String,
    text: String,
    nest: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ParseValueToTreeOutput {
    tree: Option<JsonTreeNode>,
    value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatTextInput {
    language: String,
    text: String,
    indent: Option<i32>,
    nest: Option<bool>,
    sort_keys: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinifyTextInput {
    language: String,
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertTextInput {
    source_language: String,
    target_format: String,
    text: String,
    indent: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonTreeNode {
    kind: i32,
    sem_type: i32,
    tag: String,
    value: String,
    children: Vec<JsonTreeNode>,
}

#[wasm_bindgen]
pub fn parse_value_to_tree(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: ParseValueToTreeInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let result = parse_value_to_tree_impl(&input.language, &input.text, input.nest)
        .map_err(|e| JsValue::from_str(&e))?;
    to_json_compatible_js_value(&result)
}

#[wasm_bindgen]
pub fn format_text(spec: JsValue) -> Result<String, JsValue> {
    let input: FormatTextInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    format_text_impl(&input).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn minify_text(spec: JsValue) -> Result<String, JsValue> {
    let input: MinifyTextInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    minify_text_impl(&input).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn convert_text(spec: JsValue) -> Result<String, JsValue> {
    let input: ConvertTextInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    convert_text_impl(&input).map_err(|e| JsValue::from_str(&e))
}

fn format_text_impl(input: &FormatTextInput) -> Result<String, String> {
    let opts = crate::wasm::default_common_format_options();
    let options = CommonFormatOptions {
        indent: input.indent.unwrap_or(opts.indent),
        nest: input.nest.unwrap_or(opts.nest),
        ..opts
    };
    crate::wasm::format_text(&input.language, &input.text, options, input.sort_keys)
}

fn minify_text_impl(input: &MinifyTextInput) -> Result<String, String> {
    let mut options = crate::wasm::default_common_format_options();
    options.indent = 0;
    crate::wasm::format_text(&input.language, &input.text, options, None)
}

fn convert_text_impl(input: &ConvertTextInput) -> Result<String, String> {
    let opts = crate::wasm::default_common_format_options();
    let options = CommonFormatOptions {
        indent: input.indent.unwrap_or(opts.indent),
        ..opts
    };
    crate::wasm::convert_text(
        &input.source_language,
        &input.target_format,
        &input.text,
        options,
    )
}

fn parse_value_to_tree_impl(
    language: &str,
    text: &str,
    _nest: bool,
) -> Result<ParseValueToTreeOutput, String> {
    use crate::wasm::decoders::decode_value_document;
    let decoded =
        decode_value_document(language, text).map_err(|e| format!("decode failed: {e:?}"))?;
    let tree = json_tree_node_from_store(&decoded.store, decoded.root);
    let value = crate::wasm::value_json_shared::encode_document_value_json(&decoded)
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or(serde_json::Value::Null);
    Ok(ParseValueToTreeOutput { tree, value })
}

fn json_tree_node_from_store(store: &TreeStore, id: NodeId) -> Option<JsonTreeNode> {
    let node = store.get(id)?;
    Some(JsonTreeNode {
        kind: tree_kind_code(node.kind),
        sem_type: crate::wasm::sem_type_code(node.resolved_sem_type()),
        tag: node.tag.to_string_value(),
        value: store.value_string_for(id).ok()?,
        children: node
            .content
            .iter()
            .filter_map(|child| json_tree_node_from_store(store, *child))
            .collect(),
    })
}

fn tree_kind_code(kind: crate::tree::TreeNodeKind) -> i32 {
    match kind {
        crate::tree::TreeNodeKind::Sequence => 0,
        crate::tree::TreeNodeKind::Mapping => 1,
        crate::tree::TreeNodeKind::Scalar => 2,
        crate::tree::TreeNodeKind::Alias => 3,
        crate::tree::TreeNodeKind::Unknown => 4,
    }
}

// ── findJsonBlockAtPosition export ──────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindJsonBlockAtPositionInput {
    language: String,
    text: String,
    row: u32,
    column: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FindJsonBlockAtPositionOutput {
    found: bool,
    start_byte: u32,
    end_byte: u32,
    start_row: u32,
    start_column: u32,
    end_row: u32,
    end_column: u32,
}

#[wasm_bindgen]
pub fn find_json_block_at_position_wasm(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: FindJsonBlockAtPositionInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let span = find_json_block_at_position(&input.language, &input.text, input.row, input.column);
    let output = FindJsonBlockAtPositionOutput {
        found: span.found,
        start_byte: span.start_byte,
        end_byte: span.end_byte,
        start_row: span.start_row,
        start_column: span.start_column,
        end_row: span.end_row,
        end_column: span.end_column,
    };
    Ok(serde_wasm_bindgen::to_value(&output).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

// ── get_diagnostics export ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetDiagnosticsInput {
    language: String,
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GetDiagnosticsOutput {
    diagnostics: Vec<u32>,
}

#[wasm_bindgen]
pub fn get_diagnostics(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: GetDiagnosticsInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let output = get_diagnostics_impl(&input.language, &input.text);
    Ok(serde_wasm_bindgen::to_value(&output).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

fn get_diagnostics_impl(language: &str, text: &str) -> GetDiagnosticsOutput {
    let analysis = analyze_document_internal_with_demand(
        language,
        text.as_bytes(),
        false,
        DocumentAnalysisDemand::diagnostics_only(),
    );
    debug_assert!(analysis.stored.is_none());
    GetDiagnosticsOutput {
        diagnostics: analysis.diagnostics_raw,
    }
}

// ── compareStructured export ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompareStructuredInput {
    language: String,
    left: String,
    right: String,
}

#[wasm_bindgen]
pub fn compare_structured_wasm(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: CompareStructuredInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let equal = compare_structured_impl(&input.language, &input.left, &input.right)
        .map_err(|e| JsValue::from_str(&e))?;
    Ok(serde_wasm_bindgen::to_value(&equal).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

fn compare_structured_impl(language: &str, left: &str, right: &str) -> Result<bool, String> {
    crate::compare::compare_texts_structured(language, left, right)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiffInput {
    language: Option<String>,
    left: String,
    right: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffPairOutput {
    has_left: bool,
    has_right: bool,
    left: DiffOutput,
    right: DiffOutput,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffOutput {
    byte_offset: i32,
    byte_length: i32,
    #[serde(rename = "type")]
    diff_type: u8,
    inline_diffs: Vec<DiffOutput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffTextOutput {
    pairs: Vec<DiffPairOutput>,
}

fn diff_type_to_output(diff_type: DiffType) -> u8 {
    match diff_type {
        DiffType::Insert => 0,
        DiffType::Delete => 1,
    }
}

fn diff_to_output(diff: Option<&Diff>) -> DiffOutput {
    match diff {
        Some(value) => DiffOutput {
            byte_offset: value.offset,
            byte_length: value.length,
            diff_type: diff_type_to_output(value.diff_type),
            inline_diffs: value
                .inline_diffs
                .iter()
                .map(|inline| diff_to_output(Some(inline)))
                .collect(),
        },
        None => DiffOutput {
            byte_offset: 0,
            byte_length: 0,
            diff_type: diff_type_to_output(DiffType::Insert),
            inline_diffs: Vec::new(),
        },
    }
}

#[wasm_bindgen]
pub fn diff_structured_wasm(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: DiffInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let language = input
        .language
        .as_deref()
        .ok_or_else(|| JsValue::from_str("language is required"))?;
    let pairs = diff_texts_structured(language, &input.left, &input.right)
        .map_err(|e| JsValue::from_str(&e))?;
    let output = DiffTextOutput {
        pairs: pairs
            .into_iter()
            .map(|pair| DiffPairOutput {
                has_left: pair.left.is_some(),
                has_right: pair.right.is_some(),
                left: diff_to_output(pair.left.as_ref()),
                right: diff_to_output(pair.right.as_ref()),
            })
            .collect(),
    };
    to_json_compatible_js_value(&output)
}

// ── diffText export ───────────────────────────────────────────────────

#[wasm_bindgen]
pub fn diff_text_wasm(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: DiffInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let pairs = compare_text(&input.left, &input.right);
    let output = DiffTextOutput {
        pairs: pairs
            .into_iter()
            .map(|p| DiffPairOutput {
                has_left: p.left.is_some(),
                has_right: p.right.is_some(),
                left: diff_to_output(p.left.as_ref()),
                right: diff_to_output(p.right.as_ref()),
            })
            .collect(),
    };
    to_json_compatible_js_value(&output)
}

// ── applyValueEdit export ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyValueEditInput {
    language: String,
    text: String,
    path: Vec<ValueEditPathSeg>,
    prefer_key: bool,
    value: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyValueEditCanonicalOutput {
    text: String,
    tree: Option<JsonTreeNode>,
    value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValueEditPathSeg {
    tag: i32,
    key: String,
    index: i32,
}

#[wasm_bindgen]
pub fn apply_value_edit_wasm(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: ApplyValueEditInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let output = apply_value_edit_impl(&input).map_err(|e| JsValue::from_str(&e))?;
    Ok(JsValue::from_str(&output))
}
#[wasm_bindgen]
pub fn apply_value_edit_canonical_wasm(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: ApplyValueEditInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let output = apply_value_edit_canonical_impl(&input).map_err(|e| JsValue::from_str(&e))?;
    to_json_compatible_js_value(&output)
}
fn apply_value_edit_canonical_impl(
    input: &ApplyValueEditInput,
) -> Result<ApplyValueEditCanonicalOutput, String> {
    let text = apply_value_edit_impl(input)?;
    let parsed = parse_value_to_tree_impl(&input.language, &text, false)?;
    Ok(ApplyValueEditCanonicalOutput {
        text,
        tree: parsed.tree,
        value: parsed.value,
    })
}

fn apply_value_edit_impl(input: &ApplyValueEditInput) -> Result<String, String> {
    crate::tree::value_edit::apply_value_edit_text(
        &input.language,
        &input.text,
        &value_edit_path_segments(&input.path),
        input.prefer_key,
        &input.value,
    )
    .map_err(|e| e.to_string())
}

fn value_edit_path_segments(
    input: &[ValueEditPathSeg],
) -> Vec<crate::tree::value_edit::DocumentPathSeg> {
    input
        .iter()
        .map(|segment| crate::tree::value_edit::DocumentPathSeg {
            tag: segment.tag,
            key: segment.key.clone(),
            index: segment.index,
        })
        .collect()
}

// ── runYqText export ──────────────────────────────────────────────────

#[cfg(not(feature = "lite"))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunYqTextInput {
    language: String,
    text: String,
    expression: String,
    indent: Option<i32>,
}

#[cfg(not(feature = "lite"))]
#[wasm_bindgen]
pub fn run_yq_text_wasm(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: RunYqTextInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let text = run_yq_text_impl(
        &input.language,
        &input.text,
        &input.expression,
        input.indent.unwrap_or(2),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(JsValue::from_str(&text))
}

#[cfg(not(feature = "lite"))]
fn run_yq_text_impl(
    language: &str,
    text: &str,
    expression: &str,
    indent: i32,
) -> Result<String, String> {
    let output = parse_value_to_tree_impl(language, text, false)?;
    let eval_val = json_to_evaluator_value(&output.value);
    let result = crate::expression_pipeline::evaluate(&eval_val, expression)
        .map_err(|e| format!("{e:?}"))?;
    evaluator_value_to_display_text(&result, indent).map_err(|e| e.to_string())
}

#[cfg(not(feature = "lite"))]
fn evaluator_value_to_display_text(
    value: &crate::evaluator::Value,
    indent: i32,
) -> Result<String, serde_json::Error> {
    match value {
        crate::evaluator::Value::String(text) => Ok(text.clone()),
        crate::evaluator::Value::Null => Ok("null".to_owned()),
        crate::evaluator::Value::Bool(value) => Ok(value.to_string()),
        crate::evaluator::Value::Number(value) => Ok(value.to_string()),
        crate::evaluator::Value::Array(_) | crate::evaluator::Value::Object(_) => {
            let json = evaluator_value_to_json(value);
            if indent == 0 {
                serde_json::to_string(&json)
            } else {
                serde_json::to_string_pretty(&json)
            }
        }
    }
}

#[cfg(not(feature = "lite"))]
fn json_to_evaluator_value(value: &serde_json::Value) -> crate::evaluator::Value {
    use crate::evaluator::Value;
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(flag) => Value::Bool(*flag),
        serde_json::Value::Number(number) => Value::Number(number.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(text) => Value::String(text.clone()),
        serde_json::Value::Array(items) => {
            Value::Array(items.iter().map(json_to_evaluator_value).collect())
        }
        serde_json::Value::Object(entries) => Value::Object(
            entries
                .iter()
                .map(|(key, entry)| (key.clone(), json_to_evaluator_value(entry)))
                .collect(),
        ),
    }
}

#[cfg(not(feature = "lite"))]
fn evaluator_value_to_json(value: &crate::evaluator::Value) -> serde_json::Value {
    use crate::evaluator::Value;
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(flag) => serde_json::Value::Bool(*flag),
        Value::Number(number) => serde_json::json!(*number),
        Value::String(text) => serde_json::Value::String(text.clone()),
        Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(evaluator_value_to_json).collect())
        }
        Value::Object(entries) => serde_json::Value::Object(
            entries
                .iter()
                .map(|(key, entry)| (key.clone(), evaluator_value_to_json(entry)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm_types::PathSegTag;

    #[cfg(not(feature = "lite"))]
    #[test]
    fn run_yq_text_returns_display_ready_scalar_string_and_document_text() {
        crate::wasm::init_wasm();
        assert_eq!(
            run_yq_text_impl(
                "json",
                r#"{"items":[{"name":"Alice"}]}"#,
                ".items[0].name",
                2
            )
            .expect("scalar string should render"),
            "Alice",
        );
        assert_eq!(
            run_yq_text_impl("json", r#"{"enabled":true}"#, ".enabled", 2)
                .expect("scalar bool should render"),
            "true",
        );
        assert_eq!(
            run_yq_text_impl("json", r#"{"items":[{"name":"Alice"}]}"#, ".items[0]", 2)
                .expect("document should render"),
            "{\n  \"name\": \"Alice\"\n}",
        );
    }

    #[test]
    fn compat_exports_format_minify_and_convert_text_preserve_behavior() {
        crate::wasm::init_wasm();

        assert_eq!(
            format_text_impl(&FormatTextInput {
                language: "json".into(),
                text: "{\"b\":2,\"a\":1}".into(),
                indent: Some(2),
                nest: None,
                sort_keys: Some(true),
            })
            .expect("format text should succeed"),
            "{\"a\": 1, \"b\": 2}\n",
        );
        assert_eq!(
            format_text_impl(&FormatTextInput {
                language: "json".into(),
                text: "{\"nested\":\"{\\\"inner\\\":42}\"}".into(),
                indent: Some(2),
                nest: Some(true),
                sort_keys: Some(false),
            })
            .expect("nest-aware format text should succeed"),
            "{\n  \"nested\": {\"inner\": 42}\n}\n",
        );
        assert_eq!(
            minify_text_impl(&MinifyTextInput {
                language: "json".into(),
                text: "{\n  \"a\": 1,\n  \"b\": 2\n}".into(),
            })
            .expect("minify text should succeed"),
            "{\"a\":1,\"b\":2}\n",
        );
        #[cfg(not(feature = "lite"))]
        assert_eq!(
            convert_text_impl(&ConvertTextInput {
                source_language: "json".into(),
                target_format: "yaml".into(),
                text: "{\"a\":1}".into(),
                indent: Some(2),
            })
            .expect("convert text should succeed"),
            "a: 1\n",
        );
        #[cfg(not(feature = "lite"))]
        assert_eq!(
            convert_text_impl(&ConvertTextInput {
                source_language: "csv".into(),
                target_format: "json".into(),
                text: "name,age\nAlice,18\nBob,20\n".into(),
                indent: Some(2),
            })
            .expect("csv to json convert should succeed"),
            "[\n  {\"name\": \"Alice\", \"age\": 18},\n  {\"name\": \"Bob\", \"age\": 20}\n]\n",
        );
    }

    #[test]
    fn compat_exports_parse_compare_and_diagnostics_preserve_behavior() {
        crate::wasm::init_wasm();

        let parsed = parse_value_to_tree_impl("json", "{\"name\":\"Ada\"}", false)
            .expect("parse value to tree should succeed");
        assert_eq!(parsed.value, serde_json::json!({ "name": "Ada" }));

        assert!(
            compare_structured_impl("json", "{\"a\":1,\"b\":2}", "{\n  \"a\": 1,\n  \"b\": 2\n}",)
                .expect("compare structured should succeed")
        );

        let diagnostics = get_diagnostics_impl("json", "{\"a\": }");
        assert!(!diagnostics.diagnostics.is_empty());
    }

    #[test]
    fn compat_exports_apply_value_edit_rewrites_existing_json_path() {
        crate::wasm::init_wasm();

        let output = apply_value_edit_impl(&ApplyValueEditInput {
            language: "json".into(),
            text: r#"{"arr":[1,2,3]}"#.into(),
            path: vec![
                ValueEditPathSeg {
                    tag: PathSegTag::KEY_VALUE as i32,
                    key: "arr".into(),
                    index: 0,
                },
                ValueEditPathSeg {
                    tag: PathSegTag::INDEX_VALUE as i32,
                    key: String::new(),
                    index: 1,
                },
            ],
            prefer_key: false,
            value: r#""test""#.into(),
        })
        .expect("existing json path rewrite should succeed");

        let value: serde_json::Value =
            serde_json::from_str(&output).expect("edited json should stay valid");
        assert_eq!(value, serde_json::json!({ "arr": [1, "test", 3] }));
    }
    #[test]
    fn compat_exports_apply_value_edit_canonical_returns_text_tree_and_value() {
        crate::wasm::init_wasm();
        let output = apply_value_edit_canonical_impl(&ApplyValueEditInput {
            language: "json".into(),
            text: r#"{"arr":[1,2,3]}"#.into(),
            path: vec![
                ValueEditPathSeg {
                    tag: PathSegTag::KEY_VALUE as i32,
                    key: "arr".into(),
                    index: 0,
                },
                ValueEditPathSeg {
                    tag: PathSegTag::INDEX_VALUE as i32,
                    key: String::new(),
                    index: 1,
                },
            ],
            prefer_key: false,
            value: r#""test""#.into(),
        })
        .expect("canonical apply should succeed");
        let reparsed: serde_json::Value =
            serde_json::from_str(&output.text).expect("canonical text should stay valid json");
        assert_eq!(reparsed, serde_json::json!({ "arr": [1, "test", 3] }));
        assert_eq!(output.value, serde_json::json!({ "arr": [1, "test", 3] }));
        assert!(output.tree.is_some());
    }
}
