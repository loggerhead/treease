pub mod incremental_edit;
pub mod json_block;
pub mod tree_navigator;
pub mod tree_node;
pub mod tree_ops;
pub mod tree_path;
pub mod tree_path_index;
pub mod tree_store;
pub mod value_edit;

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
pub use tree_node::{
    CommentBlock, CompactTag, NodeExtra, NodeExtraId, NodeId, NodeInfo, NodeList, NodeValueRef,
    ParsedKey, TreeNode, TreeNodeKind, ValueId, ValueRep, infer_scalar_tag,
};
pub use tree_ops::{
    MapEntry, PathElement, create_scalar_node, ensure_map, ensure_seq, ensure_seq_index,
    get_map_entry, get_or_create_map_value,
};
pub use tree_path::{
    EMPTY_PATH, PathSpanResolver, build_tree_path_parts, compute_graph_path_span,
    compute_path_span, compute_path_span_for_document, compute_path_span_for_document_with_index,
    compute_tree_path_segments, compute_tree_path_segments_for_document, find_node_by_graph_path,
    find_node_by_path, find_node_by_path_with_index, format_tree_path, format_tree_path_segment,
    is_punctuation_type, is_simple_key, normalize_key_text, path_seg_index, path_seg_key,
    path_seg_key_slice, unescape_json_string,
};
pub use tree_path_index::{OwnedPathSeg, PathLookup, TreePathIndex, TreePathIndexStructuralUpdate};
pub use tree_store::{DocumentAnalysis, DocumentMeta, GraphEntry, TokenSpan, TreeEntry, TreeStore};
