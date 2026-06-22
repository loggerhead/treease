use std::borrow::Cow;

use tree_sitter::StreamingIterator;

use super::lang_spec::{LangSpec, lang_from_name};

#[cfg(target_arch = "wasm32")]
pub(crate) fn ensure_tree_sitter_runtime() {
    crate::wasm::allocator::install_tree_sitter_allocator();
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn ensure_tree_sitter_runtime() {}
// ---------------------------------------------------------------------------
// Span / summary types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeSitterSpan {
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_row: u32,
    pub start_column: u32,
    pub end_row: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeSitterParseSummary {
    pub root: TreeSitterSpan,
    pub node_at_cursor: Option<TreeSitterSpan>,
    pub has_error: bool,
}

// ---------------------------------------------------------------------------
// Tree edit
// ---------------------------------------------------------------------------

/// Describes a single edit to a source document, used to keep a tree-sitter
/// [`Tree`](tree_sitter::Tree) in sync with the edited source so that it can
/// be re-used for incremental parsing.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeEdit {
    pub start_byte: u32,
    pub old_end_byte: u32,
    pub new_end_byte: u32,
    pub start_row: u32,
    pub start_column: u32,
    pub old_end_row: u32,
    pub old_end_column: u32,
    pub new_end_row: u32,
    pub new_end_column: u32,
}

impl From<TreeEdit> for tree_sitter::InputEdit {
    fn from(e: TreeEdit) -> Self {
        Self {
            start_byte: e.start_byte as usize,
            old_end_byte: e.old_end_byte as usize,
            new_end_byte: e.new_end_byte as usize,
            start_position: tree_sitter::Point {
                row: e.start_row as usize,
                column: e.start_column as usize,
            },
            old_end_position: tree_sitter::Point {
                row: e.old_end_row as usize,
                column: e.old_end_column as usize,
            },
            new_end_position: tree_sitter::Point {
                row: e.new_end_row as usize,
                column: e.new_end_column as usize,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Query execution
// ---------------------------------------------------------------------------

/// A match returned by executing a tree-sitter query.
///
#[derive(Debug, Clone)]
pub struct TreeSitterQueryMatch<'tree> {
    pub pattern_index: u16,
    pub captures: Vec<TreeSitterQueryCapture<'tree>>,
}

/// A single capture within a query match.
///
#[derive(Debug, Clone, Copy)]
pub struct TreeSitterQueryCapture<'tree> {
    pub node: tree_sitter::Node<'tree>,
    pub index: u32,
}

/// Create a new [`Query`](tree_sitter::Query) from a query source string.
///
/// Returns `None` if the query source is invalid.
///
pub fn query_new(language: &tree_sitter::Language, source: &str) -> Option<tree_sitter::Query> {
    ensure_tree_sitter_runtime();
    tree_sitter::Query::new(language, source).ok()
}

/// Create a new [`QueryCursor`](tree_sitter::QueryCursor).
///
pub fn query_cursor_new() -> tree_sitter::QueryCursor {
    ensure_tree_sitter_runtime();
    tree_sitter::QueryCursor::new()
}

/// Execute a query on a given node, returning all captures.
///
/// loop.
pub fn query_cursor_exec<'tree>(
    cursor: &mut tree_sitter::QueryCursor,
    query: &tree_sitter::Query,
    node: tree_sitter::Node<'tree>,
    source: &[u8],
) -> Vec<TreeSitterQueryMatch<'tree>> {
    let mut matches: Vec<TreeSitterQueryMatch<'tree>> = Vec::new();
    let mut captures = cursor.captures(query, node, source);
    let mut last_match_id: Option<u32> = None;
    while let Some((query_match, capture_index)) = captures.next() {
        let capture = query_match.captures[*capture_index];
        let match_id = query_match.id();
        if last_match_id == Some(match_id) {
            if let Some(last) = matches.last_mut() {
                last.captures.push(TreeSitterQueryCapture {
                    node: capture.node,
                    index: capture.index,
                });
            } else {
                matches.push(TreeSitterQueryMatch {
                    pattern_index: query_match.pattern_index as u16,
                    captures: vec![TreeSitterQueryCapture {
                        node: capture.node,
                        index: capture.index,
                    }],
                });
                last_match_id = Some(match_id);
            }
        } else {
            matches.push(TreeSitterQueryMatch {
                pattern_index: query_match.pattern_index as u16,
                captures: vec![TreeSitterQueryCapture {
                    node: capture.node,
                    index: capture.index,
                }],
            });
            last_match_id = Some(match_id);
        }
    }
    matches
}

/// Get the capture name for a given capture index from a query.
///
/// Returns `None` if the index is out of bounds.
///
pub fn query_capture_name_for_id<'a>(query: &'a tree_sitter::Query, id: u32) -> Option<&'a str> {
    query.capture_names().get(id as usize).copied()
}

// ---------------------------------------------------------------------------
// Parser helpers
// ---------------------------------------------------------------------------

/// Parse source text with a given language, optionally re-using an old tree
/// for incremental parsing.
///
///
///
pub fn parse_with_tree(
    language: &tree_sitter::Language,
    source: &[u8],
    old_tree: Option<&tree_sitter::Tree>,
) -> Option<tree_sitter::Tree> {
    ensure_tree_sitter_runtime();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(language).ok()?;
    parser.parse(source, old_tree)
}

pub fn parse_supported_language(
    language: &str,
    source: &str,
    cursor: Option<(u32, u32)>,
) -> Option<TreeSitterParseSummary> {
    ensure_tree_sitter_runtime();
    let language_name = language;
    let language = tree_sitter_language(language_name)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).ok()?;
    let syntax_source = tree_sitter_syntax_source(language_name, source.as_bytes());
    let tree = parser.parse(syntax_source.as_ref(), None)?;
    let root = tree.root_node();
    let cursor_byte = cursor.map(|(row, column)| byte_offset_for_position(source, row, column));
    Some(TreeSitterParseSummary {
        root: span_from_node(root),
        node_at_cursor: cursor_byte
            .and_then(|byte| deepest_named_descendant(root, byte).map(span_from_node)),
        has_error: root.has_error(),
    })
}

pub(crate) fn tree_sitter_syntax_source<'a>(language: &str, source: &'a [u8]) -> Cow<'a, [u8]> {
    if !uses_javascript_string_grammar(language) {
        return Cow::Borrowed(source);
    }

    mask_javascript_string_escapes_for_tree_sitter(source)
        .map(Cow::Owned)
        .unwrap_or(Cow::Borrowed(source))
}

fn uses_javascript_string_grammar(language: &str) -> bool {
    let normalized = language.trim().to_ascii_lowercase();
    matches!(normalized.as_str(), "javascript" | "js")
}

fn mask_javascript_string_escapes_for_tree_sitter(source: &[u8]) -> Option<Vec<u8>> {
    let mut out: Option<Vec<u8>> = None;
    let mut quote: Option<u8> = None;
    let mut escaped = false;

    for (index, byte) in source.iter().copied().enumerate() {
        match quote {
            None => {
                if matches!(byte, b'"' | b'\'' | b'`') {
                    quote = Some(byte);
                }
            }
            Some(active_quote) => {
                if escaped {
                    if !matches!(byte, b'\n' | b'\r') {
                        out.get_or_insert_with(|| source.to_vec())[index] = b'x';
                    }
                    escaped = false;
                    continue;
                }

                if byte == b'\\' {
                    out.get_or_insert_with(|| source.to_vec())[index] = b'x';
                    escaped = true;
                    continue;
                }

                if byte == active_quote {
                    quote = None;
                    continue;
                }

                if !matches!(active_quote, b'`') && matches!(byte, b'\n' | b'\r') {
                    quote = None;
                }
            }
        }
    }

    out
}

pub fn tree_sitter_language(language: &str) -> Option<tree_sitter::Language> {
    ensure_tree_sitter_runtime();
    let spec = lang_from_name(language)?;
    tree_sitter_language_for_spec(spec)
}

pub(crate) fn tree_sitter_language_for_spec(spec: &LangSpec<'_>) -> Option<tree_sitter::Language> {
    ensure_tree_sitter_runtime();
    match spec.name {
        // The repo does not vendor tree-sitter-json. For Rust-side query helpers,
        // use the JavaScript grammar, which can parse JSON object/array literals
        // and supports the `pair key:/value:` queries exercised by the tests.
        // Actual JSON semantic tokens still go through the streaming codec path.
        "json" => Some(tree_sitter::Language::new(tree_sitter_javascript::LANGUAGE)),
        #[cfg(not(feature = "lite"))]
        "yaml" => Some(tree_sitter::Language::new(tree_sitter_yaml::LANGUAGE)),
        #[cfg(not(feature = "lite"))]
        "toml" => Some(tree_sitter::Language::new(tree_sitter_toml_ng::LANGUAGE)),
        #[cfg(not(feature = "lite"))]
        "python" => Some(tree_sitter::Language::new(tree_sitter_python::LANGUAGE)),
        "javascript" => Some(tree_sitter::Language::new(tree_sitter_javascript::LANGUAGE)),
        _ => None,
    }
}

fn deepest_named_descendant(
    node: tree_sitter::Node<'_>,
    byte: usize,
) -> Option<tree_sitter::Node<'_>> {
    if byte < node.start_byte() || byte > node.end_byte() {
        return None;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = deepest_named_descendant(child, byte) {
            return Some(found);
        }
    }
    Some(node)
}

fn span_from_node(node: tree_sitter::Node<'_>) -> TreeSitterSpan {
    let start = node.start_position();
    let end = node.end_position();
    TreeSitterSpan {
        start_byte: node.start_byte() as u32,
        end_byte: node.end_byte() as u32,
        start_row: start.row as u32,
        start_column: start.column as u32,
        end_row: end.row as u32,
        end_column: end.column as u32,
    }
}

fn byte_offset_for_position(source: &str, row: u32, column: u32) -> usize {
    let index = super::LineIndex::build(source);
    index.line_column_to_offset(row, column)
}
