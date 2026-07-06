use std::{cell::RefCell, collections::HashMap};

#[cfg(not(feature = "lite"))]
use crate::formats::{CsvEncoder, JavascriptEncoder, PythonEncoder, TomlEncoder, YamlEncoder};
use crate::formats::{Encode, FormatPreferences, JsonEncoder, default_language_preferences};
use crate::graph::graph_projection_service;
use crate::language::SemType;
use crate::tree::{NodeId, TreeNodeKind, TreeStore};
use crate::wasm_types::CommonFormatOptions;
use wasm_bindgen::prelude::*;

pub(crate) mod allocator;
pub mod compat_exports;
pub(crate) mod decoders;
pub mod document_analysis_shared;
pub(crate) mod runtime;
pub mod semantic_tokens_shared;
pub mod value_json_shared;

use self::decoders::{
    convert_output_options, decode_document_for_stream_session, ensure_formats, normalize_language,
    normalize_output_format,
};
use self::runtime::{StoredAnalysis, clear_compare_arena, clear_graph_arena, clear_tree_arena};

/// Internal status/error code for wasm submodule operations.
/// NOTE: No #[repr] attributes — not used across the C ABI anymore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustWasmStatus {
    Ok = 0,
    NullPointer = 1,
    InvalidUtf8 = 2,
    UnsupportedLanguage = 3,
    CoreError = 4,
}

impl std::fmt::Display for RustWasmStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Internal buffer type for wasm submodules.
/// NOTE: No #[repr(C)] — not used across the C ABI anymore.
#[derive(Debug, Clone)]
pub struct RustWasmBuffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub status: RustWasmStatus,
}

impl RustWasmBuffer {
    pub const fn empty(status: RustWasmStatus) -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            len: 0,
            status,
        }
    }
}

pub type WasmResult<T> = Result<T, String>;

thread_local! {
    static STORED_ANALYSES: RefCell<HashMap<String, StoredAnalysis>> =
        RefCell::new(HashMap::new());
}

// ── Initialization ────────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn init_wasm() {
    allocator::install_tree_sitter_allocator();
    runtime::REGISTRY_OWNER.with(|owner| {
        let mut owner = owner.borrow_mut();
        if owner.is_none() {
            if let Ok(registry_owner) = crate::init() {
                *owner = Some(registry_owner);
            }
        }
    });
    ensure_formats();
    runtime::GLOBAL_STORE.with(|_| {});
}

pub fn reset_test_runtime() {
    init_wasm();
    STORED_ANALYSES.with(|stored| stored.borrow_mut().clear());
    runtime::GLOBAL_STORE.with(|store| store.borrow_mut().clear());
    graph_projection_service::reset_builder_config();
    clear_graph_arena();
    clear_tree_arena();
    clear_compare_arena();
}

// ── sem_type_code ─────────────────────────────────────────────────────────

pub(crate) fn sem_type_code(sem_type: Option<SemType>) -> i32 {
    match sem_type {
        Some(SemType::Map) => 0,
        Some(SemType::Seq) => 1,
        Some(SemType::Str) => 2,
        Some(SemType::Int) => 3,
        Some(SemType::Float) => 4,
        Some(SemType::Boolean) => 5,
        Some(SemType::Nil) => 6,
        None => 255,
    }
}

// ── Format / convert utilities (pub(crate), used by wasm_document) ────────

pub(crate) fn format_text(
    language: &str,
    source: &str,
    options: CommonFormatOptions,
    sort_keys: Option<bool>,
) -> Result<String, String> {
    let mut decoded =
        decode_document_for_stream_session(language, source, options.nest).map_err(to_error)?;
    if sort_keys.unwrap_or(false) {
        sort_mapping_keys(&mut decoded.store, decoded.root);
    }
    encode_document_to_format(&decoded.store, decoded.root, language, options)
}

pub(crate) fn convert_text(
    source_language: &str,
    target_format: &str,
    source: &str,
    options: CommonFormatOptions,
) -> Result<String, String> {
    let decoded = decode_document_for_stream_session(source_language, source, options.nest)
        .map_err(to_error)?;
    let converted_options = normalize_output_format(target_format)
        .map(|format| convert_output_options(format, options))
        .unwrap_or(options);

    if normalize_language(source_language) == Some("csv") {
        let mut store = TreeStore::new();
        let root =
            clone_tree_into_store(&decoded.store, decoded.root, &mut store).map_err(to_error)?;
        let wrapped_root =
            wrap_csv_sequence_for_target(&mut store, source_language, target_format, root)?;
        return encode_document_to_format(&store, wrapped_root, target_format, converted_options);
    }

    encode_document_to_format(
        &decoded.store,
        decoded.root,
        target_format,
        converted_options,
    )
}

pub(crate) fn default_common_format_options() -> CommonFormatOptions {
    CommonFormatOptions {
        indent: 2,
        smart: true,
        max_line_length: 100,
        max_inline_complexity: 1,
        max_array_inline_items: 6,
        align_object_arrays: true,
        nest: false,
    }
}

fn encode_document_to_format(
    store: &TreeStore,
    root: NodeId,
    format: &str,
    options: CommonFormatOptions,
) -> Result<String, String> {
    let format =
        normalize_output_format(format).ok_or_else(|| format!("unsupported format: {format}"))?;
    let prefs = format_preferences_for(format, options);
    match format {
        "json" => JsonEncoder::new(prefs)
            .encode_to_string(store, root)
            .map_err(|e| e.to_string()),
        #[cfg(not(feature = "lite"))]
        "yaml" => YamlEncoder::new(prefs)
            .encode_to_string(store, root)
            .map_err(|e| e.to_string()),
        #[cfg(not(feature = "lite"))]
        "toml" => TomlEncoder::new(prefs)
            .encode_to_string(store, root)
            .map_err(|e| e.to_string()),
        #[cfg(not(feature = "lite"))]
        "python" => PythonEncoder::new(prefs)
            .encode_to_string(store, root)
            .map_err(|e| e.to_string()),
        #[cfg(not(feature = "lite"))]
        "javascript" => JavascriptEncoder::new(prefs)
            .encode_to_string(store, root)
            .map_err(|e| e.to_string()),
        #[cfg(not(feature = "lite"))]
        "csv" => CsvEncoder::new(prefs)
            .encode_to_string(store, root)
            .map_err(|e| e.to_string()),
        _ => Err(format!("unsupported format: {format}")),
    }
}

fn format_preferences_for(format: &str, options: CommonFormatOptions) -> FormatPreferences {
    let language = match format {
        "json" => crate::language::FormatLanguage::Json,
        "yaml" => crate::language::FormatLanguage::Yaml,
        "toml" => crate::language::FormatLanguage::Toml,
        "python" => crate::language::FormatLanguage::Python,
        "javascript" => crate::language::FormatLanguage::Javascript,
        "csv" => crate::language::FormatLanguage::Csv,
        _ => crate::language::FormatLanguage::Json,
    };
    let mut prefs = default_language_preferences().effective(language);
    prefs.indent = options.indent;
    prefs.smart = options.smart;
    prefs.max_line_length = options.max_line_length;
    prefs.max_inline_complexity = options.max_inline_complexity;
    prefs.max_array_inline_items = options.max_array_inline_items;
    prefs.align_object_arrays = options.align_object_arrays;
    prefs.auto_parse = prefs.auto_parse || options.nest;
    prefs
}

fn sort_mapping_keys(store: &mut TreeStore, id: NodeId) {
    let Some(node) = store.get(id).cloned() else {
        return;
    };
    for child in &node.content {
        sort_mapping_keys(store, *child);
    }
    if node.kind != TreeNodeKind::Mapping {
        return;
    }
    let mut pairs = node
        .content
        .chunks_exact(2)
        .map(|pair| {
            let key = store.value_string_for(pair[0]).unwrap_or_default();
            (key, pair[0], pair[1])
        })
        .collect::<Vec<_>>();
    pairs.sort_by(|left, right| left.0.cmp(&right.0));
    if let Some(node) = store.get_mut(id) {
        node.content.clear();
        for (_, key, value) in pairs {
            node.content.push(key);
            node.content.push(value);
        }
    }
}

fn clone_tree_into_store(
    source_store: &TreeStore,
    source_id: NodeId,
    target_store: &mut TreeStore,
) -> Result<NodeId, String> {
    let source = source_store
        .get(source_id)
        .ok_or_else(|| "missing node".to_string())?;
    let mut out = crate::tree::TreeNode {
        kind: source.kind,
        sem_type: source.sem_type,
        tag: source.tag.clone(),
        value: crate::tree::NodeValueRef::Missing,
        start_byte: source.start_byte,
        end_byte: source.end_byte,
        document: source.document,
        line: source.line,
        column: source.column,
        is_map_key: source.is_map_key,
        sequence_index: source.sequence_index,
        ..crate::tree::TreeNode::default()
    };
    out.set_encode_separate(source.encode_separate());
    out.set_evaluate_together(source.evaluate_together());
    let id = target_store.add(out);
    if source_store
        .value_ref_for(source_id)
        .map_err(to_error)?
        .is_some()
    {
        target_store
            .set_value(
                id,
                source_store.value_string_for(source_id).map_err(to_error)?,
            )
            .map_err(to_error)?;
    }
    target_store.set_document_meta(
        source.document,
        source_store
            .filename_for(source_id)
            .unwrap_or_default()
            .to_owned(),
        source_store.file_index_for(source_id).unwrap_or_default(),
    );
    let _ = target_store.set_anchor(
        id,
        source_store
            .anchor_for(source_id)
            .unwrap_or_default()
            .to_owned(),
    );
    let _ = target_store.set_comments(
        id,
        source_store
            .head_comment_for(source_id)
            .unwrap_or_default()
            .to_owned(),
        source_store
            .line_comment_for(source_id)
            .unwrap_or_default()
            .to_owned(),
        source_store
            .foot_comment_for(source_id)
            .unwrap_or_default()
            .to_owned(),
    );
    let _ = target_store.set_leading_content(
        id,
        source_store
            .leading_content_for(source_id)
            .unwrap_or_default()
            .to_owned(),
    );

    for child in &source.content {
        let child_id = clone_tree_into_store(source_store, *child, target_store)?;
        crate::formats::append_child(target_store, id, child_id)
            .map_err(|e| format!("append child: {e}"))?;
    }

    Ok(id)
}

fn wrap_csv_sequence_for_target(
    store: &mut TreeStore,
    source_language: &str,
    target_format: &str,
    root: NodeId,
) -> Result<NodeId, String> {
    if normalize_language(source_language) != Some("csv") {
        return Ok(root);
    }
    let is_sequence = store
        .get(root)
        .is_some_and(|node| node.kind == TreeNodeKind::Sequence);
    if !is_sequence {
        return Ok(root);
    }
    match normalize_output_format(target_format) {
        Some("toml") => {
            let wrapper = crate::formats::add_mapping(store);
            crate::formats::append_key_value(store, wrapper, "rows", root)
                .map_err(|e| format!("append: {e}"))?;
            Ok(wrapper)
        }
        _ => Ok(root),
    }
}

// ── Helper for converting old error types ─────────────────────────────────

fn to_error<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
