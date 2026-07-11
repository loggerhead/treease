// ── Document Job WASM Façade (wasm-bindgen) ──────────────────────────────

// ── Document Job WASM Façade (wasm-bindgen) ──────────────────────────────

use crate::document::job::entry::DocumentJobHandle;
use crate::document::job::{advance_global_job, cancel_global_job, start_global_job};
use crate::document::protocol::{
    AdvanceInput, DocumentInputPlan, DocumentJobKind, DocumentJobSettings, DocumentJobSpec,
    EventBatch, GraphValueEditPlan, GraphValueEditRequest, OutputPlan, ProjectionRequest,
    QueryKind, QueryResult, QueryTargetKind, SnapshotId, SnapshotQuery, SnapshotReadResult,
};
use crate::document::runtime::{
    build_global_hover_subgraph_projection, plan_global_graph_value_edit, query_global_snapshot,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// ── Request types ────────────────────────────────────────────────────────

pub(super) struct StartDocumentJobRequest {
    pub(super) document_key: String,
    pub(super) language: String,
    pub(super) output_graph: bool,
    pub(super) output_analysis: bool,
    pub(super) builder_config: Option<BuilderConfigInput>,
    pub(super) base_snapshot_id: Option<SnapshotId>,
    pub(super) edits: Vec<crate::tree::incremental_edit::DocumentTextEdit>,
    pub(super) settings: DocumentJobSettings,
}

pub(super) struct QuerySnapshotRequest {
    pub(super) document_key: String,
    pub(super) snapshot_id: u32,
    pub(super) query_kind: u8,
    pub(super) has_path: bool,
    pub(super) path_pattern: String,
    pub(super) span_start: u32,
    pub(super) span_end: u32,
    pub(super) target: QueryTargetKind,
}

// ── Serde JSON input/output types ───────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BuilderConfigInput {
    key_width: Option<i32>,
    value_width: Option<i32>,
    row_height: Option<i32>,
    row_padding_x: Option<i32>,
    row_padding_y: Option<i32>,
    node_border_width: Option<i32>,
    v_gap: Option<i32>,
    h_gap: Option<i32>,
    table_max_height: Option<i32>,
    table_row_height: Option<i32>,
    table_header_height: Option<i32>,
    table_column_width: Option<i32>,
    avg_char_width_x10: Option<i32>,
    font_size: Option<i32>,
    meta_path_min_segments: Option<i32>,
    meta_path_min_chars: Option<i32>,
    meta_path_keep_tail_segments: Option<i32>,
    corner_radius: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartDocumentJobInput {
    document_key: String,
    language: String,
    #[allow(dead_code)] // kept for WASM ABI backward compatibility
    #[serde(default)]
    text: String,
    #[allow(dead_code)] // kept for WASM ABI backward compatibility
    #[serde(default)]
    nest: bool,
    output_graph: bool,
    output_analysis: bool,
    builder_config: Option<BuilderConfigInput>,
    #[serde(default)]
    base_snapshot_id: Option<SnapshotId>,
    #[serde(default)]
    edits: Vec<crate::tree::incremental_edit::DocumentTextEdit>,
    #[serde(default)]
    settings: Option<DocumentJobSettings>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CancelDocumentJobInput {
    job_handle: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuerySnapshotInput {
    document_key: String,
    snapshot_id: u32,
    query_kind: QueryKind,
    path_pattern: Option<String>,
    span_start: Option<u32>,
    span_end: Option<u32>,
    target: Option<QueryTargetKind>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildHoverSubgraphProjectionInput {
    snapshot_id: u32,
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartDocumentJobOutput {
    job_handle: u64,
    batch: EventBatch,
}

// ── wasm-bindgen exports ─────────────────────────────────────────────────
fn normalize_job_settings(input: &StartDocumentJobInput) -> DocumentJobSettings {
    let mut settings = input.settings.unwrap_or_default();
    if input.nest {
        settings.parser.enable_nest = true;
    }
    settings.parser.nest_max_depth = settings.parser.nest_max_depth.min(8);
    settings
}

#[wasm_bindgen]
pub fn start_document_job(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: StartDocumentJobInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let settings = normalize_job_settings(&input);
    let req = StartDocumentJobRequest {
        document_key: input.document_key,
        language: input.language,
        output_graph: input.output_graph,
        output_analysis: input.output_analysis,
        builder_config: input.builder_config,
        base_snapshot_id: input.base_snapshot_id,
        edits: input.edits,
        settings,
    };
    let result = start_document_job_impl(req).map_err(|e| JsValue::from_str(&e))?;
    Ok(serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

#[wasm_bindgen]
pub fn cancel_document_job(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: CancelDocumentJobInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let batch = cancel_document_job_impl(input.job_handle);
    Ok(serde_wasm_bindgen::to_value(&batch).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdvanceDocumentJobInput {
    job_handle: u32,
    advance_kind: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    data: Option<Vec<u8>>,
}

#[wasm_bindgen]
pub fn advance_document_job(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: AdvanceDocumentJobInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let batch = advance_document_job_impl(
        input.job_handle,
        &input.advance_kind,
        input.text.as_deref(),
        input.data.as_deref(),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(serde_wasm_bindgen::to_value(&batch).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

#[wasm_bindgen]
pub fn query_snapshot(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: QuerySnapshotInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let req = QuerySnapshotRequest {
        document_key: input.document_key,
        snapshot_id: input.snapshot_id,
        query_kind: input.query_kind as u8,
        has_path: input.path_pattern.is_some(),
        path_pattern: input.path_pattern.unwrap_or_default(),
        span_start: input.span_start.unwrap_or(0),
        span_end: input.span_end.unwrap_or(0),
        target: input.target.unwrap_or(QueryTargetKind::Value),
    };
    let result = query_snapshot_impl(req).map_err(|e| JsValue::from_str(&e))?;
    Ok(serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

#[wasm_bindgen]
pub fn build_hover_subgraph_projection(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: BuildHoverSubgraphProjectionInput =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let req = ProjectionRequest {
        snapshot_id: SnapshotId(input.snapshot_id as u64),
        path: input.path,
    };
    let result = build_global_hover_subgraph_projection(&req).map_err(JsValue::from_str)?;
    Ok(serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

#[wasm_bindgen]
pub fn plan_graph_value_edit(spec: JsValue) -> Result<JsValue, JsValue> {
    let input: GraphValueEditRequest =
        serde_wasm_bindgen::from_value(spec).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let result = plan_graph_value_edit_impl(input).map_err(|e| JsValue::from_str(&e))?;
    Ok(serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

// ── Implementation functions ─────────────────────────────────────────────

fn apply_builder_config(config: Option<&BuilderConfigInput>) {
    if let Some(c) = config {
        use crate::graph::graph_projection_service::BuilderConfigState;
        let merged = BuilderConfigState::default();
        let state = BuilderConfigState {
            key_width: c.key_width.unwrap_or(merged.key_width),
            value_width: c.value_width.unwrap_or(merged.value_width),
            row_height: c.row_height.unwrap_or(merged.row_height),
            row_padding_x: c.row_padding_x.unwrap_or(merged.row_padding_x),
            row_padding_y: c.row_padding_y.unwrap_or(merged.row_padding_y),
            node_border_width: c.node_border_width.unwrap_or(merged.node_border_width),
            v_gap: c.v_gap.unwrap_or(merged.v_gap),
            h_gap: c.h_gap.unwrap_or(merged.h_gap),
            table_max_height: c.table_max_height.unwrap_or(merged.table_max_height),
            table_row_height: c.table_row_height.unwrap_or(merged.table_row_height),
            table_header_height: c.table_header_height.unwrap_or(merged.table_header_height),
            table_column_width: c.table_column_width.unwrap_or(merged.table_column_width),
            avg_char_width_x10: c.avg_char_width_x10.unwrap_or(merged.avg_char_width_x10),
            font_size: c.font_size.unwrap_or(merged.font_size),
            meta_path_min_segments: c
                .meta_path_min_segments
                .unwrap_or(merged.meta_path_min_segments),
            meta_path_min_chars: c.meta_path_min_chars.unwrap_or(merged.meta_path_min_chars),
            meta_path_keep_tail_segments: c
                .meta_path_keep_tail_segments
                .unwrap_or(merged.meta_path_keep_tail_segments),
            corner_radius: c.corner_radius.unwrap_or(merged.corner_radius),
            expand_header_table_rows: merged.expand_header_table_rows,
        };
        crate::graph::graph_projection_service::set_builder_config(state);
    }
}

fn start_document_job_impl(req: StartDocumentJobRequest) -> Result<StartDocumentJobOutput, String> {
    apply_builder_config(req.builder_config.as_ref());

    let has_edits = !req.edits.is_empty();
    let spec = DocumentJobSpec {
        kind: if has_edits {
            DocumentJobKind::ApplyEdits
        } else {
            DocumentJobKind::AnalyzeSource
        },
        document_key: req.document_key.clone(),
        language: req.language.clone(),
        input: if has_edits {
            DocumentInputPlan::BaseTextWithEdits
        } else {
            DocumentInputPlan::SourceText
        },
        settings: req.settings,
        output: OutputPlan {
            analysis: req.output_analysis,
            graph: req.output_graph,
        },
        base_snapshot_id: req.base_snapshot_id,
        edits: req.edits,
    };
    let started = start_global_job(spec).map_err(|_| "failed to start job".to_string())?;

    // Start the job without feeding any source. The caller feeds chunks
    // via advance_document_job and closes when done. For one-shot usage,
    // callers should use the convenience wrapper that bundles start+feed+close.
    Ok(StartDocumentJobOutput {
        job_handle: started.handle.0,
        batch: EventBatch {
            request_seq: started.request_seq,
            events: Vec::new(),
            terminal: None,
        },
    })
}

fn cancel_document_job_impl(handle_raw: u32) -> EventBatch {
    let handle = DocumentJobHandle(handle_raw as u64);
    cancel_global_job(handle).unwrap_or(EventBatch {
        request_seq: 0,
        events: Vec::new(),
        terminal: None,
    })
}

fn advance_document_job_impl(
    handle_raw: u32,
    kind: &str,
    text: Option<&str>,
    data: Option<&[u8]>,
) -> Result<EventBatch, String> {
    let handle = DocumentJobHandle(handle_raw as u64);
    let input = match kind {
        "textChunk" => {
            let t = text.ok_or_else(|| "text field required for textChunk".to_string())?;
            AdvanceInput::TextChunk(t.to_owned())
        }
        "close" => AdvanceInput::Close,
        "poll" => AdvanceInput::Poll,
        "binaryChunk" => {
            let d = data.ok_or_else(|| "data field required for binaryChunk".to_string())?;
            AdvanceInput::BinaryChunk(d.to_vec())
        }
        other => return Err(format!("unknown advance kind: {other}")),
    };
    advance_global_job(handle, input).map_err(|_| "engine advance failed".to_string())
}

fn query_snapshot_impl(
    req: QuerySnapshotRequest,
) -> Result<SnapshotReadResult<QueryResult>, String> {
    let requested_snapshot_id = req.snapshot_id as u64;
    let kind = match req.query_kind {
        0 => QueryKind::FindAnchors,
        1 => QueryKind::ResolvePath,
        2 => QueryKind::ResolveHover,
        3 => QueryKind::RootValueKind,
        4 => QueryKind::NodePreview,
        5 => QueryKind::PathValue,
        6 => QueryKind::FieldLabels,
        _ => QueryKind::SearchIndex,
    };
    let query = SnapshotQuery {
        snapshot_id: SnapshotId(requested_snapshot_id),
        kind,
        path_pattern: if req.has_path {
            Some(req.path_pattern.clone())
        } else {
            None
        },
        span: if matches!(kind, QueryKind::ResolvePath | QueryKind::ResolveHover) {
            Some((req.span_start, req.span_end))
        } else {
            None
        },
        target: Some(req.target),
    };
    query_global_snapshot(&req.document_key, &query).map_err(|_| "query runtime error".to_string())
}

fn plan_graph_value_edit_impl(
    request: GraphValueEditRequest,
) -> Result<SnapshotReadResult<GraphValueEditPlan>, String> {
    plan_global_graph_value_edit(&request)
        .map_err(|_| "plan graph value edit runtime error".to_string())
}
#[cfg(test)]
mod tests;
