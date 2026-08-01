use std::collections::BTreeMap;

use crate::context::Context as CoreContext;
use crate::errors::CoreError;
use crate::io::codec_service::CodecService;
use crate::io::encoding::Reader;
use crate::io::printer::{Encoder, Printer};
use crate::io::printer_writer::PrinterWriter;
use crate::language::SemType;
use crate::registry::expression::{ExpressionNode, OperationId};
use crate::registry::format::format_string_from_filename;
use crate::tree::tree_navigator::TreeEngine;
use crate::tree::tree_node::{CompactTag, NodeId, TreeNode as CoreTreeNode, TreeNodeKind};
use crate::tree::tree_store::TreeStore as CoreTreeStore;

use super::stream_evaluator::print_value_result;
use super::{EvaluationError, Numeric, Value as EvalValue};

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
        let parsed = crate::parser::parse_expression(expression)
            .map_err(|e| EvaluationError::UnsupportedOperation(format!("{:?}", e)))?;
        let mut results = Vec::new();
        self.evaluate_nodes_into(store, nodes, parsed.as_deref(), |value| {
            results.push(value);
            Ok(())
        })?;
        Ok(results)
    }

    pub fn evaluate_nodes_into(
        &self,
        store: &CoreTreeStore,
        nodes: &[NodeId],
        expression: Option<&ExpressionNode>,
        mut on_value: impl FnMut(EvalValue) -> Result<(), EvaluationError>,
    ) -> Result<(), EvaluationError> {
        for &id in nodes {
            self.evaluate_tree_into(store, id, expression, &mut on_value)?;
        }

        Ok(())
    }

    pub fn evaluate_tree_into(
        &self,
        store: &CoreTreeStore,
        root: NodeId,
        expression: Option<&ExpressionNode>,
        mut on_value: impl FnMut(EvalValue) -> Result<(), EvaluationError>,
    ) -> Result<(), EvaluationError> {
        let flatten_top_level_array = should_flatten_top_level_array_result(expression);
        let result = self.evaluate_input(InputValue::TreeNode { store, id: root }, expression)?;
        if flatten_top_level_array {
            match result.into_owned()? {
                EvalValue::Array(values) => {
                    for value in values {
                        on_value(value)?;
                    }
                }
                value => on_value(value)?,
            }
        } else {
            on_value(result.into_owned()?)?;
        }
        Ok(())
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
        let mut processed_documents = 0usize;
        let mut result_index = 0_u32;
        let parsed = crate::parser::parse_expression(expression)
            .map_err(|e| EvaluationError::UnsupportedOperation(format!("{:?}", e)))?;

        for input in inputs.iter_mut() {
            let bytes = input
                .reader
                .read_all()
                .map_err(|e| EvaluationError::Core(e))?;
            let source = String::from_utf8(bytes).map_err(|_| {
                EvaluationError::Core(CoreError::System(crate::errors::SystemError::InvalidUtf8))
            })?;
            if source.trim().is_empty() {
                continue;
            }
            let format = input_format.unwrap_or_else(|| format_string_from_filename(input.name));
            let decoded_docs = codec
                .decode_all(format, &source)
                .map_err(|e| EvaluationError::Core(e))?;

            for mut decoded in decoded_docs {
                processed_documents += 1;
                decoded.store.discard_value_index();
                self.evaluate_tree_into(
                    &decoded.store,
                    decoded.root,
                    parsed.as_deref(),
                    |value| {
                        print_value_result(ctx, printer, &value, None, result_index)?;
                        result_index = result_index.saturating_add(1);
                        Ok(())
                    },
                )?;
            }
        }

        if processed_documents == 0 {
            let null_value = EvalValue::Null;
            self.evaluate_many_into(
                std::slice::from_ref(&null_value),
                parsed.as_deref(),
                |value| {
                    print_value_result(ctx, printer, &value, None, result_index)?;
                    result_index = result_index.saturating_add(1);
                    Ok(())
                },
            )?;
        }

        Ok(())
    }

    pub fn evaluate_many(
        &self,
        inputs: &[EvalValue],
        expression: Option<&ExpressionNode>,
    ) -> Result<Vec<EvalValue>, EvaluationError> {
        let mut results = Vec::new();
        self.evaluate_many_into(inputs, expression, |value| {
            results.push(value);
            Ok(())
        })?;
        Ok(results)
    }

    pub fn evaluate_many_into(
        &self,
        inputs: &[EvalValue],
        expression: Option<&ExpressionNode>,
        mut on_value: impl FnMut(EvalValue) -> Result<(), EvaluationError>,
    ) -> Result<(), EvaluationError> {
        let flatten_top_level_array = should_flatten_top_level_array_result(expression);

        for input in inputs {
            let result = self.evaluate(input, expression)?;
            if flatten_top_level_array {
                match result {
                    EvalValue::Array(values) => {
                        for value in values {
                            on_value(value)?;
                        }
                    }
                    value => on_value(value)?,
                }
            } else {
                on_value(result)?;
            }
        }

        Ok(())
    }

    pub fn evaluate(
        &self,
        input: &EvalValue,
        expression: Option<&ExpressionNode>,
    ) -> Result<EvalValue, EvaluationError> {
        match expression {
            Some(node) => self.evaluate_owned_node(input, node),
            None => Ok(input.clone()),
        }
    }

    fn evaluate_input<'a>(
        &self,
        input: InputValue<'a>,
        expression: Option<&ExpressionNode>,
    ) -> Result<InputValue<'a>, EvaluationError> {
        match expression {
            Some(node) => self.evaluate_input_node(input, node),
            None => Ok(input),
        }
    }

    fn evaluate_input_node<'a>(
        &self,
        input: InputValue<'a>,
        node: &ExpressionNode,
    ) -> Result<InputValue<'a>, EvaluationError> {
        use OperationId::*;

        match node.operation.operation_type.id {
            SelfRef => return Ok(input),
            Value => {
                return Ok(InputValue::Owned(EvalValue::from_literal(
                    &node.operation.string_value,
                )?));
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
                let piped = self.evaluate_input_node(input, lhs)?;
                if pipe_rhs_consumes_array(rhs) {
                    return self.evaluate_input_node(piped, rhs);
                }
                if let Some((store, children)) = piped.clone().sequence_children() {
                    let mut out = Vec::with_capacity(children.len());
                    for child in children {
                        let result = self
                            .evaluate_input_node(InputValue::TreeNode { store, id: child }, rhs)?;
                        let owned = result.into_owned()?;
                        if matches!(rhs.operation.operation_type.id, Select | Filter)
                            && owned == EvalValue::Null
                        {
                            continue;
                        }
                        out.push(owned);
                    }
                    return Ok(InputValue::Owned(EvalValue::Array(out)));
                }

                let value = piped.into_owned()?;
                if matches!(lhs.operation.operation_type.id, Select | Filter)
                    && value == EvalValue::Null
                {
                    return Ok(InputValue::Owned(EvalValue::Array(Vec::new())));
                }
                return self.evaluate_input_node(InputValue::Owned(value), rhs);
            }
            TraversePath | TraverseArray => {
                let lhs = match node.lhs.as_deref() {
                    Some(lhs_node) => self.evaluate_input_node(input.clone(), lhs_node)?,
                    None => input.clone(),
                };
                let rhs = match node.rhs.as_deref() {
                    Some(rhs_node) => self.evaluate_input_node(input, rhs_node)?.into_owned()?,
                    None if node.operation.operation_type.id == TraversePath => {
                        EvalValue::String(node.operation.string_value.clone())
                    }
                    None => EvalValue::Array(Vec::new()),
                };
                return self.evaluate_tree_backed_traversal(
                    node.operation.operation_type.id,
                    lhs,
                    rhs,
                );
            }
            Map => {
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                if let Some((store, children)) = input.clone().sequence_children() {
                    let mut out = Vec::with_capacity(children.len());
                    for child in children {
                        out.push(
                            self.evaluate_input_node(
                                InputValue::TreeNode { store, id: child },
                                rhs,
                            )?
                            .into_owned()?,
                        );
                    }
                    return Ok(InputValue::Owned(EvalValue::Array(out)));
                }
            }
            Select | Filter => {
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                if let Some((store, children)) = input.clone().sequence_children() {
                    let mut out = Vec::new();
                    for child in children {
                        let item = InputValue::TreeNode { store, id: child };
                        if self.evaluate_input_node(item.clone(), rhs)?.truthy()? {
                            out.push(item.into_owned()?);
                        }
                    }
                    return Ok(InputValue::Owned(EvalValue::Array(out)));
                }
            }
            Length => {
                if let Some(length) = input.clone().direct_length()? {
                    return Ok(InputValue::Owned(EvalValue::Number(Numeric::Int(
                        length as i64,
                    ))));
                }
            }
            Keys => {
                if let Some(keys) = input.clone().direct_keys()? {
                    return Ok(InputValue::Owned(EvalValue::Array(keys)));
                }
            }
            Has => {
                let lhs = match node.lhs.as_deref() {
                    Some(lhs_node) => self.evaluate_input_node(input.clone(), lhs_node)?,
                    None => input.clone(),
                };
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let rhs = self.evaluate_input_node(input.clone(), rhs)?.into_owned()?;
                if let Some(result) = lhs.clone().direct_has(&rhs)? {
                    return Ok(InputValue::Owned(EvalValue::Bool(result)));
                }
            }
            _ => {}
        }

        let owned_input = input.into_owned()?;
        Ok(InputValue::Owned(
            self.evaluate_owned_node(&owned_input, node)?,
        ))
    }

    fn evaluate_owned_node(
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
                let value = self.evaluate_owned_node(input, rhs)?;
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
                let piped = self.evaluate_owned_node(input, lhs)?;
                match piped {
                    EvalValue::Array(values) => {
                        if pipe_rhs_consumes_array(rhs) {
                            return self.evaluate_owned_node(&EvalValue::Array(values), rhs);
                        }
                        let mut out = Vec::new();
                        for value in values {
                            let result = self.evaluate_owned_node(&value, rhs)?;
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
                        self.evaluate_owned_node(&value, rhs)
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
                let _ = self.evaluate_owned_node(input, lhs)?;
                self.evaluate_owned_node(input, rhs)
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
                    let current = self.evaluate_owned_node(input, lhs)?;
                    let assigned = match current {
                        EvalValue::Array(values) => EvalValue::Array(
                            values
                                .iter()
                                .map(|value| self.evaluate_owned_node(value, rhs))
                                .collect::<Result<Vec<_>, _>>()?,
                        ),
                        value => self.evaluate_owned_node(&value, rhs)?,
                    };

                    if is_top_level_array_update_assign(lhs) {
                        return Ok(assigned);
                    }

                    return self.evaluate_assign(input, lhs, assigned);
                }
                let assigned = self.evaluate_owned_node(input, rhs)?;
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
                    Some(lhs_node) => self.evaluate_owned_node(input, lhs_node)?,
                    None => input.clone(),
                };
                let rhs = match node.rhs.as_deref() {
                    Some(rhs_node) => self.evaluate_owned_node(input, rhs_node)?,
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
                        .map(|value| self.evaluate_owned_node(value, rhs))
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
                        .filter_map(|value| match self.evaluate_owned_node(value, rhs) {
                            Ok(result) if result.truthy() => Some(Ok(value.clone())),
                            Ok(_) => None,
                            Err(err) => Some(Err(err)),
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map(EvalValue::Array),
                    value => {
                        let result = self.evaluate_owned_node(value, rhs)?;
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
                            .map(|value| Ok((self.evaluate_owned_node(value, rhs)?, value.clone())))
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
                    Some(lhs) => self.evaluate_owned_node(input, lhs)?,
                    None => input.clone(),
                };
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let rhs = self.evaluate_owned_node(input, rhs)?;
                Ok(EvalValue::Bool(self.value_contains(&lhs, &rhs)?))
            }
            Has => {
                let lhs = match node.lhs.as_deref() {
                    Some(lhs) => self.evaluate_owned_node(input, lhs)?,
                    None => input.clone(),
                };
                let rhs = node
                    .rhs
                    .as_deref()
                    .ok_or(EvaluationError::MissingOperand("rhs"))?;
                let rhs = self.evaluate_owned_node(input, rhs)?;
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
                let lhs = self.evaluate_owned_node(input, lhs_node)?;
                let rhs = self.evaluate_owned_node(input, rhs_node)?;
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
            _ => Ok(vec![self.evaluate_owned_node(input, node)?]),
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
                let key = self.evaluate_owned_node(input, lhs)?;
                let value = self.evaluate_owned_node(input, rhs)?;
                out.insert(value_to_key(&key), value);
                Ok(())
            }
            _ => match self.evaluate_owned_node(input, node)? {
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
        let key = self.evaluate_owned_node(input, lhs)?;
        let value = self.evaluate_owned_node(input, rhs)?;
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

    fn evaluate_tree_backed_traversal<'a>(
        &self,
        id: OperationId,
        lhs: InputValue<'a>,
        rhs: EvalValue,
    ) -> Result<InputValue<'a>, EvaluationError> {
        match id {
            OperationId::TraversePath => {
                let key = match rhs {
                    EvalValue::String(value) => value,
                    EvalValue::Number(value) if numeric_path_key(value).is_some() => {
                        numeric_path_key(value).expect("guard ensures a path key")
                    }
                    other => {
                        return Err(EvaluationError::TypeMismatch {
                            expected: "string",
                            actual: render_type(&other).to_string(),
                        });
                    }
                };
                if let Some((store, id)) = lhs.clone().tree_node() {
                    if let Some(child) = tree_mapping_child(store, id, &key)? {
                        return Ok(InputValue::TreeNode { store, id: child });
                    }
                    if store
                        .get(id)
                        .is_some_and(|node| node.kind == TreeNodeKind::Mapping)
                    {
                        return Ok(InputValue::Owned(EvalValue::Null));
                    }
                }
                let owned = lhs.into_owned()?;
                self.evaluate_traversal(id, owned, EvalValue::String(key))
                    .map(InputValue::Owned)
            }
            OperationId::TraverseArray => {
                if let Some((store, id)) = lhs.clone().tree_node() {
                    let Some(node) = store.get(id) else {
                        return Ok(InputValue::Owned(EvalValue::Null));
                    };
                    if node.kind == TreeNodeKind::Sequence {
                        if matches!(rhs, EvalValue::Array(ref values) if values.is_empty()) {
                            return Ok(InputValue::TreeNode { store, id });
                        }
                        if let EvalValue::Number(index) = rhs {
                            let Some(index) = numeric_index(index) else {
                                return Err(EvaluationError::TypeMismatch {
                                    expected: "non-negative integer",
                                    actual: index.display(),
                                });
                            };
                            return Ok(node
                                .content
                                .get(index)
                                .copied()
                                .map(|child| InputValue::TreeNode { store, id: child })
                                .unwrap_or(InputValue::Owned(EvalValue::Null)));
                        }
                        if let EvalValue::Array(indices) = rhs {
                            let mut selected = Vec::with_capacity(indices.len());
                            for index_value in indices {
                                let index = index_value.as_numeric()?;
                                let Some(index) = numeric_index(index) else {
                                    return Err(EvaluationError::TypeMismatch {
                                        expected: "non-negative integer",
                                        actual: index.display(),
                                    });
                                };
                                selected.push(
                                    node.content
                                        .get(index)
                                        .copied()
                                        .map(|child| tree_node_to_value(store, child))
                                        .transpose()?
                                        .unwrap_or(EvalValue::Null),
                                );
                            }
                            return Ok(InputValue::Owned(if selected.len() == 1 {
                                selected.into_iter().next().unwrap_or(EvalValue::Null)
                            } else {
                                EvalValue::Array(selected)
                            }));
                        }
                    }
                }
                let owned = lhs.into_owned()?;
                self.evaluate_traversal(id, owned, rhs)
                    .map(InputValue::Owned)
            }
            _ => Err(EvaluationError::UnsupportedOperation(
                id.as_str().to_string(),
            )),
        }
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
                    EvalValue::Number(value) if numeric_path_key(value).is_some() => {
                        numeric_path_key(value).expect("guard ensures a path key")
                    }
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
            OperationId::Length => Ok(EvalValue::Number(Numeric::Int(match input {
                EvalValue::Null => 0,
                EvalValue::Bool(_) => 1,
                EvalValue::Number(value) => render_number(*value).len() as i64,
                EvalValue::String(value) => value.chars().count() as i64,
                EvalValue::Array(values) => values.len() as i64,
                EvalValue::Object(values) => values.len() as i64,
            }))),
            OperationId::Keys => match input {
                EvalValue::Array(values) => Ok(EvalValue::Array(
                    (0..values.len())
                        .map(|idx| EvalValue::Number(Numeric::Int(idx as i64)))
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
            OperationId::ToNumber => Ok(EvalValue::Number(input.as_numeric()?)),
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
                    Ok(EvalValue::Number(Numeric::Float(selected)))
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
                EvalValue::Number(index) if numeric_index(*index).is_some() => {
                    Ok(numeric_index(*index).expect("guard ensures an index") < values.len())
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
                    Ok(EvalValue::Number(add_numbers(lhs, rhs)))
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
            Subtract => Ok(EvalValue::Number(subtract_numbers(
                lhs.as_numeric()?,
                rhs.as_numeric()?,
            ))),
            Multiply => Ok(EvalValue::Number(multiply_numbers(
                lhs.as_numeric()?,
                rhs.as_numeric()?,
            ))),
            Divide => Ok(EvalValue::Number(divide_numbers(
                lhs.as_numeric()?,
                rhs.as_numeric()?,
            )?)),
            Modulo => Ok(EvalValue::Number(modulo_numbers(
                lhs.as_numeric()?,
                rhs.as_numeric()?,
            )?)),
            And => Ok(EvalValue::Bool(lhs.truthy() && rhs.truthy())),
            Or => Ok(EvalValue::Bool(lhs.truthy() || rhs.truthy())),
            Equals => Ok(EvalValue::Bool(values_equal(&lhs, &rhs))),
            NotEquals => Ok(EvalValue::Bool(!values_equal(&lhs, &rhs))),
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

#[derive(Clone)]
enum InputValue<'a> {
    TreeNode {
        store: &'a CoreTreeStore,
        id: NodeId,
    },
    Owned(EvalValue),
}

impl<'a> InputValue<'a> {
    fn tree_node(self) -> Option<(&'a CoreTreeStore, NodeId)> {
        match self {
            InputValue::TreeNode { store, id } => Some((store, id)),
            InputValue::Owned(_) => None,
        }
    }

    fn sequence_children(self) -> Option<(&'a CoreTreeStore, Vec<NodeId>)> {
        let (store, id) = self.tree_node()?;
        let node = store.get(id)?;
        (node.kind == TreeNodeKind::Sequence).then(|| (store, node.content.clone()))
    }

    fn direct_length(self) -> Result<Option<usize>, EvaluationError> {
        let Some((store, id)) = self.tree_node() else {
            return Ok(None);
        };
        let Some(node) = store.get(id) else {
            return Ok(Some(0));
        };
        let length = match node.kind {
            TreeNodeKind::Sequence => node.content.len(),
            TreeNodeKind::Mapping => node.content.len() / 2,
            TreeNodeKind::Scalar => match store.value_rep_for(id)? {
                crate::tree::tree_node::ValueRep::Nil => 0,
                crate::tree::tree_node::ValueRep::Boolean(_) => 1,
                crate::tree::tree_node::ValueRep::Int(value) => value.to_string().len(),
                crate::tree::tree_node::ValueRep::Float(value) => value.to_string().len(),
                crate::tree::tree_node::ValueRep::Str(value) => value.chars().count(),
            },
            TreeNodeKind::Alias | TreeNodeKind::Unknown => return Ok(None),
        };
        Ok(Some(length))
    }

    fn direct_keys(self) -> Result<Option<Vec<EvalValue>>, EvaluationError> {
        let Some((store, id)) = self.tree_node() else {
            return Ok(None);
        };
        let Some(node) = store.get(id) else {
            return Ok(Some(Vec::new()));
        };
        let keys = match node.kind {
            TreeNodeKind::Sequence => node
                .content
                .iter()
                .enumerate()
                .map(|(index, _)| EvalValue::Number(Numeric::Int(index as i64)))
                .collect(),
            TreeNodeKind::Mapping => {
                let mut out = Vec::with_capacity(node.content.len() / 2);
                for pair in node.content.chunks(2) {
                    if pair.len() != 2 {
                        return Err(
                            CoreError::Parse(crate::errors::ParseError::InvalidSyntax).into()
                        );
                    }
                    out.push(EvalValue::String(store.value_string_for(pair[0])?));
                }
                out
            }
            TreeNodeKind::Scalar | TreeNodeKind::Alias | TreeNodeKind::Unknown => return Ok(None),
        };
        Ok(Some(keys))
    }

    fn direct_has(self, rhs: &EvalValue) -> Result<Option<bool>, EvaluationError> {
        let Some((store, id)) = self.tree_node() else {
            return Ok(None);
        };
        let Some(node) = store.get(id) else {
            return Ok(Some(false));
        };
        match node.kind {
            TreeNodeKind::Mapping => Ok(Some(
                tree_mapping_child(store, id, &value_to_key(rhs))?.is_some(),
            )),
            TreeNodeKind::Sequence => match rhs {
                EvalValue::Number(index) if numeric_index(*index).is_some() => Ok(Some(
                    numeric_index(*index).expect("guard ensures an index") < node.content.len(),
                )),
                other => Err(EvaluationError::TypeMismatch {
                    expected: "non-negative integer",
                    actual: render_type(other).to_string(),
                }),
            },
            TreeNodeKind::Scalar | TreeNodeKind::Alias | TreeNodeKind::Unknown => Ok(None),
        }
    }

    fn truthy(self) -> Result<bool, EvaluationError> {
        Ok(self.into_owned()?.truthy())
    }

    fn into_owned(self) -> Result<EvalValue, EvaluationError> {
        match self {
            InputValue::TreeNode { store, id } => tree_node_to_value(store, id),
            InputValue::Owned(value) => Ok(value),
        }
    }
}

fn tree_mapping_child(
    store: &CoreTreeStore,
    id: NodeId,
    key: &str,
) -> Result<Option<NodeId>, EvaluationError> {
    let Some(node) = store.get(id) else {
        return Ok(None);
    };
    if node.kind != TreeNodeKind::Mapping {
        return Ok(None);
    }
    for pair in node.content.chunks(2) {
        if pair.len() != 2 {
            return Err(CoreError::Parse(crate::errors::ParseError::InvalidSyntax).into());
        }
        if store.value_string_for(pair[0])? == key {
            return Ok(Some(pair[1]));
        }
    }
    Ok(None)
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

fn add_numbers(lhs: Numeric, rhs: Numeric) -> Numeric {
    match (lhs, rhs) {
        (Numeric::Int(lhs), Numeric::Int(rhs)) => lhs
            .checked_add(rhs)
            .map(Numeric::Int)
            .unwrap_or_else(|| Numeric::Float(lhs as f64 + rhs as f64)),
        (lhs, rhs) => Numeric::Float(lhs.as_f64() + rhs.as_f64()),
    }
}

fn subtract_numbers(lhs: Numeric, rhs: Numeric) -> Numeric {
    match (lhs, rhs) {
        (Numeric::Int(lhs), Numeric::Int(rhs)) => lhs
            .checked_sub(rhs)
            .map(Numeric::Int)
            .unwrap_or_else(|| Numeric::Float(lhs as f64 - rhs as f64)),
        (lhs, rhs) => Numeric::Float(lhs.as_f64() - rhs.as_f64()),
    }
}

fn multiply_numbers(lhs: Numeric, rhs: Numeric) -> Numeric {
    match (lhs, rhs) {
        (Numeric::Int(lhs), Numeric::Int(rhs)) => lhs
            .checked_mul(rhs)
            .map(Numeric::Int)
            .unwrap_or_else(|| Numeric::Float(lhs as f64 * rhs as f64)),
        (lhs, rhs) => Numeric::Float(lhs.as_f64() * rhs.as_f64()),
    }
}

fn divide_numbers(lhs: Numeric, rhs: Numeric) -> Result<Numeric, EvaluationError> {
    if rhs.is_zero() {
        return Err(EvaluationError::DivisionByZero);
    }
    match (lhs, rhs) {
        (Numeric::Int(lhs), Numeric::Int(rhs)) if lhs.checked_rem(rhs) == Some(0) => lhs
            .checked_div(rhs)
            .map(Numeric::Int)
            .ok_or(EvaluationError::DivisionByZero),
        (lhs, rhs) => Ok(Numeric::Float(lhs.as_f64() / rhs.as_f64())),
    }
}

fn modulo_numbers(lhs: Numeric, rhs: Numeric) -> Result<Numeric, EvaluationError> {
    if rhs.is_zero() {
        return Err(EvaluationError::DivisionByZero);
    }
    Ok(match (lhs, rhs) {
        (Numeric::Int(lhs), Numeric::Int(rhs)) => lhs
            .checked_rem(rhs)
            .map(Numeric::Int)
            .unwrap_or_else(|| Numeric::Float(lhs as f64 % rhs as f64)),
        (lhs, rhs) => Numeric::Float(lhs.as_f64() % rhs.as_f64()),
    })
}

fn numeric_path_key(value: Numeric) -> Option<String> {
    match value {
        Numeric::Int(value) => Some(value.to_string()),
        Numeric::Float(value) if value.fract() == 0.0 => Some(format!("{value:.0}")),
        Numeric::Float(_) => None,
    }
}

fn numeric_index(value: Numeric) -> Option<usize> {
    match value {
        Numeric::Int(value) => usize::try_from(value).ok(),
        Numeric::Float(value)
            if value.fract() == 0.0 && value >= 0.0 && value <= usize::MAX as f64 =>
        {
            Some(value as usize)
        }
        Numeric::Float(_) => None,
    }
}

fn render_number(value: Numeric) -> String {
    value.display()
}

fn numeric_equal(lhs: Numeric, rhs: Numeric) -> bool {
    match (lhs, rhs) {
        (Numeric::Int(lhs), Numeric::Int(rhs)) => lhs == rhs,
        (Numeric::Float(lhs), Numeric::Float(rhs)) => lhs == rhs,
        (Numeric::Int(integer), Numeric::Float(float))
        | (Numeric::Float(float), Numeric::Int(integer)) => {
            float.is_finite()
                && float.fract() == 0.0
                && float >= i64::MIN as f64
                && float <= i64::MAX as f64
                && float as i64 == integer
        }
    }
}

fn values_equal(lhs: &EvalValue, rhs: &EvalValue) -> bool {
    match (lhs, rhs) {
        (EvalValue::Number(lhs), EvalValue::Number(rhs)) => numeric_equal(*lhs, *rhs),
        (EvalValue::Array(lhs), EvalValue::Array(rhs)) => {
            lhs.len() == rhs.len() && lhs.iter().zip(rhs).all(|(lhs, rhs)| values_equal(lhs, rhs))
        }
        (EvalValue::Object(lhs), EvalValue::Object(rhs)) => {
            lhs.len() == rhs.len()
                && lhs
                    .iter()
                    .all(|(key, lhs)| rhs.get(key).is_some_and(|rhs| values_equal(lhs, rhs)))
        }
        _ => lhs == rhs,
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
        EvalValue::Number(value) => format!("2:{:020}", value.as_f64()),
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
        EvalValue::Number(value) if numeric_path_key(value).is_some() => {
            Ok(numeric_path_key(value).expect("guard ensures a path key"))
        }
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
        EvalValue::Number(value) if numeric_index(value).is_some() => {
            Ok(numeric_index(value).expect("guard ensures an index"))
        }
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
    let node = store
        .get(id)
        .ok_or(CoreError::Eval(crate::errors::EvalError::MissingTreeNode))?;
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
                    return Err(CoreError::Parse(crate::errors::ParseError::InvalidSyntax).into());
                }
                store
                    .get(pair[0])
                    .ok_or(CoreError::Eval(crate::errors::EvalError::MissingTreeNode))?;
                out.insert(
                    store.value_string_for(pair[0])?,
                    tree_node_to_value(store, pair[1])?,
                );
            }
            Ok(EvalValue::Object(out))
        }
        TreeNodeKind::Scalar => scalar_node_to_value(store, id),
        TreeNodeKind::Alias | TreeNodeKind::Unknown => Err(EvaluationError::UnsupportedOperation(
            "tree node".to_string(),
        )),
    }
}

/// Convert a scalar [`CoreTreeNode`] to an [`EvalValue`].
fn scalar_node_to_value(store: &CoreTreeStore, id: NodeId) -> Result<EvalValue, EvaluationError> {
    store
        .get(id)
        .ok_or(CoreError::Eval(crate::errors::EvalError::MissingTreeNode))?;
    if store.resolved_sem_type_for(id)? == Some(SemType::Int)
        && store.value_for(id)?.parse::<i64>().is_err()
    {
        return Ok(EvalValue::String(store.value_string_for(id)?));
    }

    match store.value_rep_for(id)? {
        crate::tree::tree_node::ValueRep::Nil => Ok(EvalValue::Null),
        crate::tree::tree_node::ValueRep::Boolean(value) => Ok(EvalValue::Bool(value)),
        crate::tree::tree_node::ValueRep::Int(value) => Ok(EvalValue::Number(Numeric::Int(value))),
        crate::tree::tree_node::ValueRep::Float(value) => {
            Ok(EvalValue::Number(Numeric::Float(value)))
        }
        crate::tree::tree_node::ValueRep::Str(value) => {
            if store.resolved_sem_type_for(id)? == Some(SemType::Nil) {
                Ok(EvalValue::Null)
            } else {
                Ok(EvalValue::String(value))
            }
        }
    }
}

/// Convert an [`EvalValue`] into a [`CoreTreeNode`], add it to `store`,
/// and return its [`NodeId`].
pub(super) fn value_to_tree_node(
    store: &mut CoreTreeStore,
    value: &EvalValue,
) -> Result<NodeId, EvaluationError> {
    let root_id = store.add(shallow_tree_node_for_value(value));
    populate_tree_children(store, root_id, value)?;
    Ok(root_id)
}

fn shallow_tree_node_for_value(value: &EvalValue) -> CoreTreeNode {
    match value {
        EvalValue::Null => CoreTreeNode::scalar(SemType::Nil, "null"),
        EvalValue::Bool(v) => CoreTreeNode::scalar(SemType::Boolean, v.to_string()),
        EvalValue::Number(Numeric::Int(value)) => {
            CoreTreeNode::scalar(SemType::Int, value.to_string())
        }
        EvalValue::Number(Numeric::Float(value)) => {
            CoreTreeNode::scalar(SemType::Float, value.to_string())
        }
        EvalValue::String(v) => CoreTreeNode::scalar(SemType::Str, v.clone()),
        EvalValue::Array(_) => CoreTreeNode {
            kind: TreeNodeKind::Sequence,
            sem_type: Some(SemType::Seq),
            tag: CompactTag::from_sem_type(SemType::Seq),
            ..CoreTreeNode::default()
        },
        EvalValue::Object(_) => CoreTreeNode {
            kind: TreeNodeKind::Mapping,
            sem_type: Some(SemType::Map),
            tag: CompactTag::from_sem_type(SemType::Map),
            ..CoreTreeNode::default()
        },
    }
}

fn populate_tree_children(
    store: &mut CoreTreeStore,
    parent_id: NodeId,
    value: &EvalValue,
) -> Result<(), EvaluationError> {
    match value {
        EvalValue::Array(values) => {
            for value in values {
                let child_id = store
                    .add_child(parent_id, shallow_tree_node_for_value(value))
                    .map_err(EvaluationError::Core)?;
                populate_tree_children(store, child_id, value)?;
            }
            Ok(())
        }
        EvalValue::Object(values) => {
            for (key, value) in values {
                let key_node = CoreTreeNode::scalar(SemType::Str, key.clone());
                let (_, value_id) = store
                    .add_key_value_child(parent_id, key_node, shallow_tree_node_for_value(value))
                    .map_err(EvaluationError::Core)?;
                populate_tree_children(store, value_id, value)?;
            }
            Ok(())
        }
        EvalValue::Null | EvalValue::Bool(_) | EvalValue::Number(_) | EvalValue::String(_) => {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context as CoreContext;
    use crate::errors::{CoreError, SystemError};
    use crate::evaluator::Value;
    use crate::formats::{Encode, JsonEncoder};
    use crate::io::CodecService;
    use crate::io::VecPrinterWriter;
    use crate::io::printer::Encoder as PrintEncoder;
    use crate::parser::parse_expression;
    use std::collections::BTreeMap;

    struct TestPrinterEncoder(JsonEncoder);

    impl PrintEncoder for TestPrinterEncoder {
        fn encode(
            &self,
            ctx: &mut CoreContext,
            node: NodeId,
            writer: &mut dyn std::io::Write,
        ) -> Result<(), CoreError> {
            let store = ctx
                .current_print_store()
                .ok_or(CoreError::System(SystemError::Error))?;
            self.0.encode(store, node, writer)
        }

        fn encode_evaluated_value(
            &self,
            value: &crate::evaluator::Value,
            writer: &mut dyn std::io::Write,
        ) -> Result<bool, CoreError> {
            self.0.encode_evaluated_value(value, writer)
        }
    }

    #[test]
    fn evaluates_basic_arithmetic() {
        let tree = parse_expression("1 + 2 * 3")
            .expect("parse should succeed")
            .expect("tree should exist");
        let value = AllAtOnceEvaluator::new()
            .evaluate(&Value::Null, Some(&tree))
            .expect("evaluation should succeed");

        assert_eq!(value, Value::Number(Numeric::Int(7)));
    }

    #[test]
    fn numeric_extremes_promote_to_float_instead_of_panicking() {
        assert!(matches!(add_numbers(Numeric::Int(i64::MAX), Numeric::Int(1)), Numeric::Float(_)));
        assert!(matches!(subtract_numbers(Numeric::Int(i64::MIN), Numeric::Int(1)), Numeric::Float(_)));
        assert!(matches!(multiply_numbers(Numeric::Int(i64::MAX), Numeric::Int(2)), Numeric::Float(_)));
        assert!(matches!(divide_numbers(Numeric::Int(i64::MIN), Numeric::Int(-1)), Ok(Numeric::Float(_))));
        assert!(matches!(modulo_numbers(Numeric::Int(i64::MIN), Numeric::Int(-1)), Ok(Numeric::Float(_))));
    }

    #[test]
    fn value_to_tree_node_builds_dense_tree_without_orphans() {
        let mut inner = BTreeMap::new();
        inner.insert("c".to_string(), Value::String("x".to_string()));

        let mut root = BTreeMap::new();
        root.insert(
            "a".to_string(),
            Value::Array(vec![
                Value::Number(Numeric::Int(1)),
                Value::Number(Numeric::Int(2)),
            ]),
        );
        root.insert("b".to_string(), Value::Object(inner));

        let value = Value::Object(root);
        let mut store = CoreTreeStore::new();
        let root_id = value_to_tree_node(&mut store, &value).expect("tree should build");

        let mut reachable = Vec::new();
        collect_reachable_ids(&store, root_id, &mut reachable);
        reachable.sort_by_key(|id| id.0);
        reachable.dedup_by_key(|id| id.0);

        assert_eq!(store.len(), 9);
        assert_eq!(reachable.len(), store.len());
        for node_id in reachable.into_iter().filter(|id| *id != root_id) {
            let node = store.get(node_id).expect("reachable node should exist");
            assert!(
                node.parent.is_some(),
                "node {node_id:?} should have a parent"
            );
        }
    }

    fn collect_reachable_ids(store: &CoreTreeStore, node_id: NodeId, out: &mut Vec<NodeId>) {
        out.push(node_id);
        let children = store
            .get(node_id)
            .expect("reachable node should exist")
            .content
            .clone();
        for child in children {
            collect_reachable_ids(store, child, out);
        }
    }

    #[test]
    fn evaluate_readers_streams_documents_without_batching_all_inputs() {
        let mut registry = crate::init().expect("registry should initialize");
        let result = (|| {
            let mut ctx = CoreContext::empty(registry.handle());
            let encoder = TestPrinterEncoder(JsonEncoder::default());
            let writer = VecPrinterWriter::new();
            let mut printer = Printer::new(encoder, writer);
            let codec = CodecService::new();
            let mut first = std::io::Cursor::new(br#"{"value":1}"#.to_vec());
            let mut second = std::io::Cursor::new(br#"{"value":2}"#.to_vec());
            let mut inputs = [
                Input::new("first.json", Reader::new(&mut first)),
                Input::new("second.json", Reader::new(&mut second)),
            ];

            AllAtOnceEvaluator::new()
                .evaluate_readers_with_format(
                    &mut ctx,
                    ".value",
                    &mut inputs,
                    &mut printer,
                    &codec,
                    Some("json"),
                )
                .expect("reader evaluation should succeed");

            let output = String::from_utf8(printer.into_writer().into_bytes())
                .expect("output should be utf-8");
            assert_eq!(output, "12");
            Ok::<(), CoreError>(())
        })();
        crate::deinit(&mut registry);
        result.expect("test body should succeed");
    }

    #[test]
    fn evaluate_tree_into_skips_unrelated_unknown_subtrees_after_tree_backed_traversal() {
        let expression = parse_expression(".target")
            .expect("parse should succeed")
            .expect("expression should exist");

        let mut store = CoreTreeStore::new();
        let root_id = store.add(CoreTreeNode {
            kind: TreeNodeKind::Mapping,
            sem_type: Some(SemType::Map),
            tag: CompactTag::from_sem_type(SemType::Map),
            ..CoreTreeNode::default()
        });
        let target_key = store.add(CoreTreeNode::scalar(SemType::Str, "target"));
        let target_value = store.add(CoreTreeNode {
            kind: TreeNodeKind::Sequence,
            sem_type: Some(SemType::Seq),
            tag: CompactTag::from_sem_type(SemType::Seq),
            parent: Some(root_id),
            ..CoreTreeNode::default()
        });
        let first_item = store.add(CoreTreeNode::scalar(SemType::Int, "1"));
        let second_item = store.add(CoreTreeNode::scalar(SemType::Int, "2"));
        let broken_key = store.add(CoreTreeNode::scalar(SemType::Str, "broken"));
        let broken_value = store.add(CoreTreeNode {
            kind: TreeNodeKind::Unknown,
            parent: Some(root_id),
            ..CoreTreeNode::default()
        });

        store.get_mut(root_id).expect("root exists").content =
            vec![target_key, target_value, broken_key, broken_value];
        store.get_mut(target_key).expect("target key exists").parent = Some(root_id);
        store
            .get_mut(target_value)
            .expect("target value exists")
            .content = vec![first_item, second_item];
        store.get_mut(first_item).expect("first item exists").parent = Some(target_value);
        store
            .get_mut(second_item)
            .expect("second item exists")
            .parent = Some(target_value);
        store.get_mut(broken_key).expect("broken key exists").parent = Some(root_id);

        let mut values = Vec::new();
        AllAtOnceEvaluator::new()
            .evaluate_tree_into(&store, root_id, Some(&expression), |value| {
                values.push(value);
                Ok(())
            })
            .expect("tree-backed evaluation should succeed");

        assert_eq!(
            values,
            vec![Value::Array(vec![
                Value::Number(Numeric::Int(1)),
                Value::Number(Numeric::Int(2)),
            ])]
        );
    }
}
