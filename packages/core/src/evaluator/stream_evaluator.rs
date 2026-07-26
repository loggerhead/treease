use std::io::Read;

use crate::{
    core::{
        CodecService, CoreError,
        context::Context,
        diagnostics::DiagnosticStage,
        expression::{ExpressionNode, OperationId},
        format::format_string_from_filename,
        io_adapters::AnyReader,
        printer::{Encoder, Printer},
        printer_writer::PrinterWriter,
        tree_navigator::TreeEngine,
        tree_node::ParsedKey,
        tree_node::{NodeId, TreeNodeKind},
        tree_store::TreeStore,
    },
    formats::DecodedDocument,
    stream::{
        build_tree_from_events, streaming_decoder, streaming_events::StreamingEvent,
        streaming_json, tree_builder,
    },
};

use super::all_at_once_evaluator::{AllAtOnceEvaluator, value_to_tree_node};
use super::{EvaluationError, Numeric, Value};

/// Input descriptor for reader-based streaming evaluation.
///
pub struct ReaderInput<'a> {
    pub name: &'a str,
    pub reader: AnyReader<'a>,
}

impl<'a> ReaderInput<'a> {
    pub fn new(name: &'a str, reader: AnyReader<'a>) -> Self {
        Self { name, reader }
    }
}

/// Streaming evaluator that processes documents one-at-a-time from readers,
/// printing results per-document with `file_index` carry-over.
///
#[derive(Debug, Default)]
pub struct StreamEvaluator {
    /// Tree navigator used for tree-level dispatch during evaluation.
    pub tree_navigator: TreeEngine,
    /// Tracks the current file index across multiple inputs.
    pub file_index: i32,
}

impl StreamEvaluator {
    pub fn new() -> Self {
        Self {
            tree_navigator: TreeEngine::new(),
            file_index: 0,
        }
    }

    // ── Context helpers ─────────────────────────────────────────────

    /// Create an inherited context that copies `dont_auto_create`,
    /// `diagnostics`, and `user_data` from the base context.
    ///
    fn inherited_context(base: &Context) -> Context {
        let mut c = Context::empty(base.codec_registry.clone());
        c.dont_auto_create = base.dont_auto_create;
        c.diagnostics = base.diagnostics.clone();
        c.user_data = base.user_data;
        c
    }

    // ── evaluateNew / evaluateNoInput ────────────────────────────────

    /// Evaluate an expression with no input documents. Creates an empty
    /// scalar node, executes the expression, and prints results through
    /// `printer`.
    ///
    pub fn evaluate_new<E, W>(
        &mut self,
        ctx: &mut Context,
        expression: &str,
        printer: &mut Printer<E, W>,
    ) -> Result<(), EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        let parsed = crate::parser::parse_expression(expression)
            .map_err(|e| EvaluationError::UnsupportedOperation(format!("{:?}", e)))?;
        self.evaluate_no_input(ctx, parsed.as_deref(), printer)
    }

    /// Evaluate with no input using a pre-parsed expression node.
    /// Creates an empty scalar node, executes the pipeline, and prints
    /// results through `printer`.
    ///
    pub fn evaluate_no_input<E, W>(
        &mut self,
        ctx: &mut Context,
        node: Option<&ExpressionNode>,
        printer: &mut Printer<E, W>,
    ) -> Result<(), EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        let mut result_index = 0_u32;
        AllAtOnceEvaluator::new().evaluate_many_into(&[Value::Null], node, |value| {
            print_value_result(ctx, printer, &value, None, result_index)?;
            result_index = result_index.saturating_add(1);
            Ok(())
        })?;

        Ok(())
    }

    // ── evaluateReaders ──────────────────────────────────────────────

    /// Evaluate an expression against multiple reader inputs, processing
    /// each document one-at-a-time and printing results per-document.
    ///
    /// If no documents are processed across all inputs, falls back to
    /// [`evaluate_new`].
    ///
    pub fn evaluate_readers<E, W>(
        &mut self,
        ctx: &mut Context,
        expression: &str,
        inputs: &mut [ReaderInput<'_>],
        printer: &mut Printer<E, W>,
    ) -> Result<(), EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        self.evaluate_readers_with_format(ctx, expression, inputs, printer, None)
    }

    pub fn evaluate_readers_with_format<E, W>(
        &mut self,
        ctx: &mut Context,
        expression: &str,
        inputs: &mut [ReaderInput<'_>],
        printer: &mut Printer<E, W>,
        input_format: Option<&str>,
    ) -> Result<(), EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        let mut total_processed_docs: u32 = 0;

        let parsed = crate::parser::parse_expression(expression)
            .map_err(|e| EvaluationError::UnsupportedOperation(format!("{:?}", e)))?;

        for input in inputs.iter_mut() {
            let processed_docs = self.evaluate_with_format(
                ctx,
                input.name,
                &mut input.reader,
                parsed.as_deref(),
                printer,
                input_format,
            )?;
            total_processed_docs += processed_docs;
        }

        if total_processed_docs == 0 {
            return self.evaluate_no_input(ctx, parsed.as_deref(), printer);
        }

        Ok(())
    }

    // ── evaluate (per-document incremental loop) ─────────────────────

    /// Process a single input reader document-by-document: decode one
    /// document at a time, stamp metadata, execute the expression
    /// pipeline, and print results.
    ///
    /// Returns the number of documents processed.  On decode errors
    /// (other than end-of-stream), emits a `bad file '{}'` diagnostic
    /// and returns the error.
    ///
    pub fn evaluate<E, W>(
        &mut self,
        ctx: &mut Context,
        filename: &str,
        reader: &mut AnyReader<'_>,
        node: Option<&ExpressionNode>,
        printer: &mut Printer<E, W>,
    ) -> Result<u32, EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        self.evaluate_with_format(ctx, filename, reader, node, printer, None)
    }

    pub fn evaluate_with_format<E, W>(
        &mut self,
        ctx: &mut Context,
        filename: &str,
        reader: &mut AnyReader<'_>,
        node: Option<&ExpressionNode>,
        printer: &mut Printer<E, W>,
        input_format: Option<&str>,
    ) -> Result<u32, EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        let format = input_format.unwrap_or_else(|| format_string_from_filename(filename));
        let eval_ctx = Self::inherited_context(ctx);

        let current_index = match streaming_decoder::stream_kind(format) {
            streaming_decoder::StreamKind::Json => {
                self.evaluate_streaming_format(ctx, &eval_ctx, filename, reader, node, printer)?
            }
            streaming_decoder::StreamKind::NonStreaming => {
                let bytes = reader.read_all().map_err(EvaluationError::Core)?;
                let source = String::from_utf8(bytes).map_err(|_| {
                    EvaluationError::Core(CoreError::System(
                        crate::errors::SystemError::InvalidUtf8,
                    ))
                })?;

                if source.trim().is_empty() {
                    self.file_index += 1;
                    return Ok(0);
                }

                self.evaluate_non_streaming_format(
                    ctx, &eval_ctx, filename, format, &source, node, printer,
                )?
            }
        };

        self.file_index += 1;
        Ok(current_index)
    }

    /// Process a streaming-format (JSON) source: decode events,
    /// split by document markers, build trees, and evaluate each
    /// document.
    fn evaluate_streaming_format<E, W>(
        &mut self,
        ctx: &mut Context,
        eval_ctx: &Context,
        filename: &str,
        reader: &mut AnyReader<'_>,
        node: Option<&ExpressionNode>,
        printer: &mut Printer<E, W>,
    ) -> Result<u32, EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        let Some(mut decoded) = decode_streaming_document_reader(reader).map_err(|err| {
            self.emit_bad_file_diagnostic(ctx, filename, &err);
            EvaluationError::Core(CoreError::Parse(crate::errors::ParseError::InvalidSyntax))
        })?
        else {
            return Ok(0);
        };

        decoded
            .store
            .set_document_meta_if_absent(0, filename, self.file_index);
        if let Some(root_node) = decoded.store.get_mut(decoded.root) {
            if root_node.document == 0 {
                root_node.document = 0;
            }
        }
        decoded.store.discard_value_index();

        self.evaluate_and_print_one(ctx, eval_ctx, &decoded.store, decoded.root, node, printer)?;
        Ok(1)
    }

    /// Process a non-streaming format (YAML, TOML, CSV, etc.): decode
    /// the entire source, handling multi-document formats (e.g. YAML with
    /// `---` separators), stamp metadata, evaluate each document, and print.
    fn evaluate_non_streaming_format<E, W>(
        &mut self,
        ctx: &mut Context,
        eval_ctx: &Context,
        filename: &str,
        format: &str,
        source: &str,
        node: Option<&ExpressionNode>,
        printer: &mut Printer<E, W>,
    ) -> Result<u32, EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        let codec = CodecService::new();
        let decoded_docs = codec.decode_all(format, source).map_err(|err| {
            self.emit_bad_file_diagnostic(ctx, filename, &err.to_string());
            err
        })?;

        if decoded_docs.is_empty() {
            return Ok(0);
        }

        let mut current_index: u32 = 0;
        for (doc_index, mut decoded) in decoded_docs.into_iter().enumerate() {
            // Stamp file-level metadata on the root.
            decoded
                .store
                .set_document_meta_if_absent(doc_index as u32, filename, self.file_index);
            if let Some(root_node) = decoded.store.get_mut(decoded.root) {
                if root_node.document == 0 {
                    root_node.document = doc_index as u32;
                }
            }
            decoded.store.discard_value_index();

            self.evaluate_and_print_one(
                ctx,
                eval_ctx,
                &decoded.store,
                decoded.root,
                node,
                printer,
            )?;
            current_index += 1;
        }

        Ok(current_index)
    }

    /// Execute the expression pipeline against a single document root
    /// and print the results.
    fn evaluate_and_print_one<E, W>(
        &mut self,
        ctx: &mut Context,
        _eval_ctx: &Context,
        store: &TreeStore,
        root: NodeId,
        node: Option<&ExpressionNode>,
        printer: &mut Printer<E, W>,
    ) -> Result<(), EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        let root_meta = store.get(root).map(|node| {
            (
                node.document,
                store.file_index_for(root).unwrap_or_default(),
                store.filename_for(root).unwrap_or_default().to_owned(),
                node.evaluate_together(),
            )
        });

        if let Some(matches) = direct_lookup_matches(store, root, node)? {
            if matches.is_empty() {
                print_value_result(ctx, printer, &Value::Null, root_meta.as_ref(), 0)?;
            } else {
                printer
                    .print_results(ctx, store, &matches)
                    .map_err(EvaluationError::Core)?;
            }
            return Ok(());
        }

        if let Some(matches) = direct_traversal_matches(store, root, node)? {
            printer
                .print_results(ctx, store, &matches)
                .map_err(EvaluationError::Core)?;
            return Ok(());
        }

        if let Some(values) = direct_unary_values(store, root, node)? {
            let mut result_index = 0_u32;
            for value in values {
                print_value_result(ctx, printer, &value, root_meta.as_ref(), result_index)?;
                result_index = result_index.saturating_add(1);
            }
            return Ok(());
        }

        let evaluator = AllAtOnceEvaluator::new();
        let mut result_index = 0_u32;
        evaluator.evaluate_tree_into(store, root, node, |value| {
            print_value_result(ctx, printer, &value, root_meta.as_ref(), result_index)?;
            result_index = result_index.saturating_add(1);
            Ok(())
        })?;

        Ok(())
    }

    /// Emit a `bad file '{}'` diagnostic when a document fails to decode.
    fn emit_bad_file_diagnostic(&self, ctx: &Context, filename: &str, error_msg: &str) {
        if let Some(diagnostics) = &ctx.diagnostics {
            let mut d = diagnostics.borrow_mut();
            if d.message.is_empty() {
                d.set_message(
                    DiagnosticStage::Decode,
                    format!("bad file '{}': {}", filename, error_msg),
                );
            }
            if d.location.filename.is_empty() {
                d.location.filename = filename.to_string();
            }
        }
    }

    // ── evaluate_events (backward-compatible event-based path) ───────

    /// Evaluate an expression against pre-decoded streaming events.
    ///
    /// This is a convenience path for callers that already have events
    /// (e.g. from an in-memory source).  For reader-based evaluation,
    /// prefer [`evaluate_readers`] or [`evaluate`].
    pub fn evaluate_events(
        &self,
        expression: Option<&ExpressionNode>,
        events: &[StreamingEvent],
    ) -> Result<Vec<Value>, EvaluationError> {
        let documents = split_event_documents(events);
        let evaluator = AllAtOnceEvaluator::new();
        let mut results = Vec::with_capacity(documents.len().max(1));

        for document in documents {
            let mut decoded = build_tree_from_events(document)?;
            if let Some((filename, file_index, document_index)) = document_metadata(document) {
                merge_document_meta(
                    &mut decoded.store,
                    decoded.root,
                    &filename,
                    file_index,
                    document_index,
                )?;
            }
            decoded.store.discard_value_index();
            evaluator.evaluate_tree_into(&decoded.store, decoded.root, expression, |value| {
                results.push(value);
                Ok(())
            })?;
        }

        if results.is_empty() {
            evaluator.evaluate_many_into(&[Value::Null], expression, |value| {
                results.push(value);
                Ok(())
            })?;
        }

        Ok(results)
    }
}

// ── Event document splitting ────────────────────────────────────────

/// Split a flat slice of [`StreamingEvent`]s into per-document slices
/// using `DocStart`/`DocEnd` markers (and `ParseError` as a single-event
/// document).
fn split_event_documents(events: &[StreamingEvent]) -> Vec<&[StreamingEvent]> {
    if events.is_empty() {
        return Vec::new();
    }

    let mut documents = Vec::new();
    let mut start = 0usize;
    let mut seen_doc_marker = false;

    for (index, event) in events.iter().enumerate() {
        match event {
            StreamingEvent::DocStart(_) => {
                seen_doc_marker = true;
                start = index;
            }
            StreamingEvent::DocEnd(_) => {
                seen_doc_marker = true;
                documents.push(&events[start..=index]);
                start = index + 1;
            }
            StreamingEvent::ParseError { .. } => {
                seen_doc_marker = true;
                documents.push(&events[index..=index]);
                start = index + 1;
            }
            _ => {}
        }
    }

    if documents.is_empty() && !seen_doc_marker {
        documents.push(events);
    }

    if start < events.len() && seen_doc_marker {
        documents.push(&events[start..]);
    }

    documents.retain(|document| !document.is_empty());
    documents
}

// ── Document metadata helpers ───────────────────────────────────────

/// Extract aggregated document metadata from a slice of events.
fn document_metadata(events: &[StreamingEvent]) -> Option<(String, i32, u32)> {
    let mut aggregated = None;

    for meta in events.iter().filter_map(event_meta) {
        let current = aggregated
            .get_or_insert_with(|| (meta.filename.clone(), meta.file_index, meta.document));
        if !meta.filename.is_empty() {
            current.0 = meta.filename.clone();
        }
        if meta.file_index != 0 || current.1 == 0 {
            current.1 = meta.file_index;
        }
        if meta.document != 0 || current.2 == 0 {
            current.2 = meta.document;
        }
    }

    aggregated
}

fn event_meta(event: &StreamingEvent) -> Option<&crate::stream::Meta> {
    match event {
        StreamingEvent::DocStart(meta)
        | StreamingEvent::DocEnd(meta)
        | StreamingEvent::MapStart(meta)
        | StreamingEvent::MapEnd(meta)
        | StreamingEvent::SeqStart(meta)
        | StreamingEvent::SeqEnd(meta) => Some(meta),
        StreamingEvent::MapKey { meta, .. }
        | StreamingEvent::Alias { meta, .. }
        | StreamingEvent::ParseError { meta, .. }
        | StreamingEvent::Scalar { meta, .. } => Some(meta),
    }
}

/// Recursively stamp `filename`, `file_index`, and `document` on a tree
/// node and all its descendants.
fn merge_document_meta(
    store: &mut TreeStore,
    id: NodeId,
    filename: &str,
    file_index: i32,
    document: u32,
) -> Result<(), EvaluationError> {
    let children = {
        let node = store
            .get_mut(id)
            .ok_or(CoreError::Eval(crate::errors::EvalError::MissingTreeNode))?;
        if document != 0 || node.document == 0 {
            node.document = document;
        }
        node.content.clone()
    };
    store.set_document_meta_if_absent(document, filename, file_index);
    for child in children {
        merge_document_meta(store, child, filename, file_index, document)?;
    }
    Ok(())
}

// ── Tree-node / Value conversion ────────────────────────────────────

#[cfg(test)]
fn tree_to_value(store: &TreeStore, id: NodeId) -> Result<Value, EvaluationError> {
    let node = store
        .get(id)
        .ok_or(CoreError::Eval(crate::errors::EvalError::MissingTreeNode))?;
    match node.kind {
        TreeNodeKind::Sequence => node
            .content
            .iter()
            .map(|child| tree_to_value(store, *child))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        TreeNodeKind::Mapping => {
            let mut out = std::collections::BTreeMap::new();
            for pair in node.content.chunks(2) {
                if pair.len() != 2 {
                    return Err(CoreError::Parse(crate::errors::ParseError::InvalidSyntax).into());
                }
                store
                    .get(pair[0])
                    .ok_or(CoreError::Eval(crate::errors::EvalError::MissingTreeNode))?;
                out.insert(
                    store.value_string_for(pair[0])?,
                    tree_to_value(store, pair[1])?,
                );
            }
            Ok(Value::Object(out))
        }
        TreeNodeKind::Scalar => scalar_node_to_value(store, id),
        TreeNodeKind::Alias | TreeNodeKind::Unknown => Err(EvaluationError::UnsupportedOperation(
            "tree node".to_string(),
        )),
    }
}

#[cfg(test)]
fn scalar_node_to_value(store: &TreeStore, id: NodeId) -> Result<Value, EvaluationError> {
    if store.resolved_sem_type_for(id)? == Some(crate::language::SemType::Int)
        && store.value_for(id)?.parse::<i64>().is_err()
    {
        return Ok(Value::String(store.value_string_for(id)?));
    }

    match store.value_rep_for(id)? {
        crate::tree::ValueRep::Nil => Ok(Value::Null),
        crate::tree::ValueRep::Boolean(value) => Ok(Value::Bool(value)),
        crate::tree::ValueRep::Int(value) => Ok(Value::Number(Numeric::Int(value))),
        crate::tree::ValueRep::Float(value) => Ok(Value::Number(Numeric::Float(value))),
        crate::tree::ValueRep::Str(value) => {
            if store.resolved_sem_type_for(id)? == Some(crate::language::SemType::Nil) {
                Ok(Value::Null)
            } else {
                Ok(Value::String(value))
            }
        }
    }
}

fn direct_lookup_matches(
    store: &TreeStore,
    root: NodeId,
    node: Option<&ExpressionNode>,
) -> Result<Option<Vec<NodeId>>, EvaluationError> {
    let mut path = Vec::new();
    if !collect_direct_lookup_path(node, &mut path) {
        return Ok(None);
    }
    if path.is_empty() {
        return Ok(Some(vec![root]));
    }
    let found = store.find_descendant_by_path(root, &path, false)?;
    Ok(Some(found.into_iter().collect()))
}

fn direct_traversal_matches(
    store: &TreeStore,
    root: NodeId,
    node: Option<&ExpressionNode>,
) -> Result<Option<Vec<NodeId>>, EvaluationError> {
    let Some(node) = node else {
        return Ok(Some(vec![root]));
    };
    if collect_direct_lookup_path(Some(node), &mut Vec::new()) {
        return Ok(None);
    }
    evaluate_direct_traversal_expr(store, vec![root], node)
}

fn direct_unary_values(
    store: &TreeStore,
    root: NodeId,
    node: Option<&ExpressionNode>,
) -> Result<Option<Vec<Value>>, EvaluationError> {
    let Some(node) = node else {
        return Ok(None);
    };
    let unary_id = node.operation.operation_type.id;
    if unary_id != OperationId::Length || node.lhs.is_some() || node.rhs.is_some() {
        match node.operation.operation_type.id {
            OperationId::Pipe | OperationId::ShortPipe => {
                let Some(lhs) = node.lhs.as_deref() else {
                    return Ok(None);
                };
                let Some(rhs) = node.rhs.as_deref() else {
                    return Ok(None);
                };
                if rhs.operation.operation_type.id != OperationId::Length
                    || rhs.lhs.is_some()
                    || rhs.rhs.is_some()
                {
                    return Ok(None);
                }
                let inputs = if let Some(matches) = direct_lookup_matches(store, root, Some(lhs))? {
                    matches
                } else if let Some(matches) =
                    evaluate_direct_traversal_expr(store, vec![root], lhs)?
                {
                    matches
                } else {
                    return Ok(None);
                };
                return direct_length_values_for_nodes(store, &inputs).map(Some);
            }
            _ => return Ok(None),
        }
    }
    direct_length_values_for_nodes(store, &[root]).map(Some)
}

fn direct_length_values_for_nodes(
    store: &TreeStore,
    node_ids: &[NodeId],
) -> Result<Vec<Value>, EvaluationError> {
    let mut out = Vec::with_capacity(node_ids.len());
    for &node_id in node_ids {
        let Some(node) = store.get(node_id) else {
            return Ok(Vec::new());
        };
        let length = match node.kind {
            TreeNodeKind::Sequence => node.content.len(),
            TreeNodeKind::Mapping => node.content.len() / 2,
            TreeNodeKind::Scalar => match store.value_rep_for(node_id)? {
                crate::tree::ValueRep::Nil => 0,
                crate::tree::ValueRep::Boolean(_) => 1,
                crate::tree::ValueRep::Int(value) => value.to_string().len(),
                crate::tree::ValueRep::Float(value) => value.to_string().len(),
                crate::tree::ValueRep::Str(value) => value.chars().count(),
            },
            TreeNodeKind::Alias | TreeNodeKind::Unknown => return Ok(Vec::new()),
        };
        out.push(Value::Number(Numeric::Int(length as i64)));
    }
    Ok(out)
}

fn evaluate_direct_traversal_expr(
    store: &TreeStore,
    current: Vec<NodeId>,
    node: &ExpressionNode,
) -> Result<Option<Vec<NodeId>>, EvaluationError> {
    match node.operation.operation_type.id {
        OperationId::SelfRef => Ok(Some(current)),
        OperationId::TraversePath => {
            let inputs = if let Some(lhs) = node.lhs.as_deref() {
                let Some(inputs) = evaluate_direct_traversal_expr(store, current, lhs)? else {
                    return Ok(None);
                };
                inputs
            } else {
                current
            };
            let Some(segment) = parse_traverse_path_segment(node) else {
                return Ok(None);
            };
            let Some(outputs) = traverse_path_segment_nodes(store, inputs, &segment)? else {
                return Ok(None);
            };
            Ok(Some(outputs))
        }
        OperationId::TraverseArray => {
            let inputs = if let Some(lhs) = node.lhs.as_deref() {
                let Some(inputs) = evaluate_direct_traversal_expr(store, current, lhs)? else {
                    return Ok(None);
                };
                inputs
            } else {
                current
            };
            traverse_array_segment_nodes(store, inputs, node)
        }
        OperationId::Pipe | OperationId::ShortPipe => {
            let Some(lhs) = node.lhs.as_deref() else {
                return Ok(None);
            };
            let Some(rhs) = node.rhs.as_deref() else {
                return Ok(None);
            };
            let Some(inputs) = evaluate_direct_traversal_expr(store, current, lhs)? else {
                return Ok(None);
            };
            evaluate_direct_traversal_expr(store, inputs, rhs)
        }
        _ => Ok(None),
    }
}

fn traverse_path_segment_nodes(
    store: &TreeStore,
    inputs: Vec<NodeId>,
    segment: &ParsedKey,
) -> Result<Option<Vec<NodeId>>, EvaluationError> {
    let mut out = Vec::with_capacity(inputs.len());
    for node_id in inputs {
        let Some(found) =
            store.find_descendant_by_path(node_id, std::slice::from_ref(segment), false)?
        else {
            return Ok(None);
        };
        out.push(found);
    }
    Ok(Some(out))
}

fn traverse_array_segment_nodes(
    store: &TreeStore,
    inputs: Vec<NodeId>,
    node: &ExpressionNode,
) -> Result<Option<Vec<NodeId>>, EvaluationError> {
    let Some(rhs) = node.rhs.as_deref() else {
        return Ok(None);
    };

    if rhs.operation.operation_type.id == OperationId::Collect
        && rhs
            .rhs
            .as_deref()
            .is_some_and(|inner| inner.operation.operation_type.id == OperationId::Empty)
    {
        let mut out = Vec::new();
        for node_id in inputs {
            let Some(current) = store.get(node_id) else {
                return Ok(None);
            };
            if current.kind != TreeNodeKind::Sequence {
                return Ok(None);
            }
            out.extend(current.content.iter().copied());
        }
        return Ok(Some(out));
    }

    let Some(segment) = parse_traverse_array_segment(node) else {
        return Ok(None);
    };
    let ParsedKey::Int(index) = segment else {
        return Ok(None);
    };
    if index < 0 {
        return Ok(None);
    }

    let mut out = Vec::with_capacity(inputs.len());
    for node_id in inputs {
        let Some(current) = store.get(node_id) else {
            return Ok(None);
        };
        if current.kind != TreeNodeKind::Sequence {
            return Ok(None);
        }
        let Some(child_id) = current.content.get(index as usize).copied() else {
            return Ok(None);
        };
        out.push(child_id);
    }
    Ok(Some(out))
}

fn collect_direct_lookup_path(node: Option<&ExpressionNode>, path: &mut Vec<ParsedKey>) -> bool {
    let Some(node) = node else {
        return true;
    };

    match node.operation.operation_type.id {
        OperationId::SelfRef => node.lhs.is_none() && node.rhs.is_none(),
        OperationId::TraversePath => {
            if let Some(lhs) = node.lhs.as_deref() {
                if !collect_direct_lookup_path(Some(lhs), path) {
                    return false;
                }
            }
            let Some(segment) = parse_traverse_path_segment(node) else {
                return false;
            };
            path.push(segment);
            true
        }
        OperationId::TraverseArray => {
            if let Some(lhs) = node.lhs.as_deref() {
                if !collect_direct_lookup_path(Some(lhs), path) {
                    return false;
                }
            }
            let Some(segment) = parse_traverse_array_segment(node) else {
                return false;
            };
            path.push(segment);
            true
        }
        OperationId::ShortPipe | OperationId::Pipe => {
            collect_direct_lookup_path(node.lhs.as_deref(), path)
                && collect_direct_lookup_path(node.rhs.as_deref(), path)
        }
        _ => false,
    }
}

fn parse_traverse_path_segment(node: &ExpressionNode) -> Option<ParsedKey> {
    if let Some(rhs) = node.rhs.as_deref() {
        return parse_direct_lookup_literal_segment(rhs);
    }
    Some(parse_traverse_segment_text(&node.operation.string_value))
}

fn parse_traverse_array_segment(node: &ExpressionNode) -> Option<ParsedKey> {
    let rhs = node.rhs.as_deref()?;
    if rhs.operation.operation_type.id == OperationId::Collect {
        return rhs
            .rhs
            .as_deref()
            .and_then(parse_direct_lookup_literal_segment);
    }
    parse_direct_lookup_literal_segment(rhs)
}

fn parse_direct_lookup_literal_segment(node: &ExpressionNode) -> Option<ParsedKey> {
    if node.operation.operation_type.id != OperationId::Value {
        return None;
    }
    match Value::from_literal(&node.operation.string_value).ok()? {
        Value::String(value) => Some(ParsedKey::Str(value)),
        Value::Number(Numeric::Int(value)) => Some(ParsedKey::Int(value)),
        Value::Number(Numeric::Float(value))
            if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 =>
        {
            Some(ParsedKey::Int(value as i64))
        }
        _ => None,
    }
}

fn parse_traverse_segment_text(raw: &str) -> ParsedKey {
    raw.parse::<i64>()
        .map(ParsedKey::Int)
        .unwrap_or_else(|_| ParsedKey::Str(raw.to_owned()))
}

pub(super) fn print_value_result<E, W>(
    ctx: &mut Context,
    printer: &mut Printer<E, W>,
    value: &Value,
    root_meta: Option<&(u32, i32, String, bool)>,
    result_index: u32,
) -> Result<(), EvaluationError>
where
    E: Encoder,
    W: PrinterWriter,
{
    if printer.can_print_evaluated_value() {
        return printer
            .print_evaluated_value(ctx, value)
            .map_err(EvaluationError::Core);
    }

    let mut result_store = TreeStore::new();
    let result_id = value_to_tree_node(&mut result_store, value)?;
    if let Some(node) = result_store.get_mut(result_id) {
        if let Some(meta) = root_meta {
            node.document = meta.0;
            node.set_evaluate_together(meta.3);
            result_store.set_document_meta(meta.0, meta.2.clone(), meta.1);
        } else {
            node.document = result_index;
            node.set_evaluate_together(true);
            result_store.set_document_meta(result_index, "", 0);
        }
    }
    printer
        .print_results(ctx, &result_store, &[result_id])
        .map_err(EvaluationError::Core)
}

fn decode_streaming_document_reader(
    reader: &mut AnyReader<'_>,
) -> Result<Option<DecodedDocument>, String> {
    const CHUNK_SIZE: usize = 64 * 1024;

    let mut builder = tree_builder::Builder::new();
    let mut parser = streaming_json::StreamingParser::with_sink(false, false, &mut builder);
    let mut saw_non_whitespace = false;
    let mut chunk = [0_u8; CHUNK_SIZE];

    loop {
        let read = reader.read(&mut chunk).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        if !saw_non_whitespace {
            saw_non_whitespace = chunk[..read].iter().any(|byte| !byte.is_ascii_whitespace());
        }
        parser
            .feed_bytes(&chunk[..read])
            .map_err(|_| "streaming decoder parse failed".to_owned())?;
    }

    if !saw_non_whitespace {
        return Ok(None);
    }

    parser
        .finish_without_events()
        .map_err(|_| "streaming decoder parse failed".to_owned())?;
    builder
        .into_document()
        .map(Some)
        .map_err(|err| err.to_string())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Read as _, mem::size_of};

    use super::*;
    use crate::stream::Meta;
    use crate::{
        core::{
            Context as CoreContext, CoreError, SemType, TreeNode, VecPrinterWriter,
            io_adapters::reader_from_pointer, printer::Encoder as PrintEncoder,
        },
        parser::parse_expression,
    };

    struct DirectOnlyEncoder;

    impl PrintEncoder for DirectOnlyEncoder {
        fn encode(
            &self,
            _ctx: &mut CoreContext,
            _node: NodeId,
            _writer: &mut dyn std::io::Write,
        ) -> Result<(), CoreError> {
            panic!("store-backed encode should not be called");
        }

        fn encode_evaluated_value(
            &self,
            value: &crate::evaluator::Value,
            writer: &mut dyn std::io::Write,
        ) -> Result<bool, CoreError> {
            match value {
                crate::evaluator::Value::String(value) => writer.write_all(value.as_bytes())?,
                crate::evaluator::Value::Number(value) => {
                    writer.write_all(value.to_string().as_bytes())?
                }
                crate::evaluator::Value::Bool(value) => {
                    writer.write_all(if *value { b"true" } else { b"false" })?
                }
                crate::evaluator::Value::Null => writer.write_all(b"null")?,
                _ => writer.write_all(b"<complex>")?,
            }
            Ok(true)
        }
    }

    #[test]
    fn evaluates_scalar_stream_events() {
        let expression = parse_expression("self + 1")
            .expect("parse should succeed")
            .expect("tree should exist");
        let events = vec![StreamingEvent::Scalar {
            value: "41".to_string(),
            meta: Meta {
                sem_type: Some(SemType::Int),
                ..Meta::default()
            },
        }];

        let results = StreamEvaluator::new()
            .evaluate_events(Some(&expression), &events)
            .expect("stream evaluation should succeed");

        assert_eq!(results, vec![Value::Number(Numeric::Int(42))]);
    }

    #[test]
    fn evaluates_reader_inputs_by_filename_format() {
        let expression = parse_expression(".foo")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut reader = std::io::Cursor::new(br#"{"foo": 7}"#.to_vec());
        let mut inputs = [ReaderInput::new(
            "in.json",
            reader_from_pointer(&mut reader),
        )];

        let results = StreamEvaluator::new()
            .evaluate_events_from_readers(Some(&expression), &mut inputs)
            .expect("reader evaluation should succeed");

        assert_eq!(results, vec![Value::Number(Numeric::Int(7))]);
    }

    #[test]
    fn direct_lookup_matches_literal_path_segments() {
        let expression = parse_expression(".foo.bar")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut store = TreeStore::new();
        let root_id = store.add(TreeNode {
            kind: TreeNodeKind::Mapping,
            sem_type: Some(SemType::Map),
            ..TreeNode::default()
        });
        let (_, foo_value_id) = store
            .add_key_value_child(
                root_id,
                TreeNode::scalar(SemType::Str, "foo"),
                TreeNode {
                    kind: TreeNodeKind::Mapping,
                    sem_type: Some(SemType::Map),
                    ..TreeNode::default()
                },
            )
            .expect("foo entry");
        let (_, bar_value_id) = store
            .add_key_value_child(
                foo_value_id,
                TreeNode::scalar(SemType::Str, "bar"),
                TreeNode::scalar(SemType::Int, "7"),
            )
            .expect("bar entry");

        let matches = direct_lookup_matches(&store, root_id, Some(&expression))
            .expect("direct path lookup should succeed")
            .expect("pure path should use direct lookup");
        assert_eq!(matches.len(), 1);
        assert_eq!(store.value_for(matches[0]).unwrap(), "7");
        assert_eq!(matches[0], bar_value_id);
    }

    #[test]
    fn direct_lookup_matches_array_index_segments() {
        let expression = parse_expression(".items[1].name")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut store = TreeStore::new();
        let root_id = store.add(TreeNode {
            kind: TreeNodeKind::Mapping,
            sem_type: Some(SemType::Map),
            ..TreeNode::default()
        });
        let (_, items_id) = store
            .add_key_value_child(
                root_id,
                TreeNode::scalar(SemType::Str, "items"),
                TreeNode {
                    kind: TreeNodeKind::Sequence,
                    sem_type: Some(SemType::Seq),
                    ..TreeNode::default()
                },
            )
            .expect("items entry");
        let first_id = store
            .add_child(items_id, TreeNode::scalar(SemType::Str, "first"))
            .expect("first array item");
        let second_map_id = store
            .add_child(
                items_id,
                TreeNode {
                    kind: TreeNodeKind::Mapping,
                    sem_type: Some(SemType::Map),
                    ..TreeNode::default()
                },
            )
            .expect("second array item");
        let (_, name_value_id) = store
            .add_key_value_child(
                second_map_id,
                TreeNode::scalar(SemType::Str, "name"),
                TreeNode::scalar(SemType::Str, "target"),
            )
            .expect("name entry");

        let matches = direct_lookup_matches(&store, root_id, Some(&expression))
            .expect("direct path lookup should succeed")
            .expect("pure array path should use direct lookup");
        assert_eq!(store.value_for(first_id).unwrap(), "first");
        assert_eq!(matches, vec![name_value_id]);
    }

    #[test]
    fn direct_lookup_matches_literal_rhs_segments() {
        let expression = parse_expression(".foo[\"bar\"]")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut store = TreeStore::new();
        let root_id = store.add(TreeNode {
            kind: TreeNodeKind::Mapping,
            sem_type: Some(SemType::Map),
            ..TreeNode::default()
        });
        let (_, foo_value_id) = store
            .add_key_value_child(
                root_id,
                TreeNode::scalar(SemType::Str, "foo"),
                TreeNode {
                    kind: TreeNodeKind::Mapping,
                    sem_type: Some(SemType::Map),
                    ..TreeNode::default()
                },
            )
            .expect("foo entry");
        let (_, bar_value_id) = store
            .add_key_value_child(
                foo_value_id,
                TreeNode::scalar(SemType::Str, "bar"),
                TreeNode::scalar(SemType::Int, "9"),
            )
            .expect("bar entry");

        let matches = direct_lookup_matches(&store, root_id, Some(&expression))
            .expect("direct path lookup should succeed")
            .expect("literal rhs path should use direct lookup");
        assert_eq!(matches, vec![bar_value_id]);
    }

    #[test]
    fn direct_lookup_rejects_non_path_expressions() {
        let expression = parse_expression(".foo + 1")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut store = TreeStore::new();
        let root_id = value_to_tree_node(&mut store, &Value::Null).expect("null root");

        assert!(
            direct_lookup_matches(&store, root_id, Some(&expression))
                .expect("lookup should not error")
                .is_none()
        );
    }

    #[test]
    fn direct_traversal_matches_sequence_splat_chain() {
        let expression = parse_expression(".items[].name")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut store = TreeStore::new();
        let root_id = store.add(TreeNode {
            kind: TreeNodeKind::Mapping,
            sem_type: Some(SemType::Map),
            ..TreeNode::default()
        });
        let (_, items_id) = store
            .add_key_value_child(
                root_id,
                TreeNode::scalar(SemType::Str, "items"),
                TreeNode {
                    kind: TreeNodeKind::Sequence,
                    sem_type: Some(SemType::Seq),
                    ..TreeNode::default()
                },
            )
            .expect("items entry");
        let first_map_id = store
            .add_child(
                items_id,
                TreeNode {
                    kind: TreeNodeKind::Mapping,
                    sem_type: Some(SemType::Map),
                    ..TreeNode::default()
                },
            )
            .expect("first map");
        let second_map_id = store
            .add_child(
                items_id,
                TreeNode {
                    kind: TreeNodeKind::Mapping,
                    sem_type: Some(SemType::Map),
                    ..TreeNode::default()
                },
            )
            .expect("second map");
        let (_, first_name_id) = store
            .add_key_value_child(
                first_map_id,
                TreeNode::scalar(SemType::Str, "name"),
                TreeNode::scalar(SemType::Str, "alpha"),
            )
            .expect("first name");
        let (_, second_name_id) = store
            .add_key_value_child(
                second_map_id,
                TreeNode::scalar(SemType::Str, "name"),
                TreeNode::scalar(SemType::Str, "beta"),
            )
            .expect("second name");

        let matches = direct_traversal_matches(&store, root_id, Some(&expression))
            .expect("direct traversal should succeed")
            .expect("pure traversal chain should use direct traversal");
        assert_eq!(matches, vec![first_name_id, second_name_id]);
    }

    #[test]
    fn direct_traversal_rejects_shape_mismatch_and_falls_back() {
        let expression = parse_expression(".items[].name")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut store = TreeStore::new();
        let root_id = store.add(TreeNode {
            kind: TreeNodeKind::Mapping,
            sem_type: Some(SemType::Map),
            ..TreeNode::default()
        });
        let (_, items_id) = store
            .add_key_value_child(
                root_id,
                TreeNode::scalar(SemType::Str, "items"),
                TreeNode {
                    kind: TreeNodeKind::Sequence,
                    sem_type: Some(SemType::Seq),
                    ..TreeNode::default()
                },
            )
            .expect("items entry");
        store
            .add_child(items_id, TreeNode::scalar(SemType::Str, "not-a-map"))
            .expect("scalar child");

        assert!(
            direct_traversal_matches(&store, root_id, Some(&expression))
                .expect("direct traversal should not error")
                .is_none()
        );
    }

    #[test]
    fn direct_lookup_matches_top_level_array_index() {
        let expression = parse_expression(".[1]")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut store = TreeStore::new();
        let root_id = store.add(TreeNode {
            kind: TreeNodeKind::Sequence,
            sem_type: Some(SemType::Seq),
            ..TreeNode::default()
        });
        let first_id = store
            .add_child(root_id, TreeNode::scalar(SemType::Str, "first"))
            .expect("first item");
        let second_id = store
            .add_child(root_id, TreeNode::scalar(SemType::Str, "second"))
            .expect("second item");

        let matches = direct_lookup_matches(&store, root_id, Some(&expression))
            .expect("direct path lookup should succeed")
            .expect("top-level index should use direct lookup");
        assert_eq!(store.value_for(first_id).unwrap(), "first");
        assert_eq!(matches, vec![second_id]);
    }

    #[test]
    fn direct_traversal_matches_top_level_sequence_splat_chain() {
        let expression = parse_expression(".[] | .name")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut store = TreeStore::new();
        let root_id = store.add(TreeNode {
            kind: TreeNodeKind::Sequence,
            sem_type: Some(SemType::Seq),
            ..TreeNode::default()
        });
        let first_map_id = store
            .add_child(
                root_id,
                TreeNode {
                    kind: TreeNodeKind::Mapping,
                    sem_type: Some(SemType::Map),
                    ..TreeNode::default()
                },
            )
            .expect("first map");
        let second_map_id = store
            .add_child(
                root_id,
                TreeNode {
                    kind: TreeNodeKind::Mapping,
                    sem_type: Some(SemType::Map),
                    ..TreeNode::default()
                },
            )
            .expect("second map");
        let (_, first_name_id) = store
            .add_key_value_child(
                first_map_id,
                TreeNode::scalar(SemType::Str, "name"),
                TreeNode::scalar(SemType::Str, "alpha"),
            )
            .expect("first name");
        let (_, second_name_id) = store
            .add_key_value_child(
                second_map_id,
                TreeNode::scalar(SemType::Str, "name"),
                TreeNode::scalar(SemType::Str, "beta"),
            )
            .expect("second name");

        let matches = direct_traversal_matches(&store, root_id, Some(&expression))
            .expect("direct traversal should succeed")
            .expect("top-level splat chain should use direct traversal");
        assert_eq!(matches, vec![first_name_id, second_name_id]);
    }

    #[test]
    fn direct_unary_length_matches_path_result() {
        let expression = parse_expression(".items | length")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut store = TreeStore::new();
        let root_id = store.add(TreeNode {
            kind: TreeNodeKind::Mapping,
            sem_type: Some(SemType::Map),
            ..TreeNode::default()
        });
        let (_, items_id) = store
            .add_key_value_child(
                root_id,
                TreeNode::scalar(SemType::Str, "items"),
                TreeNode {
                    kind: TreeNodeKind::Sequence,
                    sem_type: Some(SemType::Seq),
                    ..TreeNode::default()
                },
            )
            .expect("items entry");
        store
            .add_child(items_id, TreeNode::scalar(SemType::Str, "a"))
            .expect("first item");
        store
            .add_child(items_id, TreeNode::scalar(SemType::Str, "b"))
            .expect("second item");

        let values = direct_unary_values(&store, root_id, Some(&expression))
            .expect("direct unary should succeed")
            .expect("path + length should use direct unary");
        assert_eq!(values, vec![Value::Number(Numeric::Int(2))]);
    }

    #[test]
    fn direct_unary_length_matches_top_level_splat_results() {
        let expression = parse_expression(".[] | length")
            .expect("parse should succeed")
            .expect("tree should exist");
        let mut store = TreeStore::new();
        let root_id = store.add(TreeNode {
            kind: TreeNodeKind::Sequence,
            sem_type: Some(SemType::Seq),
            ..TreeNode::default()
        });
        let first_map_id = store
            .add_child(
                root_id,
                TreeNode {
                    kind: TreeNodeKind::Mapping,
                    sem_type: Some(SemType::Map),
                    ..TreeNode::default()
                },
            )
            .expect("first map");
        let second_map_id = store
            .add_child(
                root_id,
                TreeNode {
                    kind: TreeNodeKind::Mapping,
                    sem_type: Some(SemType::Map),
                    ..TreeNode::default()
                },
            )
            .expect("second map");
        store
            .add_key_value_child(
                first_map_id,
                TreeNode::scalar(SemType::Str, "name"),
                TreeNode::scalar(SemType::Str, "alpha"),
            )
            .expect("first entry");
        store
            .add_key_value_child(
                second_map_id,
                TreeNode::scalar(SemType::Str, "name"),
                TreeNode::scalar(SemType::Str, "beta"),
            )
            .expect("second entry");
        store
            .add_key_value_child(
                second_map_id,
                TreeNode::scalar(SemType::Str, "extra"),
                TreeNode::scalar(SemType::Str, "gamma"),
            )
            .expect("third entry");

        let values = direct_unary_values(&store, root_id, Some(&expression))
            .expect("direct unary should succeed")
            .expect("splat + length should use direct unary");
        assert_eq!(
            values,
            vec![
                Value::Number(Numeric::Int(1)),
                Value::Number(Numeric::Int(2))
            ]
        );
    }

    #[test]
    fn decode_streaming_document_reader_returns_none_for_whitespace_only_input() {
        let mut cursor = std::io::Cursor::new(b" \n\t\r ".to_vec());
        let mut reader = reader_from_pointer(&mut cursor);

        let decoded = decode_streaming_document_reader(&mut reader)
            .expect("whitespace input should not error");

        assert!(decoded.is_none(), "whitespace-only input should be skipped");
    }

    #[test]
    fn decode_streaming_document_reader_builds_tree_without_document_runtime() {
        let mut cursor = std::io::Cursor::new(br#"{"items":[1,2],"ok":true}"#.to_vec());
        let mut reader = reader_from_pointer(&mut cursor);

        let decoded = decode_streaming_document_reader(&mut reader)
            .expect("json input should decode")
            .expect("json input should produce a document");

        let root = decoded
            .store
            .get(decoded.root)
            .expect("root node should exist");
        assert_eq!(root.kind, TreeNodeKind::Mapping);
    }

    #[test]
    #[ignore = "diagnostic harness for local large-json decode profiling"]
    fn profile_large_json_decode_pipeline_from_env_path() {
        let path = std::env::var("TREEASE_PROFILE_JSON")
            .expect("set TREEASE_PROFILE_JSON to a local large JSON file path");
        let input_bytes = std::fs::metadata(&path)
            .expect("profile input should exist")
            .len();

        const CHUNK_SIZE: usize = 64 * 1024;

        let mut file = File::open(&path).expect("profile input should open");
        let mut builder = tree_builder::Builder::new();
        let mut parser = streaming_json::StreamingParser::with_sink(false, false, &mut builder);
        let mut saw_non_whitespace = false;
        let mut chunk = [0_u8; CHUNK_SIZE];

        crate::test_timing::reset();
        crate::test_timing::mark("profile.decode.start");

        loop {
            let read = file.read(&mut chunk).expect("profile input should read");
            if read == 0 {
                break;
            }
            if !saw_non_whitespace {
                saw_non_whitespace = chunk[..read].iter().any(|byte| !byte.is_ascii_whitespace());
            }
            parser
                .feed_bytes(&chunk[..read])
                .expect("parser feed should succeed");
        }
        crate::test_timing::mark("profile.decode.feed_complete");

        assert!(
            saw_non_whitespace,
            "profile input should not be whitespace only"
        );

        parser
            .finish_without_events()
            .expect("parser finish should succeed");
        crate::test_timing::mark("profile.decode.finish_complete");

        drop(parser);

        let document = builder
            .into_document()
            .expect("builder should produce a decoded document");
        crate::test_timing::mark("profile.decode.document_built");
        crate::test_timing::report();

        let stats = document.store.stats();
        let estimated_node_bytes = stats.node_capacity * size_of::<crate::tree::TreeNode>();
        let estimated_content_bytes = stats.total_content_slots * size_of::<crate::tree::NodeId>();
        eprintln!("--- decode profile ---");
        eprintln!("input_bytes={input_bytes}");
        eprintln!("node_count={}", stats.node_count);
        eprintln!("node_capacity={}", stats.node_capacity);
        eprintln!("scalar_node_count={}", stats.scalar_node_count);
        eprintln!("mapping_node_count={}", stats.mapping_node_count);
        eprintln!("sequence_node_count={}", stats.sequence_node_count);
        eprintln!("alias_node_count={}", stats.alias_node_count);
        eprintln!("unknown_node_count={}", stats.unknown_node_count);
        eprintln!(
            "nodes_with_content_count={}",
            stats.nodes_with_content_count
        );
        eprintln!("total_content_slots={}", stats.total_content_slots);
        eprintln!(
            "nodes_with_stored_value_count={}",
            stats.nodes_with_stored_value_count
        );
        eprintln!(
            "nodes_with_missing_value_count={}",
            stats.nodes_with_missing_value_count
        );
        eprintln!("value_count={}", stats.value_count);
        eprintln!("value_capacity={}", stats.value_capacity);
        eprintln!("interned_value_bytes={}", stats.interned_value_bytes);
        eprintln!("node_extra_count={}", stats.node_extra_count);
        eprintln!("document_meta_count={}", stats.document_meta_count);
        eprintln!("value_index_entry_count={}", stats.value_index_entry_count);
        eprintln!("sizeof<TreeNode>={}", size_of::<crate::tree::TreeNode>());
        eprintln!("sizeof<NodeId>={}", size_of::<crate::tree::NodeId>());
        eprintln!(
            "sizeof<Option<NodeId>>={}",
            size_of::<Option<crate::tree::NodeId>>()
        );
        eprintln!(
            "sizeof<NodeValueRef>={}",
            size_of::<crate::tree::NodeValueRef>()
        );
        eprintln!(
            "sizeof<CompactTag>={}",
            size_of::<crate::tree::CompactTag>()
        );
        eprintln!(
            "sizeof<Option<SemType>>={}",
            size_of::<Option<crate::language::SemType>>()
        );
        eprintln!(
            "sizeof<Vec<NodeId>>={}",
            size_of::<Vec<crate::tree::NodeId>>()
        );
        eprintln!("sizeof<NodeExtra>={}", size_of::<crate::tree::NodeExtra>());
        eprintln!(
            "sizeof<DocumentMeta>={}",
            size_of::<crate::tree::tree_store::DocumentMeta>()
        );
        eprintln!("estimated_node_bytes={estimated_node_bytes}");
        eprintln!("estimated_content_bytes={estimated_content_bytes}");
        eprintln!("--- end decode profile ---");

        assert!(stats.node_count > 0, "decoded tree should contain nodes");
    }

    #[test]
    fn print_value_result_uses_direct_encoder_without_rebuilding_store() {
        let mut registry = crate::init().expect("registry should initialize");
        let result = (|| {
            let mut ctx = CoreContext::empty(registry.handle());
            let writer = VecPrinterWriter::new();
            let mut printer = Printer::new(DirectOnlyEncoder, writer);

            print_value_result(
                &mut ctx,
                &mut printer,
                &Value::String("ok".to_string()),
                None,
                0,
            )
            .expect("direct print should succeed");

            let output = String::from_utf8(printer.into_writer().into_bytes())
                .expect("output should be utf-8");
            assert_eq!(output, "ok");
            Ok::<(), CoreError>(())
        })();
        crate::deinit(&mut registry);
        result.expect("test body should succeed");
    }

    // Helper: the old evaluate_readers that returned Vec<Value> is now
    // replaced by the printer-based path.  This test helper bridges the
    // two so existing tests continue to work.
    impl StreamEvaluator {
        fn evaluate_events_from_readers(
            &self,
            expression: Option<&ExpressionNode>,
            inputs: &mut [ReaderInput<'_>],
        ) -> Result<Vec<Value>, EvaluationError> {
            let codec = CodecService::new();
            let mut values = Vec::with_capacity(inputs.len().max(1));

            for (file_index, input) in inputs.iter_mut().enumerate() {
                let bytes = input.reader.read_all()?;
                let source = String::from_utf8(bytes)
                    .map_err(|_| CoreError::System(crate::errors::SystemError::InvalidUtf8))?;
                if source.trim().is_empty() {
                    continue;
                }
                let format = format_string_from_filename(input.name);

                match streaming_decoder::stream_kind(format) {
                    streaming_decoder::StreamKind::Json => {
                        let mut decoded = streaming_decoder::decode_to_tree(
                            &crate::registry::RegistryOwner::init_owned(),
                            format,
                            &source,
                            crate::stream::DecodeOptions::default(),
                        )
                        .map_err(EvaluationError::Core)?;
                        decoded
                            .store
                            .set_document_meta_if_absent(0, input.name, file_index as i32);
                        if let Some(root_node) = decoded.store.get_mut(decoded.root) {
                            if root_node.document == 0 {
                                root_node.document = 0;
                            }
                        }
                        values.push(tree_to_value(&decoded.store, decoded.root)?);
                    }
                    streaming_decoder::StreamKind::NonStreaming => {
                        let decoded_docs = codec.decode_all(format, &source)?;
                        for (document_index, mut decoded) in decoded_docs.into_iter().enumerate() {
                            decoded.store.set_document_meta_if_absent(
                                document_index as u32,
                                input.name,
                                file_index as i32,
                            );
                            if let Some(root_node) = decoded.store.get_mut(decoded.root) {
                                if root_node.document == 0 {
                                    root_node.document = document_index as u32;
                                }
                            }
                            values.push(tree_to_value(&decoded.store, decoded.root)?);
                        }
                    }
                }
            }

            if values.is_empty() {
                values.push(Value::Null);
            }

            Ok(crate::expression_pipeline::execute_many(
                &values, expression,
            )?)
        }
    }
}
