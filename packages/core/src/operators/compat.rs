// Temporary operator-side compatibility models.
//
// These definitions are still required by the translated operator modules, but
// they no longer live in `operators/mod.rs`. Keep this file as a shrinking
// compatibility surface and prefer `crate::core` for new migrations.

// Operators module — Phase A.4 compatibility layer.
//
// Architecture follows PORTING.md, LIFETIMES.tsv, and CYCLEBREAK.md.
// Phase A compromises (Phase B cancelled — not needed):
//   - TreeNode.content stays Vec<TreeNode> for direct operator access
//   - Context.matching_nodes stays Vec<TreeNode>
//   - Diagnostics kept as Box (instead of &mut)
// Architecture follows PORTING.md, LIFETIMES.tsv, and CYCLEBREAK.md.

use std::collections::HashMap;
use std::ffi::c_void;

use super::TreeEngine;

// ── NodeId & TreeStore (PORTING.md §2.3, LIFETIMES.tsv) ─────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// Owns all TreeNode instances. Nodes reference each other via NodeId.
/// Phase B: content/children will also use NodeId instead of Vec<TreeNode>.
pub struct TreeStore {
    nodes: Vec<TreeNode>,
}

impl TreeStore {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn get(&self, id: NodeId) -> &TreeNode {
        &self.nodes[id.0]
    }

    pub fn get_mut(&mut self, id: NodeId) -> &mut TreeNode {
        &mut self.nodes[id.0]
    }

    pub fn add(&mut self, node: TreeNode) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for TreeStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Error types (PORTING.md §3.1) ────────────────────────────────
//
// CoreError wraps the 4 error categories, preserving type information through
// proper From impls (no lossy folding to Unsupported).

#[derive(Debug, Clone)]
pub enum SystemError {
    EndOfStream,
    StreamTooLong,
    Io(String),
}

#[derive(Debug, Clone)]
pub enum ParseError {
    InvalidSyntax,
    InvalidYaml,
    InvalidJson,
    InvalidPython,
    InvalidJavaScript,
    InvalidToml { detail: String },
    TreeSitterFailed,
    UnknownToken,
    UnterminatedString,
    BadCsv,
    BadParameter,
    InvalidCharacter,
    InvalidPadding,
    NegativeIndex,
    Utf8CannotEncodeSurrogateHalf,
    CodepointTooLarge,
}

#[derive(Debug, Clone)]
pub enum FormatError {
    UnknownFormat,
    TomlRequiresMap,
    TomlEmptyPath,
    TomlNoAliases,
    TomlUnsupportedKind,
}

#[derive(Debug, Clone)]
pub enum EvalError {
    UnknownOperator { op: String },
    MissingRegistry,
    MissingRhs,
    MissingLhs,
    MissingTreeNode,
    MustUseVariableWithPipe,
    InvalidVariableRhs,
    KeysOnlyWorksForMapsAndArrays,
    CannotConvertValueToNumber,
    CannotConvertNodeToNumber,
    NodeIsNotArray,
    ExpectedSingleNumber,
    UniqueOnlySupportsArrays,
    CannotModuloByZero,
    CannotModuloTypes,
    CannotModuloNull,
    CannotModuloNonScalars,
    CannotDivideTypes,
    CannotDivideNull,
    CannotDivideNonScalars,
    StringsCannotBeSubtracted,
    CannotSubtractTypes,
    MapsNotSupportedForSubtraction,
    CannotSubtractNonSequence,
    CannotSubtractNonScalar,
    Unsupported { op: String, message: String },
    ExpectedMap,
    NoKeys,
    FromEntriesOnlyRunsAgainstArrays,
    CannotIndexArray,
    CannotPickIndicesFromType,
    NegativeRepeat,
    RepeatTooLarge,
    CannotMultiplyTypes,
    CannotAddTypes,
    CannotAddNonMapToMap,
    CannotAddNonScalarToScalar,
    IndexOutOfRange { index: i64, len: usize },
    OutOfRange,
    Overflow,
    InvalidSyntax,
    InvalidFormat,
    InvalidYaml,
    InvalidJson,
    InvalidPython,
    InvalidJavaScript,
    UnknownOperatorFlat,
    UnsupportedFlat,
}

/// Preserves the 4 error categories so callers can match on the error kind.
#[derive(Debug, Clone)]
pub enum CoreError {
    OutOfMemory,
    System(SystemError),
    Parse(ParseError),
    Format(FormatError),
    Eval(EvalError),
    ParseMessage {
        line: usize,
        column: usize,
        message: String,
    },
    OperatorMessage {
        op: String,
        message: String,
    },
    WasmProtocol {
        code: i32,
    },
    Io(String),
}

impl From<SystemError> for CoreError {
    fn from(e: SystemError) -> Self {
        CoreError::System(e)
    }
}

impl From<ParseError> for CoreError {
    fn from(e: ParseError) -> Self {
        CoreError::Parse(e)
    }
}

impl From<FormatError> for CoreError {
    fn from(e: FormatError) -> Self {
        CoreError::Format(e)
    }
}

impl From<EvalError> for CoreError {
    fn from(e: EvalError) -> Self {
        CoreError::Eval(e)
    }
}

// ── SemType (LIFETIMES.tsv, PORTING.md §6.1) ────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemType {
    Nil,
    Str,
    Int,
    Float,
    Boolean,
    Map,
    Seq,
}

impl SemType {
    /// Strict parsing: only accepts YAML-standard "!!" prefixed tags.
    /// Mirrors `crate::core::SemType::from_string`.
    pub fn from_string(s: &str) -> Option<SemType> {
        match s {
            "!!null" => Some(SemType::Nil),
            "!!str" => Some(SemType::Str),
            "!!int" => Some(SemType::Int),
            "!!float" => Some(SemType::Float),
            "!!bool" => Some(SemType::Boolean),
            "!!map" => Some(SemType::Map),
            "!!seq" => Some(SemType::Seq),
            _ => None,
        }
    }

    /// Loose parsing: accepts both YAML-standard "!!" tags and human-readable
    /// aliases (e.g. "string", "null", "object", "array"). Kept for backward
    /// compatibility with operator code that passes non-standard tag strings.
    pub fn from_string_loose(s: &str) -> Option<SemType> {
        match s {
            "!!null" | "nil" | "null" => Some(SemType::Nil),
            "!!str" | "str" | "string" => Some(SemType::Str),
            "!!int" | "int" | "integer" => Some(SemType::Int),
            "!!float" | "float" | "number" => Some(SemType::Float),
            "!!bool" | "bool" | "boolean" => Some(SemType::Boolean),
            "!!map" | "map" | "mapping" | "object" => Some(SemType::Map),
            "!!seq" | "seq" | "sequence" | "array" => Some(SemType::Seq),
            _ => None,
        }
    }

    pub fn to_string(self) -> &'static str {
        self.tag()
    }

    pub fn tag(self) -> &'static str {
        match self {
            SemType::Nil => "!!null",
            SemType::Str => "!!str",
            SemType::Int => "!!int",
            SemType::Float => "!!float",
            SemType::Boolean => "!!bool",
            SemType::Map => "!!map",
            SemType::Seq => "!!seq",
        }
    }

    pub fn has_tag_prefix(tag: &str) -> bool {
        tag.starts_with("!!")
    }
}

// Bridge to/from the core sem_type::SemType.
impl From<crate::core::sem_type::SemType> for SemType {
    fn from(s: crate::core::sem_type::SemType) -> Self {
        use crate::core::sem_type::SemType as Core;
        match s {
            Core::Map => SemType::Map,
            Core::Seq => SemType::Seq,
            Core::Str => SemType::Str,
            Core::Int => SemType::Int,
            Core::Float => SemType::Float,
            Core::Boolean => SemType::Boolean,
            Core::Nil => SemType::Nil,
        }
    }
}

impl From<SemType> for crate::core::sem_type::SemType {
    fn from(s: SemType) -> Self {
        use crate::core::sem_type::SemType as Core;
        match s {
            SemType::Map => Core::Map,
            SemType::Seq => Core::Seq,
            SemType::Str => Core::Str,
            SemType::Int => Core::Int,
            SemType::Float => Core::Float,
            SemType::Boolean => Core::Boolean,
            SemType::Nil => Core::Nil,
        }
    }
}

// ── NodeKind ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Scalar,
    Mapping,
    Sequence,
    Alias,
    Unknown,
}

/// Normalized representation of a node's value.
/// Used for test assertions and type-branching logic.
/// Covers the subset needed: integer/float/boolean/null/string.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueRep {
    Int(i64),
    Float(f64),
    Boolean(bool),
    Nil,
    Str(String),
}

/// Infer the standard scalar tag (.boolean/.int/.float/.str) from `value`.
///
/// If `current_tag` already has a standard "!!" prefix, it is returned as-is.
/// When `value` is empty, no inference is performed and `current_tag` is returned.
/// Falls back to `current_tag` (or `.str` if `current_tag` is empty) when
/// inference fails.
pub fn infer_scalar_tag<'a>(current_tag: &'a str, value: &str) -> &'a str {
    if SemType::has_tag_prefix(current_tag) || value.is_empty() {
        return current_tag;
    }
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "false" | "y" | "n" | "yes" | "no"
    ) {
        return SemType::Boolean.tag();
    }
    if value.contains(['.', 'e', 'E']) {
        return if value.parse::<f64>().is_ok() {
            SemType::Float.tag()
        } else if current_tag.is_empty() {
            SemType::Str.tag()
        } else {
            current_tag
        };
    }
    if value.parse::<i64>().is_ok() {
        SemType::Int.tag()
    } else if current_tag.is_empty() {
        SemType::Str.tag()
    } else {
        current_tag
    }
}

fn is_truthy_value(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "y" | "yes" | "on" | "1"
    )
}

/// Lightweight node metadata for serialization/printing.
#[derive(Debug, Clone, Default)]
pub struct NodeInfo {
    pub kind: String,
    pub anchor: String,
    pub tag: String,
    pub head_comment: String,
    pub line_comment: String,
    pub foot_comment: String,
    pub value: String,
    pub line: i32,
    pub column: i32,
    pub content: Vec<NodeInfo>,
}

// ── TreeNode (LIFETIMES.tsv, PORTING.md §2.3) ────────────────────

/// TreeNode with NodeId-based references for parent/key/alias.
/// content kept as Vec<TreeNode> (Phase B cancelled — not needed).
/// Phase A: `content` kept as Vec<TreeNode> for direct operator access.
/// Phase B: migrate to Vec<NodeId> for tree-store ownership.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub kind: NodeKind,
    pub sequence_closed: bool,
    pub sem_type: Option<SemType>,
    pub tag: String,
    pub value: String,
    pub start_byte: u32,
    pub end_byte: u32,
    /// Direct child nodes (Vec<TreeNode> stays, Phase B cancelled).
    pub content: Vec<TreeNode>,
    pub leading_content: String,
    /// Non-owning reference to parent node (LIFETIMES.tsv row 1).
    pub parent: Option<NodeId>,
    /// Map value → corresponding key node (LIFETIMES.tsv row 3).
    pub key: Option<NodeId>,
    pub is_map_key: bool,
    pub sequence_index: Option<i64>,
    /// YAML alias target (LIFETIMES.tsv row 2).
    pub alias: Option<NodeId>,
    pub anchor: String,
    pub head_comment: String,
    pub line_comment: String,
    pub foot_comment: String,
    pub document: u32,
    pub filename: String,
    pub line: i32,
    pub column: i32,
    pub file_index: i32,
    pub encode_separate: bool,
    pub evaluate_together: bool,
}

impl Default for TreeNode {
    fn default() -> Self {
        Self {
            kind: NodeKind::Scalar,
            sequence_closed: true,
            sem_type: Some(SemType::Nil),
            tag: String::new(),
            value: String::new(),
            start_byte: 0,
            end_byte: 0,
            content: Vec::new(),
            leading_content: String::new(),
            parent: None,
            key: None,
            is_map_key: false,
            sequence_index: None,
            alias: None,
            anchor: String::new(),
            head_comment: String::new(),
            line_comment: String::new(),
            foot_comment: String::new(),
            document: 0,
            filename: String::new(),
            line: 0,
            column: 0,
            file_index: 0,
            encode_separate: false,
            evaluate_together: false,
        }
    }
}

impl TreeNode {
    pub fn scalar(sem_type: SemType, value: impl Into<String>) -> Self {
        let mut n = Self::default();
        n.kind = NodeKind::Scalar;
        n.sem_type = Some(sem_type);
        n.tag = sem_type.to_string().to_owned();
        n.value = value.into();
        n
    }

    pub fn resolved_sem_type(&self) -> Option<SemType> {
        self.sem_type
    }

    pub fn guess_tag_from_custom_type(&self) -> String {
        infer_scalar_tag(&self.tag, &self.value).to_owned()
    }

    pub fn can_visit_values(&self) -> bool {
        matches!(self.kind, NodeKind::Mapping | NodeKind::Sequence)
    }

    /// Set the parent NodeId reference.
    pub fn set_parent(&mut self, parent: Option<NodeId>) {
        self.parent = parent;
    }

    /// Get the normalized value representation (int/float/boolean/nil/str).
    pub fn get_value_rep(&self) -> Result<ValueRep, CoreError> {
        let real_tag = self.guess_tag_from_custom_type();
        match SemType::from_string(&real_tag) {
            Some(SemType::Int) => match self.value.parse::<i64>() {
                Ok(v) => Ok(ValueRep::Int(v)),
                Err(_) => Err(CoreError::ParseMessage {
                    line: 0,
                    column: 0,
                    message: format!(
                        "integer value '{}' out of range for target format",
                        self.value
                    ),
                }),
            },
            Some(SemType::Float) => self
                .value
                .parse::<f64>()
                .map(ValueRep::Float)
                .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax)),
            Some(SemType::Boolean) => Ok(ValueRep::Boolean(is_truthy_value(&self.value))),
            Some(SemType::Nil) => Ok(ValueRep::Nil),
            _ => Ok(ValueRep::Str(self.value.clone())),
        }
    }

    /// Recursively deinitialize content children.
    /// In the compat layer (Phase A), content is owned Vec<TreeNode>,
    /// so this simply clears the content vector. The Drop impl handles
    /// recursive cleanup naturally via Rust's ownership model.
    pub fn deinit_recursive(&mut self) {
        if self.content.is_empty() {
            return;
        }
        if matches!(self.kind, NodeKind::Mapping | NodeKind::Sequence) {
            for child in &mut self.content {
                child.deinit_recursive();
            }
            self.content.clear();
        }
    }

    pub fn add_child(&mut self, child: &TreeNode) -> Result<(), CoreError> {
        self.content.push(child.clone());
        Ok(())
    }

    pub fn add_children(&mut self, children: &[TreeNode]) -> Result<(), CoreError> {
        self.content.extend(children.iter().cloned());
        Ok(())
    }

    pub fn add_key_value_child(
        &mut self,
        key: &TreeNode,
        value: &TreeNode,
    ) -> Result<(), CoreError> {
        let mut key_clone = key.clone();
        key_clone.is_map_key = true;
        let mut value_clone = value.clone();
        value_clone.is_map_key = false;
        self.content.push(key_clone);
        self.content.push(value_clone);
        Ok(())
    }

    pub fn copy(&self) -> Result<Box<TreeNode>, CoreError> {
        Ok(Box::new(self.clone()))
    }

    pub fn copy_without_content(&self) -> Result<Box<TreeNode>, CoreError> {
        let mut n = self.clone();
        n.content = Vec::new();
        Ok(Box::new(n))
    }

    pub fn copy_as_replacement(&self, src: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
        let mut n = src.clone();
        n.parent = self.parent;
        n.key = self.key;
        n.is_map_key = self.is_map_key;
        n.sequence_index = self.sequence_index;
        n.document = self.document;
        n.filename = self.filename.clone();
        n.line = self.line;
        n.column = self.column;
        n.file_index = self.file_index;
        Ok(Box::new(n))
    }

    pub fn create_replacement(
        &self,
        kind: NodeKind,
        tag: &str,
        value: &str,
    ) -> Result<Box<TreeNode>, CoreError> {
        let mut n = TreeNode::default();
        n.kind = kind;
        n.sem_type = SemType::from_string(tag);
        n.tag = tag.to_string();
        n.value = value.to_string();
        n.parent = self.parent;
        n.key = self.key;
        n.is_map_key = self.is_map_key;
        n.sequence_index = self.sequence_index;
        n.document = self.document;
        n.filename = self.filename.clone();
        n.line = self.line;
        n.column = self.column;
        n.file_index = self.file_index;
        Ok(Box::new(n))
    }

    pub fn create_replacement_with_comments(
        &self,
        kind: NodeKind,
        tag: &str,
    ) -> Result<Box<TreeNode>, CoreError> {
        let mut replacement = (*self.create_replacement(kind, tag, "")?).clone();
        replacement.leading_content = self.leading_content.clone();
        replacement.head_comment = self.head_comment.clone();
        replacement.line_comment = self.line_comment.clone();
        replacement.foot_comment = self.foot_comment.clone();
        Ok(Box::new(replacement))
    }

    pub fn create_child(&self) -> Result<Box<TreeNode>, CoreError> {
        Ok(Box::new(TreeNode::default()))
    }

    pub fn get_key(&self) -> Result<String, CoreError> {
        let key_prefix = if self.is_map_key {
            format!("key-{}-", self.value)
        } else {
            String::new()
        };
        let key_value = if let Some(key) = self.key {
            key.0.to_string()
        } else if let Some(index) = self.sequence_index {
            index.to_string()
        } else {
            format!("self-{:p}", self)
        };
        Ok(format!("{}{} - {}", key_prefix, self.document, key_value))
    }

    pub fn get_nice_path(&self) -> Result<String, CoreError> {
        Ok(self.tag.clone())
    }
}

/// Tracks original text for decoded nodes.
/// In the compat layer (Phase A), TreeNode lacks a stable identity,
/// so we use the raw pointer address (`usize`) as the map key,
#[derive(Debug, Clone, Default)]
pub struct CodecState {
    originals: HashMap<usize, String>,
}

impl CodecState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the original text for a decoded node.
    /// Uses the node's raw pointer address as the key.
    pub fn remember_original(&mut self, decoded: &TreeNode, original: impl Into<String>) {
        let key = decoded as *const TreeNode as usize;
        self.originals.insert(key, original.into());
    }

    /// Look up the original text for a decoded node.
    /// Returns `None` if the node was never recorded.
    pub fn original_for(&self, decoded: &TreeNode) -> Option<&str> {
        let key = decoded as *const TreeNode as usize;
        self.originals.get(&key).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.originals.is_empty()
    }
}

/// Controls which resources are released during `Context::deinit`.
#[derive(Debug, Clone, Copy)]
pub struct DeinitOptions {
    /// If true, release `matching_nodes`. Default: false.
    pub release_matching_nodes: bool,
    /// If true, release `variables`. Default: true.
    pub release_variables: bool,
}

impl Default for DeinitOptions {
    fn default() -> Self {
        Self {
            release_matching_nodes: false,
            release_variables: true,
        }
    }
}

// ── Context (LIFETIMES.tsv rows 5-10) ────────────────────────────
// TODO-PhaseB: matching_nodes → Vec<NodeId> with TreeStore access

#[derive(Debug, Clone)]
pub struct Context {
    /// Phase A: direct node references. Phase B: Vec<NodeId>.
    pub matching_nodes: Vec<TreeNode>,
    /// Allocator marker. Rust uses the global allocator; this field
    /// Use `()` to make it zero-sized.
    pub alloc: (),
    /// Handle to the registry.
    pub codec_registry: usize,
    /// Diagnostics collector (owned for Phase A simplicity).
    pub diagnostics: Option<Box<Diagnostics>>,
    pub user_data: Option<*mut c_void>,
    /// Source filename for the current document.
    pub source_filename: String,
    /// Optional codec state for tracking original text of decoded nodes.
    pub codec_state: Option<Box<CodecState>>,
    pub dont_auto_create: bool,
    pub string_interpolation_enabled: bool,
    /// Variable bindings: name → node list (LIFETIMES.tsv row 6).
    /// NodeList (`ArrayListUnmanaged(*TreeNode)`), so variables share
    /// pointers with matching_nodes. In Rust Phase A, variables own
    /// independent copies. Phase B: migrate to Vec<NodeId>.
    pub variables: HashMap<String, Vec<TreeNode>>,
    pub assign_prefs: AssignPreferences,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            matching_nodes: Vec::new(),
            alloc: (),
            codec_registry: 0,
            diagnostics: None,
            user_data: None,
            source_filename: String::new(),
            codec_state: None,
            dont_auto_create: false,
            string_interpolation_enabled: true,
            variables: HashMap::new(),
            assign_prefs: AssignPreferences::default(),
        }
    }
}

impl Context {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Create a Context with a specific codec registry handle.
    pub fn with_codec_registry(mut self, registry: usize) -> Self {
        self.codec_registry = registry;
        self
    }

    /// Return a new Context with the given diagnostics pointer.
    pub fn with_diagnostics(mut self, diagnostics: Option<Box<Diagnostics>>) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// Return a new Context with the given user data pointer.
    pub fn with_user_data(mut self, user_data: Option<*mut c_void>) -> Self {
        self.user_data = user_data;
        self
    }

    /// Get the current user data pointer.
    pub fn get_user_data(&self) -> Option<*mut c_void> {
        self.user_data
    }

    /// Set the user data pointer on this context.
    pub fn set_user_data(&mut self, user_data: Option<*mut c_void>) {
        self.user_data = user_data;
    }

    /// Ensure a CodecState exists, creating one if necessary.
    pub fn ensure_codec_state(&mut self) -> &mut CodecState {
        self.codec_state
            .get_or_insert_with(|| Box::new(CodecState::new()))
            .as_mut()
    }

    /// Append a matching node to `matching_nodes`.
    pub fn append_matching_node(&mut self, node: TreeNode) -> Result<(), CoreError> {
        self.matching_nodes.push(node);
        Ok(())
    }

    pub fn single_child_context(&self, child: &TreeNode) -> Result<Self, CoreError> {
        let mut ctx = self.clone();
        ctx.matching_nodes = vec![child.clone()];
        Ok(ctx)
    }

    pub fn single_readonly_child_context(&self, child: &TreeNode) -> Result<Self, CoreError> {
        let mut ctx = self.single_child_context(child)?;
        ctx.dont_auto_create = true;
        Ok(ctx)
    }

    pub fn child_context(&self, nodes: Vec<TreeNode>) -> Result<Self, CoreError> {
        let mut ctx = self.clone();
        ctx.matching_nodes = nodes;
        Ok(ctx)
    }

    /// Deep-clone: recursively copies all matching nodes via `TreeNode::copy`.
    pub fn deep_clone(&self) -> Result<Self, CoreError> {
        let mut cloned_nodes = Vec::with_capacity(self.matching_nodes.len());
        for n in &self.matching_nodes {
            cloned_nodes.push((*n.copy()?).clone());
        }
        self.child_context(cloned_nodes)
    }

    /// Shallow clone: shares node pointers with the parent context.
    pub fn shallow_clone(&self) -> Result<Self, CoreError> {
        self.child_context(self.matching_nodes.clone())
    }

    pub fn read_only_clone(&self) -> Result<Self, CoreError> {
        let mut ctx = self.deep_clone()?;
        ctx.dont_auto_create = true;
        Ok(ctx)
    }

    /// Create a writable clone (forces `dont_auto_create = false`).
    pub fn writable_clone(&self) -> Result<Self, CoreError> {
        let mut ctx = self.deep_clone()?;
        ctx.dont_auto_create = false;
        Ok(ctx)
    }

    pub fn set_variable(&mut self, name: &str, nodes: Vec<TreeNode>) -> Result<(), CoreError> {
        self.variables.insert(name.to_string(), nodes);
        Ok(())
    }

    pub fn get_variable(&self, name: &str) -> Option<&Vec<TreeNode>> {
        self.variables.get(name)
    }

    /// Release resources held by this context according to `options`.
    pub fn deinit(&mut self, options: DeinitOptions) {
        // Drop CodecState if present (frees the originals map).
        let _ = self.codec_state.take();
        if options.release_matching_nodes {
            self.matching_nodes.clear();
        }
        if options.release_variables {
            self.variables.clear();
        }
    }
}

// ── Diagnostics ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Diagnostics;

impl Diagnostics {
    pub fn set_message(&self, _level: &str, _msg: &str) -> Result<(), CoreError> {
        Ok(())
    }

    pub fn set_messagef(&self, _level: &str, _msg: &str) -> Result<(), CoreError> {
        Ok(())
    }
}

// ── Operation types (PORTING.md §6.3: OperationId as enum) ───────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum OperationId {
    Pipe = 0,
    ShortPipe = 1,
    SelfReference = 2,
    Expression = 3,
    Value = 4,
    Block = 5,
    Empty = 6,
    Or = 7,
    And = 8,
    Not = 9,
    Alternative = 10,
    Any = 11,
    All = 12,
    AnyCondition = 13,
    AllCondition = 14,
    Assign = 15,
    AddAssign = 16,
    SubtractAssign = 17,
    MultiplyAssign = 18,
    AssignVariable = 19,
    Add = 20,
    Subtract = 21,
    Multiply = 22,
    Divide = 23,
    Modulo = 24,
    Equals = 25,
    NotEquals = 26,
    Relational = 27,
    Min = 28,
    Max = 29,
    CreateMap = 30,
    Collect = 31,
    CollectObject = 32,
    Map = 33,
    MapValues = 34,
    Pick = 35,
    Omit = 36,
    Union = 37,
    Unique = 38,
    UniqueBy = 39,
    GroupBy = 40,
    Flatten = 41,
    Length = 42,
    Encode = 43,
    Decode = 44,
    ToEntries = 45,
    FromEntries = 46,
    WithEntries = 47,
    ToNumber = 48,
    JoinString = 49,
    SubString = 50,
    Match = 51,
    Capture = 52,
    Test = 53,
    SplitString = 54,
    ChangeCase = 55,
    Trim = 56,
    ToString = 57,
    StringInterpolation = 58,
    GetVariable = 59,
    GetTag = 60,
    GetKind = 61,
    GetKey = 62,
    IsKey = 63,
    Keys = 64,
    GetParent = 65,
    GetParents = 66,
    Contains = 67,
    Has = 68,
    TraversePath = 69,
    TraverseArray = 70,
    RecursiveDescent = 71,
    GetPath = 72,
    SetPath = 73,
    DelPaths = 74,
    Delete = 75,
    SortBy = 76,
    Sort = 77,
    SortKeys = 78,
    Reverse = 79,
    Shuffle = 80,
    First = 81,
    Reduce = 82,
    With = 83,
    Select = 84,
    Filter = 85,
}

impl OperationId {
    pub const fn name(self) -> &'static str {
        match self {
            OperationId::Pipe => "pipe",
            OperationId::ShortPipe => "short_pipe",
            OperationId::SelfReference => "self_reference",
            OperationId::Expression => "expression",
            OperationId::Value => "value",
            OperationId::Block => "block",
            OperationId::Empty => "empty",
            OperationId::Or => "or",
            OperationId::And => "and",
            OperationId::Not => "not",
            OperationId::Alternative => "alternative",
            OperationId::Any => "any",
            OperationId::All => "all",
            OperationId::AnyCondition => "any_condition",
            OperationId::AllCondition => "all_condition",
            OperationId::Assign => "assign",
            OperationId::AddAssign => "add_assign",
            OperationId::SubtractAssign => "subtract_assign",
            OperationId::MultiplyAssign => "multiply_assign",
            OperationId::AssignVariable => "assign_variable",
            OperationId::Add => "add",
            OperationId::Subtract => "subtract",
            OperationId::Multiply => "multiply",
            OperationId::Divide => "divide",
            OperationId::Modulo => "modulo",
            OperationId::Equals => "equals",
            OperationId::NotEquals => "not_equals",
            OperationId::Relational => "relational",
            OperationId::Min => "min",
            OperationId::Max => "max",
            OperationId::CreateMap => "create_map",
            OperationId::Collect => "collect",
            OperationId::CollectObject => "collect_object",
            OperationId::Map => "map",
            OperationId::MapValues => "map_values",
            OperationId::Pick => "pick",
            OperationId::Omit => "omit",
            OperationId::Union => "union",
            OperationId::Unique => "unique",
            OperationId::UniqueBy => "unique_by",
            OperationId::GroupBy => "group_by",
            OperationId::Flatten => "flatten",
            OperationId::Length => "length",
            OperationId::Encode => "encode",
            OperationId::Decode => "decode",
            OperationId::ToEntries => "to_entries",
            OperationId::FromEntries => "from_entries",
            OperationId::WithEntries => "with_entries",
            OperationId::ToNumber => "to_number",
            OperationId::JoinString => "join_string",
            OperationId::SubString => "sub_string",
            OperationId::Match => "match",
            OperationId::Capture => "capture",
            OperationId::Test => "test",
            OperationId::SplitString => "split_string",
            OperationId::ChangeCase => "change_case",
            OperationId::Trim => "trim",
            OperationId::ToString => "to_string",
            OperationId::StringInterpolation => "string_interpolation",
            OperationId::GetVariable => "get_variable",
            OperationId::GetTag => "get_tag",
            OperationId::GetKind => "get_kind",
            OperationId::GetKey => "get_key",
            OperationId::IsKey => "is_key",
            OperationId::Keys => "keys",
            OperationId::GetParent => "get_parent",
            OperationId::GetParents => "get_parents",
            OperationId::Contains => "contains",
            OperationId::Has => "has",
            OperationId::TraversePath => "traverse_path",
            OperationId::TraverseArray => "traverse_array",
            OperationId::RecursiveDescent => "recursive_descent",
            OperationId::GetPath => "get_path",
            OperationId::SetPath => "set_path",
            OperationId::DelPaths => "del_paths",
            OperationId::Delete => "delete",
            OperationId::SortBy => "sort_by",
            OperationId::Sort => "sort",
            OperationId::SortKeys => "sort_keys",
            OperationId::Reverse => "reverse",
            OperationId::Shuffle => "shuffle",
            OperationId::First => "first",
            OperationId::Reduce => "reduce",
            OperationId::With => "with",
            OperationId::Select => "select",
            OperationId::Filter => "filter",
        }
    }
}

/// Thin wrapper around OperationId providing the name() accessor.
#[derive(Debug, Clone, Copy)]
pub struct OperationType {
    pub id: OperationId,
}

impl OperationType {
    pub const fn new(id: OperationId) -> Self {
        Self { id }
    }

    pub fn name(&self) -> &'static str {
        self.id.name()
    }
}

pub static PIPE_OP_TYPE: OperationType = OperationType::new(OperationId::Pipe);
pub static SHORT_PIPE_OP_TYPE: OperationType = OperationType::new(OperationId::ShortPipe);
pub static SELF_REFERENCE_OP_TYPE: OperationType = OperationType::new(OperationId::SelfReference);
pub static EXPRESSION_OP_TYPE: OperationType = OperationType::new(OperationId::Expression);
pub static VALUE_OP_TYPE: OperationType = OperationType::new(OperationId::Value);
pub static BLOCK_OP_TYPE: OperationType = OperationType::new(OperationId::Block);
pub static EMPTY_OP_TYPE: OperationType = OperationType::new(OperationId::Empty);
pub static OR_OP_TYPE: OperationType = OperationType::new(OperationId::Or);
pub static AND_OP_TYPE: OperationType = OperationType::new(OperationId::And);
pub static NOT_OP_TYPE: OperationType = OperationType::new(OperationId::Not);
pub static ALTERNATIVE_OP_TYPE: OperationType = OperationType::new(OperationId::Alternative);
pub static ANY_OP_TYPE: OperationType = OperationType::new(OperationId::Any);
pub static ALL_OP_TYPE: OperationType = OperationType::new(OperationId::All);
pub static ANY_CONDITION_OP_TYPE: OperationType = OperationType::new(OperationId::AnyCondition);
pub static ALL_CONDITION_OP_TYPE: OperationType = OperationType::new(OperationId::AllCondition);
pub static ASSIGN_OP_TYPE: OperationType = OperationType::new(OperationId::Assign);
pub static ADD_ASSIGN_OP_TYPE: OperationType = OperationType::new(OperationId::AddAssign);
pub static SUBTRACT_ASSIGN_OP_TYPE: OperationType = OperationType::new(OperationId::SubtractAssign);
pub static MULTIPLY_ASSIGN_OP_TYPE: OperationType = OperationType::new(OperationId::MultiplyAssign);
pub static ASSIGN_VARIABLE_OP_TYPE: OperationType = OperationType::new(OperationId::AssignVariable);
pub static ADD_OP_TYPE: OperationType = OperationType::new(OperationId::Add);
pub static SUBTRACT_OP_TYPE: OperationType = OperationType::new(OperationId::Subtract);
pub static MULTIPLY_OP_TYPE: OperationType = OperationType::new(OperationId::Multiply);
pub static DIVIDE_OP_TYPE: OperationType = OperationType::new(OperationId::Divide);
pub static MODULO_OP_TYPE: OperationType = OperationType::new(OperationId::Modulo);
pub static EQUALS_OP_TYPE: OperationType = OperationType::new(OperationId::Equals);
pub static NOT_EQUALS_OP_TYPE: OperationType = OperationType::new(OperationId::NotEquals);
pub static RELATIONAL_OP_TYPE: OperationType = OperationType::new(OperationId::Relational);
pub static MIN_OP_TYPE: OperationType = OperationType::new(OperationId::Min);
pub static MAX_OP_TYPE: OperationType = OperationType::new(OperationId::Max);
pub static CREATE_MAP_OP_TYPE: OperationType = OperationType::new(OperationId::CreateMap);
pub static COLLECT_OP_TYPE: OperationType = OperationType::new(OperationId::Collect);
pub static COLLECT_OBJECT_OP_TYPE: OperationType = OperationType::new(OperationId::CollectObject);
pub static MAP_OP_TYPE: OperationType = OperationType::new(OperationId::Map);
pub static MAP_VALUES_OP_TYPE: OperationType = OperationType::new(OperationId::MapValues);
pub static PICK_OP_TYPE: OperationType = OperationType::new(OperationId::Pick);
pub static OMIT_OP_TYPE: OperationType = OperationType::new(OperationId::Omit);
pub static UNION_OP_TYPE: OperationType = OperationType::new(OperationId::Union);
pub static UNIQUE_OP_TYPE: OperationType = OperationType::new(OperationId::Unique);
pub static UNIQUE_BY_OP_TYPE: OperationType = OperationType::new(OperationId::UniqueBy);
pub static GROUP_BY_OP_TYPE: OperationType = OperationType::new(OperationId::GroupBy);
pub static FLATTEN_OP_TYPE: OperationType = OperationType::new(OperationId::Flatten);
pub static LENGTH_OP_TYPE: OperationType = OperationType::new(OperationId::Length);
pub static ENCODE_OP_TYPE: OperationType = OperationType::new(OperationId::Encode);
pub static DECODE_OP_TYPE: OperationType = OperationType::new(OperationId::Decode);
pub static TO_ENTRIES_OP_TYPE: OperationType = OperationType::new(OperationId::ToEntries);
pub static FROM_ENTRIES_OP_TYPE: OperationType = OperationType::new(OperationId::FromEntries);
pub static WITH_ENTRIES_OP_TYPE: OperationType = OperationType::new(OperationId::WithEntries);
pub static TO_NUMBER_OP_TYPE: OperationType = OperationType::new(OperationId::ToNumber);
pub static JOIN_STRING_OP_TYPE: OperationType = OperationType::new(OperationId::JoinString);
pub static SUB_STRING_OP_TYPE: OperationType = OperationType::new(OperationId::SubString);
pub static MATCH_OP_TYPE: OperationType = OperationType::new(OperationId::Match);
pub static CAPTURE_OP_TYPE: OperationType = OperationType::new(OperationId::Capture);
pub static TEST_OP_TYPE: OperationType = OperationType::new(OperationId::Test);
pub static SPLIT_STRING_OP_TYPE: OperationType = OperationType::new(OperationId::SplitString);
pub static CHANGE_CASE_OP_TYPE: OperationType = OperationType::new(OperationId::ChangeCase);
pub static TRIM_OP_TYPE: OperationType = OperationType::new(OperationId::Trim);
pub static TO_STRING_OP_TYPE: OperationType = OperationType::new(OperationId::ToString);
pub static STRING_INTERPOLATION_OP_TYPE: OperationType =
    OperationType::new(OperationId::StringInterpolation);
pub static GET_VARIABLE_OP_TYPE: OperationType = OperationType::new(OperationId::GetVariable);
pub static GET_TAG_OP_TYPE: OperationType = OperationType::new(OperationId::GetTag);
pub static GET_KIND_OP_TYPE: OperationType = OperationType::new(OperationId::GetKind);
pub static GET_KEY_OP_TYPE: OperationType = OperationType::new(OperationId::GetKey);
pub static IS_KEY_OP_TYPE: OperationType = OperationType::new(OperationId::IsKey);
pub static KEYS_OP_TYPE: OperationType = OperationType::new(OperationId::Keys);
pub static GET_PARENT_OP_TYPE: OperationType = OperationType::new(OperationId::GetParent);
pub static GET_PARENTS_OP_TYPE: OperationType = OperationType::new(OperationId::GetParents);
pub static CONTAINS_OP_TYPE: OperationType = OperationType::new(OperationId::Contains);
pub static HAS_OP_TYPE: OperationType = OperationType::new(OperationId::Has);
pub static TRAVERSE_PATH_OP_TYPE: OperationType = OperationType::new(OperationId::TraversePath);
pub static TRAVERSE_ARRAY_OP_TYPE: OperationType = OperationType::new(OperationId::TraverseArray);
pub static RECURSIVE_DESCENT_OP_TYPE: OperationType =
    OperationType::new(OperationId::RecursiveDescent);
pub static GET_PATH_OP_TYPE: OperationType = OperationType::new(OperationId::GetPath);
pub static SET_PATH_OP_TYPE: OperationType = OperationType::new(OperationId::SetPath);
pub static DEL_PATHS_OP_TYPE: OperationType = OperationType::new(OperationId::DelPaths);
pub static DELETE_OP_TYPE: OperationType = OperationType::new(OperationId::Delete);
pub static SORT_BY_OP_TYPE: OperationType = OperationType::new(OperationId::SortBy);
pub static SORT_OP_TYPE: OperationType = OperationType::new(OperationId::Sort);
pub static SORT_KEYS_OP_TYPE: OperationType = OperationType::new(OperationId::SortKeys);
pub static REVERSE_OP_TYPE: OperationType = OperationType::new(OperationId::Reverse);
pub static SHUFFLE_OP_TYPE: OperationType = OperationType::new(OperationId::Shuffle);
pub static FIRST_OP_TYPE: OperationType = OperationType::new(OperationId::First);
pub static REDUCE_OP_TYPE: OperationType = OperationType::new(OperationId::Reduce);
pub static WITH_OP_TYPE: OperationType = OperationType::new(OperationId::With);
pub static SELECT_OP_TYPE: OperationType = OperationType::new(OperationId::Select);
pub static FILTER_OP_TYPE: OperationType = OperationType::new(OperationId::Filter);

// ── Operation / ExpressionNode ───────────────────────────────────

#[derive(Debug, Clone)]
pub enum OperationPreference {
    Traverse(TraversePreferences),
    Flatten(FlattenPreferences),
    RecursiveDescent(RecursiveDescentPreferences),
    Parent(ParentOpPreferences),
    Relational(RelationalPref),
    ChangeCase(ChangeCasePrefs),
    Encoder(EncoderPreferences),
    Decoder(DecoderPreferences),
    Assign(AssignPreferences),
    AssignVar(AssignVarPreferences),
    Expression(ExpressionOpPreferences),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlattenPreferences {
    pub depth: i32,
}

impl Default for FlattenPreferences {
    fn default() -> Self {
        Self { depth: -1 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveDescentPreferences {
    pub traverse_preferences: TraversePreferences,
    pub recurse_array: bool,
}

impl Default for RecursiveDescentPreferences {
    fn default() -> Self {
        Self {
            traverse_preferences: TraversePreferences::default(),
            recurse_array: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentOpPreferences {
    pub level: i32,
}

impl Default for ParentOpPreferences {
    fn default() -> Self {
        Self { level: 1 }
    }
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub operation_type: &'static OperationType,
    pub value: Option<Box<TreeNode>>,
    pub string_value: String,
    pub tree_node: Option<Box<TreeNode>>,
    pub preferences: Option<Box<OperationPreference>>,
    pub update_assign: bool,
}

impl Operation {
    pub fn name(&self) -> &'static str {
        self.operation_type.name()
    }
}

#[derive(Debug, Clone)]
pub struct ExpressionNode {
    pub operation: Box<Operation>,
    pub lhs: Option<Box<ExpressionNode>>,
    pub rhs: Option<Box<ExpressionNode>>,
}

// ── Preference types ─────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraversePreferences {
    pub optional_traverse: bool,
    pub dont_follow_alias: bool,
    pub include_map_keys: bool,
    pub dont_include_map_values: bool,
    pub dont_auto_create: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RelationalPref {
    pub greater: bool,
    pub or_equal: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ChangeCasePrefs {
    pub to_upper_case: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EncoderPreferences {
    pub format: String,
    pub indent: i32,
    pub unwrap_scalar: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DecoderPreferences {
    pub format: String,
}

#[derive(Debug, Clone, Default)]
pub struct AssignPreferences {
    pub clobber_custom_tags: bool,
    pub dont_overwrite_anchor: bool,
    pub only_write_null: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ExpressionOpPreferences {
    pub expression: String,
}

#[derive(Debug, Clone, Default)]
pub struct AssignVarPreferences {
    pub is_reference: bool,
}

// ── Operator handler type (PORTING.md §7.2) ──────────────────────

/// The canonical operator signature:
///   fn(ctx: Context, d: &mut TreeEngine, expr: &mut ExpressionNode) -> Result<Context, CoreError>
pub type OperatorHandler =
    fn(ctx: Context, d: &mut TreeEngine, expr: &mut ExpressionNode) -> Result<Context, CoreError>;

/// Cross-function calculation: (navigator, context, lhs?, rhs?) → result?
pub type CrossFunctionCalculation = fn(
    d: &mut TreeEngine,
    ctx: Context,
    lhs: Option<&TreeNode>,
    rhs: Option<&TreeNode>,
) -> Result<Option<Box<TreeNode>>, CoreError>;

pub type LhsResultValueFn =
    fn(ctx: Context, lhs: Option<&TreeNode>) -> Result<Option<Box<TreeNode>>, CoreError>;

pub struct CrossFunctionPreferences {
    pub calc_when_empty: bool,
    pub lhs_result_value: Option<LhsResultValueFn>,
    pub calculation: CrossFunctionCalculation,
}

pub type CompoundCalculation =
    fn(lhs: &mut ExpressionNode, rhs: &ExpressionNode) -> Box<ExpressionNode>;

// ── Operator Registry ────────────────────────────────────────────

pub struct OperatorRegistry {
    handlers: HashMap<OperationId, OperatorHandler>,
}

impl OperatorRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register_operator(
        &mut self,
        id: OperationId,
        handler: OperatorHandler,
    ) -> Result<(), CoreError> {
        self.handlers.insert(id, handler);
        Ok(())
    }

    pub fn get_handler(&self, op_type: &OperationType) -> Option<&OperatorHandler> {
        self.handlers.get(&op_type.id)
    }
}

// ── Helper constructors for tree nodes ───────────────────────────

pub fn create_string_scalar_node(value: &str) -> Result<Box<TreeNode>, CoreError> {
    let mut n = TreeNode::default();
    n.kind = NodeKind::Scalar;
    n.sem_type = Some(SemType::Str);
    n.tag = SemType::Str.to_string().into();
    n.value = value.to_string();
    Ok(Box::new(n))
}

pub fn create_scalar_node_i64(value: i64) -> Result<Box<TreeNode>, CoreError> {
    let mut n = TreeNode::default();
    n.kind = NodeKind::Scalar;
    n.sem_type = Some(SemType::Int);
    n.tag = SemType::Int.to_string().into();
    n.value = value.to_string();
    Ok(Box::new(n))
}

pub fn create_scalar_node_f64(value: f64) -> Result<Box<TreeNode>, CoreError> {
    let mut n = TreeNode::default();
    n.kind = NodeKind::Scalar;
    n.sem_type = Some(SemType::Float);
    n.tag = SemType::Float.to_string().into();
    n.value = value.to_string();
    Ok(Box::new(n))
}

pub fn create_scalar_node_bool(value: bool) -> Result<Box<TreeNode>, CoreError> {
    let mut n = TreeNode::default();
    n.kind = NodeKind::Scalar;
    n.sem_type = Some(SemType::Boolean);
    n.tag = SemType::Boolean.to_string().into();
    n.value = if value {
        "true".to_string()
    } else {
        "false".to_string()
    };
    Ok(Box::new(n))
}

pub fn create_scalar_node_null() -> Result<Box<TreeNode>, CoreError> {
    let mut n = TreeNode::default();
    n.kind = NodeKind::Scalar;
    n.sem_type = Some(SemType::Nil);
    n.tag = SemType::Nil.to_string().into();
    n.value = "null".to_string();
    Ok(Box::new(n))
}

pub fn create_value_operation(node: Box<TreeNode>) -> Result<Box<Operation>, CoreError> {
    Ok(Box::new(Operation {
        operation_type: &VALUE_OP_TYPE,
        value: None,
        string_value: node.value.clone(),
        tree_node: Some(node),
        preferences: None,
        update_assign: false,
    }))
}
