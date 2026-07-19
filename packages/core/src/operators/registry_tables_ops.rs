// Maps operation IDs to their handler functions, gated by build feature flags.
use crate::operators::*;

pub struct OpEntry {
    pub id: OperationId,
    pub handler: OperatorHandler,
}

pub struct OpFlags {
    pub traversal: bool,
    pub math: bool,
    pub relational: bool,
    pub logic: bool,
    pub assign: bool,
    pub collection: bool,
    pub codec: bool,
    pub strings: bool,
    pub sort: bool,
    pub meta: bool,
    pub special: bool,
}

impl Default for OpFlags {
    fn default() -> Self {
        Self {
            traversal: true,
            math: true,
            relational: true,
            logic: true,
            assign: true,
            collection: true,
            codec: true,
            strings: true,
            sort: true,
            meta: true,
            special: true,
        }
    }
}

fn base_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: PIPE_OP_TYPE.id,
            handler: pipe::pipe_operator,
        },
        OpEntry {
            id: SHORT_PIPE_OP_TYPE.id,
            handler: pipe::pipe_operator,
        },
        OpEntry {
            id: SELF_REFERENCE_OP_TYPE.id,
            handler: operator_helpers::identity_operator,
        },
        OpEntry {
            id: EXPRESSION_OP_TYPE.id,
            handler: expression::expression_operator,
        },
        OpEntry {
            id: VALUE_OP_TYPE.id,
            handler: value::value_operator,
        },
    ]
}

fn traversal_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: TRAVERSE_PATH_OP_TYPE.id,
            handler: traverse_path::traverse_path_operator,
        },
        OpEntry {
            id: TRAVERSE_ARRAY_OP_TYPE.id,
            handler: traverse_path::traverse_array_operator,
        },
        OpEntry {
            id: RECURSIVE_DESCENT_OP_TYPE.id,
            handler: recursive_descent::recursive_descent_operator,
        },
        OpEntry {
            id: GET_PATH_OP_TYPE.id,
            handler: path::get_path_operator,
        },
        OpEntry {
            id: SET_PATH_OP_TYPE.id,
            handler: path::set_path_operator,
        },
        OpEntry {
            id: DEL_PATHS_OP_TYPE.id,
            handler: delete::del_paths_operator,
        },
        OpEntry {
            id: DELETE_OP_TYPE.id,
            handler: delete::delete_child_operator,
        },
    ]
}

fn logic_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: OR_OP_TYPE.id,
            handler: booleans::or_operator,
        },
        OpEntry {
            id: AND_OP_TYPE.id,
            handler: booleans::and_operator,
        },
        OpEntry {
            id: NOT_OP_TYPE.id,
            handler: booleans::not_operator,
        },
        OpEntry {
            id: ALTERNATIVE_OP_TYPE.id,
            handler: alternative::alternative_operator,
        },
        OpEntry {
            id: ANY_OP_TYPE.id,
            handler: booleans::any_operator,
        },
        OpEntry {
            id: ALL_OP_TYPE.id,
            handler: booleans::all_operator,
        },
        OpEntry {
            id: ANY_CONDITION_OP_TYPE.id,
            handler: booleans::any_operator,
        },
        OpEntry {
            id: ALL_CONDITION_OP_TYPE.id,
            handler: booleans::all_operator,
        },
    ]
}

fn assign_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: ASSIGN_OP_TYPE.id,
            handler: assign::assign_update_operator,
        },
        OpEntry {
            id: ADD_ASSIGN_OP_TYPE.id,
            handler: add::add_assign_operator,
        },
        OpEntry {
            id: SUBTRACT_ASSIGN_OP_TYPE.id,
            handler: subtract::subtract_assign_operator,
        },
        OpEntry {
            id: MULTIPLY_ASSIGN_OP_TYPE.id,
            handler: multiply::multiply_assign_operator,
        },
        OpEntry {
            id: ASSIGN_VARIABLE_OP_TYPE.id,
            handler: variables::use_with_pipe,
        },
    ]
}

fn math_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: ADD_OP_TYPE.id,
            handler: add::add_operator,
        },
        OpEntry {
            id: SUBTRACT_OP_TYPE.id,
            handler: subtract::subtract_operator,
        },
        OpEntry {
            id: MULTIPLY_OP_TYPE.id,
            handler: multiply::multiply_operator,
        },
        OpEntry {
            id: DIVIDE_OP_TYPE.id,
            handler: divide::divide_operator,
        },
        OpEntry {
            id: MODULO_OP_TYPE.id,
            handler: modulo::modulo_operator,
        },
    ]
}

fn relational_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: EQUALS_OP_TYPE.id,
            handler: relational::equals_operator,
        },
        OpEntry {
            id: NOT_EQUALS_OP_TYPE.id,
            handler: relational::not_equals_operator,
        },
        OpEntry {
            id: RELATIONAL_OP_TYPE.id,
            handler: relational::relational_operator,
        },
        OpEntry {
            id: MIN_OP_TYPE.id,
            handler: relational::min_operator,
        },
        OpEntry {
            id: MAX_OP_TYPE.id,
            handler: relational::max_operator,
        },
    ]
}

fn collection_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: CREATE_MAP_OP_TYPE.id,
            handler: create_map::create_map_operator,
        },
        OpEntry {
            id: COLLECT_OP_TYPE.id,
            handler: collect::collect_operator,
        },
        OpEntry {
            id: COLLECT_OBJECT_OP_TYPE.id,
            handler: collect_object::collect_object_operator,
        },
        OpEntry {
            id: COMPACT_OP_TYPE.id,
            handler: compact::compact_operator,
        },
        OpEntry {
            id: MAP_OP_TYPE.id,
            handler: map::map_operator,
        },
        OpEntry {
            id: MAP_VALUES_OP_TYPE.id,
            handler: map::map_values_operator,
        },
        OpEntry {
            id: PICK_OP_TYPE.id,
            handler: pick::pick_operator,
        },
        OpEntry {
            id: OMIT_OP_TYPE.id,
            handler: omit::omit_operator,
        },
        OpEntry {
            id: UNION_OP_TYPE.id,
            handler: union::union_operator,
        },
        OpEntry {
            id: UNIQUE_OP_TYPE.id,
            handler: unique::unique,
        },
        OpEntry {
            id: UNIQUE_BY_OP_TYPE.id,
            handler: unique::unique_by,
        },
        OpEntry {
            id: GROUP_BY_OP_TYPE.id,
            handler: group_by::group_by,
        },
        OpEntry {
            id: FLATTEN_OP_TYPE.id,
            handler: flatten::flatten_op,
        },
        OpEntry {
            id: LENGTH_OP_TYPE.id,
            handler: length::length_operator,
        },
    ]
}

fn codec_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: ENCODE_OP_TYPE.id,
            handler: encoder_decoder::op_encode,
        },
        OpEntry {
            id: DECODE_OP_TYPE.id,
            handler: encoder_decoder::op_decode,
        },
        OpEntry {
            id: TO_ENTRIES_OP_TYPE.id,
            handler: entries::to_entries_operator,
        },
        OpEntry {
            id: FROM_ENTRIES_OP_TYPE.id,
            handler: entries::from_entries_operator,
        },
        OpEntry {
            id: WITH_ENTRIES_OP_TYPE.id,
            handler: entries::with_entries_operator,
        },
        OpEntry {
            id: TO_NUMBER_OP_TYPE.id,
            handler: to_number::to_number_operator,
        },
    ]
}

fn strings_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: JOIN_STRING_OP_TYPE.id,
            handler: strings::join_string_operator,
        },
        OpEntry {
            id: SUB_STRING_OP_TYPE.id,
            handler: strings::substitute_string_operator,
        },
        OpEntry {
            id: MATCH_OP_TYPE.id,
            handler: strings::match_operator,
        },
        OpEntry {
            id: CAPTURE_OP_TYPE.id,
            handler: strings::capture_operator,
        },
        OpEntry {
            id: TEST_OP_TYPE.id,
            handler: strings::test_operator,
        },
        OpEntry {
            id: SPLIT_STRING_OP_TYPE.id,
            handler: strings::split_string_operator,
        },
        OpEntry {
            id: CHANGE_CASE_OP_TYPE.id,
            handler: strings::change_case_operator,
        },
        OpEntry {
            id: TRIM_OP_TYPE.id,
            handler: strings::trim_space_operator,
        },
        OpEntry {
            id: TO_STRING_OP_TYPE.id,
            handler: strings::to_string_operator,
        },
        OpEntry {
            id: STRING_INTERPOLATION_OP_TYPE.id,
            handler: strings::string_interpolation_operator,
        },
    ]
}

fn sort_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: SORT_BY_OP_TYPE.id,
            handler: sort::sort_by_operator,
        },
        OpEntry {
            id: SORT_OP_TYPE.id,
            handler: sort::sort_operator,
        },
        OpEntry {
            id: SORT_KEYS_OP_TYPE.id,
            handler: sort_keys::sort_keys_operator,
        },
        OpEntry {
            id: REVERSE_OP_TYPE.id,
            handler: reverse::reverse_operator,
        },
        OpEntry {
            id: SHUFFLE_OP_TYPE.id,
            handler: shuffle::shuffle_operator,
        },
    ]
}

fn meta_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: GET_VARIABLE_OP_TYPE.id,
            handler: variables::get_variable_operator,
        },
        OpEntry {
            id: GET_TAG_OP_TYPE.id,
            handler: tag::get_tag_operator,
        },
        OpEntry {
            id: GET_KIND_OP_TYPE.id,
            handler: kind::get_kind_operator,
        },
        OpEntry {
            id: GET_KEY_OP_TYPE.id,
            handler: keys::get_key_operator,
        },
        OpEntry {
            id: IS_KEY_OP_TYPE.id,
            handler: keys::is_key_operator,
        },
        OpEntry {
            id: KEYS_OP_TYPE.id,
            handler: keys::keys_operator,
        },
        OpEntry {
            id: GET_PARENT_OP_TYPE.id,
            handler: parent::get_parent_operator,
        },
        OpEntry {
            id: GET_PARENTS_OP_TYPE.id,
            handler: parent::get_parents_operator,
        },
        OpEntry {
            id: CONTAINS_OP_TYPE.id,
            handler: contains::contains_operator,
        },
        OpEntry {
            id: HAS_OP_TYPE.id,
            handler: has::has_operator,
        },
    ]
}

fn special_entries() -> Vec<OpEntry> {
    vec![
        OpEntry {
            id: REDUCE_OP_TYPE.id,
            handler: reduce::reduce_operator,
        },
        OpEntry {
            id: BLOCK_OP_TYPE.id,
            handler: operator_helpers::empty_operator,
        },
        OpEntry {
            id: EMPTY_OP_TYPE.id,
            handler: operator_helpers::empty_operator,
        },
        OpEntry {
            id: WITH_OP_TYPE.id,
            handler: with::with_operator,
        },
        OpEntry {
            id: FIRST_OP_TYPE.id,
            handler: first::first_operator,
        },
        OpEntry {
            id: SELECT_OP_TYPE.id,
            handler: select::select_operator,
        },
        OpEntry {
            id: FILTER_OP_TYPE.id,
            handler: filter::filter_operator,
        },
    ]
}

/// append_ops: collect all enabled operator entries into the given Vec.
pub fn append_ops(entries: &mut Vec<OpEntry>, flags: &OpFlags) {
    // Base entries are always included
    entries.extend(base_entries());

    if flags.traversal {
        entries.extend(traversal_entries());
    }
    if flags.logic {
        entries.extend(logic_entries());
    }
    if flags.assign {
        entries.extend(assign_entries());
    }
    if flags.math {
        entries.extend(math_entries());
    }
    if flags.relational {
        entries.extend(relational_entries());
    }
    if flags.collection {
        entries.extend(collection_entries());
    }
    if flags.codec {
        entries.extend(codec_entries());
    }
    if flags.strings {
        entries.extend(strings_entries());
    }
    if flags.sort {
        entries.extend(sort_entries());
    }
    if flags.meta {
        entries.extend(meta_entries());
    }
    if flags.special {
        entries.extend(special_entries());
    }
}
