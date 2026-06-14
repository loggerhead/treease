use super::errors::{CoreError, ParseError};
use super::sem_type::SemType;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

pub type NodeList = Vec<NodeId>;

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
pub struct TreeNode {
    pub kind: TreeNodeKind,
    pub sequence_closed: bool,
    pub sem_type: Option<SemType>,
    pub tag: String,
    pub value: String,
    pub start_byte: u32,
    pub end_byte: u32,
    pub anchor: String,
    pub alias: Option<NodeId>,
    pub content: Vec<NodeId>,
    pub head_comment: String,
    pub line_comment: String,
    pub foot_comment: String,
    pub parent: Option<NodeId>,
    pub key: Option<NodeId>,
    pub sequence_index: Option<i64>,
    pub leading_content: String,
    pub document: u32,
    pub filename: String,
    pub line: i32,
    pub column: i32,
    pub file_index: i32,
    pub evaluate_together: bool,
    pub is_map_key: bool,
    pub encode_separate: bool,
}

impl Default for TreeNode {
    fn default() -> Self {
        Self {
            kind: TreeNodeKind::Scalar,
            sequence_closed: true,
            sem_type: None,
            tag: String::new(),
            value: String::new(),
            start_byte: 0,
            end_byte: 0,
            anchor: String::new(),
            alias: None,
            content: Vec::new(),
            head_comment: String::new(),
            line_comment: String::new(),
            foot_comment: String::new(),
            parent: None,
            key: None,
            sequence_index: None,
            leading_content: String::new(),
            document: 0,
            filename: String::new(),
            line: 0,
            column: 0,
            file_index: 0,
            evaluate_together: false,
            is_map_key: false,
            encode_separate: false,
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
            tag: sem_type.to_string(),
            value: value.into(),
            ..Self::default()
        }
    }

    pub fn resolved_sem_type(&self) -> Option<SemType> {
        self.sem_type.or_else(|| SemType::from_string(&self.tag))
    }

    pub fn set_sem_type(&mut self, sem_type: SemType) {
        self.sem_type = Some(sem_type);
        self.tag = sem_type.to_string();
    }

    pub fn set_document(&mut self, document: u32) {
        self.document = document;
    }

    pub fn is_leaf(&self) -> bool {
        self.content.is_empty()
    }

    pub fn set_filename(&mut self, filename: impl Into<String>) {
        self.filename = filename.into();
    }

    pub fn set_file_index(&mut self, file_index: i32) {
        self.file_index = file_index;
    }

    pub fn set_parent(&mut self, parent: Option<NodeId>) {
        self.parent = parent;
    }

    pub fn guess_tag_from_custom_type(&self) -> String {
        infer_scalar_tag(&self.tag, &self.value).to_owned()
    }

    pub fn get_value_rep(&self) -> Result<ValueRep, CoreError> {
        match SemType::from_string(&self.guess_tag_from_custom_type()) {
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
                .map_err(|_| ParseError::InvalidSyntax.into()),
            Some(SemType::Boolean) => Ok(ValueRep::Boolean(is_truthy_value(&self.value))),
            Some(SemType::Nil) => Ok(ValueRep::Nil),
            _ => Ok(ValueRep::Str(self.value.clone())),
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
        node.sequence_index = self.sequence_index;
        node.document = self.document;
        node.filename = self.filename.clone();
        node.line = self.line;
        node.column = self.column;
        node.file_index = self.file_index;
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
            tag: tag.to_owned(),
            value: value.to_owned(),
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
        replacement.leading_content = self.leading_content.clone();
        replacement.head_comment = self.head_comment.clone();
        replacement.line_comment = self.line_comment.clone();
        replacement.foot_comment = self.foot_comment.clone();
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
