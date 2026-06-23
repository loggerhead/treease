use crate::{
    core::{
        CodecService, CoreError,
        context::Context,
        diagnostics::DiagnosticStage,
        expression::ExpressionNode,
        format::format_string_from_filename,
        io_adapters::AnyReader,
        printer::{Encoder, Printer},
        printer_writer::PrinterWriter,
        sem_type::SemType,
        tree_navigator::TreeEngine,
        tree_node::{NodeId, TreeNode, TreeNodeKind},
        tree_store::TreeStore,
    },
    expression_pipeline,
    stream::{build_tree_from_events, streaming_decoder, streaming_events::StreamingEvent},
};

use super::all_at_once_evaluator::{AllAtOnceEvaluator, value_to_tree_node};
use super::{EvaluationError, Value};

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
        let values = AllAtOnceEvaluator::new().evaluate_many(&[Value::Null], node)?;
        let mut result_store = TreeStore::new();
        let mut result_ids = Vec::with_capacity(values.len());
        for value in &values {
            result_ids.push(value_to_tree_node(&mut result_store, value)?);
        }

        printer
            .print_results(ctx, &result_store, &result_ids)
            .map_err(EvaluationError::Core)?;

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
            return self.evaluate_new(ctx, expression, printer);
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
        let bytes = reader.read_all().map_err(|e| EvaluationError::Core(e))?;
        let source = String::from_utf8(bytes).map_err(|_| {
            EvaluationError::Core(CoreError::System(
                crate::core::errors::SystemError::InvalidUtf8,
            ))
        })?;

        if source.trim().is_empty() {
            self.file_index += 1;
            return Ok(0);
        }

        let format = input_format.unwrap_or_else(|| format_string_from_filename(filename));
        let eval_ctx = Self::inherited_context(ctx);

        let current_index = match streaming_decoder::stream_kind(format) {
            streaming_decoder::StreamKind::Json => self.evaluate_streaming_format(
                ctx, &eval_ctx, filename, format, &source, node, printer,
            )?,
            streaming_decoder::StreamKind::NonStreaming => self.evaluate_non_streaming_format(
                ctx, &eval_ctx, filename, format, &source, node, printer,
            )?,
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
        format: &str,
        source: &str,
        node: Option<&ExpressionNode>,
        printer: &mut Printer<E, W>,
    ) -> Result<u32, EvaluationError>
    where
        E: Encoder,
        W: PrinterWriter,
    {
        let events = streaming_decoder::decode(format, source).map_err(|err| {
            self.emit_bad_file_diagnostic(ctx, filename, &err.to_string());
            EvaluationError::Core(CoreError::Parse(
                crate::core::errors::ParseError::InvalidSyntax,
            ))
        })?;

        let documents = split_event_documents(&events);
        if documents.is_empty() {
            return Ok(0);
        }

        let mut current_index: u32 = 0;
        for document in documents {
            let mut decoded = build_tree_from_events(document).map_err(|err| {
                self.emit_bad_file_diagnostic(ctx, filename, &err.to_string());
                err
            })?;

            // Stamp document metadata on the root and its descendants.
            if let Some((doc_filename, doc_file_index, doc_document)) = document_metadata(document)
            {
                merge_document_meta(
                    &mut decoded.store,
                    decoded.root,
                    &doc_filename,
                    doc_file_index,
                    doc_document,
                )?;
            }

            // Also stamp the file-level metadata.
            if let Some(root_node) = decoded.store.get_mut(decoded.root) {
                if root_node.filename.is_empty() {
                    root_node.filename = filename.to_owned();
                }
                if root_node.file_index == 0 {
                    root_node.file_index = self.file_index;
                }
                if root_node.document == 0 {
                    root_node.document = current_index;
                }
            }

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
            if let Some(root_node) = decoded.store.get_mut(decoded.root) {
                if root_node.filename.is_empty() {
                    root_node.filename = filename.to_owned();
                }
                if root_node.file_index == 0 {
                    root_node.file_index = self.file_index;
                }
                if root_node.document == 0 {
                    root_node.document = doc_index as u32;
                }
            }

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
        let input = tree_to_value(store, root)?;
        let values = expression_pipeline::execute_many(&[input], node)?;
        let mut result_store = TreeStore::new();
        let mut result_ids = Vec::with_capacity(values.len());
        let root_meta = store.get(root).map(|node| {
            (
                node.document,
                node.file_index,
                node.filename.clone(),
                node.evaluate_together,
            )
        });
        for value in &values {
            let result_id = value_to_tree_node(&mut result_store, value)?;
            if let (Some(meta), Some(node)) = (root_meta.as_ref(), result_store.get_mut(result_id))
            {
                node.document = meta.0;
                node.file_index = meta.1;
                node.filename = meta.2.clone();
                node.evaluate_together = meta.3;
            }
            result_ids.push(result_id);
        }

        printer
            .print_results(ctx, &result_store, &result_ids)
            .map_err(EvaluationError::Core)?;

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
        let mut inputs = Vec::with_capacity(documents.len().max(1));

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
            inputs.push(tree_to_value(&decoded.store, decoded.root)?);
        }

        if inputs.is_empty() {
            inputs.push(Value::Null);
        }

        Ok(expression_pipeline::execute_many(&inputs, expression)?)
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
        let node = store.get_mut(id).ok_or(CoreError::Eval(
            crate::core::errors::EvalError::MissingTreeNode,
        ))?;
        if document != 0 || node.document == 0 {
            node.document = document;
        }
        if !filename.is_empty() || node.filename.is_empty() {
            node.filename = filename.to_owned();
        }
        if file_index != 0 || node.file_index == 0 {
            node.file_index = file_index;
        }
        node.content.clone()
    };
    for child in children {
        merge_document_meta(store, child, filename, file_index, document)?;
    }
    Ok(())
}

// ── Tree-node / Value conversion ────────────────────────────────────

fn tree_to_value(store: &TreeStore, id: NodeId) -> Result<Value, EvaluationError> {
    let node = store.get(id).ok_or(CoreError::Eval(
        crate::core::errors::EvalError::MissingTreeNode,
    ))?;
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
                    return Err(
                        CoreError::Parse(crate::core::errors::ParseError::InvalidSyntax).into(),
                    );
                }
                let key = store.get(pair[0]).ok_or(CoreError::Eval(
                    crate::core::errors::EvalError::MissingTreeNode,
                ))?;
                out.insert(key.value.clone(), tree_to_value(store, pair[1])?);
            }
            Ok(Value::Object(out))
        }
        TreeNodeKind::Scalar => scalar_node_to_value(node),
        TreeNodeKind::Alias | TreeNodeKind::Unknown => Err(EvaluationError::UnsupportedOperation(
            "tree node".to_string(),
        )),
    }
}

fn scalar_node_to_value(node: &TreeNode) -> Result<Value, EvaluationError> {
    match node.get_value_rep()? {
        crate::core::ValueRep::Nil => Ok(Value::Null),
        crate::core::ValueRep::Boolean(value) => Ok(Value::Bool(value)),
        crate::core::ValueRep::Int(value) => Ok(Value::Number(value as f64)),
        crate::core::ValueRep::Float(value) => Ok(Value::Number(value)),
        crate::core::ValueRep::Str(value) => {
            if node.resolved_sem_type() == Some(SemType::Nil) {
                Ok(Value::Null)
            } else {
                Ok(Value::String(value))
            }
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::Meta;
    use crate::{core::SemType, core::io_adapters::reader_from_pointer, parser::parse_expression};

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

        assert_eq!(results, vec![Value::Number(42.0)]);
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

        assert_eq!(results, vec![Value::Number(7.0)]);
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
                let source = String::from_utf8(bytes).map_err(|_| {
                    CoreError::System(crate::core::errors::SystemError::InvalidUtf8)
                })?;
                if source.trim().is_empty() {
                    continue;
                }
                let format = format_string_from_filename(input.name);

                match streaming_decoder::stream_kind(format) {
                    streaming_decoder::StreamKind::Json => {
                        let events = streaming_decoder::decode(format, &source).map_err(|_| {
                            EvaluationError::Core(CoreError::Parse(
                                crate::core::errors::ParseError::InvalidSyntax,
                            ))
                        })?;
                        let documents = split_event_documents(&events);
                        for (document_index, document) in documents.iter().enumerate() {
                            let mut decoded = build_tree_from_events(document)?;
                            if let Some((doc_filename, doc_file_index, doc_document)) =
                                document_metadata(document)
                            {
                                merge_document_meta(
                                    &mut decoded.store,
                                    decoded.root,
                                    &doc_filename,
                                    doc_file_index,
                                    doc_document,
                                )?;
                            }
                            // Also stamp file-level metadata.
                            if let Some(root_node) = decoded.store.get_mut(decoded.root) {
                                if root_node.filename.is_empty() {
                                    root_node.filename = input.name.to_owned();
                                }
                                if root_node.file_index == 0 {
                                    root_node.file_index = file_index as i32;
                                }
                                if root_node.document == 0 {
                                    root_node.document = document_index as u32;
                                }
                            }
                            values.push(tree_to_value(&decoded.store, decoded.root)?);
                        }
                    }
                    streaming_decoder::StreamKind::NonStreaming => {
                        let decoded_docs = codec.decode_all(format, &source)?;
                        for (document_index, mut decoded) in decoded_docs.into_iter().enumerate() {
                            if let Some(root_node) = decoded.store.get_mut(decoded.root) {
                                if root_node.filename.is_empty() {
                                    root_node.filename = input.name.to_owned();
                                }
                                if root_node.file_index == 0 {
                                    root_node.file_index = file_index as i32;
                                }
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

            Ok(expression_pipeline::execute_many(&values, expression)?)
        }
    }
}
