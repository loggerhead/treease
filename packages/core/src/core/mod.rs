pub use crate::context;
pub use crate::errors;
pub use crate::operators::core_helpers;

pub use crate::analysis::diagnostics;
pub use crate::analysis::diagnostics::{
    DiagnosticLocation, DiagnosticSnippet, DiagnosticStage, Diagnostics, ParseErrorInfo,
    compute_location_and_snippet,
};
pub use crate::analysis::document_analysis;
pub use crate::analysis::document_analysis::{
    DocumentAnalysisDemand, ErrorSpan, StoredDocumentAnalysisDemand, StoredDocumentAnalysisOwned,
    TransientDocumentAnalysis, analyze_document_internal,
    analyze_document_internal_via_streaming_codec, analyze_document_internal_with_demand,
    analyze_document_internal_with_prepared_tree,
    analyze_document_internal_with_prepared_tree_and_demand, collect_error_spans,
    error_spans_to_diagnostics_raw, store_transient_document_analysis,
};
pub use crate::analysis::line_index;
pub use crate::analysis::line_index::{LineBounds, LineColumn, LineIndex};
pub use crate::analysis::span_index;
pub use crate::analysis::span_index::StructuralSpanIndex;
pub use crate::context::{CodecState, Context};
pub use crate::errors::{CoreError, EvalError, FormatError, ParseError, SystemError};
pub use crate::graph::authoritative_graph_service;
pub use crate::graph::graph_builder;
pub use crate::graph::graph_builder_preorder;
pub use crate::graph::graph_delta;
pub use crate::graph::graph_delta_service;
pub use crate::graph::graph_fragment_index;
pub use crate::graph::graph_identity;
pub use crate::graph::graph_materialize;
pub use crate::graph::graph_model;
pub use crate::graph::graph_model_index;
pub use crate::graph::graph_projection_service;
pub use crate::graph::graph_relayout;
pub use crate::graph::graph_shape;
pub use crate::graph::graph_topology;
pub use crate::graph::streaming_delta_differ;
pub use crate::graph::streaming_graph_projector;
pub use crate::io::codec_service;
pub use crate::io::codec_service::{CodecService, canonical_format_name, language_for_format};
pub use crate::io::encoding;
pub use crate::io::encoding::{
    ContextEncoder, DecodedDocument, Decoder, Reader, ValueEncoder, Writer,
};
pub use crate::io::io_adapters;
pub use crate::io::io_adapters::VecWriter;
pub use crate::io::literal_format;
pub use crate::io::literal_format::{
    LiteralStyle, format_buffer_literal, format_json_string, format_literal, format_python_string,
};
pub use crate::io::printer;
pub use crate::io::printer::{Encoder, Printer};
pub use crate::io::printer_writer;
pub use crate::io::printer_writer::{IoPrinterWriter, PrinterWriter, VecPrinterWriter};
pub use crate::language::SemType;
pub use crate::language::edit_rules;
pub use crate::language::guess_language::guess_language;
pub use crate::language::lang_spec;
pub use crate::language::lang_spec::{
    FormatLanguage, GraphValueEditRuleKind, JSON_SPEC, LANG_SPECS, LangSpec, NodeTypeSpec,
    SmartFormatOptions, StreamKind, YAML_SPEC, find_cli_format_spec,
    find_cli_format_spec_from_filename, find_format_spec, find_spec, format_name_from_filename,
    has_streaming_token_spans_fallback, has_structured_path, is_array_node_type, is_pair_node_type,
    lang_from_extension, lang_from_name, node_type_spec_for_language, parse_tree,
    query_from_language, stream_kind_for_language, supports_incremental_edits,
    supports_value_only_decode,
};
pub use crate::language::language;
pub use crate::language::language::Language;
pub use crate::language::parse_scalar_edit_replacement;
pub use crate::language::sem_type;
pub use crate::language::semantic_tokens;
pub use crate::language::semantic_tokens::{
    TOKEN_TYPES as SEMANTIC_TOKEN_TYPES, collect_token_spans_with_tree,
    encode_and_cache_semantic_tokens, encode_semantic_tokens, semantic_tokens_inner,
    streaming_language_adapter,
};
pub use crate::language::tree_sitter_support;
pub use crate::language::tree_sitter_support::{
    TreeSitterParseSummary, TreeSitterQueryCapture, TreeSitterQueryMatch, TreeSitterSpan,
    parse_supported_language, parse_with_tree, query_capture_name_for_id, query_cursor_exec,
    query_cursor_new, query_new, tree_sitter_language,
};
pub use crate::layout::LayoutEngine;
pub use crate::layout::full_layout_adapter;
pub use crate::layout::layout_engine;
pub use crate::operators::core_helpers::{
    DocumentWithMeta, read_documents_with_meta, recursive_node_compare, require_single_document,
};
pub use crate::registry::expression;
pub use crate::registry::expression_builder;
pub use crate::registry::format;
pub use crate::registry::format_registry;
pub use crate::registry::operation;
pub use crate::registry::operation_defs;
pub use crate::registry::operation_prefs;
pub use crate::registry::operator_registry;
pub use crate::registry::registry;
pub use crate::registry::traversal_builder;
pub use crate::registry::{
    ExpressionBuildError, ExpressionNode, FORMATS, Format, Operation, OperationId, OperationType,
    build_expression_tree_from_postfix_ops, build_recursive_descent_expression,
    build_traversal_expression, format_from_string, format_string_from_filename,
};
pub use crate::tree::incremental_edit;
pub use crate::tree::json_block;
pub use crate::tree::tree_navigator;
pub use crate::tree::tree_node;
pub use crate::tree::tree_ops;
pub use crate::tree::tree_path;
pub use crate::tree::tree_path_index;
pub use crate::tree::tree_store;
pub use crate::tree::value_edit;
pub use authoritative_graph_service::{AuthoritativeGraphService, graph_language_from_name};
pub use format_registry::{
    FormatDefinition, FormatPreferences as RegistryFormatPreferences, FormatRegistry,
};
pub use graph_builder::{
    BuilderConfig, GraphBuilder, GraphKind, GraphLanguage, GraphModelSnapshot, PathSeg,
    default_config,
};
pub use graph_builder_preorder::{
    Builder as GraphBuilderPreorder, GraphDelta as GraphBuilderDelta,
};
pub use graph_delta::{GraphDelta, GraphTableCellPatch, build_graph_delta};
pub use graph_delta_service::{IncrementalGraphDeltaResult, build_incremental_graph_delta};
pub use graph_fragment_index::{FragmentInfo, GraphFragment, GraphFragmentIndex, TableCellImpact};
pub use graph_model::{
    BezierArgs, BoxArgs, CellBounds, GraphCell, GraphEdge, GraphModel, GraphNode, GraphNodeKey,
    GraphRow, GraphTable, TextAlign, TextArgs, TextVerticalAlign,
};
pub use graph_model_index::GraphModelIndex;
pub use graph_relayout::compute_ancestor_relayout_chain;
pub use incremental_edit::{
    DocumentTextEdit, StructuralOffsetUpdate, adjust_tree_store_offsets_from,
    adjust_tree_store_offsets_from_collecting, apply_delta, apply_edit_to_source,
    apply_edits_to_tree_and_source, collect_subtree_node_ids, edit_byte_range, edit_delta,
    find_affected_node_id_for_edit, find_exact_node_for_edit, find_reparse_boundary_id,
    input_edit_for_source, point_after_replacement, point_for_offset,
    recompute_tree_store_locations, recompute_tree_store_locations_for,
    recompute_tree_store_locations_with_span_index, replaced_byte_count,
};
pub use json_block::{JsonBlockSpan, find_json_block_at_position};
pub use operation::{
    OperationHandler, OperationKind, OperationName, OperationNode,
    OperationPreferences as OperationModulePreferences, create_value_operation,
    create_value_operation_with_node, get_matching_nodes as dispatch_matching_nodes,
};
pub use operation_defs::{
    ADD_OP_TYPE, ALTERNATIVE_OP_TYPE, AND_OP_TYPE, ASSIGN_OP_TYPE, ASSIGN_VARIABLE_OP_TYPE,
    BLOCK_OP_TYPE, CAPTURE_OP_TYPE, CHANGE_CASE_OP_TYPE, COLLECT_OBJECT_OP_TYPE, COLLECT_OP_TYPE,
    CONTAINS_OP_TYPE, CREATE_MAP_OP_TYPE, DECODE_OP_TYPE, DEL_PATHS_OP_TYPE, DELETE_OP_TYPE,
    DIVIDE_OP_TYPE, EMPTY_OP_TYPE, ENCODE_OP_TYPE, EQUALS_OP_TYPE, EXPRESSION_OP_TYPE,
    FILTER_OP_TYPE, FIRST_OP_TYPE, FLATTEN_OP_TYPE, FROM_ENTRIES_OP_TYPE, GET_KEY_OP_TYPE,
    GET_KIND_OP_TYPE, GET_PARENT_OP_TYPE, GET_PARENTS_OP_TYPE, GET_PATH_OP_TYPE, GET_TAG_OP_TYPE,
    GET_VARIABLE_OP_TYPE, GROUP_BY_OP_TYPE, HAS_OP_TYPE, IS_KEY_OP_TYPE, JOIN_STRING_OP_TYPE,
    KEYS_OP_TYPE, LENGTH_OP_TYPE, MAP_OP_TYPE, MAP_VALUES_OP_TYPE, MATCH_OP_TYPE, MAX_OP_TYPE,
    MIN_OP_TYPE, MODULO_OP_TYPE, MULTIPLY_ASSIGN_OP_TYPE, MULTIPLY_OP_TYPE, NOT_EQUALS_OP_TYPE,
    NOT_OP_TYPE, OMIT_OP_TYPE, OR_OP_TYPE, PICK_OP_TYPE, PIPE_OP_TYPE, RECURSIVE_DESCENT_OP_TYPE,
    REDUCE_OP_TYPE, RELATIONAL_OP_TYPE, REVERSE_OP_TYPE, SELECT_OP_TYPE, SELF_REFERENCE_OP_TYPE,
    SHORT_PIPE_OP_TYPE, SHUFFLE_OP_TYPE, SORT_BY_OP_TYPE, SORT_KEYS_OP_TYPE, SORT_OP_TYPE,
    SPLIT_STRING_OP_TYPE, STRING_INTERPOLATION_OP_TYPE, SUB_STRING_OP_TYPE,
    SUBTRACT_ASSIGN_OP_TYPE, SUBTRACT_OP_TYPE, TEST_OP_TYPE, TO_ENTRIES_OP_TYPE, TO_NUMBER_OP_TYPE,
    TO_STRING_OP_TYPE, TRAVERSE_ARRAY_OP_TYPE, TRAVERSE_PATH_OP_TYPE, TRIM_OP_TYPE, UNION_OP_TYPE,
    UNIQUE_BY_OP_TYPE, UNIQUE_OP_TYPE, VALUE_OP_TYPE, WITH_ENTRIES_OP_TYPE, WITH_OP_TYPE,
};
pub use operation_prefs::{
    AssignPreferences, AssignVarPreferences, ChangeCasePrefs, DecoderPreferences,
    EncoderPreferences, ExpressionOpPreferences, FlattenPreferences, OperationPreferences,
    ParentOpPreferences, RecursiveDescentPreferences, RelationalPref, TraversePreferences,
    get_assign_preference, get_assign_var_preference, get_change_case_preference,
    get_decoder_preference, get_encoder_preference, get_expression_preference,
    get_flatten_preference, get_parent_preference, get_recursive_descent_preference,
    get_relational_preference, get_traverse_preference,
};
pub use registry::{Registry, RegistryHandle, RegistryOwner, from_handle, to_handle};
pub use tree_node::{
    CommentBlock, CompactTag, NodeExtra, NodeExtraId, NodeId, NodeInfo, NodeList, NodeValueRef,
    ParsedKey, TreeNode, TreeNodeKind, ValueId, ValueRep, infer_scalar_tag,
};
pub use tree_ops::{
    MapEntry, PathElement, create_scalar_node, ensure_map, ensure_seq, ensure_seq_index,
    get_map_entry, get_or_create_map_value,
};
pub use tree_path::{
    EMPTY_PATH, PathSpanResolver, borrowed_tree_path, build_tree_path_parts,
    compute_graph_path_span, compute_path_span, compute_path_span_for_document,
    compute_path_span_for_document_with_index, compute_tree_path_segments,
    compute_tree_path_segments_for_document, find_node_by_graph_path, find_node_by_path,
    find_node_by_path_with_index, format_owned_tree_path, format_tree_path,
    format_tree_path_segment, is_punctuation_type, is_simple_key, normalize_key_text,
    parse_tree_path, path_seg_index, path_seg_key, path_seg_key_slice, unescape_json_string,
};
pub use tree_path_index::{OwnedPathSeg, PathLookup, TreePathIndex, TreePathIndexStructuralUpdate};
pub use tree_store::{DocumentAnalysis, DocumentMeta, GraphEntry, TokenSpan, TreeEntry, TreeStore};
