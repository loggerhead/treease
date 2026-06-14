use std::borrow::Cow;

use super::operation_prefs::OperationPreferences;
use super::sem_type::SemType;
use super::tree_node::{TreeNode, infer_scalar_tag};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationId {
    Custom,
    Or,
    And,
    Reduce,
    Block,
    Union,
    Pipe,
    Assign,
    AddAssign,
    SubtractAssign,
    AssignVariable,
    Multiply,
    MultiplyAssign,
    Divide,
    Modulo,
    Add,
    Subtract,
    Alternative,
    Equals,
    NotEquals,
    Relational,
    Min,
    Max,
    CreateMap,
    ShortPipe,
    Exp,
    Collect,
    Map,
    Pick,
    Omit,
    MapValues,
    Encode,
    Decode,
    Any,
    All,
    Contains,
    AnyCondition,
    AllCondition,
    ToEntries,
    FromEntries,
    WithEntries,
    With,
    GetVariable,
    GetTag,
    GetKind,
    GetKey,
    IsKey,
    GetParent,
    GetParents,
    GetPath,
    SetPath,
    DelPaths,
    SortBy,
    First,
    Reverse,
    Sort,
    Shuffle,
    SortKeys,
    Join,
    Substr,
    Match,
    Capture,
    Test,
    Split,
    ChangeCase,
    Trim,
    ToString,
    StringInterp,
    Keys,
    Length,
    CollectObject,
    TraversePath,
    TraverseArray,
    SelfRef,
    Value,
    Not,
    ToNumber,
    Empty,
    RecursiveDescent,
    Select,
    Filter,
    Has,
    Unique,
    UniqueBy,
    GroupBy,
    Flatten,
    Delete,
}

impl OperationId {
    pub fn as_str(self) -> &'static str {
        match self {
            OperationId::Custom => "custom",
            OperationId::Or => "or",
            OperationId::And => "and",
            OperationId::Reduce => "reduce",
            OperationId::Block => "block",
            OperationId::Union => "union",
            OperationId::Pipe => "pipe",
            OperationId::Assign => "assign",
            OperationId::AddAssign => "add_assign",
            OperationId::SubtractAssign => "subtract_assign",
            OperationId::AssignVariable => "assign_variable",
            OperationId::Multiply => "multiply",
            OperationId::MultiplyAssign => "multiply_assign",
            OperationId::Divide => "divide",
            OperationId::Modulo => "modulo",
            OperationId::Add => "add",
            OperationId::Subtract => "subtract",
            OperationId::Alternative => "alternative",
            OperationId::Equals => "equals",
            OperationId::NotEquals => "not_equals",
            OperationId::Relational => "relational",
            OperationId::Min => "min",
            OperationId::Max => "max",
            OperationId::CreateMap => "create_map",
            OperationId::ShortPipe => "short_pipe",
            OperationId::Exp => "exp",
            OperationId::Collect => "collect",
            OperationId::Map => "map",
            OperationId::Pick => "pick",
            OperationId::Omit => "omit",
            OperationId::MapValues => "map_values",
            OperationId::Encode => "encode",
            OperationId::Decode => "decode",
            OperationId::Any => "any",
            OperationId::All => "all",
            OperationId::Contains => "contains",
            OperationId::AnyCondition => "any_condition",
            OperationId::AllCondition => "all_condition",
            OperationId::ToEntries => "to_entries",
            OperationId::FromEntries => "from_entries",
            OperationId::WithEntries => "with_entries",
            OperationId::With => "with",
            OperationId::GetVariable => "get_variable",
            OperationId::GetTag => "get_tag",
            OperationId::GetKind => "get_kind",
            OperationId::GetKey => "get_key",
            OperationId::IsKey => "is_key",
            OperationId::GetParent => "get_parent",
            OperationId::GetParents => "get_parents",
            OperationId::GetPath => "get_path",
            OperationId::SetPath => "set_path",
            OperationId::DelPaths => "del_paths",
            OperationId::SortBy => "sort_by",
            OperationId::First => "first",
            OperationId::Reverse => "reverse",
            OperationId::Sort => "sort",
            OperationId::Shuffle => "shuffle",
            OperationId::SortKeys => "sort_keys",
            OperationId::Join => "join",
            OperationId::Substr => "substr",
            OperationId::Match => "match",
            OperationId::Capture => "capture",
            OperationId::Test => "test",
            OperationId::Split => "split",
            OperationId::ChangeCase => "change_case",
            OperationId::Trim => "trim",
            OperationId::ToString => "to_string",
            OperationId::StringInterp => "string_interp",
            OperationId::Keys => "keys",
            OperationId::Length => "length",
            OperationId::CollectObject => "collect_object",
            OperationId::TraversePath => "traverse_path",
            OperationId::TraverseArray => "traverse_array",
            OperationId::SelfRef => "self",
            OperationId::Value => "value",
            OperationId::Not => "not",
            OperationId::ToNumber => "to_number",
            OperationId::Empty => "empty",
            OperationId::RecursiveDescent => "recursive_descent",
            OperationId::Select => "select",
            OperationId::Filter => "filter",
            OperationId::Has => "has",
            OperationId::Unique => "unique",
            OperationId::UniqueBy => "unique_by",
            OperationId::GroupBy => "group_by",
            OperationId::Flatten => "flatten",
            OperationId::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationType {
    pub id: OperationId,
    pub custom_name: Option<Cow<'static, str>>,
    pub to_string_name: Option<&'static str>,
    pub num_args: u32,
    pub precedence: u32,
    pub check_for_post_traverse: bool,
}

impl OperationType {
    pub fn new(id: OperationId, num_args: u32, precedence: u32) -> Self {
        Self {
            id,
            custom_name: None,
            to_string_name: None,
            num_args,
            precedence,
            check_for_post_traverse: false,
        }
    }

    pub fn custom(name: impl Into<Cow<'static, str>>, num_args: u32, precedence: u32) -> Self {
        Self {
            id: OperationId::Custom,
            custom_name: Some(name.into()),
            to_string_name: None,
            num_args,
            precedence,
            check_for_post_traverse: false,
        }
    }

    pub fn name(&self) -> &str {
        if self.id == OperationId::Custom {
            return self.custom_name.as_deref().unwrap_or_default();
        }

        match self.id {
            OperationId::Or => "or",
            OperationId::And => "and",
            OperationId::Union => "union",
            OperationId::Not => "not",
            OperationId::Test => "test",
            _ => self.id.as_str(),
        }
    }

    /// Returns the string representation of this operation type.
    /// For standard operations this is the name (e.g. "pipe", "map");
    /// for custom operations this is the custom_name.
    pub fn to_string(&self) -> String {
        match self.id {
            OperationId::Value | OperationId::StringInterp => "value".to_string(),
            _ => self
                .to_string_name
                .unwrap_or_else(|| self.name())
                .to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub operation_type: OperationType,
    pub value: Option<String>,
    pub string_value: String,
    pub tree_node: Option<Box<TreeNode>>,
    pub preferences: Option<Box<OperationPreferences>>,
    pub update_assign: bool,
    pub token_start: Option<usize>,
    pub token_end: Option<usize>,
}

impl Operation {
    pub fn new(
        id: OperationId,
        string_value: impl Into<String>,
        num_args: u32,
        precedence: u32,
    ) -> Self {
        Self {
            operation_type: OperationType::new(id, num_args, precedence),
            value: None,
            string_value: string_value.into(),
            tree_node: None,
            preferences: None,
            update_assign: false,
            token_start: None,
            token_end: None,
        }
    }

    pub fn custom(
        name: impl Into<Cow<'static, str>>,
        string_value: impl Into<String>,
        num_args: u32,
        precedence: u32,
    ) -> Self {
        Self {
            operation_type: OperationType::custom(name, num_args, precedence),
            value: None,
            string_value: string_value.into(),
            tree_node: None,
            preferences: None,
            update_assign: false,
            token_start: None,
            token_end: None,
        }
    }

    pub fn value(string_value: impl Into<String>) -> Self {
        let string_value = string_value.into();
        let trimmed = string_value.trim();
        let sem_type = if matches!(trimmed, "null" | "~") {
            SemType::Nil
        } else {
            let tag = infer_scalar_tag("", trimmed);
            SemType::from_string(tag).unwrap_or(SemType::Str)
        };

        Self {
            operation_type: OperationType::new(OperationId::Value, 0, u32::MAX),
            value: None,
            string_value: string_value.clone(),
            tree_node: Some(Box::new(TreeNode::scalar(sem_type, string_value))),
            preferences: None,
            update_assign: false,
            token_start: None,
            token_end: None,
        }
    }

    pub fn unary(id: OperationId, string_value: impl Into<String>, precedence: u32) -> Self {
        Self::new(id, string_value, 1, precedence)
    }

    pub fn binary(id: OperationId, string_value: impl Into<String>, precedence: u32) -> Self {
        Self::new(id, string_value, 2, precedence)
    }

    pub fn to_string_name(&self) -> &str {
        match self.operation_type.id {
            OperationId::Value | OperationId::StringInterp => "value",
            _ => self.operation_type.name(),
        }
    }

    pub fn to_string(&self) -> String {
        self.operation_type.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionNode {
    pub operation: Operation,
    pub lhs: Option<Box<ExpressionNode>>,
    pub rhs: Option<Box<ExpressionNode>>,
}

impl ExpressionNode {
    pub fn leaf(operation: Operation) -> Self {
        Self {
            operation,
            lhs: None,
            rhs: None,
        }
    }

    pub fn unary(operation: Operation, rhs: ExpressionNode) -> Self {
        Self {
            operation,
            lhs: None,
            rhs: Some(Box::new(rhs)),
        }
    }

    pub fn binary(operation: Operation, lhs: ExpressionNode, rhs: ExpressionNode) -> Self {
        Self {
            operation,
            lhs: Some(Box::new(lhs)),
            rhs: Some(Box::new(rhs)),
        }
    }
}
