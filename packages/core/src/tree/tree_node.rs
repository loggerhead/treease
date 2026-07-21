use crate::errors::{CoreError, ParseError};
use crate::language::SemType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

impl NodeId {
    pub const NONE: Self = Self(NODE_INDEX_NONE);

    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeExtraId(pub u32);

impl NodeExtraId {
    pub const NONE: Self = Self(NODE_INDEX_NONE);

    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId(pub u32);

impl ValueId {
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum NodeValueRef {
    #[default]
    Missing,
    Inline(Box<String>),
    Stored(ValueId),
}

impl From<String> for NodeValueRef {
    fn from(value: String) -> Self {
        Self::Inline(Box::new(value))
    }
}

impl From<&str> for NodeValueRef {
    fn from(value: &str) -> Self {
        Self::Inline(Box::new(value.to_owned()))
    }
}

impl From<Box<String>> for NodeValueRef {
    fn from(value: Box<String>) -> Self {
        Self::Inline(value)
    }
}

pub type NodeList = Vec<NodeId>;

const NODE_FLAG_CONTAINER_CLOSED: u8 = 1 << 0;
const NODE_FLAG_EVALUATE_TOGETHER: u8 = 1 << 1;
const NODE_FLAG_ENCODE_SEPARATE: u8 = 1 << 2;
const NODE_INDEX_NONE: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TreeNodeKind {
    Sequence,
    Mapping,
    Scalar,
    Alias,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueRep {
    Int(i64),
    Float(f64),
    Boolean(bool),
    Nil,
    Str(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedKey {
    Str(String),
    Int(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub content: Vec<NodeId>,
}

impl Default for NodeInfo {
    fn default() -> Self {
        Self {
            kind: String::new(),
            anchor: String::new(),
            tag: String::new(),
            head_comment: String::new(),
            line_comment: String::new(),
            foot_comment: String::new(),
            value: String::new(),
            line: 0,
            column: 0,
            content: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactTag {
    Empty,
    Builtin(SemType),
    Custom(Box<String>),
}

impl Default for CompactTag {
    fn default() -> Self {
        Self::Empty
    }
}

impl CompactTag {
    pub fn from_sem_type(sem_type: SemType) -> Self {
        Self::Builtin(sem_type)
    }

    pub fn from_text(tag: impl Into<String>) -> Self {
        let tag = tag.into();
        if tag.is_empty() {
            Self::Empty
        } else if let Some(sem_type) = SemType::from_string(&tag) {
            Self::Builtin(sem_type)
        } else {
            Self::Custom(Box::new(tag))
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Empty => None,
            Self::Builtin(sem_type) => Some(sem_type.tag()),
            Self::Custom(tag) => Some(tag.as_str()),
        }
    }

    pub fn to_string_value(&self) -> String {
        self.as_str().unwrap_or_default().to_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommentBlock {
    pub head: Option<Box<String>>,
    pub line: Option<Box<String>>,
    pub foot: Option<Box<String>>,
}

impl CommentBlock {
    pub fn is_empty(&self) -> bool {
        self.head.is_none() && self.line.is_none() && self.foot.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeExtra {
    pub anchor: Option<Box<String>>,
    pub leading_content: Option<Box<String>>,
    pub comments: Option<CommentBlock>,
}

impl NodeExtra {
    pub fn is_empty(&self) -> bool {
        self.anchor.is_none()
            && self.leading_content.is_none()
            && self.comments.as_ref().is_none_or(CommentBlock::is_empty)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub kind: TreeNodeKind,
    pub sem_type: Option<SemType>,
    pub tag: CompactTag,
    pub value: NodeValueRef,
    pub start_byte: u32,
    pub end_byte: u32,
    #[doc(hidden)]
    pub alias: NodeId,
    pub content: Vec<NodeId>,
    pub parent: Option<NodeId>,
    #[doc(hidden)]
    pub key: NodeId,
    #[doc(hidden)]
    pub sequence_index: u32,
    pub document: u32,
    pub line: i32,
    pub column: i32,
    pub is_map_key: bool,
    #[doc(hidden)]
    pub flags: u8,
    #[doc(hidden)]
    pub extra: NodeExtraId,
}

impl Default for TreeNode {
    fn default() -> Self {
        Self {
            kind: TreeNodeKind::Scalar,
            sem_type: None,
            tag: CompactTag::Empty,
            value: NodeValueRef::Missing,
            start_byte: 0,
            end_byte: 0,
            alias: NodeId::NONE,
            content: Vec::new(),
            parent: None,
            key: NodeId::NONE,
            sequence_index: NODE_INDEX_NONE,
            document: 0,
            line: 0,
            column: 0,
            is_map_key: false,
            flags: NODE_FLAG_CONTAINER_CLOSED,
            extra: NodeExtraId::NONE,
        }
    }
}

impl TreeNode {
    /// Construct a scalar node with an explicit semantic type.
    ///
    /// Contract: when a TreeStore is carrying a same-language JSON document
    /// session, `SemType::Int` / `SemType::Float` values must already be valid
    /// JSON number lexemes. Writers, evaluators, and edit helpers that mutate a
    /// JSON-typed tree own that invariant; `JsonEncoder` does not repair or
    /// re-validate numeric scalar text at encode time.

    pub fn scalar(sem_type: SemType, value: impl Into<String>) -> Self {
        Self {
            kind: TreeNodeKind::Scalar,
            sem_type: Some(sem_type),
            tag: CompactTag::from_sem_type(sem_type),
            value: NodeValueRef::from(value.into()),
            ..Self::default()
        }
    }

    pub fn resolved_sem_type(&self) -> Option<SemType> {
        self.sem_type
            .or_else(|| self.tag.as_str().and_then(SemType::from_string))
    }

    pub fn tag_str(&self) -> &str {
        self.tag.as_str().unwrap_or_default()
    }

    pub fn set_sem_type(&mut self, sem_type: SemType) {
        self.sem_type = Some(sem_type);
        self.tag = CompactTag::from_sem_type(sem_type);
    }

    pub fn set_document(&mut self, document: u32) {
        self.document = document;
    }

    pub fn alias(&self) -> Option<NodeId> {
        (self.alias != NodeId::NONE).then_some(self.alias)
    }

    pub fn set_alias(&mut self, alias: Option<NodeId>) {
        self.alias = alias.unwrap_or(NodeId::NONE);
    }

    pub fn key(&self) -> Option<NodeId> {
        (self.key != NodeId::NONE).then_some(self.key)
    }

    pub fn set_key(&mut self, key: Option<NodeId>) {
        self.key = key.unwrap_or(NodeId::NONE);
    }

    pub fn sequence_index(&self) -> Option<u32> {
        (self.sequence_index != NODE_INDEX_NONE).then_some(self.sequence_index)
    }

    pub fn set_sequence_index(&mut self, index: Option<u32>) {
        self.sequence_index = index.unwrap_or(NODE_INDEX_NONE);
    }

    pub fn sequence_closed(&self) -> bool {
        self.container_closed()
    }

    pub fn set_sequence_closed(&mut self, enabled: bool) {
        self.set_container_closed(enabled);
    }

    /// Streaming decoders keep a container open until its closing event arrives.
    /// Sequence callers retain `sequence_closed`; mapping schema classification
    /// uses this generic form to avoid treating a partial mapping as complete.
    pub fn container_closed(&self) -> bool {
        self.flags & NODE_FLAG_CONTAINER_CLOSED != 0
    }

    pub fn set_container_closed(&mut self, enabled: bool) {
        if enabled {
            self.flags |= NODE_FLAG_CONTAINER_CLOSED;
        } else {
            self.flags &= !NODE_FLAG_CONTAINER_CLOSED;
        }
    }

    pub fn evaluate_together(&self) -> bool {
        self.flags & NODE_FLAG_EVALUATE_TOGETHER != 0
    }

    pub fn set_evaluate_together(&mut self, enabled: bool) {
        if enabled {
            self.flags |= NODE_FLAG_EVALUATE_TOGETHER;
        } else {
            self.flags &= !NODE_FLAG_EVALUATE_TOGETHER;
        }
    }

    pub fn encode_separate(&self) -> bool {
        self.flags & NODE_FLAG_ENCODE_SEPARATE != 0
    }

    pub fn set_encode_separate(&mut self, enabled: bool) {
        if enabled {
            self.flags |= NODE_FLAG_ENCODE_SEPARATE;
        } else {
            self.flags &= !NODE_FLAG_ENCODE_SEPARATE;
        }
    }

    pub fn extra(&self) -> Option<NodeExtraId> {
        (self.extra != NodeExtraId::NONE).then_some(self.extra)
    }

    pub fn set_extra(&mut self, extra: Option<NodeExtraId>) {
        self.extra = extra.unwrap_or(NodeExtraId::NONE);
    }

    pub fn is_leaf(&self) -> bool {
        self.content.is_empty()
    }

    pub fn set_parent(&mut self, parent: Option<NodeId>) {
        self.parent = parent;
    }

    pub fn guess_tag_from_value(&self, value: &str) -> String {
        infer_scalar_tag(self.tag.as_str().unwrap_or_default(), value).to_owned()
    }

    pub fn get_value_rep_with(&self, value: &str) -> Result<ValueRep, CoreError> {
        match SemType::from_string(&self.guess_tag_from_value(value)) {
            Some(SemType::Int) => match value.parse::<i64>() {
                Ok(v) => Ok(ValueRep::Int(v)),
                Err(_) => Err(CoreError::ParseMessage {
                    line: 0,
                    column: 0,
                    message: format!("integer value '{}' out of range for target format", value),
                }),
            },
            Some(SemType::Float) => value
                .parse::<f64>()
                .map(ValueRep::Float)
                .map_err(|_| ParseError::InvalidSyntax.into()),
            Some(SemType::Boolean) => Ok(ValueRep::Boolean(is_truthy_value(value))),
            Some(SemType::Nil) => Ok(ValueRep::Nil),
            _ => Ok(ValueRep::Str(value.to_owned())),
        }
    }

    pub fn can_visit_values(&self) -> bool {
        matches!(self.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence)
    }

    pub fn value_child_ids(&self) -> Vec<NodeId> {
        match self.kind {
            TreeNodeKind::Mapping => self.content.iter().copied().skip(1).step_by(2).collect(),
            TreeNodeKind::Sequence => self.content.clone(),
            _ => Vec::new(),
        }
    }

    pub fn copy_without_content(&self) -> Result<Box<TreeNode>, CoreError> {
        let mut node = self.clone();
        node.content.clear();
        Ok(Box::new(node))
    }

    pub fn copy_as_replacement(&self, replacement: &TreeNode) -> Result<Box<TreeNode>, CoreError> {
        let mut node = replacement.clone();
        node.content.clear();
        node.parent = self.parent;
        node.key = self.key;
        node.is_map_key = self.is_map_key;
        node.flags = self.flags;
        node.sequence_index = self.sequence_index;
        node.document = self.document;
        node.line = self.line;
        node.column = self.column;
        Ok(Box::new(node))
    }

    pub fn create_replacement(
        &self,
        kind: TreeNodeKind,
        tag: &str,
        value: &str,
    ) -> Result<Box<TreeNode>, CoreError> {
        let replacement = TreeNode {
            kind,
            sem_type: SemType::from_string(tag),
            tag: CompactTag::from_text(tag),
            value: value.into(),
            ..TreeNode::default()
        };
        self.copy_as_replacement(&replacement)
    }

    pub fn create_replacement_with_comments(
        &self,
        kind: TreeNodeKind,
        tag: &str,
    ) -> Result<Box<TreeNode>, CoreError> {
        let mut replacement = *self.create_replacement(kind, tag, "")?;
        replacement.extra = self.extra;
        Ok(Box::new(replacement))
    }
}

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
