#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamKind {
    Json,
    NonStreaming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphValueEditRuleKind {
    Json,
    ScalarYaml,
    ScalarToml,
    ScalarCsv,
    ScalarPython,
    ScalarJavascript,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormatLanguage {
    Json,
    Yaml,
    Toml,
    Python,
    Javascript,
    Csv,
}

impl FormatLanguage {
    /// Parse a language name string into a `FormatLanguage`.
    /// Returns `None` for unrecognized or empty names.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "json" | "j" => Some(Self::Json),
            "yaml" | "yml" | "y" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "python" | "py" => Some(Self::Python),
            "javascript" | "js" => Some(Self::Javascript),
            "csv" | "c" => Some(Self::Csv),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartFormatOptions {
    pub indent: i32,
    pub smart: bool,
    pub max_line_length: i32,
    pub max_inline_complexity: i32,
    pub max_array_inline_items: i32,
    pub align_object_arrays: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeTypeSpec<'a> {
    pub pair_types: &'a [&'a str],
    pub array_types: &'a [&'a str],
    pub array_item_type: Option<&'a str>,
}

impl<'a> Default for NodeTypeSpec<'a> {
    fn default() -> Self {
        DEFAULT_NODE_SPEC
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LangSpec<'a> {
    pub name: &'a str,
    pub enabled: bool,
    pub is_format: bool,
    pub format_language: Option<FormatLanguage>,
    pub aliases: &'a [&'a str],
    pub extensions: &'a [&'a str],
    pub default_pretty_print: bool,
    pub web_label: Option<&'a str>,
    pub web_editor_supported: bool,
    pub web_import_supported: bool,
    pub web_import_extensions: &'a [&'a str],
    pub web_example_extensions: &'a [&'a str],
    pub has_tree_sitter: bool,
    pub stream_kind: Option<StreamKind>,
    pub streaming_token_spans_fallback: bool,
    pub has_structured_path: bool,
    pub supports_value_only_decode: bool,
    pub default_nest: bool,
    pub supports_graph_delta_diff: bool,
    pub supports_incremental_edits: bool,
    pub graph_value_edit_rule: GraphValueEditRuleKind,
    pub node_type_spec: NodeTypeSpec<'a>,
    pub query_src: &'a str,
}

impl<'a> LangSpec<'a> {
    pub fn matches_name(&self, candidate: &str) -> bool {
        let normalized = normalize_language_name(candidate);
        self.name.eq_ignore_ascii_case(normalized)
            || self
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(normalized))
    }

    pub fn matches_extension(&self, candidate: &str) -> bool {
        let normalized = candidate.trim().trim_start_matches('.');
        self.extensions
            .iter()
            .any(|extension| extension.eq_ignore_ascii_case(normalized))
    }

    pub fn embedded_query(&self) -> Option<&'a str> {
        (!self.query_src.is_empty()).then_some(self.query_src)
    }

    pub fn supports_streaming_token_spans_fallback(&self) -> bool {
        self.streaming_token_spans_fallback
    }
}

fn normalize_language_name(candidate: &str) -> &str {
    let normalized = candidate.trim();
    match normalized {
        "python-object" | "python_object" | "python object" => "python",
        "js-object" | "js_object" | "js object" => "javascript",
        _ => normalized,
    }
}

const DEFAULT_NODE_SPEC: NodeTypeSpec<'static> = NodeTypeSpec {
    pair_types: &["pair"],
    array_types: &["array"],
    array_item_type: None,
};

const YAML_NODE_SPEC: NodeTypeSpec<'static> = NodeTypeSpec {
    pair_types: &["block_mapping_pair", "flow_pair"],
    array_types: &["block_sequence", "flow_sequence"],
    array_item_type: Some("block_sequence_item"),
};

const JSON_QUERY_SRC: &str = include_str!("../queries/json.scm");
const YAML_QUERY_SRC: &str = include_str!("../queries/yaml.scm");
const TOML_QUERY_SRC: &str = include_str!("../queries/toml.scm");
const PYTHON_QUERY_SRC: &str = include_str!("../queries/python.scm");
const JAVASCRIPT_QUERY_SRC: &str = include_str!("../queries/javascript.scm");

pub const JSON_SPEC: LangSpec<'static> = LangSpec {
    name: "json",
    enabled: true,
    is_format: true,
    format_language: Some(FormatLanguage::Json),
    aliases: &["j"],
    extensions: &["json"],
    default_pretty_print: true,
    web_label: Some("JSON"),
    web_editor_supported: true,
    web_import_supported: true,
    web_import_extensions: &["json"],
    web_example_extensions: &["json"],
    has_tree_sitter: true,
    stream_kind: Some(StreamKind::Json),
    streaming_token_spans_fallback: false,
    has_structured_path: false,
    supports_value_only_decode: false,
    default_nest: true,
    supports_graph_delta_diff: true,
    supports_incremental_edits: true,
    graph_value_edit_rule: GraphValueEditRuleKind::Json,
    node_type_spec: DEFAULT_NODE_SPEC,
    query_src: JSON_QUERY_SRC,
};

pub const YAML_SPEC: LangSpec<'static> = LangSpec {
    name: "yaml",
    enabled: true,
    is_format: true,
    format_language: Some(FormatLanguage::Yaml),
    aliases: &["y", "yml"],
    extensions: &["yaml", "yml"],
    default_pretty_print: true,
    web_label: Some("YAML"),
    web_editor_supported: true,
    web_import_supported: true,
    web_import_extensions: &[],
    web_example_extensions: &["yaml", "yml"],
    has_tree_sitter: true,
    stream_kind: None,
    streaming_token_spans_fallback: false,
    has_structured_path: true,
    supports_value_only_decode: false,
    default_nest: false,
    supports_graph_delta_diff: true,
    supports_incremental_edits: true,
    graph_value_edit_rule: GraphValueEditRuleKind::ScalarYaml,
    node_type_spec: YAML_NODE_SPEC,
    query_src: YAML_QUERY_SRC,
};

pub const TOML_SPEC: LangSpec<'static> = LangSpec {
    name: "toml",
    enabled: true,
    is_format: true,
    format_language: Some(FormatLanguage::Toml),
    aliases: &[],
    extensions: &["toml"],
    default_pretty_print: true,
    web_label: Some("TOML"),
    web_editor_supported: true,
    web_import_supported: true,
    web_import_extensions: &[],
    web_example_extensions: &["toml"],
    has_tree_sitter: true,
    stream_kind: None,
    streaming_token_spans_fallback: false,
    has_structured_path: true,
    supports_value_only_decode: true,
    default_nest: false,
    supports_graph_delta_diff: true,
    supports_incremental_edits: true,
    graph_value_edit_rule: GraphValueEditRuleKind::ScalarToml,
    node_type_spec: DEFAULT_NODE_SPEC,
    query_src: TOML_QUERY_SRC,
};

pub const PYTHON_SPEC: LangSpec<'static> = LangSpec {
    name: "python",
    enabled: true,
    is_format: true,
    format_language: Some(FormatLanguage::Python),
    aliases: &["py"],
    extensions: &["py"],
    default_pretty_print: true,
    web_label: Some("Python Dict"),
    web_editor_supported: true,
    web_import_supported: false,
    web_import_extensions: &[],
    web_example_extensions: &["py"],
    has_tree_sitter: true,
    stream_kind: None,
    streaming_token_spans_fallback: false,
    has_structured_path: true,
    supports_value_only_decode: false,
    default_nest: false,
    supports_graph_delta_diff: true,
    supports_incremental_edits: true,
    graph_value_edit_rule: GraphValueEditRuleKind::ScalarPython,
    node_type_spec: DEFAULT_NODE_SPEC,
    query_src: PYTHON_QUERY_SRC,
};

pub const JAVASCRIPT_SPEC: LangSpec<'static> = LangSpec {
    name: "javascript",
    enabled: true,
    is_format: true,
    format_language: Some(FormatLanguage::Javascript),
    aliases: &["js"],
    extensions: &["js", "mjs", "cjs"],
    default_pretty_print: true,
    web_label: Some("JS Object"),
    web_editor_supported: true,
    web_import_supported: false,
    web_import_extensions: &[],
    web_example_extensions: &["js"],
    has_tree_sitter: true,
    stream_kind: None,
    streaming_token_spans_fallback: false,
    has_structured_path: true,
    supports_value_only_decode: false,
    default_nest: false,
    supports_graph_delta_diff: true,
    supports_incremental_edits: true,
    graph_value_edit_rule: GraphValueEditRuleKind::ScalarJavascript,
    node_type_spec: DEFAULT_NODE_SPEC,
    query_src: JAVASCRIPT_QUERY_SRC,
};

pub const CSV_SPEC: LangSpec<'static> = LangSpec {
    name: "csv",
    enabled: true,
    is_format: true,
    format_language: Some(FormatLanguage::Csv),
    aliases: &["c"],
    extensions: &["csv"],
    default_pretty_print: false,
    web_label: Some("CSV"),
    web_editor_supported: false,
    web_import_supported: true,
    web_import_extensions: &[],
    web_example_extensions: &["csv"],
    has_tree_sitter: false,
    stream_kind: None,
    streaming_token_spans_fallback: false,
    has_structured_path: false,
    supports_value_only_decode: false,
    default_nest: false,
    supports_graph_delta_diff: false,
    supports_incremental_edits: true,
    graph_value_edit_rule: GraphValueEditRuleKind::ScalarCsv,
    node_type_spec: DEFAULT_NODE_SPEC,
    query_src: "",
};

pub const LANG_SPECS: &[LangSpec<'static>] = &[
    JSON_SPEC,
    #[cfg(not(feature = "lite"))]
    YAML_SPEC,
    #[cfg(not(feature = "lite"))]
    TOML_SPEC,
    #[cfg(not(feature = "lite"))]
    PYTHON_SPEC,
    JAVASCRIPT_SPEC,
    #[cfg(not(feature = "lite"))]
    CSV_SPEC,
];

pub fn lang_from_name(candidate: &str) -> Option<&'static LangSpec<'static>> {
    LANG_SPECS.iter().find(|spec| spec.matches_name(candidate))
}

pub fn lang_from_extension(candidate: &str) -> Option<&'static LangSpec<'static>> {
    LANG_SPECS
        .iter()
        .find(|spec| spec.matches_extension(candidate))
}

pub fn query_from_language(candidate: &str) -> Option<&'static str> {
    lang_from_name(candidate).and_then(LangSpec::embedded_query)
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

/// Find a [`LangSpec`] by language name (alias for [`lang_from_name`]).
///
#[inline]
pub fn find_spec(name: &str) -> Option<&'static LangSpec<'static>> {
    lang_from_name(name)
}

/// Find a [`LangSpec`] by language name, restricted to format specs only.
///
/// completeness).
pub fn find_format_spec(candidate: &str) -> Option<&'static LangSpec<'static>> {
    LANG_SPECS
        .iter()
        .find(|spec| spec.enabled && spec.is_format && spec.matches_name(candidate))
}

/// Find a CLI-usable format spec by name (must be a format with a
/// `format_language`).
///
pub fn find_cli_format_spec(candidate: &str) -> Option<&'static LangSpec<'static>> {
    let spec = find_format_spec(candidate)?;
    if spec.format_language.is_none() {
        return None;
    }
    Some(spec)
}

/// Determine the format name from a filename by matching its extension against
/// the registered [`LangSpec`] entries.
///
pub fn format_name_from_filename(filename: &str) -> &str {
    if filename.is_empty() {
        return "json";
    }

    let last_segment = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let Some(dot_pos) = last_segment.rfind('.') else {
        return "json";
    };
    let ext = &last_segment[dot_pos + 1..];
    if ext.len() < 1 {
        return "json";
    }

    for spec in LANG_SPECS {
        if !spec.enabled || !spec.is_format {
            continue;
        }
        if spec.matches_extension(ext) {
            return spec.name;
        }
    }
    ext
}

/// Find a CLI-usable format spec from a filename.
///
pub fn find_cli_format_spec_from_filename(filename: &str) -> Option<&'static LangSpec<'static>> {
    let name = format_name_from_filename(filename);
    find_cli_format_spec(name)
}

/// Get the [`NodeTypeSpec`] for a language, falling back to the default spec
/// when the language is unknown.
///
pub fn node_type_spec_for_language(language_name: &str) -> &'static NodeTypeSpec<'static> {
    find_spec(language_name)
        .map(|spec| &spec.node_type_spec)
        .unwrap_or(&DEFAULT_NODE_SPEC)
}

/// Check whether a tree-sitter node type name represents a "pair" (key-value)
/// node for the given language.
///
pub fn is_pair_node_type(node_type: &str, language_name: &str) -> bool {
    let spec = node_type_spec_for_language(language_name);
    spec.pair_types
        .iter()
        .any(|pair_type| pair_type.eq_ignore_ascii_case(node_type))
}

/// Check whether a tree-sitter node type name represents an "array" (sequence)
/// node for the given language.
///
pub fn is_array_node_type(node_type: &str, language_name: &str) -> bool {
    let spec = node_type_spec_for_language(language_name);
    spec.array_types
        .iter()
        .any(|array_type| array_type.eq_ignore_ascii_case(node_type))
}

/// Return whether the given language supports structured (tree-sitter) paths.
///
pub fn has_structured_path(language_name: &str) -> bool {
    find_spec(language_name)
        .map(|spec| spec.has_structured_path)
        .unwrap_or(false)
}

/// Return the [`StreamKind`] for a language, defaulting to
/// [`StreamKind::NonStreaming`] when the language is unknown or does not
/// declare a stream kind.
///
pub fn stream_kind_for_language(language: &str) -> StreamKind {
    find_spec(language)
        .and_then(|spec| spec.stream_kind)
        .unwrap_or(StreamKind::NonStreaming)
}
/// Return whether the given language uses a streaming token-spans fallback.
pub fn has_streaming_token_spans_fallback(language: &str) -> bool {
    find_spec(language)
        .map(|spec| spec.streaming_token_spans_fallback)
        .unwrap_or(false)
}

/// Return whether the given language supports value-only decoding (e.g. TOML).
///
pub fn supports_value_only_decode(language_name: &str) -> bool {
    find_spec(language_name)
        .map(|spec| spec.supports_value_only_decode)
        .unwrap_or(false)
}

/// Return whether the given language supports any structural incremental edit path.
///
pub fn supports_incremental_edits(language_name: &str) -> bool {
    find_spec(language_name)
        .map(|spec| spec.supports_incremental_edits)
        .unwrap_or(false)
}

/// Parse source text with the tree-sitter grammar for the given language name.
///
/// Returns `None` when the language is unknown, has no tree-sitter grammar, or
/// parsing fails.
///
pub fn parse_tree(language_name: &str, source: &[u8]) -> Option<tree_sitter::Tree> {
    let language = crate::language::tree_sitter_support::tree_sitter_language(language_name)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).ok()?;
    let syntax_source =
        crate::language::tree_sitter_support::tree_sitter_syntax_source(language_name, source);
    parser.parse(syntax_source.as_ref(), None)
}

/// Parse with an optional old tree for incremental re-parse.
///
/// When `old_tree` is provided, tree-sitter will only re-parse the changed
/// portions of the source. The old tree MUST have been edited (e.g. via
/// `tree.edit()`) to keep byte positions in sync with the new source.
pub fn parse_tree_incremental(
    language_name: &str,
    source: &[u8],
    old_tree: Option<&tree_sitter::Tree>,
) -> Option<tree_sitter::Tree> {
    let language = crate::language::tree_sitter_support::tree_sitter_language(language_name)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).ok()?;
    let syntax_source =
        crate::language::tree_sitter_support::tree_sitter_syntax_source(language_name, source);
    parser.parse(syntax_source.as_ref(), old_tree)
}
