use serde::{Deserialize, Serialize};
use tsify::Tsify;

use crate::tree::incremental_edit::DocumentTextEdit;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Tsify)]
#[serde(transparent)]
pub struct SnapshotId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum CommitMode {
    #[default]
    Authoritative,
    DiagnosticsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum DocumentJobKind {
    #[default]
    AnalyzeSource,
    ApplyEdits,
}

// ── Input / Output plans ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum DocumentInputPlan {
    #[default]
    SourceText,
    BaseTextWithEdits,
    ByteStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct OutputPlan {
    pub analysis: bool,
    pub graph: bool,
}

// ── Document job ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentParserSettings {
    pub enable_nest: bool,
    pub nest_max_depth: u8,
}

impl Default for DocumentParserSettings {
    fn default() -> Self {
        Self {
            enable_nest: false,
            nest_max_depth: 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFormattingSettings {
    pub indent: i32,
    pub smart: bool,
    pub format_source_on_close: bool,
    pub max_line_length: i32,
    pub max_inline_complexity: i32,
    pub max_array_inline_items: i32,
    pub align_object_arrays: bool,
}

impl Default for DocumentFormattingSettings {
    fn default() -> Self {
        Self {
            indent: 2,
            smart: false,
            format_source_on_close: true,
            max_line_length: 100,
            max_inline_complexity: 1,
            max_array_inline_items: 6,
            align_object_arrays: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentJobSettings {
    pub parser: DocumentParserSettings,
    pub formatting: DocumentFormattingSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentJobSpec {
    pub kind: DocumentJobKind,
    pub document_key: String,
    pub language: String,
    pub input: DocumentInputPlan,
    #[serde(default)]
    pub settings: DocumentJobSettings,
    pub output: OutputPlan,
    pub base_snapshot_id: Option<SnapshotId>,
    #[tsify(type = "any[]")]
    pub edits: Vec<DocumentTextEdit>,
}

// ── Graph delta types (serde-wasm-bindgen wire format) ──────────────────

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphPathSeg {
    pub tag: i32,
    pub key: String,
    pub index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum GraphValueEditPlanMode {
    #[default]
    Edits,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum GraphValueEditFallbackReason {
    #[default]
    UnsupportedEdit,
    SnapshotNotReady,
    MissingAnalysis,
    MissingDocument,
    InvalidPath,
    InvalidReplacement,
    UnsupportedLanguage,
    UnsafeEdit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphValueEditRequest {
    pub document_key: String,
    pub snapshot_id: SnapshotId,
    pub language: String,
    pub path: Vec<GraphPathSeg>,
    pub prefer_key: bool,
    #[tsify(type = "any")]
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphValueEditPlan {
    pub mode: GraphValueEditPlanMode,
    pub edits: Vec<DocumentTextEdit>,
    pub reason: Option<GraphValueEditFallbackReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphBoxArgs {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub corner_radius: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphTextArgs {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text: String,
    pub text_align: u8,
    pub text_vertical_align: u8,
    pub editable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphCellData {
    pub sem_type: u32,
    pub is_missing: bool,
    pub path: Vec<GraphPathSeg>,
    pub text: String,
    pub value: String,
    pub format_text: String,
    pub box_args: GraphBoxArgs,
    pub text_args: GraphTextArgs,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphRowData {
    pub index: i32,
    pub box_args: GraphBoxArgs,
    pub cell_box_args: GraphBoxArgs,
    pub cells: Vec<GraphCellData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphTableData {
    pub columns: Vec<GraphCellData>,
    pub rows: Vec<GraphRowData>,
    pub header_height: i32,
    pub total_height: i32,
    pub view_height: i32,
    pub row_height: i32,
}
/// A node in the graph projection tree. Represents a value (scalar, collection,
/// table, etc.) at a specific document path.
///
/// ## Table metadata contract
///
/// When `table` is `Some(…)`, this node represents a table-type value.
///
/// - **Initial projection** (first `GraphDelta` with `clear: true`):
///   `table` is fully populated — `columns`, `rows`, and sizing info are all
///   present. Consumers can read `table.columns[].text` for column headers
///   directly from the node.
///
/// - **Incremental updates** (subsequent deltas with `clear: false`):
///   Table-type nodes do NOT appear in `nodes_added` / `nodes_updated`.
///   Table mutations are communicated exclusively via
///   [`GraphDelta::table_patches`] (see [`TablePatch`] variants).
///   Consumers MUST NOT rely on `table` being present on incremental nodes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphNodeData {
    pub render_handle: u32,
    pub kind: i32,
    pub path: Vec<GraphPathSeg>,
    pub depth: u32,
    pub box_args: GraphBoxArgs,
    pub meta: Option<GraphCellData>,
    pub rows: Vec<GraphRowData>,
    /// Table metadata for table-type nodes.
    ///
    /// **Contract:** populated on initial projection (`clear: true`); absent on
    /// incremental updates — see struct-level docs for details.
    /// Consumers that need table column info after incremental updates should
    /// read [`GraphDelta::table_patches`] instead.
    pub table: Option<GraphTableData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphBezierArgsData {
    pub from_x: i32,
    pub from_y: i32,
    pub c1x: i32,
    pub c1y: i32,
    pub c2x: i32,
    pub c2y: i32,
    pub to_x: i32,
    pub to_y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdgeData {
    pub from_render_handle: u32,
    pub from_kind: i32,
    pub from_path: Vec<GraphPathSeg>,
    pub from_row: i32,
    pub to_render_handle: u32,
    pub to_kind: i32,
    pub to_path: Vec<GraphPathSeg>,
    pub to_row: i32,
    pub bezier_args: GraphBezierArgsData,
    pub bezier_from_x: i32,
    pub bezier_from_y: i32,
    pub bezier_c1x: i32,
    pub bezier_c1y: i32,
    pub bezier_c2x: i32,
    pub bezier_c2y: i32,
    pub bezier_to_x: i32,
    pub bezier_to_y: i32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdgeRemoved {
    pub from: u32,
    pub to: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct GraphDelta {
    pub nodes_added: Vec<GraphNodeData>,
    pub nodes_updated: Vec<GraphNodeData>,
    pub nodes_removed: Vec<u32>,
    pub edges_added: Vec<GraphEdgeData>,
    pub edges_removed: Vec<GraphEdgeRemoved>,
    /// Incremental table patches — the authoritative source for table mutations
    /// in streaming graph projection.
    ///
    /// **Contract:** On initial projection (`clear: true`), a `TablePatch::TableCreated`
    /// patch is emitted alongside the full `GraphNodeData.table` on the
    /// corresponding node. On incremental updates (`clear: false`), table
    /// mutations are communicated **exclusively** through these patches
    /// (RowsAppended, ColumnsAdded, CellsUpdated, TableReplaced). Consumers that maintain
    /// table state across deltas MUST subscribe to `table_patches` rather than
    /// reading `GraphNodeData.table` from incremental nodes.
    #[serde(default)]
    pub table_patches: Vec<TablePatch>,
    /// Incremental layout patches (streaming optimization).
    #[serde(default)]
    pub layout_patches: Vec<LayoutPatch>,
}
// ── Table / Layout patch types (streaming extension) ─────────────────────

/// Incremental table mutation emitted during streaming graph projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TablePatch {
    /// A new table node is created with its initial columns.
    TableCreated {
        table_handle: u32,
        columns: Vec<GraphCellData>,
    },
    /// One or more rows are appended to an existing table.
    /// Carries updated table sizing so consumers can resize the
    /// viewport without a full node re-send.
    RowsAppended {
        table_handle: u32,
        /// Row index of the first new row (must equal current row count).
        start_index: u32,
        rows: Vec<GraphRowData>,
        total_height: i32,
        view_height: i32,
        header_height: i32,
        row_height: i32,
    },
    /// Specific cells in an existing table are updated.
    CellsUpdated {
        table_handle: u32,
        cells: Vec<TableCellPatchData>,
    },
    /// New columns are added to an existing table.
    ColumnsAdded {
        table_handle: u32,
        columns: Vec<GraphCellData>,
    },
    /// Replace a table's complete state after deferred streaming geometry
    /// has finalized. This is emitted as a table patch rather than an
    /// incremental `nodes_updated.table` payload.
    TableReplaced {
        table_handle: u32,
        table: GraphTableData,
    },
}

/// A single cell mutation within a table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct TableCellPatchData {
    pub row_index: u32,
    pub column_index: u32,
    pub cell: GraphCellData,
}

/// Incremental layout hint emitted during streaming graph projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LayoutPatch {
    /// A single node's bounding box has been updated.
    NodeBoundsUpdated {
        render_handle: u32,
        box_args: GraphBoxArgs,
    },
    /// A container's (group) layout summary has changed.
    GroupLayoutUpdated {
        group_handle: u32,
        width: i32,
        height: i32,
    },
    /// Viewport-level layout hint (e.g. total height after appending).
    ViewportLayoutHint {
        total_height: i32,
        appended_height: i32,
    },
}
// ── Analysis payloads ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDiagnostic {
    pub start_line_number: u32,
    pub start_column: u32,
    pub end_line_number: u32,
    pub end_column: u32,
    pub kind: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensPayload {
    pub data: Vec<u32>,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTreeNode {
    pub kind: i32,
    pub sem_type: i32,
    pub tag: String,
    pub value: String,
    pub children: Vec<DocumentTreeNode>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentAnalysisPayload {
    pub tree: Option<DocumentTreeNode>,
    #[serde(default, rename = "valueJson")]
    pub value_json: Option<String>,
    pub diagnostics: Vec<DocumentDiagnostic>,
    pub semantic_tokens: SemanticTokensPayload,
    pub source_byte_length: u32,
    #[serde(default)]
    pub source_line_count: u32,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotReady {
    #[serde(rename = "snapshotId")]
    pub snapshot_id: SnapshotId,
    pub analysis: Option<DocumentAnalysisPayload>,
    pub main_graph: Option<ProjectionDelta>,
    #[serde(default, rename = "sourceText")]
    pub source_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ParseFailed {
    #[serde(rename = "snapshotId")]
    pub snapshot_id: SnapshotId,
    pub analysis: DocumentAnalysisPayload,
}

/// A request to build a hover subgraph projection on an existing snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionRequest {
    pub snapshot_id: SnapshotId,
    pub path: String,
}

// ── Projection ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionDelta {
    pub clear: bool,
    pub graph_data: Option<GraphDelta>,
    /// Monotonically-increasing patch sequence number within this job.
    #[serde(default)]
    pub patch_seq: u64,
    /// Graph version the UI must be at to apply this patch.
    #[serde(default)]
    pub base_graph_version: u64,
    /// Graph version after applying this patch.
    #[serde(default)]
    pub graph_version: u64,
}

// ── Advance / events ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum AdvanceInput {
    #[default]
    Poll,
    TextChunk(String),
    BinaryChunk(Vec<u8>),
    Close,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DocumentEvent {
    Progress {
        #[serde(rename = "processedBytes")]
        processed_bytes: u32,
    },
    AnalysisDelta {
        analysis: DocumentAnalysisPayload,
    },
    SnapshotReady {
        #[serde(rename = "snapshotId")]
        snapshot_id: SnapshotId,
        analysis: Option<DocumentAnalysisPayload>,
        #[serde(rename = "mainGraph")]
        main_graph: Option<ProjectionDelta>,
        #[serde(default, rename = "sourceText")]
        source_text: Option<String>,
    },
    ParseFailed {
        #[serde(rename = "snapshotId")]
        snapshot_id: SnapshotId,
        analysis: DocumentAnalysisPayload,
    },
    /// Incremental graph projection update emitted during streaming decode.
    ProjectionDelta {
        clear: bool,
        #[serde(rename = "graphData")]
        graph_data: Option<GraphDelta>,
        #[serde(default, rename = "patchSeq")]
        patch_seq: u64,
        #[serde(default, rename = "baseGraphVersion")]
        base_graph_version: u64,
        #[serde(default, rename = "graphVersion")]
        graph_version: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct EventBatch {
    #[serde(rename = "requestSeq")]
    pub request_seq: u64,
    pub events: Vec<DocumentEvent>,
    pub terminal: Option<JobTerminal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JobTerminal {
    Completed,
    ParseFailed,
    Rejected { code: String, detail: String },
    Cancelled,
}

// ── Snapshot query API ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentAnchor {
    pub snapshot_id: SnapshotId,
    pub path: String,
    pub span_start: u32,
    pub span_end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum QueryKind {
    #[default]
    FindAnchors,
    ResolvePath,
    ResolveHover,
    RootValueKind,
    NodePreview,
    PathValue,
    FieldLabels,
    SearchIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum QueryTargetKind {
    Key,
    #[default]
    Value,
    Node,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotQuery {
    pub snapshot_id: SnapshotId,
    pub kind: QueryKind,
    pub path_pattern: Option<String>,
    pub span: Option<(u32, u32)>,
    pub target: Option<QueryTargetKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSearchItem {
    pub path: String,
    #[serde(rename = "pathText")]
    pub path_text: String,
    pub label: String,
    #[serde(rename = "keyText")]
    pub key_text: String,
    #[serde(rename = "valueText")]
    pub value_text: String,
    pub target: QueryTargetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub anchors: Vec<DocumentAnchor>,
    #[serde(default, rename = "rootValueKind")]
    pub root_value_kind: Option<String>,
    #[serde(default, rename = "nodePreview")]
    pub node_preview: Option<DocumentNodePreview>,
    #[serde(default, rename = "pathValue")]
    pub path_value: Option<DocumentPathValue>,
    #[serde(default, rename = "fieldLabels")]
    pub field_labels: Vec<String>,
    #[serde(default, rename = "searchItems")]
    pub search_items: Vec<DocumentSearchItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentNodePreview {
    pub kind: i32,
    pub sem_type: i32,
    pub tag: String,
    pub value: String,
    #[serde(default, rename = "valueType")]
    pub value_type: String,
    #[serde(default, rename = "isScalar")]
    pub is_scalar: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPathValue {
    #[serde(rename = "valueType")]
    pub value_type: String,
    pub value: String,
    #[serde(rename = "sourceText")]
    pub source_text: String,
    #[serde(rename = "displayText")]
    pub display_text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum SnapshotReadResult<T> {
    Ready { data: T },
    SnapshotNotReady,
}
