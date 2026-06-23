use std::collections::BTreeMap;

use crate::core::codec_service::CodecService;
use crate::core::context::Context as CoreContext;
use crate::core::encoding::Reader;
use crate::core::errors::CoreError;
use crate::core::expression::{ExpressionNode, OperationId};
use crate::core::format::format_string_from_filename;
use crate::core::printer::{Encoder, Printer};
use crate::core::printer_writer::PrinterWriter;
use crate::core::sem_type::SemType;
use crate::core::tree_navigator::TreeEngine;
use crate::core::tree_node::{NodeId, TreeNode as CoreTreeNode, TreeNodeKind};
use crate::core::tree_store::TreeStore as CoreTreeStore;

use super::{EvaluationError, Value as EvalValue};

/// Input descriptor for reader-based evaluation.
///
pub struct Input<'a> {
    pub name: &'a str,
    pub reader: Reader<'a>,
}

impl<'a> Input<'a> {
    pub fn new(name: &'a str, reader: Reader<'a>) -> Self {
        Self { name, reader }
    }
}

/// One-shot evaluator that loads all inputs at once and evaluates an
/// expression against them.
///
#[derive(Debug, Default)]
pub struct AllAtOnceEvaluator {
    /// Tree navigator used for tree-level dispatch during evaluation.
    pub tree_navigator: TreeEngine,
}

impl AllAtOnceEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluate an expression against a list of tree nodes (by [`NodeId`])
    /// and return the matching results as [`EvalValue`]s.
    ///
    /// Each node is converted to an [`EvalValue`] via [`tree_node_to_value`],
    /// then the expression is evaluated against the flat value list.
    ///
    pub fn evaluate_nodes(
        &self,
        store: &CoreTreeStore,
        expression: &str,
        nodes: &[NodeId],
    ) -> Result<Vec<EvalValue>, EvaluationError> {
        let inputs: Vec<EvalValue> = nodes
            .iter()
            .map(|&id| tree_node_to_value(store, id))
            .collect::<Result<_, _>>()?;
        let parsed = crate::parser::parse_expression(expression)
            .map_err(|e| EvaluationError::UnsupportedOperation(format!("{:?}", e)))?;
        self.evaluate_many(&inputs, parsed.as_deref())
    }

    /// Evaluate an expression against multiple reader inputs, decode each
    /// document, aggregate all root nodes, evaluate, and print the results
    /// through `printer`.
    ///
    /// This is the full pipeline: decoder -> docs -> evaluate -> printResults.
    ///
    pub fn evaluate_readers<E, W>(
        &self,
        ctx: &mut CoreContext,
        expression: &str,
        inputs: &mut [Input<'_>],
        printer: &mut Printer<E, W>,
        codec: &CodecService,
    ) -> Result<(), EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        self.evaluate_readers_with_format(ctx, expression, inputs, printer, codec, None)
    }

    pub fn evaluate_readers_with_format<E, W>(
        &self,
        ctx: &mut CoreContext,
        expression: &str,
        inputs: &mut [Input<'_>],
        printer: &mut Printer<E, W>,
        codec: &CodecService,
        input_format: Option<&str>,
    ) -> Result<(), EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        let mut file_index: i32 = 0;
        let mut all_documents: Vec<NodeId> = Vec::new();
        let mut store = CoreTreeStore::new();

        for input in inputs.iter_mut() {
            let bytes = input
                .reader
                .read_all()
                .map_err(|e| EvaluationError::Core(e))?;
            let source = String::from_utf8(bytes).map_err(|_| {
                EvaluationError::Core(CoreError::System(
                    crate::core::errors::SystemError::InvalidUtf8,
                ))
            })?;
            if source.trim().is_empty() {
                file_index += 1;
                continue;
            }
            let format = input_format.unwrap_or_else(|| format_string_from_filename(input.name));
            let decoded_docs = codec
                .decode_all(format, &source)
                .map_err(|e| EvaluationError::Core(e))?;

            for (doc_index, decoded) in decoded_docs.into_iter().enumerate() {
                // Merge the decoded document's nodes into our local store and
                // collect root NodeIds.
                let root_id = merge_decoded_document(&mut store, &decoded.store, decoded.root);
                if let Some(node) = store.get_mut(root_id) {
                    node.filename = input.name.to_string();
                    node.file_index = file_index;
                    node.document = doc_index as u32;
                    node.evaluate_together = true;
                }
                all_documents.push(root_id);
            }
            file_index += 1;
        }

        if all_documents.is_empty() {
            let null_node = CoreTreeNode::scalar(SemType::Nil, "");
            let null_id = store.add(null_node);
            all_documents.push(null_id);
        }

        // Evaluate expression against all document roots.
        let parsed = crate::parser::parse_expression(expression)
            .map_err(|e| EvaluationError::UnsupportedOperation(format!("{:?}", e)))?;
        let doc_values: Vec<EvalValue> = all_documents
            .iter()
            .map(|&id| tree_node_to_value(&store, id))
            .collect::<Result<_, _>>()?;
        let results = self.evaluate_many(&doc_values, parsed.as_deref())?;

        // Convert results back to tree nodes and print.
        let mut result_ids: Vec<NodeId> = Vec::with_capacity(results.len());
        for (index, value) in results.iter().enumerate() {
            let id = value_to_tree_node(&mut store, value)?;
            if let Some(node) = store.get_mut(id) {
                node.document = index as u32;
                node.file_index = 0;
                node.evaluate_together = true;
            }
            result_ids.push(id);
        }

        printer
            .print_results(ctx, &store, &result_ids)
            .map_err(|e| EvaluationError::Core(e))?;

        Ok(())
    }

    pub fn evaluate_many(
        &self,
        inputs: &[EvalValue],
        expression: Option<&ExpressionNode>,
    ) -> Result<Vec<EvalValue>, EvaluationError> {
        let flatten_top_level_array = should_flatten_top_level_array_result(expression);
        let mut results = Vec::new();

        for input in inputs {
            let result = self.evaluate(input, expression)?;
            if flatten_top_level_array {
                match result {
                    EvalValue::Array(values) => results.extend(values),
                    value => results.push(value),
                }
            } else {
                results.push(result);
            }
        }

        Ok(results)
    }

    pub fn evaluate(
        &self,
        input: &EvalValue,
        expression: Option<&ExpressionNode>,
    ) -> Result<EvalValue, EvaluationError> {
        match expression {
            Some(node) => self.evaluate_node(input, node),
            None => Ok(input.clone()),
        }
    }

    fn evaluate_node(
        &self,
        input: &EvalValue,
        node: &ExpressionNode,
    ) -> Result<EvalValue, EvaluationError> {
        use OperationId::*;

        match node.operation.operation_type.id {
            Value => EvalValue::from_literal(&node.operation.string_value),
            SelfRef => Ok(input.clone()),
            Empty => Ok(EvalValue::Array(Vec::new())),
            Not => {
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let value = self.evaluate_node(input, rhs)?;
                Ok(EvalValue::Bool(!value.truthy()))
            }
            Pipe | ShortPipe => {
                let lhs = node
                    .lhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("lhs"))?;
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let piped = self.evaluate_node(input, lhs)?;
                match piped {
                    EvalValue::Array(values) => {
                        if pipe_rhs_consumes_array(rhs) {
                            return self.evaluate_node(&EvalValue::Array(values), rhs);
                        }
                        let mut out = Vec::new();
                        for value in values {
                            let result = self.evaluate_node(&value, rhs)?;
                            if matches!(rhs.operation.operation_type.id, Select | Filter)
                                && result == EvalValue::Null
                            {
                                continue;
                            }
                            out.push(result);
                        }
                        Ok(EvalValue::Array(out))
                    }
                    value => {
                        if matches!(lhs.operation.operation_type.id, Select | Filter)
                            && value == EvalValue::Null
                        {
                            return Ok(EvalValue::Array(Vec::new()));
                        }
                        self.evaluate_node(&value, rhs)
                    }
                }
            }
            Block => {
                let lhs = node
                    .lhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("lhs"))?;
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let _ = self.evaluate_node(input, lhs)?;
                self.evaluate_node(input, rhs)
            }
            Assign => {
                let lhs = node
                    .lhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("lhs"))?;
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                if node.operation.update_assign {
                    let current = self.evaluate_node(input, lhs)?;
                    let assigned = match current {
                        EvalValue::Array(values) => EvalValue::Array(
                            values
                                .iter()
                                .map(|value| self.evaluate_node(value, rhs))
                                .collect::<Result<Vec<_>, _>>()?,
                        ),
                        value => self.evaluate_node(&value, rhs)?,
                    };

                    if is_top_level_array_update_assign(lhs) {
                        return Ok(assigned);
                    }

                    return self.evaluate_assign(input, lhs, assigned);
                }
                let assigned = self.evaluate_node(input, rhs)?;
                self.evaluate_assign(input, lhs, assigned)
            }
            CreateMap => {
                let lhs = node
                    .lhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("lhs"))?;
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                self.evaluate_create_map(input, lhs, rhs)
            }
            Collect => {
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                Ok(EvalValue::Array(self.collect_array_items(input, rhs)?))
            }
            CollectObject => {
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                Ok(EvalValue::Object(self.collect_object_entries(input, rhs)?))
            }
            Union => Ok(EvalValue::Array(self.collect_union_items(input, node)?)),
            TraversePath | TraverseArray => {
                let lhs = match node.lhs.as_deref() {
                    Some(lhs_node) => self.evaluate_node(input, lhs_node)?,
                    None => input.clone(),
                };
                let rhs = match node.rhs.as_deref() {
                    Some(rhs_node) => self.evaluate_node(input, rhs_node)?,
                    None if node.operation.operation_type.id == TraversePath => {
                        EvalValue::String(node.operation.string_value.clone())
                    }
                    None => EvalValue::Array(Vec::new()),
                };
                self.evaluate_traversal(node.operation.operation_type.id, lhs, rhs)
            }
            Map => {
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                match input {
                    EvalValue::Array(values) => values
                        .iter()
                        .map(|value| self.evaluate_node(value, rhs))
                        .collect::<Result<Vec<_>, _>>()
                        .map(EvalValue::Array),
                    other => Err(EvaluationError::TypeMismatch {
                        expected: "array",
                        actual: render_type(other).to_string(),
                    }),
                }
            }
            Select | Filter => {
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                match input {
                    EvalValue::Array(values) => values
                        .iter()
                        .filter_map(|value| match self.evaluate_node(value, rhs) {
                            Ok(result) if result.truthy() => Some(Ok(value.clone())),
                            Ok(_) => None,
                            Err(err) => Some(Err(err)),
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map(EvalValue::Array),
                    value => {
                        let result = self.evaluate_node(value, rhs)?;
                        Ok(if result.truthy() {
                            value.clone()
                        } else {
                            EvalValue::Null
                        })
                    }
                }
            }
            SortBy => {
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                match input {
                    EvalValue::Array(values) => {
                        let mut keyed = values
                            .iter()
                            .map(|value| Ok((self.evaluate_node(value, rhs)?, value.clone())))
                            .collect::<Result<Vec<_>, EvaluationError>>()?;
                        keyed.sort_by(|(left_key, _), (right_key, _)| {
                            value_sort_key(left_key).cmp(&value_sort_key(right_key))
                        });
                        Ok(EvalValue::Array(
                            keyed.into_iter().map(|(_, value)| value).collect(),
                        ))
                    }
                    other => Err(EvaluationError::TypeMismatch {
                        expected: "array",
                        actual: render_type(other).to_string(),
                    }),
                }
            }
            Length | Keys | ToNumber | ToString | Trim | ChangeCase | Split | Join | First
            | Reverse | Flatten | Min | Max | Any | All | Sort | Unique => self
                .evaluate_input_unary(
                    node.operation.operation_type.id,
                    &node.operation.string_value,
                    input,
                ),
            Contains => {
                let lhs = match node.lhs.as_deref() {
                    Some(lhs) => self.evaluate_node(input, lhs)?,
                    None => input.clone(),
                };
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let rhs = self.evaluate_node(input, rhs)?;
                Ok(EvalValue::Bool(self.value_contains(&lhs, &rhs)?))
            }
            Has => {
                let lhs = match node.lhs.as_deref() {
                    Some(lhs) => self.evaluate_node(input, lhs)?,
                    None => input.clone(),
                };
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let rhs = self.evaluate_node(input, rhs)?;
                Ok(EvalValue::Bool(self.value_has(&lhs, &rhs)?))
            }
            Add | Subtract | Multiply | Divide | Modulo | And | Or | Equals | NotEquals
            | Alternative | Relational => {
                let lhs_node = node
                    .lhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("lhs"))?;
                let rhs_node = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let lhs = self.evaluate_node(input, lhs_node)?;
                let rhs = self.evaluate_node(input, rhs_node)?;
                self.evaluate_binary(
                    &node.operation.string_value,
                    node.operation.operation_type.id,
                    lhs,
                    rhs,
                )
            }
            other => Err(EvaluationError::UnsupportedOperation(
                other.as_str().to_string(),
            )),
        }
    }

    fn collect_array_items(
        &self,
        input: &EvalValue,
        node: &ExpressionNode,
    ) -> Result<Vec<EvalValue>, EvaluationError> {
        self.collect_union_items(input, node)
    }

    fn collect_union_items(
        &self,
        input: &EvalValue,
        node: &ExpressionNode,
    ) -> Result<Vec<EvalValue>, EvaluationError> {
        match node.operation.operation_type.id {
            OperationId::Union => {
                let lhs = node
                    .lhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("lhs"))?;
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let mut items = self.collect_union_items(input, lhs)?;
                items.extend(self.collect_union_items(input, rhs)?);
                Ok(items)
            }
            OperationId::Empty => Ok(Vec::new()),
            _ => Ok(vec![self.evaluate_node(input, node)?]),
        }
    }

    fn collect_object_entries(
        &self,
        input: &EvalValue,
        node: &ExpressionNode,
    ) -> Result<std::collections::BTreeMap<String, EvalValue>, EvaluationError> {
        let mut out = std::collections::BTreeMap::new();
        self.collect_object_entries_into(input, node, &mut out)?;
        Ok(out)
    }

    fn collect_object_entries_into(
        &self,
        input: &EvalValue,
        node: &ExpressionNode,
        out: &mut std::collections::BTreeMap<String, EvalValue>,
    ) -> Result<(), EvaluationError> {
        match node.operation.operation_type.id {
            OperationId::Union => {
                let lhs = node
                    .lhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("lhs"))?;
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                self.collect_object_entries_into(input, lhs, out)?;
                self.collect_object_entries_into(input, rhs, out)?;
                Ok(())
            }
            OperationId::Empty => Ok(()),
            OperationId::CreateMap => {
                let lhs = node
                    .lhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("lhs"))?;
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let key = self.evaluate_node(input, lhs)?;
                let value = self.evaluate_node(input, rhs)?;
                out.insert(value_to_key(&key), value);
                Ok(())
            }
            _ => match self.evaluate_node(input, node)? {
                EvalValue::Object(values) => {
                    out.extend(values);
                    Ok(())
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "object",
                    actual: render_type(&other).to_string(),
                }),
            },
        }
    }

    fn evaluate_create_map(
        &self,
        input: &EvalValue,
        lhs: &ExpressionNode,
        rhs: &ExpressionNode,
    ) -> Result<EvalValue, EvaluationError> {
        let key = self.evaluate_node(input, lhs)?;
        let value = self.evaluate_node(input, rhs)?;
        Ok(EvalValue::Object(std::collections::BTreeMap::from([(
            value_to_key(&key),
            value,
        )])))
    }

    fn evaluate_assign(
        &self,
        input: &EvalValue,
        lhs: &ExpressionNode,
        assigned: EvalValue,
    ) -> Result<EvalValue, EvaluationError> {
        let mut target = input.clone();
        let path = collect_assign_path(lhs)?;
        assign_path_value(&mut target, &path, assigned)?;
        Ok(target)
    }

    fn evaluate_traversal(
        &self,
        id: OperationId,
        lhs: EvalValue,
        rhs: EvalValue,
    ) -> Result<EvalValue, EvaluationError> {
        match id {
            OperationId::TraversePath => {
                let key = match rhs {
                    EvalValue::String(value) => value,
                    EvalValue::Number(value) if value.fract() == 0.0 => format!("{value:.0}"),
                    other => {
                        return Err(EvaluationError::TypeMismatch {
                            expected: "string",
                            actual: render_type(&other).to_string(),
                        });
                    }
                };
                match lhs {
                    EvalValue::Object(values) => {
                        Ok(values.get(&key).cloned().unwrap_or(EvalValue::Null))
                    }
                    other => Err(EvaluationError::TypeMismatch {
                        expected: "object",
                        actual: render_type(&other).to_string(),
                    }),
                }
            }
            OperationId::TraverseArray => {
                if matches!(rhs, EvalValue::Array(ref values) if values.is_empty()) {
                    return match lhs {
                        EvalValue::Array(values) => Ok(EvalValue::Array(values)),
                        other => Err(EvaluationError::TypeMismatch {
                            expected: "array",
                            actual: render_type(&other).to_string(),
                        }),
                    };
                }
                if let EvalValue::Array(indices) = rhs {
                    return match lhs {
                        EvalValue::Array(values) => select_array_indices(&values, &indices),
                        other => Err(EvaluationError::TypeMismatch {
                            expected: "array",
                            actual: render_type(&other).to_string(),
                        }),
                    };
                }
                let index = rhs.as_number()?;
                if index.fract() != 0.0 || index < 0.0 {
                    return Err(EvaluationError::TypeMismatch {
                        expected: "non-negative integer",
                        actual: index.to_string(),
                    });
                }
                match lhs {
                    EvalValue::Array(values) => Ok(values
                        .get(index as usize)
                        .cloned()
                        .unwrap_or(EvalValue::Null)),
                    other => Err(EvaluationError::TypeMismatch {
                        expected: "array",
                        actual: render_type(&other).to_string(),
                    }),
                }
            }
            _ => Err(EvaluationError::UnsupportedOperation(
                id.as_str().to_string(),
            )),
        }
    }

    fn evaluate_input_unary(
        &self,
        id: OperationId,
        operation_text: &str,
        input: &EvalValue,
    ) -> Result<EvalValue, EvaluationError> {
        match id {
            OperationId::Length => Ok(EvalValue::Number(match input {
                EvalValue::Null => 0.0,
                EvalValue::Bool(_) => 1.0,
                EvalValue::Number(value) => render_number(*value).len() as f64,
                EvalValue::String(value) => value.chars().count() as f64,
                EvalValue::Array(values) => values.len() as f64,
                EvalValue::Object(values) => values.len() as f64,
            })),
            OperationId::Keys => match input {
                EvalValue::Array(values) => Ok(EvalValue::Array(
                    (0..values.len())
                        .map(|idx| EvalValue::Number(idx as f64))
                        .collect(),
                )),
                EvalValue::Object(values) => Ok(EvalValue::Array(
                    values.keys().cloned().map(EvalValue::String).collect(),
                )),
                other => Err(EvaluationError::TypeMismatch {
                    expected: "array or object",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::ToNumber => Ok(EvalValue::Number(input.as_number()?)),
            OperationId::ToString => Ok(EvalValue::String(render_value(input))),
            OperationId::Trim => match input {
                EvalValue::String(value) => Ok(EvalValue::String(value.trim().to_string())),
                other => Err(EvaluationError::TypeMismatch {
                    expected: "string",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::ChangeCase => match input {
                EvalValue::String(value) => {
                    let changed = if is_upper_case_operation(operation_text) {
                        value.to_uppercase()
                    } else {
                        value.to_lowercase()
                    };
                    Ok(EvalValue::String(changed))
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "string",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::Split => match input {
                EvalValue::String(value) => {
                    let separator =
                        extract_call_string_argument(operation_text).unwrap_or_default();
                    if separator.is_empty() {
                        Ok(EvalValue::Array(
                            value
                                .chars()
                                .map(|ch| EvalValue::String(ch.to_string()))
                                .collect(),
                        ))
                    } else {
                        Ok(EvalValue::Array(
                            value
                                .split(&separator)
                                .map(|segment| EvalValue::String(segment.to_string()))
                                .collect(),
                        ))
                    }
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "string",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::Join => match input {
                EvalValue::Array(values) => {
                    let separator =
                        extract_call_string_argument(operation_text).unwrap_or_default();
                    let mut rendered = Vec::with_capacity(values.len());
                    for value in values {
                        match value {
                            EvalValue::String(value) => rendered.push(value.clone()),
                            other => rendered.push(render_value(other)),
                        }
                    }
                    Ok(EvalValue::String(rendered.join(&separator)))
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "array",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::First => match input {
                EvalValue::Array(values) => Ok(values.first().cloned().unwrap_or(EvalValue::Null)),
                other => Err(EvaluationError::TypeMismatch {
                    expected: "array",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::Reverse => match input {
                EvalValue::Array(values) => {
                    let mut out = values.clone();
                    out.reverse();
                    Ok(EvalValue::Array(out))
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "array",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::Sort => match input {
                EvalValue::Array(values) => {
                    let mut out = values.clone();
                    out.sort_by_key(value_sort_key);
                    Ok(EvalValue::Array(out))
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "array",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::Unique => match input {
                EvalValue::Array(values) => {
                    let mut out = Vec::new();
                    for value in values {
                        if !out.contains(value) {
                            out.push(value.clone());
                        }
                    }
                    Ok(EvalValue::Array(out))
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "array",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::Flatten => match input {
                EvalValue::Array(values) => {
                    let mut out = Vec::new();
                    flatten_values(
                        values,
                        &mut out,
                        extract_call_i32_argument(operation_text)
                            .and_then(|depth| usize::try_from(depth).ok()),
                    );
                    Ok(EvalValue::Array(out))
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "array",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::Min | OperationId::Max => match input {
                EvalValue::Array(values) => {
                    let mut iter = values.iter().map(EvalValue::as_number);
                    let Some(first) = iter.next() else {
                        return Ok(EvalValue::Null);
                    };
                    let mut selected = first?;
                    for value in iter {
                        let value = value?;
                        if (id == OperationId::Min && value < selected)
                            || (id == OperationId::Max && value > selected)
                        {
                            selected = value;
                        }
                    }
                    Ok(EvalValue::Number(selected))
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "array",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::Any => match input {
                EvalValue::Array(values) => {
                    Ok(EvalValue::Bool(values.iter().any(EvalValue::truthy)))
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "array",
                    actual: render_type(other).to_string(),
                }),
            },
            OperationId::All => match input {
                EvalValue::Array(values) => {
                    Ok(EvalValue::Bool(values.iter().all(EvalValue::truthy)))
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "array",
                    actual: render_type(other).to_string(),
                }),
            },
            _ => Err(EvaluationError::UnsupportedOperation(
                id.as_str().to_string(),
            )),
        }
    }

    fn value_contains(&self, lhs: &EvalValue, rhs: &EvalValue) -> Result<bool, EvaluationError> {
        Ok(match lhs {
            EvalValue::Object(values) => match rhs {
                EvalValue::Object(required) => {
                    let mut contains_all = true;
                    for (key, rhs_value) in required {
                        let Some(lhs_value) = values.get(key) else {
                            contains_all = false;
                            break;
                        };
                        if !self.value_contains(lhs_value, rhs_value)? {
                            contains_all = false;
                            break;
                        }
                    }
                    contains_all
                }
                _ => false,
            },
            EvalValue::Array(values) => match rhs {
                EvalValue::Array(required) => {
                    let mut contains_all = true;
                    for item in required {
                        let mut found = false;
                        for candidate in values {
                            if self.value_contains(candidate, item)? {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            contains_all = false;
                            break;
                        }
                    }
                    contains_all
                }
                item => {
                    let mut found = false;
                    for candidate in values {
                        if self.value_contains(candidate, item)? {
                            found = true;
                            break;
                        }
                    }
                    found
                }
            },
            EvalValue::String(value) => match rhs {
                EvalValue::String(needle) => value.contains(needle),
                _ => false,
            },
            _ => lhs == rhs,
        })
    }

    fn value_has(&self, lhs: &EvalValue, rhs: &EvalValue) -> Result<bool, EvaluationError> {
        match lhs {
            EvalValue::Object(values) => Ok(values.contains_key(&value_to_key(rhs))),
            EvalValue::Array(values) => match rhs {
                EvalValue::Number(index) if index.fract() == 0.0 && *index >= 0.0 => {
                    Ok((*index as usize) < values.len())
                }
                other => Err(EvaluationError::TypeMismatch {
                    expected: "non-negative integer",
                    actual: render_type(other).to_string(),
                }),
            },
            other => Err(EvaluationError::TypeMismatch {
                expected: "array or object",
                actual: render_type(other).to_string(),
            }),
        }
    }

    fn evaluate_binary(
        &self,
        lexeme: &str,
        id: OperationId,
        lhs: EvalValue,
        rhs: EvalValue,
    ) -> Result<EvalValue, EvaluationError> {
        use OperationId::*;

        match id {
            Add => match (lhs, rhs) {
                (EvalValue::Number(lhs), EvalValue::Number(rhs)) => {
                    Ok(EvalValue::Number(lhs + rhs))
                }
                (EvalValue::String(lhs), EvalValue::String(rhs)) => {
                    Ok(EvalValue::String(lhs + &rhs))
                }
                (lhs, rhs) => Ok(EvalValue::String(format!(
                    "{}{}",
                    render_value(&lhs),
                    render_value(&rhs)
                ))),
            },
            Subtract => Ok(EvalValue::Number(lhs.as_number()? - rhs.as_number()?)),
            Multiply => Ok(EvalValue::Number(lhs.as_number()? * rhs.as_number()?)),
            Divide => {
                let rhs = rhs.as_number()?;
                if rhs == 0.0 {
                    return Err(EvaluationError::DivisionByZero);
                }
                Ok(EvalValue::Number(lhs.as_number()? / rhs))
            }
            Modulo => {
                let rhs = rhs.as_number()?;
                if rhs == 0.0 {
                    return Err(EvaluationError::DivisionByZero);
                }
                Ok(EvalValue::Number(lhs.as_number()? % rhs))
            }
            And => Ok(EvalValue::Bool(lhs.truthy() && rhs.truthy())),
            Or => Ok(EvalValue::Bool(lhs.truthy() || rhs.truthy())),
            Equals => Ok(EvalValue::Bool(lhs == rhs)),
            NotEquals => Ok(EvalValue::Bool(lhs != rhs)),
            Alternative => Ok(if lhs.truthy() { lhs } else { rhs }),
            Relational => {
                let lhs = lhs.as_number()?;
                let rhs = rhs.as_number()?;
                let result = match lexeme {
                    "<" => lhs < rhs,
                    "<=" => lhs <= rhs,
                    ">" => lhs > rhs,
                    ">=" => lhs >= rhs,
                    _ => {
                        return Err(EvaluationError::UnsupportedOperation(lexeme.to_string()));
                    }
                };
                Ok(EvalValue::Bool(result))
            }
            _ => Err(EvaluationError::UnsupportedOperation(
                id.as_str().to_string(),
            )),
        }
    }
}

fn flatten_values(values: &[EvalValue], out: &mut Vec<EvalValue>, remaining_depth: Option<usize>) {
    for value in values {
        match value {
            EvalValue::Array(items) if remaining_depth != Some(0) => flatten_values(
                items,
                out,
                remaining_depth.map(|depth| depth.saturating_sub(1)),
            ),
            other => out.push(other.clone()),
        }
    }
}

fn select_array_indices(
    values: &[EvalValue],
    indices: &[EvalValue],
) -> Result<EvalValue, EvaluationError> {
    let mut selected = Vec::with_capacity(indices.len());
    for index_value in indices {
        let index = index_value.as_number()?;
        if index.fract() != 0.0 || index < 0.0 {
            return Err(EvaluationError::TypeMismatch {
                expected: "non-negative integer",
                actual: index.to_string(),
            });
        }
        selected.push(
            values
                .get(index as usize)
                .cloned()
                .unwrap_or(EvalValue::Null),
        );
    }
    Ok(if selected.len() == 1 {
        selected.into_iter().next().unwrap_or(EvalValue::Null)
    } else {
        EvalValue::Array(selected)
    })
}

fn pipe_rhs_consumes_array(node: &ExpressionNode) -> bool {
    matches!(
        node.operation.operation_type.id,
        OperationId::Flatten | OperationId::Sort | OperationId::Unique | OperationId::SortBy
    ) || matches!(
        node.operation.operation_type.id,
        OperationId::Pipe | OperationId::ShortPipe
    ) && node.lhs.as_deref().is_some_and(pipe_rhs_consumes_array)
}

fn extract_call_argument(operation_text: &str) -> Option<&str> {
    let start = operation_text.find('(')? + 1;
    let end = operation_text.rfind(')')?;
    (end >= start).then_some(operation_text[start..end].trim())
}

fn extract_call_i32_argument(operation_text: &str) -> Option<i32> {
    extract_call_argument(operation_text)?.parse().ok()
}

fn extract_call_string_argument(operation_text: &str) -> Option<String> {
    let argument = extract_call_argument(operation_text)?;
    if argument.len() >= 2
        && ((argument.starts_with('"') && argument.ends_with('"'))
            || (argument.starts_with('\'') && argument.ends_with('\'')))
    {
        return Some(argument[1..argument.len() - 1].to_string());
    }
    Some(argument.to_string())
}

fn is_upper_case_operation(operation_text: &str) -> bool {
    let name = operation_text
        .split_once('(')
        .map(|(name, _)| name)
        .unwrap_or(operation_text)
        .trim();
    matches!(name, "upcase" | "ascii_upcase" | "asciiupcase")
}

fn render_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn value_to_key(value: &EvalValue) -> String {
    match value {
        EvalValue::String(value) => value.clone(),
        EvalValue::Number(value) => render_number(*value),
        EvalValue::Bool(value) => value.to_string(),
        EvalValue::Null => "null".to_string(),
        EvalValue::Array(_) | EvalValue::Object(_) => render_value(value),
    }
}

fn value_sort_key(value: &EvalValue) -> String {
    match value {
        EvalValue::Null => "0:null".to_string(),
        EvalValue::Bool(value) => format!("1:{value}"),
        EvalValue::Number(value) => format!("2:{value:020}"),
        EvalValue::String(value) => format!("3:{value}"),
        EvalValue::Array(_) | EvalValue::Object(_) => format!("4:{}", render_value(value)),
    }
}

fn render_value(value: &EvalValue) -> String {
    match value {
        EvalValue::Null => "null".to_string(),
        EvalValue::Bool(value) => value.to_string(),
        EvalValue::Number(value) => render_number(*value),
        EvalValue::String(value) => value.clone(),
        EvalValue::Array(values) => format!("{:?}", values),
        EvalValue::Object(values) => format!("{:?}", values),
    }
}

fn render_type(value: &EvalValue) -> &'static str {
    match value {
        EvalValue::Null => "null",
        EvalValue::Bool(_) => "bool",
        EvalValue::Number(_) => "number",
        EvalValue::String(_) => "string",
        EvalValue::Array(_) => "array",
        EvalValue::Object(_) => "object",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AssignPathSegment {
    Key(String),
    Index(usize),
}

fn collect_assign_path(node: &ExpressionNode) -> Result<Vec<AssignPathSegment>, EvaluationError> {
    match node.operation.operation_type.id {
        OperationId::SelfRef => Ok(Vec::new()),
        OperationId::Collect => {
            let rhs = node
                .rhs
                .as_deref()
                .ok_or(EvaluationError::MissingOperand("rhs"))?;
            collect_assign_path(rhs)
        }
        OperationId::ShortPipe => {
            let lhs = node
                .lhs
                .as_deref()
                .ok_or(EvaluationError::MissingOperand("lhs"))?;
            let rhs = node
                .rhs
                .as_deref()
                .ok_or(EvaluationError::MissingOperand("rhs"))?;
            let mut path = collect_assign_path(lhs)?;
            path.extend(collect_assign_path(rhs)?);
            Ok(path)
        }
        OperationId::TraversePath => {
            let mut path = match node.lhs.as_deref() {
                Some(lhs) => collect_assign_path(lhs)?,
                None => Vec::new(),
            };
            let key = match node.rhs.as_deref() {
                Some(rhs) => extract_path_key(rhs)?,
                None => node.operation.string_value.clone(),
            };
            path.push(AssignPathSegment::Key(key));
            Ok(path)
        }
        OperationId::TraverseArray => {
            let lhs = node
                .lhs
                .as_deref()
                .ok_or(EvaluationError::MissingOperand("lhs"))?;
            let rhs = node
                .rhs
                .as_deref()
                .ok_or(EvaluationError::MissingOperand("rhs"))?;
            let mut path = collect_assign_path(lhs)?;
            if is_collect_empty(rhs) {
                return Ok(path);
            }
            path.push(AssignPathSegment::Index(extract_path_index(rhs)?));
            Ok(path)
        }
        other => Err(EvaluationError::UnsupportedOperation(
            other.as_str().to_string(),
        )),
    }
}

fn is_top_level_array_update_assign(node: &ExpressionNode) -> bool {
    match node.operation.operation_type.id {
        OperationId::Collect => matches!(
            node.rhs.as_deref(),
            Some(rhs) if is_top_level_array_update_assign(rhs)
        ),
        OperationId::TraverseArray => {
            matches!(
                node.lhs
                    .as_deref()
                    .map(|lhs| lhs.operation.operation_type.id),
                Some(OperationId::SelfRef)
            ) && matches!(
                node.rhs.as_deref(),
                Some(rhs) if is_collect_empty(rhs) || rhs.operation.operation_type.id == OperationId::Empty
            )
        }
        _ => false,
    }
}

fn should_flatten_top_level_array_result(expression: Option<&ExpressionNode>) -> bool {
    matches!(
        expression,
        Some(node)
            if node.operation.operation_type.id == OperationId::Assign
                && node.operation.update_assign
                && matches!(node.lhs.as_deref(), Some(lhs) if is_top_level_array_update_assign(lhs))
    )
}

fn is_collect_empty(node: &ExpressionNode) -> bool {
    node.operation.operation_type.id == OperationId::Collect
        && matches!(
            node.rhs
                .as_deref()
                .map(|rhs| rhs.operation.operation_type.id),
            Some(OperationId::Empty)
        )
}

fn extract_path_key(node: &ExpressionNode) -> Result<String, EvaluationError> {
    if node.operation.operation_type.id != OperationId::Value {
        return Err(EvaluationError::UnsupportedOperation(
            node.operation.operation_type.id.as_str().to_string(),
        ));
    }
    match EvalValue::from_literal(&node.operation.string_value)? {
        EvalValue::String(value) => Ok(value),
        EvalValue::Number(value) if value.fract() == 0.0 => Ok(format!("{value:.0}")),
        other => Err(EvaluationError::TypeMismatch {
            expected: "string",
            actual: render_type(&other).to_string(),
        }),
    }
}

fn extract_path_index(node: &ExpressionNode) -> Result<usize, EvaluationError> {
    if node.operation.operation_type.id != OperationId::Value {
        return Err(EvaluationError::UnsupportedOperation(
            node.operation.operation_type.id.as_str().to_string(),
        ));
    }
    match EvalValue::from_literal(&node.operation.string_value)? {
        EvalValue::Number(value) if value.fract() == 0.0 && value >= 0.0 => Ok(value as usize),
        other => Err(EvaluationError::TypeMismatch {
            expected: "non-negative integer",
            actual: render_type(&other).to_string(),
        }),
    }
}

fn assign_path_value(
    target: &mut EvalValue,
    path: &[AssignPathSegment],
    assigned: EvalValue,
) -> Result<(), EvaluationError> {
    if path.is_empty() {
        *target = assigned;
        return Ok(());
    }

    match &path[0] {
        AssignPathSegment::Key(key) => {
            if !matches!(target, EvalValue::Object(_)) {
                *target = EvalValue::Object(Default::default());
            }
            let EvalValue::Object(values) = target else {
                unreachable!();
            };
            let entry = values.entry(key.clone()).or_insert(EvalValue::Null);
            assign_path_value(entry, &path[1..], assigned)
        }
        AssignPathSegment::Index(index) => {
            if !matches!(target, EvalValue::Array(_)) {
                *target = EvalValue::Array(Vec::new());
            }
            let EvalValue::Array(values) = target else {
                unreachable!();
            };
            while values.len() <= *index {
                values.push(EvalValue::Null);
            }
            assign_path_value(&mut values[*index], &path[1..], assigned)
        }
    }
}

// ── Tree-node / Value conversion helpers ─────────────────────────

/// Convert a [`CoreTreeNode`] (accessed via [`NodeId`] in a [`CoreTreeStore`])
/// into a flat [`EvalValue`].
///
/// This mirrors the `tree_to_value` pattern used by
/// [`crate::evaluator::stream_evaluator`].
fn tree_node_to_value(store: &CoreTreeStore, id: NodeId) -> Result<EvalValue, EvaluationError> {
    let node = store.get(id).ok_or(CoreError::Eval(
        crate::core::errors::EvalError::MissingTreeNode,
    ))?;
    match node.kind {
        TreeNodeKind::Sequence => node
            .content
            .iter()
            .map(|&child| tree_node_to_value(store, child))
            .collect::<Result<Vec<_>, _>>()
            .map(EvalValue::Array),
        TreeNodeKind::Mapping => {
            let mut out = BTreeMap::new();
            for pair in node.content.chunks(2) {
                if pair.len() != 2 {
                    return Err(
                        CoreError::Parse(crate::core::errors::ParseError::InvalidSyntax).into(),
                    );
                }
                let key = store.get(pair[0]).ok_or(CoreError::Eval(
                    crate::core::errors::EvalError::MissingTreeNode,
                ))?;
                out.insert(key.value.clone(), tree_node_to_value(store, pair[1])?);
            }
            Ok(EvalValue::Object(out))
        }
        TreeNodeKind::Scalar => scalar_node_to_value(node),
        TreeNodeKind::Alias | TreeNodeKind::Unknown => Err(EvaluationError::UnsupportedOperation(
            "tree node".to_string(),
        )),
    }
}

/// Convert a scalar [`CoreTreeNode`] to an [`EvalValue`].
fn scalar_node_to_value(node: &CoreTreeNode) -> Result<EvalValue, EvaluationError> {
    if node.resolved_sem_type() == Some(SemType::Int) && node.value.parse::<i64>().is_err() {
        return Ok(EvalValue::String(node.value.clone()));
    }

    match node.get_value_rep() {
        Ok(crate::core::tree_node::ValueRep::Nil) => Ok(EvalValue::Null),
        Ok(crate::core::tree_node::ValueRep::Boolean(value)) => Ok(EvalValue::Bool(value)),
        Ok(crate::core::tree_node::ValueRep::Int(value)) => Ok(EvalValue::Number(value as f64)),
        Ok(crate::core::tree_node::ValueRep::Float(value)) => Ok(EvalValue::Number(value)),
        Ok(crate::core::tree_node::ValueRep::Str(value)) => {
            if node.resolved_sem_type() == Some(SemType::Nil) {
                Ok(EvalValue::Null)
            } else {
                Ok(EvalValue::String(value))
            }
        }
        Err(e) => Err(EvaluationError::Core(e)),
    }
}

/// Convert an [`EvalValue`] into a [`CoreTreeNode`], add it to `store`,
/// and return its [`NodeId`].
pub(super) fn value_to_tree_node(
    store: &mut CoreTreeStore,
    value: &EvalValue,
) -> Result<NodeId, EvaluationError> {
    match value {
        EvalValue::Null => {
            let node = CoreTreeNode::scalar(SemType::Nil, "null");
            Ok(store.add(node))
        }
        EvalValue::Bool(v) => {
            let node = CoreTreeNode::scalar(SemType::Boolean, v.to_string());
            Ok(store.add(node))
        }
        EvalValue::Number(v) => {
            if v.fract() == 0.0 {
                let node = CoreTreeNode::scalar(SemType::Int, (*v as i64).to_string());
                Ok(store.add(node))
            } else {
                let node = CoreTreeNode::scalar(SemType::Float, v.to_string());
                Ok(store.add(node))
            }
        }
        EvalValue::String(v) => {
            let node = CoreTreeNode::scalar(SemType::Str, v.clone());
            Ok(store.add(node))
        }
        EvalValue::Array(values) => {
            let parent_id = store.add(CoreTreeNode {
                kind: TreeNodeKind::Sequence,
                sem_type: Some(SemType::Seq),
                tag: SemType::Seq.to_string(),
                ..CoreTreeNode::default()
            });
            for v in values {
                let child_id = value_to_tree_node(store, v)?;
                let child = store
                    .get(child_id)
                    .ok_or(CoreError::Eval(
                        crate::core::errors::EvalError::MissingTreeNode,
                    ))?
                    .clone();
                store
                    .add_child(parent_id, child)
                    .map_err(|e| EvaluationError::Core(e))?;
            }
            Ok(parent_id)
        }
        EvalValue::Object(values) => {
            let parent_id = store.add(CoreTreeNode {
                kind: TreeNodeKind::Mapping,
                sem_type: Some(SemType::Map),
                tag: SemType::Map.to_string(),
                ..CoreTreeNode::default()
            });
            for (k, v) in values {
                let key_node = CoreTreeNode::scalar(SemType::Str, k.clone());
                let value_id = value_to_tree_node(store, v)?;
                let value_node = store
                    .get(value_id)
                    .ok_or(CoreError::Eval(
                        crate::core::errors::EvalError::MissingTreeNode,
                    ))?
                    .clone();
                store
                    .add_key_value_child(parent_id, key_node, value_node)
                    .map_err(|e| EvaluationError::Core(e))?;
            }
            Ok(parent_id)
        }
    }
}

/// Merge all nodes from `source_store` into `target_store`, returning the
/// new [`NodeId`] for what was `source_root`.
///
/// This is a deep clone that recursively copies the tree rooted at
/// `source_root` from `source_store` into `target_store`.
fn merge_decoded_document(
    target_store: &mut CoreTreeStore,
    source_store: &CoreTreeStore,
    source_root: NodeId,
) -> NodeId {
    merge_node_recursive(target_store, source_store, source_root)
}

fn merge_node_recursive(
    target_store: &mut CoreTreeStore,
    source_store: &CoreTreeStore,
    source_id: NodeId,
) -> NodeId {
    let source_node = match source_store.get(source_id) {
        Some(n) => n,
        None => return target_store.add(CoreTreeNode::default()),
    };

    let mut new_node = source_node.clone();
    // Save children and clear them so we can re-add with new IDs.
    let children = std::mem::take(&mut new_node.content);
    let new_id = target_store.add(new_node);

    for child_id in children {
        let new_child_id = merge_node_recursive(target_store, source_store, child_id);
        // Push the child's ID directly to the parent's content.
        if let Some(parent) = target_store.get_mut(new_id) {
            parent.content.push(new_child_id);
        }
        // Update the child's parent reference.
        if let Some(child) = target_store.get_mut(new_child_id) {
            child.parent = Some(new_id);
        }
    }

    new_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::Value;
    use crate::parser::parse_expression;

    #[test]
    fn evaluates_basic_arithmetic() {
        let tree = parse_expression("1 + 2 * 3")
            .expect("parse should succeed")
            .expect("tree should exist");
        let value = AllAtOnceEvaluator::new()
            .evaluate(&Value::Null, Some(&tree))
            .expect("evaluation should succeed");

        assert_eq!(value, Value::Number(7.0));
    }
}
