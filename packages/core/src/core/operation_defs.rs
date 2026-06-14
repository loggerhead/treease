use super::{OperationId, OperationType};

const fn op(
    id: OperationId,
    num_args: u32,
    precedence: u32,
    check_for_post_traverse: bool,
) -> OperationType {
    OperationType {
        id,
        custom_name: None,
        to_string_name: None,
        num_args,
        precedence,
        check_for_post_traverse,
    }
}

pub const OR_OP_TYPE: OperationType = op(OperationId::Or, 2, 20, false);
pub const AND_OP_TYPE: OperationType = op(OperationId::And, 2, 20, false);
pub const REDUCE_OP_TYPE: OperationType = op(OperationId::Reduce, 2, 35, false);
pub const BLOCK_OP_TYPE: OperationType = op(OperationId::Block, 2, 10, false);
pub const UNION_OP_TYPE: OperationType = op(OperationId::Union, 2, 10, false);
pub const PIPE_OP_TYPE: OperationType = op(OperationId::Pipe, 2, 30, false);
pub const ASSIGN_OP_TYPE: OperationType = op(OperationId::Assign, 2, 40, false);
pub const ADD_ASSIGN_OP_TYPE: OperationType = op(OperationId::AddAssign, 2, 40, false);
pub const SUBTRACT_ASSIGN_OP_TYPE: OperationType = op(OperationId::SubtractAssign, 2, 40, false);
pub const ASSIGN_VARIABLE_OP_TYPE: OperationType = op(OperationId::AssignVariable, 2, 40, false);
pub const MULTIPLY_OP_TYPE: OperationType = op(OperationId::Multiply, 2, 42, false);
pub const MULTIPLY_ASSIGN_OP_TYPE: OperationType = op(OperationId::MultiplyAssign, 2, 42, false);
pub const DIVIDE_OP_TYPE: OperationType = op(OperationId::Divide, 2, 42, false);
pub const MODULO_OP_TYPE: OperationType = op(OperationId::Modulo, 2, 42, false);
pub const ADD_OP_TYPE: OperationType = op(OperationId::Add, 2, 42, false);
pub const SUBTRACT_OP_TYPE: OperationType = op(OperationId::Subtract, 2, 42, false);
pub const ALTERNATIVE_OP_TYPE: OperationType = op(OperationId::Alternative, 2, 42, false);
pub const EQUALS_OP_TYPE: OperationType = op(OperationId::Equals, 2, 40, false);
pub const NOT_EQUALS_OP_TYPE: OperationType = op(OperationId::NotEquals, 2, 40, false);
pub const RELATIONAL_OP_TYPE: OperationType = op(OperationId::Relational, 2, 40, false);
pub const MIN_OP_TYPE: OperationType = op(OperationId::Min, 0, 40, false);
pub const MAX_OP_TYPE: OperationType = op(OperationId::Max, 0, 40, false);
pub const CREATE_MAP_OP_TYPE: OperationType = op(OperationId::CreateMap, 2, 15, false);
pub const SHORT_PIPE_OP_TYPE: OperationType = op(OperationId::ShortPipe, 2, 45, false);
pub const EXPRESSION_OP_TYPE: OperationType = op(OperationId::Exp, 0, 50, false);
pub const COLLECT_OP_TYPE: OperationType = op(OperationId::Collect, 1, 50, false);
pub const MAP_OP_TYPE: OperationType = op(OperationId::Map, 1, 52, true);
pub const PICK_OP_TYPE: OperationType = op(OperationId::Pick, 1, 52, true);
pub const OMIT_OP_TYPE: OperationType = op(OperationId::Omit, 1, 52, true);
pub const MAP_VALUES_OP_TYPE: OperationType = op(OperationId::MapValues, 1, 52, true);
pub const ENCODE_OP_TYPE: OperationType = op(OperationId::Encode, 0, 50, false);
pub const DECODE_OP_TYPE: OperationType = op(OperationId::Decode, 0, 50, false);
pub const ANY_OP_TYPE: OperationType = op(OperationId::Any, 0, 50, false);
pub const ALL_OP_TYPE: OperationType = op(OperationId::All, 0, 50, false);
pub const CONTAINS_OP_TYPE: OperationType = op(OperationId::Contains, 1, 50, false);
pub const ANY_CONDITION_OP_TYPE: OperationType = op(OperationId::AnyCondition, 1, 50, false);
pub const ALL_CONDITION_OP_TYPE: OperationType = op(OperationId::AllCondition, 1, 50, false);
pub const TO_ENTRIES_OP_TYPE: OperationType = op(OperationId::ToEntries, 0, 52, true);
pub const FROM_ENTRIES_OP_TYPE: OperationType = op(OperationId::FromEntries, 0, 50, false);
pub const WITH_ENTRIES_OP_TYPE: OperationType = op(OperationId::WithEntries, 1, 50, false);
pub const WITH_OP_TYPE: OperationType = op(OperationId::With, 1, 52, true);
pub const GET_VARIABLE_OP_TYPE: OperationType = op(OperationId::GetVariable, 0, 55, false);
pub const GET_TAG_OP_TYPE: OperationType = op(OperationId::GetTag, 0, 50, false);
pub const GET_KIND_OP_TYPE: OperationType = op(OperationId::GetKind, 0, 50, false);
pub const GET_KEY_OP_TYPE: OperationType = op(OperationId::GetKey, 0, 50, false);
pub const IS_KEY_OP_TYPE: OperationType = op(OperationId::IsKey, 0, 50, false);
pub const GET_PARENT_OP_TYPE: OperationType = op(OperationId::GetParent, 0, 50, false);
pub const GET_PARENTS_OP_TYPE: OperationType = op(OperationId::GetParents, 0, 50, false);
pub const GET_PATH_OP_TYPE: OperationType = op(OperationId::GetPath, 0, 52, true);
pub const SET_PATH_OP_TYPE: OperationType = op(OperationId::SetPath, 1, 50, false);
pub const DEL_PATHS_OP_TYPE: OperationType = op(OperationId::DelPaths, 1, 52, true);
pub const SORT_BY_OP_TYPE: OperationType = op(OperationId::SortBy, 1, 52, true);
pub const FIRST_OP_TYPE: OperationType = op(OperationId::First, 1, 52, true);
pub const REVERSE_OP_TYPE: OperationType = op(OperationId::Reverse, 0, 52, true);
pub const SORT_OP_TYPE: OperationType = op(OperationId::Sort, 0, 52, true);
pub const SHUFFLE_OP_TYPE: OperationType = op(OperationId::Shuffle, 0, 52, true);
pub const SORT_KEYS_OP_TYPE: OperationType = op(OperationId::SortKeys, 1, 52, true);
pub const JOIN_STRING_OP_TYPE: OperationType = op(OperationId::Join, 1, 50, false);
pub const SUB_STRING_OP_TYPE: OperationType = op(OperationId::Substr, 1, 50, false);
pub const MATCH_OP_TYPE: OperationType = op(OperationId::Match, 1, 50, false);
pub const CAPTURE_OP_TYPE: OperationType = op(OperationId::Capture, 1, 50, false);
pub const TEST_OP_TYPE: OperationType = op(OperationId::Test, 1, 50, false);
pub const SPLIT_STRING_OP_TYPE: OperationType = op(OperationId::Split, 1, 52, true);
pub const CHANGE_CASE_OP_TYPE: OperationType = op(OperationId::ChangeCase, 0, 50, false);
pub const TRIM_OP_TYPE: OperationType = op(OperationId::Trim, 0, 50, false);
pub const TO_STRING_OP_TYPE: OperationType = op(OperationId::ToString, 0, 50, false);
pub const STRING_INTERPOLATION_OP_TYPE: OperationType = op(OperationId::StringInterp, 0, 50, false);
pub const KEYS_OP_TYPE: OperationType = op(OperationId::Keys, 0, 52, true);
pub const LENGTH_OP_TYPE: OperationType = op(OperationId::Length, 0, 50, false);
pub const COLLECT_OBJECT_OP_TYPE: OperationType = op(OperationId::CollectObject, 0, 50, false);
pub const TRAVERSE_PATH_OP_TYPE: OperationType = op(OperationId::TraversePath, 0, 55, false);
pub const TRAVERSE_ARRAY_OP_TYPE: OperationType = op(OperationId::TraverseArray, 2, 50, false);
pub const SELF_REFERENCE_OP_TYPE: OperationType = op(OperationId::SelfRef, 0, 55, false);
pub const VALUE_OP_TYPE: OperationType = op(OperationId::Value, 0, 50, false);
pub const NOT_OP_TYPE: OperationType = op(OperationId::Not, 0, 50, false);
pub const TO_NUMBER_OP_TYPE: OperationType = op(OperationId::ToNumber, 0, 50, false);
pub const EMPTY_OP_TYPE: OperationType = op(OperationId::Empty, 0, 50, false);
pub const RECURSIVE_DESCENT_OP_TYPE: OperationType =
    op(OperationId::RecursiveDescent, 0, 50, false);
pub const SELECT_OP_TYPE: OperationType = op(OperationId::Select, 1, 52, true);
pub const FILTER_OP_TYPE: OperationType = op(OperationId::Filter, 1, 52, true);
pub const HAS_OP_TYPE: OperationType = op(OperationId::Has, 1, 50, false);
pub const UNIQUE_OP_TYPE: OperationType = op(OperationId::Unique, 0, 52, true);
pub const UNIQUE_BY_OP_TYPE: OperationType = op(OperationId::UniqueBy, 1, 52, true);
pub const GROUP_BY_OP_TYPE: OperationType = op(OperationId::GroupBy, 1, 52, true);
pub const FLATTEN_OP_TYPE: OperationType = op(OperationId::Flatten, 0, 52, true);
pub const DELETE_OP_TYPE: OperationType = op(OperationId::Delete, 1, 40, false);
