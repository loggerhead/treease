use crate::language::lang_spec::{StreamKind, lang_from_name};
use serde::{Deserialize, Serialize};
use tsify::Tsify;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Tsify)]
#[repr(u8)]
pub enum PathSegTag {
    Key,
    Index,
}

impl PathSegTag {
    pub const KEY_VALUE: u8 = 0;
    pub const INDEX_VALUE: u8 = 1;

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Key => Self::KEY_VALUE,
            Self::Index => Self::INDEX_VALUE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct PathSeg<'a> {
    pub tag: PathSegTag,
    pub key: &'a str,
    pub index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PathSpan {
    pub start_byte: i32,
    pub end_byte: i32,
    pub row: i32,
    pub column: i32,
}

impl PathSpan {
    pub const EMPTY: Self = Self {
        start_byte: -1,
        end_byte: -1,
        row: -1,
        column: -1,
    };
}

pub const fn empty_path_span() -> PathSpan {
    PathSpan::EMPTY
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WasmProtocol {
    Json,
    #[cfg(not(feature = "lite"))]
    Yaml,
    #[cfg(not(feature = "lite"))]
    Toml,
    #[cfg(not(feature = "lite"))]
    Python,
    #[cfg(not(feature = "lite"))]
    Javascript,
    #[cfg(not(feature = "lite"))]
    Csv,
}

impl WasmProtocol {
    pub fn from_name(candidate: &str) -> Option<Self> {
        let normalized = candidate.trim().to_ascii_lowercase();
        let spec = lang_from_name(&normalized)?;
        if !(spec.enabled && spec.is_format) {
            return None;
        }
        match spec.name {
            "json" => Some(Self::Json),
            #[cfg(not(feature = "lite"))]
            "yaml" => Some(Self::Yaml),
            #[cfg(not(feature = "lite"))]
            "toml" => Some(Self::Toml),
            #[cfg(not(feature = "lite"))]
            "python" => Some(Self::Python),
            #[cfg(not(feature = "lite"))]
            "javascript" => Some(Self::Javascript),
            #[cfg(not(feature = "lite"))]
            "csv" => Some(Self::Csv),
            _ => None,
        }
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Json => "json",
            #[cfg(not(feature = "lite"))]
            Self::Yaml => "yaml",
            #[cfg(not(feature = "lite"))]
            Self::Toml => "toml",
            #[cfg(not(feature = "lite"))]
            Self::Python => "python",
            #[cfg(not(feature = "lite"))]
            Self::Javascript => "javascript",
            #[cfg(not(feature = "lite"))]
            Self::Csv => "csv",
        }
    }

    pub const fn stream_kind(self) -> Option<StreamKind> {
        match self {
            Self::Json => Some(StreamKind::Json),
            #[cfg(not(feature = "lite"))]
            Self::Yaml | Self::Toml | Self::Python | Self::Javascript | Self::Csv => None,
        }
    }

    pub fn supports_incremental_edits(self) -> bool {
        crate::language::lang_spec::supports_incremental_edits(self.canonical_name())
    }

    pub const fn supports_value_only_decode(self) -> bool {
        #[cfg(not(feature = "lite"))]
        {
            matches!(self, Self::Toml)
        }
        #[cfg(feature = "lite")]
        {
            false
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CommonFormatOptions {
    pub indent: i32,
    pub smart: bool,
    pub max_line_length: i32,
    pub max_inline_complexity: i32,
    pub max_array_inline_items: i32,
    pub align_object_arrays: bool,
    pub nest: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StreamInputKind {
    Source = 0,
    Analysis = 1,
    Commit = 2,
}

impl std::convert::TryFrom<u8> for StreamInputKind {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Source),
            1 => Ok(Self::Analysis),
            2 => Ok(Self::Commit),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StreamOutputMode {
    pub analysis: bool,
    pub graph: bool,
}

impl StreamOutputMode {
    pub const ANALYSIS_BIT: u8 = 0b01;
    pub const GRAPH_BIT: u8 = 0b10;

    pub const fn from_bits(bits: u8) -> Self {
        Self {
            analysis: (bits & Self::ANALYSIS_BIT) != 0,
            graph: (bits & Self::GRAPH_BIT) != 0,
        }
    }

    pub const fn is_empty(self) -> bool {
        !self.analysis && !self.graph
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCreateRequest {
    pub input_kind: StreamInputKind,
    pub output_mode: StreamOutputMode,
    pub language: String,
    pub nest: bool,
    pub document_key: String,
    pub text: String,
    pub edits: Vec<DocumentTextEditRaw>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamFeedRequest {
    pub handle: u32,
    pub chunk: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTextEditRaw {
    pub start_byte: u32,
    pub old_end_byte: u32,
    pub new_end_byte: u32,
    pub start_row: u32,
    pub start_column: u32,
    pub old_end_row: u32,
    pub old_end_column: u32,
    pub new_end_row: u32,
    pub new_end_column: u32,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisSharedArtifacts {
    pub source_byte_length: u32,
    pub semantic_tokens: Vec<u32>,
    pub value_json: String,
    pub ts_tree: Option<tree_sitter::Tree>,
    pub token_spans: Vec<crate::tree::TokenSpan>,
    pub line_index: crate::analysis::LineIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ParseAndStoreWithEditsResult {
    Ok = 1,
    DecoderMissing = 2,
    OperationFailed = 5,
    StoreMissing = 10,
    RegistryMissing = 11,
    IncrementalInvalidEditCount = 61,
    IncrementalEntryMissing = 62,
    IncrementalPathStateUnsafe = 63,
    IncrementalApplyFailed = 64,
}

impl ParseAndStoreWithEditsResult {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

// ---------------------------------------------------------------------------
// WASM protocol enums — #[repr(i32)] for FFI safety
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TreeKind {
    Sequence = 0,
    Mapping = 1,
    Scalar = 2,
    Alias = 3,
    Unknown = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Tsify)]
#[repr(i32)]
pub enum SemType {
    Map = 0,
    Seq = 1,
    Str = 2,
    Int = 3,
    Float = 4,
    Boolean = 5,
    Nil = 6,
    Unknown = 255,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Tsify)]
#[repr(i32)]
pub enum GraphKind {
    Scalar = 0,
    Object = 1,
    Table = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DiffType {
    Ins = 0,
    Del = 1,
}

// ---------------------------------------------------------------------------
// WASM protocol structs — #[repr(C)] for FFI safety
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct JsonBlockSpan {
    pub found: u8,
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_row: u32,
    pub start_column: u32,
    pub end_row: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct BoxArgs {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub corner_radius: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TextArgs<'a> {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text: &'a str,
    pub text_align: u8,
    pub text_vertical_align: u8,
    pub editable: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct BezierArgs {
    pub from_x: i32,
    pub from_y: i32,
    pub c1x: i32,
    pub c1y: i32,
    pub c2x: i32,
    pub c2y: i32,
    pub to_x: i32,
    pub to_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DocumentTextEdit<'a> {
    pub start_byte: u32,
    pub old_end_byte: u32,
    pub new_end_byte: u32,
    pub start_row: u32,
    pub start_column: u32,
    pub old_end_row: u32,
    pub old_end_column: u32,
    pub new_end_row: u32,
    pub new_end_column: u32,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TreeNode<'a> {
    pub kind: TreeKind,
    pub sem_type: SemType,
    pub tag: &'a str,
    pub value: &'a str,
    pub children: &'a [TreeNode<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct BuilderConfig {
    pub key_width: i32,
    pub value_width: i32,
    pub row_height: i32,
    pub row_padding_x: i32,
    pub row_padding_y: i32,
    pub node_border_width: i32,
    pub v_gap: i32,
    pub h_gap: i32,
    pub table_max_height: i32,
    pub table_row_height: i32,
    pub table_header_height: i32,
    pub table_column_width: i32,
    pub avg_char_width_x10: i32,
    pub font_size: i32,
    pub meta_path_min_segments: i32,
    pub meta_path_min_chars: i32,
    pub meta_path_keep_tail_segments: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GraphCell<'a> {
    pub text: &'a str,
    pub sem_type: SemType,
    pub path: &'a [PathSeg<'a>],
    pub value: &'a str,
    pub box_args: BoxArgs,
    pub text_args: TextArgs<'a>,
    pub format_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GraphRow<'a> {
    pub index: i32,
    pub box_args: BoxArgs,
    pub cell_box_args: BoxArgs,
    pub cells: &'a [GraphCell<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GraphTable<'a> {
    pub columns: &'a [GraphCell<'a>],
    pub rows: &'a [GraphRow<'a>],
    pub header_height: i32,
    pub total_height: i32,
    pub view_height: i32,
    pub row_height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GraphNodeKey<'a> {
    pub kind: GraphKind,
    pub path: &'a [PathSeg<'a>],
    pub path_key: &'a str,
    pub stable_id: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GraphNode<'a> {
    pub render_handle: u32,
    pub key: GraphNodeKey<'a>,
    pub kind: GraphKind,
    pub depth: u32,
    pub box_args: BoxArgs,
    pub path: &'a [PathSeg<'a>],
    pub meta: GraphCell<'a>,
    pub rows: &'a [GraphRow<'a>],
    pub has_table: u8,
    pub table: GraphTable<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GraphEdge<'a> {
    pub from_render_handle: u32,
    pub from: GraphNodeKey<'a>,
    pub from_row: i32,
    pub to_render_handle: u32,
    pub to: GraphNodeKey<'a>,
    pub to_row: i32,
    pub bezier_args: BezierArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GraphModel<'a> {
    pub nodes: &'a [GraphNode<'a>],
    pub edges: &'a [GraphEdge<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GraphTableCellPatch<'a> {
    pub table_render_handle: u32,
    pub row_index: u32,
    pub column_index: u32,
    pub cell: GraphCell<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GraphDelta<'a> {
    pub clear: u8,
    pub nodes_added: &'a [GraphNode<'a>],
    pub nodes_updated: &'a [GraphNode<'a>],
    pub nodes_removed: &'a [i32],
    pub edges_added: &'a [GraphEdge<'a>],
    pub edges_removed: &'a [GraphEdge<'a>],
    pub table_cell_patches: &'a [GraphTableCellPatch<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct StoredDocumentAnalysis<'a> {
    pub language: &'a str,
    pub source_byte_length: u32,
    pub has_tree: u8,
    pub tree: TreeNode<'a>,
    pub value_json: &'a str,
    pub diagnostics: &'a [u32],
    pub semantic_tokens: &'a [u32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Diff<'a> {
    pub offset: i32,
    pub length: i32,
    pub r#type: DiffType,
    pub inline_diffs: &'a [Diff<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DiffPair<'a> {
    pub has_left: u8,
    pub left: Diff<'a>,
    pub has_right: u8,
    pub right: Diff<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DiffResult<'a> {
    pub pairs: &'a [DiffPair<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct FormatPreferences {
    pub indent: i32,
    pub unwrap_scalar: u8,
    pub smart: u8,
    pub max_line_length: i32,
    pub max_inline_complexity: i32,
    pub max_array_inline_items: i32,
    pub align_object_arrays: u8,
    pub leading_content_pre_processing: u8,
    pub print_doc_separators: u8,
    pub evaluate_together: u8,
    pub fix_merge_anchor_to_spec: u8,
    pub separator: i32,
    pub auto_parse: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct LanguagePreferencesOverrides {
    pub json: FormatPreferences,
    pub yaml: FormatPreferences,
    pub toml: FormatPreferences,
    pub python: FormatPreferences,
    pub javascript: FormatPreferences,
    pub csv: FormatPreferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct LanguagePreferences {
    pub default: FormatPreferences,
    pub overrides: LanguagePreferencesOverrides,
}
