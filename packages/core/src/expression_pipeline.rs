use crate::{
    core::expression::ExpressionNode as CoreExpressionNode,
    core::expression::OperationId as CoreOperationId,
    core::operation_prefs::OperationPreferences as CoreOperationPreferences,
    evaluator::{AllAtOnceEvaluator, EvaluationError, Value},
    operators::{
        self, ADD_ASSIGN_OP_TYPE, ADD_OP_TYPE, ALL_CONDITION_OP_TYPE, ALL_OP_TYPE,
        ALTERNATIVE_OP_TYPE, AND_OP_TYPE, ANY_CONDITION_OP_TYPE, ANY_OP_TYPE, ASSIGN_OP_TYPE,
        ASSIGN_VARIABLE_OP_TYPE, BLOCK_OP_TYPE, CAPTURE_OP_TYPE, CHANGE_CASE_OP_TYPE,
        COLLECT_OBJECT_OP_TYPE, COLLECT_OP_TYPE, CONTAINS_OP_TYPE, CREATE_MAP_OP_TYPE, Context,
        CoreError, DECODE_OP_TYPE, DEL_PATHS_OP_TYPE, DELETE_OP_TYPE, DIVIDE_OP_TYPE,
        EMPTY_OP_TYPE, ENCODE_OP_TYPE, EQUALS_OP_TYPE, ExpressionNode as CompatExpressionNode,
        FILTER_OP_TYPE, FIRST_OP_TYPE, FLATTEN_OP_TYPE, FROM_ENTRIES_OP_TYPE, GET_KEY_OP_TYPE,
        GET_KIND_OP_TYPE, GET_PARENT_OP_TYPE, GET_PARENTS_OP_TYPE, GET_PATH_OP_TYPE,
        GET_TAG_OP_TYPE, GET_VARIABLE_OP_TYPE, GROUP_BY_OP_TYPE, HAS_OP_TYPE, IS_KEY_OP_TYPE,
        JOIN_STRING_OP_TYPE, KEYS_OP_TYPE, LENGTH_OP_TYPE, MAP_OP_TYPE, MAP_VALUES_OP_TYPE,
        MATCH_OP_TYPE, MAX_OP_TYPE, MIN_OP_TYPE, MODULO_OP_TYPE, MULTIPLY_ASSIGN_OP_TYPE,
        MULTIPLY_OP_TYPE, NOT_EQUALS_OP_TYPE, NOT_OP_TYPE, OMIT_OP_TYPE, OR_OP_TYPE,
        Operation as CompatOperation, OperationPreference as CompatOperationPreference,
        PICK_OP_TYPE, PIPE_OP_TYPE, RECURSIVE_DESCENT_OP_TYPE, REDUCE_OP_TYPE, RELATIONAL_OP_TYPE,
        REVERSE_OP_TYPE, SELECT_OP_TYPE, SELF_REFERENCE_OP_TYPE, SET_PATH_OP_TYPE,
        SHORT_PIPE_OP_TYPE, SHUFFLE_OP_TYPE, SORT_BY_OP_TYPE, SORT_KEYS_OP_TYPE, SORT_OP_TYPE,
        SPLIT_STRING_OP_TYPE, STRING_INTERPOLATION_OP_TYPE, SUB_STRING_OP_TYPE,
        SUBTRACT_ASSIGN_OP_TYPE, SUBTRACT_OP_TYPE, TEST_OP_TYPE, TO_ENTRIES_OP_TYPE,
        TO_NUMBER_OP_TYPE, TO_STRING_OP_TYPE, TRAVERSE_ARRAY_OP_TYPE, TRAVERSE_PATH_OP_TYPE,
        TRIM_OP_TYPE, TreeEngine, UNION_OP_TYPE, UNIQUE_BY_OP_TYPE, UNIQUE_OP_TYPE, VALUE_OP_TYPE,
        WITH_ENTRIES_OP_TYPE, WITH_OP_TYPE,
    },
    parser::{ParserError, parse_expression},
};

// Re-export for callers that need the core ExpressionNode type.
pub use crate::core::expression::ExpressionNode;

#[derive(Debug)]
pub enum PipelineError {
    Parse(ParserError),
    Evaluate(EvaluationError),
    Compat(CoreError),
}

impl From<ParserError> for PipelineError {
    fn from(value: ParserError) -> Self {
        PipelineError::Parse(value)
    }
}

impl From<EvaluationError> for PipelineError {
    fn from(value: EvaluationError) -> Self {
        PipelineError::Evaluate(value)
    }
}

impl From<CoreError> for PipelineError {
    fn from(value: CoreError) -> Self {
        PipelineError::Compat(value)
    }
}

pub fn parse(expression: &str) -> Result<Option<Box<ExpressionNode>>, PipelineError> {
    Ok(parse_expression(expression)?)
}

pub fn execute(input: &Value, node: Option<&ExpressionNode>) -> Result<Value, PipelineError> {
    match AllAtOnceEvaluator::new().evaluate(input, node) {
        Ok(value) => Ok(value),
        Err(err) if should_retry_with_tree_evaluator(&err) => {
            execute_with_tree_evaluator(input, node)
        }
        Err(err) => Err(err.into()),
    }
}

pub fn execute_many(
    inputs: &[Value],
    node: Option<&ExpressionNode>,
) -> Result<Vec<Value>, EvaluationError> {
    inputs
        .iter()
        .map(|input| {
            execute(input, node).map_err(|err| match err {
                PipelineError::Evaluate(eval) => eval,
                PipelineError::Parse(parse) => {
                    EvaluationError::UnsupportedOperation(format!("{:?}", parse))
                }
                PipelineError::Compat(core) => {
                    EvaluationError::Core(map_compat_error_to_core(core))
                }
            })
        })
        .collect()
}

pub fn evaluate(input: &Value, expression: &str) -> Result<Value, PipelineError> {
    let node = parse(expression)?;
    execute(input, node.as_deref())
}

pub fn evaluate_many(inputs: &[Value], expression: &str) -> Result<Vec<Value>, PipelineError> {
    let node = parse(expression)?;
    Ok(execute_many(inputs, node.as_deref())?)
}

fn should_retry_with_tree_evaluator(error: &EvaluationError) -> bool {
    matches!(
        error,
        EvaluationError::TypeMismatch { .. } | EvaluationError::UnsupportedOperation(_)
    )
}

fn execute_with_tree_evaluator(
    input: &Value,
    node: Option<&ExpressionNode>,
) -> Result<Value, PipelineError> {
    let Some(core_node) = node else {
        return Ok(input.clone());
    };

    let mut compat_node = convert_to_compat(core_node)?;
    let root = value_to_compat_tree(input)?;
    let mut engine = TreeEngine::default();
    let hydrated_nodes = hydrate_engine_store(&mut engine, &[root])?;
    let ctx = Context::empty().child_context(hydrated_nodes)?;
    let result_ctx = operators::get_matching_nodes(&mut engine, &ctx, Some(&mut compat_node))?;
    context_to_value(&result_ctx, core_node)
}

fn context_to_value(ctx: &Context, expression: &ExpressionNode) -> Result<Value, PipelineError> {
    match ctx.matching_nodes.len() {
        0 => Ok(Value::Array(Vec::new())),
        1 if should_wrap_single_tree_result(expression) => ctx
            .matching_nodes
            .iter()
            .map(compat_tree_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        1 => compat_tree_to_value(&ctx.matching_nodes[0]),
        _ => ctx
            .matching_nodes
            .iter()
            .map(compat_tree_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
    }
}

fn should_wrap_single_tree_result(expression: &ExpressionNode) -> bool {
    matches!(
        expression.operation.operation_type.id,
        CoreOperationId::Pipe
            | CoreOperationId::ShortPipe
            | CoreOperationId::Union
            | CoreOperationId::Collect
            | CoreOperationId::CollectObject
            | CoreOperationId::RecursiveDescent
            | CoreOperationId::Select
            | CoreOperationId::Filter
    )
}

fn compat_tree_to_value(node: &operators::TreeNode) -> Result<Value, PipelineError> {
    match node.kind {
        operators::NodeKind::Sequence => node
            .content
            .iter()
            .map(compat_tree_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        operators::NodeKind::Mapping => {
            let mut out = std::collections::BTreeMap::new();
            for pair in node.content.chunks(2) {
                if pair.len() != 2 {
                    return Err(PipelineError::Compat(CoreError::Parse(
                        operators::ParseError::InvalidSyntax,
                    )));
                }
                out.insert(pair[0].value.clone(), compat_tree_to_value(&pair[1])?);
            }
            Ok(Value::Object(out))
        }
        operators::NodeKind::Scalar => match node.resolved_sem_type() {
            Some(operators::SemType::Nil) => Ok(Value::Null),
            Some(operators::SemType::Boolean) => Ok(Value::Bool(matches!(
                node.value.to_ascii_lowercase().as_str(),
                "y" | "yes" | "on" | "true"
            ))),
            Some(operators::SemType::Int) => node
                .value
                .parse::<i64>()
                .map(|value| Value::Number(value as f64))
                .map_err(|_| {
                    PipelineError::Compat(CoreError::Eval(
                        operators::EvalError::CannotConvertNodeToNumber,
                    ))
                }),
            Some(operators::SemType::Float) => {
                node.value.parse::<f64>().map(Value::Number).map_err(|_| {
                    PipelineError::Compat(CoreError::Eval(
                        operators::EvalError::CannotConvertNodeToNumber,
                    ))
                })
            }
            _ => Ok(Value::String(node.value.clone())),
        },
        operators::NodeKind::Alias | operators::NodeKind::Unknown => Err(PipelineError::Evaluate(
            EvaluationError::UnsupportedOperation("tree node".to_string()),
        )),
    }
}

fn value_to_compat_tree(value: &Value) -> Result<operators::TreeNode, PipelineError> {
    Ok(match value {
        Value::Null => operators::TreeNode::scalar(operators::SemType::Nil, ""),
        Value::Bool(value) => operators::TreeNode::scalar(
            operators::SemType::Boolean,
            if *value { "true" } else { "false" },
        ),
        Value::Number(value) => {
            if value.fract() == 0.0 {
                operators::TreeNode::scalar(operators::SemType::Int, (*value as i64).to_string())
            } else {
                operators::TreeNode::scalar(operators::SemType::Float, value.to_string())
            }
        }
        Value::String(value) => operators::TreeNode::scalar(operators::SemType::Str, value.clone()),
        Value::Array(values) => {
            let mut node = operators::TreeNode {
                kind: operators::NodeKind::Sequence,
                sem_type: Some(operators::SemType::Seq),
                tag: operators::SemType::Seq.to_string().to_string(),
                ..operators::TreeNode::default()
            };
            node.content = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let mut child = value_to_compat_tree(value)?;
                    child.sequence_index = Some(index as i64);
                    Ok(child)
                })
                .collect::<Result<Vec<_>, PipelineError>>()?;
            node
        }
        Value::Object(values) => {
            let mut node = operators::TreeNode {
                kind: operators::NodeKind::Mapping,
                sem_type: Some(operators::SemType::Map),
                tag: operators::SemType::Map.to_string().to_string(),
                ..operators::TreeNode::default()
            };
            let mut content = Vec::with_capacity(values.len() * 2);
            for (key, value) in values {
                let mut key_node =
                    operators::TreeNode::scalar(operators::SemType::Str, key.clone());
                key_node.is_map_key = true;
                let value_node = value_to_compat_tree(value)?;
                content.push(key_node);
                content.push(value_node);
            }
            node.content = content;
            node
        }
    })
}

fn hydrate_engine_store(
    engine: &mut TreeEngine,
    nodes: &[operators::TreeNode],
) -> Result<Vec<operators::TreeNode>, PipelineError> {
    engine.store = operators::TreeStore::new();
    let mut hydrated = Vec::with_capacity(nodes.len());
    for node in nodes {
        let id = append_compat_node(&mut engine.store, node, None)?;
        hydrated.push(engine.store.get(id).clone());
    }
    Ok(hydrated)
}

fn append_compat_node(
    store: &mut operators::TreeStore,
    node: &operators::TreeNode,
    parent: Option<operators::NodeId>,
) -> Result<operators::NodeId, PipelineError> {
    let mut cloned = node.clone();
    cloned.parent = parent;
    cloned.key = None;
    cloned.alias = None;
    cloned.content.clear();

    let id = store.add(cloned);

    let mut child_nodes = Vec::with_capacity(node.content.len());
    let mut child_ids = Vec::with_capacity(node.content.len());
    for child in &node.content {
        let child_id = append_compat_node(store, child, Some(id))?;
        child_ids.push(child_id);
        let mut cloned_child = store.get(child_id).clone();
        cloned_child.parent = Some(id);
        child_nodes.push(cloned_child);
    }

    let kind = {
        let current = store.get_mut(id);
        current.content = child_nodes;
        current.kind
    };

    match kind {
        operators::NodeKind::Sequence => {
            for (index, child_id) in child_ids.iter().copied().enumerate() {
                let child = store.get_mut(child_id);
                child.sequence_index = Some(index as i64);
                child.parent = Some(id);
            }
            let current = store.get_mut(id);
            for (index, child) in current.content.iter_mut().enumerate() {
                child.sequence_index = Some(index as i64);
                child.parent = Some(id);
            }
        }
        operators::NodeKind::Mapping => {
            for pair_index in (0..child_ids.len()).step_by(2) {
                if pair_index + 1 >= child_ids.len() {
                    break;
                }
                let key_id = child_ids[pair_index];
                let value_id = child_ids[pair_index + 1];
                let key = store.get_mut(key_id);
                key.is_map_key = true;
                key.parent = Some(id);
                let value = store.get_mut(value_id);
                value.is_map_key = false;
                value.parent = Some(id);
                value.key = Some(key_id);
            }
            let current = store.get_mut(id);
            for pair_index in (0..current.content.len()).step_by(2) {
                if pair_index + 1 >= current.content.len() {
                    break;
                }
                let key_id = child_ids[pair_index];
                current.content[pair_index].is_map_key = true;
                current.content[pair_index].parent = Some(id);
                current.content[pair_index + 1].is_map_key = false;
                current.content[pair_index + 1].parent = Some(id);
                current.content[pair_index + 1].key = Some(key_id);
            }
        }
        _ => {}
    }

    if !child_ids.is_empty() {
        let finalized_children = child_ids
            .iter()
            .copied()
            .map(|child_id| store.get(child_id).clone())
            .collect::<Vec<_>>();
        store.get_mut(id).content = finalized_children;
    }

    Ok(id)
}

fn map_compat_error_to_core(error: CoreError) -> crate::core::CoreError {
    match error {
        CoreError::System(system) => crate::core::CoreError::System(match system {
            operators::SystemError::EndOfStream => crate::core::SystemError::EndOfStream,
            operators::SystemError::StreamTooLong => crate::core::SystemError::StreamTooLong,
            operators::SystemError::Io(message) => crate::core::SystemError::Io(message),
        }),
        CoreError::Parse(parse) => crate::core::CoreError::Parse(match parse {
            operators::ParseError::InvalidSyntax => crate::core::ParseError::InvalidSyntax,
            operators::ParseError::InvalidYaml => crate::core::ParseError::InvalidYaml,
            operators::ParseError::InvalidJson => crate::core::ParseError::InvalidJson,
            operators::ParseError::InvalidPython => crate::core::ParseError::InvalidPython,
            operators::ParseError::InvalidJavaScript => crate::core::ParseError::InvalidJavaScript,
            operators::ParseError::InvalidToml { .. } => crate::core::ParseError::InvalidSyntax,
            operators::ParseError::TreeSitterFailed => {
                crate::core::ParseError::TreeSitterParseFailed
            }
            operators::ParseError::UnknownToken => crate::core::ParseError::UnknownToken,
            operators::ParseError::UnterminatedString => {
                crate::core::ParseError::UnterminatedString
            }
            operators::ParseError::BadCsv => crate::core::ParseError::BadCsv,
            operators::ParseError::BadParameter => crate::core::ParseError::BadParameter,
            operators::ParseError::InvalidCharacter => crate::core::ParseError::InvalidCharacter,
            operators::ParseError::InvalidPadding => crate::core::ParseError::InvalidPadding,
            operators::ParseError::NegativeIndex => crate::core::ParseError::NegativeIndex,
            operators::ParseError::Utf8CannotEncodeSurrogateHalf => {
                crate::core::ParseError::Utf8CannotEncodeSurrogateHalf
            }
            operators::ParseError::CodepointTooLarge => crate::core::ParseError::CodepointTooLarge,
        }),
        CoreError::Format(format) => crate::core::CoreError::Format(match format {
            operators::FormatError::UnknownFormat => crate::core::FormatError::UnknownFormat,
            operators::FormatError::TomlRequiresMap => crate::core::FormatError::TomlRequiresMap,
            operators::FormatError::TomlEmptyPath => crate::core::FormatError::TomlEmptyPath,
            operators::FormatError::TomlNoAliases => crate::core::FormatError::TomlNoAliases,
            operators::FormatError::TomlUnsupportedKind => {
                crate::core::FormatError::TomlUnsupportedKind
            }
        }),
        CoreError::Eval(_) => crate::core::CoreError::Eval(crate::core::EvalError::Unsupported),
        CoreError::ParseMessage {
            line,
            column,
            message,
        } => crate::core::CoreError::ParseMessage {
            line,
            column,
            message,
        },
        CoreError::OperatorMessage { op, message } => {
            crate::core::CoreError::OperatorMessage { op, message }
        }
        CoreError::WasmProtocol { code } => crate::core::CoreError::WasmProtocol { code },
        CoreError::Io(message) => crate::core::CoreError::Io(message),
        CoreError::OutOfMemory => crate::core::CoreError::OutOfMemory,
    }
}

// ── Tree-level execution (preserves node metadata) ───────────────

/// Map a core [`CoreOperationId`] to the corresponding compat
/// [`&'static operators::OperationType`].
///
/// Returns `None` for variants that have no compat equivalent
/// (e.g. `Custom`, `Exp`).
fn core_op_to_compat_op_type(id: CoreOperationId) -> Option<&'static operators::OperationType> {
    use CoreOperationId::*;
    Some(match id {
        Or => &OR_OP_TYPE,
        And => &AND_OP_TYPE,
        Reduce => &REDUCE_OP_TYPE,
        Block => &BLOCK_OP_TYPE,
        Union => &UNION_OP_TYPE,
        Pipe => &PIPE_OP_TYPE,
        Assign => &ASSIGN_OP_TYPE,
        AddAssign => &ADD_ASSIGN_OP_TYPE,
        SubtractAssign => &SUBTRACT_ASSIGN_OP_TYPE,
        AssignVariable => &ASSIGN_VARIABLE_OP_TYPE,
        Multiply => &MULTIPLY_OP_TYPE,
        MultiplyAssign => &MULTIPLY_ASSIGN_OP_TYPE,
        Divide => &DIVIDE_OP_TYPE,
        Modulo => &MODULO_OP_TYPE,
        Add => &ADD_OP_TYPE,
        Subtract => &SUBTRACT_OP_TYPE,
        Alternative => &ALTERNATIVE_OP_TYPE,
        Equals => &EQUALS_OP_TYPE,
        NotEquals => &NOT_EQUALS_OP_TYPE,
        Relational => &RELATIONAL_OP_TYPE,
        Min => &MIN_OP_TYPE,
        Max => &MAX_OP_TYPE,
        CreateMap => &CREATE_MAP_OP_TYPE,
        ShortPipe => &SHORT_PIPE_OP_TYPE,
        Collect => &COLLECT_OP_TYPE,
        Map => &MAP_OP_TYPE,
        Pick => &PICK_OP_TYPE,
        Omit => &OMIT_OP_TYPE,
        MapValues => &MAP_VALUES_OP_TYPE,
        Encode => &ENCODE_OP_TYPE,
        Decode => &DECODE_OP_TYPE,
        Any => &ANY_OP_TYPE,
        All => &ALL_OP_TYPE,
        Contains => &CONTAINS_OP_TYPE,
        AnyCondition => &ANY_CONDITION_OP_TYPE,
        AllCondition => &ALL_CONDITION_OP_TYPE,
        ToEntries => &TO_ENTRIES_OP_TYPE,
        FromEntries => &FROM_ENTRIES_OP_TYPE,
        WithEntries => &WITH_ENTRIES_OP_TYPE,
        With => &WITH_OP_TYPE,
        GetVariable => &GET_VARIABLE_OP_TYPE,
        GetTag => &GET_TAG_OP_TYPE,
        GetKind => &GET_KIND_OP_TYPE,
        GetKey => &GET_KEY_OP_TYPE,
        IsKey => &IS_KEY_OP_TYPE,
        GetParent => &GET_PARENT_OP_TYPE,
        GetParents => &GET_PARENTS_OP_TYPE,
        GetPath => &GET_PATH_OP_TYPE,
        SetPath => &SET_PATH_OP_TYPE,
        DelPaths => &DEL_PATHS_OP_TYPE,
        SortBy => &SORT_BY_OP_TYPE,
        First => &FIRST_OP_TYPE,
        Reverse => &REVERSE_OP_TYPE,
        Sort => &SORT_OP_TYPE,
        Shuffle => &SHUFFLE_OP_TYPE,
        SortKeys => &SORT_KEYS_OP_TYPE,
        Join => &JOIN_STRING_OP_TYPE,
        Substr => &SUB_STRING_OP_TYPE,
        Match => &MATCH_OP_TYPE,
        Capture => &CAPTURE_OP_TYPE,
        Test => &TEST_OP_TYPE,
        Split => &SPLIT_STRING_OP_TYPE,
        ChangeCase => &CHANGE_CASE_OP_TYPE,
        Trim => &TRIM_OP_TYPE,
        ToString => &TO_STRING_OP_TYPE,
        StringInterp => &STRING_INTERPOLATION_OP_TYPE,
        Keys => &KEYS_OP_TYPE,
        Length => &LENGTH_OP_TYPE,
        CollectObject => &COLLECT_OBJECT_OP_TYPE,
        TraversePath => &TRAVERSE_PATH_OP_TYPE,
        TraverseArray => &TRAVERSE_ARRAY_OP_TYPE,
        SelfRef => &SELF_REFERENCE_OP_TYPE,
        Value => &VALUE_OP_TYPE,
        Not => &NOT_OP_TYPE,
        ToNumber => &TO_NUMBER_OP_TYPE,
        Empty => &EMPTY_OP_TYPE,
        RecursiveDescent => &RECURSIVE_DESCENT_OP_TYPE,
        Select => &SELECT_OP_TYPE,
        Filter => &FILTER_OP_TYPE,
        Has => &HAS_OP_TYPE,
        Unique => &UNIQUE_OP_TYPE,
        UniqueBy => &UNIQUE_BY_OP_TYPE,
        GroupBy => &GROUP_BY_OP_TYPE,
        Flatten => &FLATTEN_OP_TYPE,
        Delete => &DELETE_OP_TYPE,
        // Variants without a compat equivalent.
        Custom | Exp => return None,
    })
}

/// Convert core [`CoreOperationPreferences`] to compat [`CompatOperationPreference`].
///
/// The two enums have structurally identical variants but live in different
/// type hierarchies (core vs compat). This function bridges them so that
/// preferences set by the lexer (encode format/indent, optional traverse,
/// change case direction, flatten depth, etc.) survive the core→compat
/// conversion and reach the operator handlers at runtime.
fn convert_prefs(core: &CoreOperationPreferences) -> CompatOperationPreference {
    use CoreOperationPreferences as C;
    match core {
        C::Traverse(p) => CompatOperationPreference::Traverse(operators::TraversePreferences {
            optional_traverse: p.optional_traverse,
            dont_follow_alias: p.dont_follow_alias,
            include_map_keys: p.include_map_keys,
            dont_include_map_values: p.dont_include_map_values,
            dont_auto_create: p.dont_auto_create,
        }),
        C::Flatten(p) => {
            CompatOperationPreference::Flatten(operators::FlattenPreferences { depth: p.depth })
        }
        C::RecursiveDescent(p) => {
            CompatOperationPreference::RecursiveDescent(operators::RecursiveDescentPreferences {
                traverse_preferences: operators::TraversePreferences {
                    optional_traverse: p.traverse_preferences.optional_traverse,
                    dont_follow_alias: p.traverse_preferences.dont_follow_alias,
                    include_map_keys: p.traverse_preferences.include_map_keys,
                    dont_include_map_values: p.traverse_preferences.dont_include_map_values,
                    dont_auto_create: p.traverse_preferences.dont_auto_create,
                },
                recurse_array: p.recurse_array,
            })
        }
        C::Parent(p) => {
            CompatOperationPreference::Parent(operators::ParentOpPreferences { level: p.level })
        }
        C::Relational(p) => CompatOperationPreference::Relational(operators::RelationalPref {
            or_equal: p.or_equal,
            greater: p.greater,
        }),
        C::ChangeCase(p) => CompatOperationPreference::ChangeCase(operators::ChangeCasePrefs {
            to_upper_case: p.to_upper_case,
        }),
        C::Encoder(p) => CompatOperationPreference::Encoder(operators::EncoderPreferences {
            format: p.format.clone(),
            indent: p.indent,
            unwrap_scalar: false,
        }),
        C::Decoder(p) => CompatOperationPreference::Decoder(operators::DecoderPreferences {
            format: p.format.clone(),
        }),
        C::Assign(p) => CompatOperationPreference::Assign(operators::AssignPreferences {
            clobber_custom_tags: p.clobber_custom_tags,
            dont_overwrite_anchor: p.dont_overwrite_anchor,
            only_write_null: p.only_write_null,
        }),
        C::AssignVar(p) => CompatOperationPreference::AssignVar(operators::AssignVarPreferences {
            is_reference: p.is_reference,
        }),
        C::Expression(p) => {
            CompatOperationPreference::Expression(operators::ExpressionOpPreferences {
                expression: p.expression.clone(),
            })
        }
    }
}

/// Recursively convert a core [`CoreExpressionNode`] (from the parser)
/// into a compat [`CompatExpressionNode`] suitable for tree-level
/// dispatch via [`operators::get_matching_nodes`].
fn convert_to_compat(node: &CoreExpressionNode) -> Result<CompatExpressionNode, PipelineError> {
    let op_type = core_op_to_compat_op_type(node.operation.operation_type.id).ok_or_else(|| {
        PipelineError::Compat(CoreError::Eval(operators::EvalError::UnknownOperator {
            op: node.operation.operation_type.name().to_string(),
        }))
    })?;

    let tree_node = if node.operation.operation_type.id == CoreOperationId::Value {
        Some(Box::new(literal_tree_node(&node.operation.string_value)?))
    } else {
        None
    };

    let preferences = node
        .operation
        .preferences
        .as_deref()
        .map(|p| Box::new(convert_prefs(p)));

    let compat_op = CompatOperation {
        operation_type: op_type,
        value: None,
        string_value: node.operation.string_value.clone(),
        tree_node,
        preferences,
        update_assign: node.operation.operation_type.id == CoreOperationId::Assign
            && node.operation.string_value == "|=",
    };

    let lhs = match &node.lhs {
        Some(child) => Some(Box::new(convert_to_compat(child)?)),
        None => None,
    };
    let rhs = match &node.rhs {
        Some(child) => Some(Box::new(convert_to_compat(child)?)),
        None => None,
    };

    Ok(CompatExpressionNode {
        operation: Box::new(compat_op),
        lhs,
        rhs,
    })
}

fn literal_tree_node(literal: &str) -> Result<operators::TreeNode, PipelineError> {
    let value = Value::from_literal(literal)?;
    Ok(match value {
        Value::Null => operators::TreeNode::scalar(operators::SemType::Nil, ""),
        Value::Bool(value) => operators::TreeNode::scalar(
            operators::SemType::Boolean,
            if value { "true" } else { "false" },
        ),
        Value::Number(_value) => {
            // Use the raw literal string to decide Int vs Float.
            let is_float = literal.contains('.') || literal.contains('e') || literal.contains('E');
            if is_float {
                operators::TreeNode::scalar(operators::SemType::Float, literal)
            } else {
                operators::TreeNode::scalar(operators::SemType::Int, literal)
            }
        }
        Value::String(value) => operators::TreeNode::scalar(operators::SemType::Str, value),
        Value::Array(_) | Value::Object(_) => {
            return Err(PipelineError::Evaluate(
                EvaluationError::UnsupportedOperation("literal tree node".to_string()),
            ));
        }
    })
}

/// Parse an expression string and evaluate it against `ctx` using the
/// tree-level evaluator ([`operators::get_matching_nodes`]).
///
/// This preserves node metadata (anchors, comments, map-key flags, etc.)
/// because the expression is dispatched through the full operator
/// registry rather than being flattened to [`Value`] and back.
///
pub fn execute_on_context(
    engine: &mut TreeEngine,
    ctx: &Context,
    expression: &str,
) -> Result<Context, PipelineError> {
    let parsed = parse(expression)?;
    match parsed {
        None => Ok(ctx.clone()),
        Some(ref core_node) => {
            let mut compat_node = convert_to_compat(core_node)?;
            let eval_ctx = if ctx.matching_nodes.is_empty() {
                Context::empty().single_child_context(&operators::TreeNode::scalar(
                    operators::SemType::Nil,
                    "null",
                ))?
            } else {
                ctx.child_context(hydrate_engine_store(engine, &ctx.matching_nodes)?)?
            };
            Ok(operators::get_matching_nodes(
                engine,
                &eval_ctx,
                Some(&mut compat_node),
            )?)
        }
    }
}
