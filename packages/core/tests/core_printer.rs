use std::cell::Cell;
use std::io::Write;

use treease_core::core::{
    Context, CoreError, Encoder, NodeId, Printer, PrinterWriter, SemType, SystemError, TreeNode,
    TreeStore,
};

#[derive(Default)]
struct RecordingWriter {
    bytes: Vec<u8>,
}

impl PrinterWriter for RecordingWriter {
    fn write_for_node(
        &mut self,
        _ctx: &mut Context,
        _node: Option<NodeId>,
        bytes: &[u8],
    ) -> Result<(), CoreError> {
        self.bytes.write_all(bytes)?;
        Ok(())
    }
}

struct StaticEncoder {
    bytes: &'static [u8],
}

impl Encoder for StaticEncoder {
    fn encode(
        &self,
        _ctx: &mut Context,
        _node: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        writer.write_all(self.bytes)?;
        Ok(())
    }
}

/// An encoder that returns values from a pre-configured sequence.
/// Uses interior mutability (Cell) to track which value to return next.
struct SeqEncoder {
    values: Vec<&'static [u8]>,
    call_count: Cell<usize>,
}

impl SeqEncoder {
    fn new(values: Vec<&'static [u8]>) -> Self {
        Self {
            values,
            call_count: Cell::new(0),
        }
    }
}

impl Encoder for SeqEncoder {
    fn encode(
        &self,
        _ctx: &mut Context,
        _node: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        let i = self.call_count.get();
        self.call_count.set(i + 1);
        writer.write_all(self.values[i])?;
        Ok(())
    }
}

/// An encoder that tracks calls and inserts YAML document separators
/// (`---\n`) between encoded values when `print_doc_seps` is enabled.
struct DocSepEncoder {
    values: Vec<&'static [u8]>,
    print_doc_seps: bool,
    call_count: Cell<usize>,
}

impl DocSepEncoder {
    fn new(values: Vec<&'static [u8]>, print_doc_seps: bool) -> Self {
        Self {
            values,
            print_doc_seps,
            call_count: Cell::new(0),
        }
    }
}

impl Encoder for DocSepEncoder {
    fn encode(
        &self,
        _ctx: &mut Context,
        _node: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        let i = self.call_count.get();
        self.call_count.set(i + 1);
        if i > 0 && self.print_doc_seps {
            writer.write_all(b"---\n")?;
        }
        writer.write_all(self.values[i])?;
        if !self.values[i].ends_with(b"\n") {
            writer.write_all(b"\n")?;
        }
        Ok(())
    }
}

/// An encoder that processes YAML leading content with `$DocSeparator$`
/// markers before each value.
struct LeadingContentEncoder {
    entries: Vec<(String, String)>, // (leading_content, value)
    print_doc_seps: bool,
    call_count: Cell<usize>,
}

impl LeadingContentEncoder {
    fn new(entries: Vec<(&str, &str)>, print_doc_seps: bool) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(l, v)| (l.to_string(), v.to_string()))
                .collect(),
            print_doc_seps,
            call_count: Cell::new(0),
        }
    }
}

impl Encoder for LeadingContentEncoder {
    fn encode(
        &self,
        _ctx: &mut Context,
        _node: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        let i = self.call_count.get();
        self.call_count.set(i + 1);
        let (ref leading, ref value) = self.entries[i];

        if self.print_doc_seps {
            let processed = leading.replace("$DocSeparator$", "---");
            writer.write_all(processed.as_bytes())?;
        }
        writer.write_all(value.as_bytes())?;
        if !value.ends_with('\n') {
            writer.write_all(b"\n")?;
        }
        Ok(())
    }

    fn print_document_separator(&self, writer: &mut dyn Write) -> Result<(), CoreError> {
        if self.print_doc_seps {
            writer.write_all(b"---\n")?;
        }
        Ok(())
    }
}

/// A JSON-style encoder that never prints document separators.
struct JsonEncoder {
    values: Vec<&'static str>,
    call_count: Cell<usize>,
}

impl JsonEncoder {
    fn new(values: Vec<&'static str>) -> Self {
        Self {
            values,
            call_count: Cell::new(0),
        }
    }
}

impl Encoder for JsonEncoder {
    fn encode(
        &self,
        _ctx: &mut Context,
        _node: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        let i = self.call_count.get();
        self.call_count.set(i + 1);
        writer.write_all(self.values[i].as_bytes())?;
        writer.write_all(b"\n")?;
        Ok(())
    }

    fn print_document_separator(&self, _writer: &mut dyn Write) -> Result<(), CoreError> {
        Ok(())
    }
}

fn build_store(meta: &[(u32, i32, &str)]) -> (TreeStore, Vec<NodeId>) {
    let mut store = TreeStore::new();
    let mut ids = Vec::with_capacity(meta.len());
    for (document, file_index, leading_content) in meta {
        let mut node = TreeNode::scalar(SemType::Str, "value");
        node.document = *document;
        let id = store.add(node);
        store.set_document_meta(*document, "", *file_index);
        let _ = store.set_leading_content(id, (*leading_content).to_string());
        ids.push(id);
    }
    (store, ids)
}

#[test]
fn printer_remove_last_eol_trims_lf_and_crlf() {
    let mut lf = b"hello\n".to_vec();
    Printer::<StaticEncoder, RecordingWriter>::remove_last_eol(&mut lf);
    assert_eq!(lf, b"hello");

    let mut crlf = b"hello\r\n".to_vec();
    Printer::<StaticEncoder, RecordingWriter>::remove_last_eol(&mut crlf);
    assert_eq!(crlf, b"hello");

    let mut only_cr = b"hello\r".to_vec();
    Printer::<StaticEncoder, RecordingWriter>::remove_last_eol(&mut only_cr);
    assert_eq!(only_cr, b"hello");

    let mut empty = Vec::new();
    Printer::<StaticEncoder, RecordingWriter>::remove_last_eol(&mut empty);
    assert!(empty.is_empty());
}

#[test]
fn printer_writes_appendix_even_when_matching_nodes_are_empty() {
    let mut printer = Printer::new(
        StaticEncoder { bytes: b"value\n" },
        RecordingWriter::default(),
    );
    printer.set_appendix(Some(b"# done\n".to_vec()));
    let mut ctx = Context::default();
    let store = TreeStore::new();

    printer.print_results(&mut ctx, &store, &[]).unwrap();

    assert!(!printer.printed_anything());
    assert_eq!(printer.into_writer().bytes, b"# done\n");
}

#[test]
fn printer_nul_separator_strips_trailing_eol_before_appending_nul() {
    let mut printer = Printer::new(
        StaticEncoder { bytes: b"value\n" },
        RecordingWriter::default(),
    );
    printer.set_nul_sep_output(true);
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, "")]);

    printer.print_results(&mut ctx, &store, &[ids[0]]).unwrap();

    assert!(printer.printed_anything());
    assert_eq!(printer.into_writer().bytes, b"value\0");
}

#[test]
fn printer_printed_anything_flips_only_after_printing_matches() {
    let mut printer = Printer::new(
        StaticEncoder { bytes: b"value\n" },
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, "")]);

    assert!(!printer.printed_anything());
    printer.print_results(&mut ctx, &store, &[]).unwrap();
    assert!(!printer.printed_anything());
    printer.print_results(&mut ctx, &store, &[ids[0]]).unwrap();
    assert!(printer.printed_anything());
}

#[test]
fn printer_can_toggle_nul_separator_back_to_plain_output() {
    let mut printer = Printer::new(
        StaticEncoder { bytes: b"value\n" },
        RecordingWriter::default(),
    );
    printer.set_nul_sep_output(true);
    printer.set_nul_sep_output(false);
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, "")]);

    printer.print_results(&mut ctx, &store, &[ids[0]]).unwrap();

    assert_eq!(printer.into_writer().bytes, b"value\n");
}

#[test]
fn printer_rejects_embedded_nul_in_nul_separated_output() {
    let mut printer = Printer::new(
        StaticEncoder {
            bytes: b"bad\0value\n",
        },
        RecordingWriter::default(),
    );
    printer.set_nul_sep_output(true);
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, "")]);

    let error = printer
        .print_results(&mut ctx, &store, &[ids[0]])
        .unwrap_err();

    assert_eq!(
        error,
        CoreError::System(SystemError::NulInNulSeparatedOutput)
    );
}

// ---------------------------------------------------------------------------
// printer prints document separators across docs
// ---------------------------------------------------------------------------

#[test]
fn printer_prints_document_separators_across_docs() {
    let mut printer = Printer::new(
        DocSepEncoder::new(vec![b"a: banana", b"a: apple", b"a: coconut"], true),
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, ""), (1, 0, ""), (2, 0, "")]);

    printer.print_results(&mut ctx, &store, &ids).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"a: banana\n---\na: apple\n---\na: coconut\n"
    );
}

// ---------------------------------------------------------------------------
// printer respects leading content doc separator markers
// ---------------------------------------------------------------------------

#[test]
fn printer_respects_leading_content_doc_separator_markers() {
    let mut printer = Printer::new(
        LeadingContentEncoder::new(
            vec![
                ("# go cats\n$DocSeparator$\n", "a: banana"),
                ("$DocSeparator$\n", "a: apple"),
                ("$DocSeparator$\n# cool\n", "a: coconut"),
            ],
            true,
        ),
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[
        (0, 0, "# go cats\n$DocSeparator$\n"),
        (1, 0, "$DocSeparator$\n"),
        (2, 0, "$DocSeparator$\n# cool\n"),
    ]);

    printer.print_results(&mut ctx, &store, &ids).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"# go cats\n---\na: banana\n---\na: apple\n---\n# cool\na: coconut\n"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterMultipleDocsInSequenceOnly
// ---------------------------------------------------------------------------

#[test]
fn printer_multiple_docs_in_sequence_only() {
    let mut printer = Printer::new(
        DocSepEncoder::new(vec![b"a: banana", b"a: apple", b"a: coconut"], true),
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, ""), (1, 0, ""), (2, 0, "")]);

    printer.print_results(&mut ctx, &store, &[ids[0]]).unwrap();
    printer.print_results(&mut ctx, &store, &[ids[1]]).unwrap();
    printer.print_results(&mut ctx, &store, &[ids[2]]).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"a: banana\n---\na: apple\n---\na: coconut\n"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterMultipleDocsInSequenceWithLeadingContent
// ---------------------------------------------------------------------------

#[test]
fn printer_multiple_docs_in_sequence_with_leading_content() {
    let mut printer = Printer::new(
        LeadingContentEncoder::new(
            vec![
                ("# go cats\n$DocSeparator$\n", "a: banana"),
                ("$DocSeparator$\n", "a: apple"),
                ("$DocSeparator$\n# cool\n", "a: coconut"),
            ],
            true,
        ),
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[
        (0, 0, "# go cats\n$DocSeparator$\n"),
        (1, 0, "$DocSeparator$\n"),
        (2, 0, "$DocSeparator$\n# cool\n"),
    ]);

    printer.print_results(&mut ctx, &store, &[ids[0]]).unwrap();
    printer.print_results(&mut ctx, &store, &[ids[1]]).unwrap();
    printer.print_results(&mut ctx, &store, &[ids[2]]).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"# go cats\n---\na: banana\n---\na: apple\n---\n# cool\na: coconut\n"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterMultipleFilesInSequence
// ---------------------------------------------------------------------------

#[test]
fn printer_multiple_files_in_sequence() {
    let mut printer = Printer::new(
        DocSepEncoder::new(vec![b"a: banana", b"a: apple", b"a: coconut"], true),
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, ""), (0, 1, ""), (0, 2, "")]);

    printer.print_results(&mut ctx, &store, &[ids[0]]).unwrap();
    printer.print_results(&mut ctx, &store, &[ids[1]]).unwrap();
    printer.print_results(&mut ctx, &store, &[ids[2]]).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"a: banana\n---\na: apple\n---\na: coconut\n"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterMultipleFilesInSequenceWithLeadingContent
// ---------------------------------------------------------------------------

#[test]
fn printer_multiple_files_in_sequence_with_leading_content() {
    let mut printer = Printer::new(
        LeadingContentEncoder::new(
            vec![
                ("# go cats\n$DocSeparator$\n", "a: banana"),
                ("$DocSeparator$\n", "a: apple"),
                ("$DocSeparator$\n# cool\n", "a: coconut"),
            ],
            true,
        ),
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[
        (0, 0, "# go cats\n$DocSeparator$\n"),
        (0, 1, "$DocSeparator$\n"),
        (0, 2, "$DocSeparator$\n# cool\n"),
    ]);

    printer.print_results(&mut ctx, &store, &[ids[0]]).unwrap();
    printer.print_results(&mut ctx, &store, &[ids[1]]).unwrap();
    printer.print_results(&mut ctx, &store, &[ids[2]]).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"# go cats\n---\na: banana\n---\na: apple\n---\n# cool\na: coconut\n"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterMultipleDocsInSinglePrintWithLeadingDoc
// ---------------------------------------------------------------------------

#[test]
fn printer_multiple_docs_in_single_print_with_leading_doc() {
    let mut printer = Printer::new(
        DocSepEncoder::new(
            vec![b"# go cats\n---\na: banana", b"a: apple", b"a: coconut"],
            true,
        ),
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, ""), (1, 0, ""), (2, 0, "")]);

    printer.print_results(&mut ctx, &store, &ids).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"# go cats\n---\na: banana\n---\na: apple\n---\na: coconut\n"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterMultipleDocsInSinglePrintWithLeadingDocTrailing
// ---------------------------------------------------------------------------

#[test]
fn printer_multiple_docs_in_single_print_with_leading_doc_trailing() {
    let mut printer = Printer::new(
        DocSepEncoder::new(vec![b"---\na: banana", b"a: apple", b"a: coconut"], true),
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, ""), (1, 0, ""), (2, 0, "")]);

    printer.print_results(&mut ctx, &store, &ids).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"---\na: banana\n---\na: apple\n---\na: coconut\n"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterScalarWithLeadingCont
// ---------------------------------------------------------------------------

#[test]
fn printer_scalar_with_leading_cont() {
    let mut printer = Printer::new(
        DocSepEncoder::new(vec![b"banana", b"apple", b"coconut"], true),
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, ""), (1, 0, ""), (2, 0, "")]);

    printer.print_results(&mut ctx, &store, &[ids[0]]).unwrap();
    printer.print_results(&mut ctx, &store, &[ids[1]]).unwrap();
    printer.print_results(&mut ctx, &store, &[ids[2]]).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"banana\n---\napple\n---\ncoconut\n"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterMultipleDocsJson
// ---------------------------------------------------------------------------

#[test]
fn printer_multiple_docs_json() {
    let mut printer = Printer::new(
        JsonEncoder::new(vec![
            "{\"a\":\"banana\"}",
            "{\"a\":\"apple\"}",
            "{\"a\":\"coconut\"}",
        ]),
        RecordingWriter::default(),
    );
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, ""), (1, 0, ""), (2, 0, "")]);

    printer.print_results(&mut ctx, &store, &ids).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"{\"a\":\"banana\"}\n{\"a\":\"apple\"}\n{\"a\":\"coconut\"}\n"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterNulSeparator
// ---------------------------------------------------------------------------

#[test]
fn printer_nul_separator_multi_doc() {
    let mut printer = Printer::new(
        SeqEncoder::new(vec![b"banana\n", b"apple\n", b"coconut\n"]),
        RecordingWriter::default(),
    );
    printer.set_nul_sep_output(true);
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, ""), (1, 0, ""), (2, 0, "")]);

    printer.print_results(&mut ctx, &store, &ids).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"banana\x00apple\x00coconut\x00"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterNulSeparatorWithJson
// ---------------------------------------------------------------------------

#[test]
fn printer_nul_separator_with_json() {
    let mut printer = Printer::new(
        JsonEncoder::new(vec![
            "{\"a\":\"banana\"}",
            "{\"a\":\"apple\"}",
            "{\"a\":\"coconut\"}",
        ]),
        RecordingWriter::default(),
    );
    printer.set_nul_sep_output(true);
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, ""), (1, 0, ""), (2, 0, "")]);

    printer.print_results(&mut ctx, &store, &ids).unwrap();

    assert_eq!(
        printer.into_writer().bytes,
        b"{\"a\":\"banana\"}\x00{\"a\":\"apple\"}\x00{\"a\":\"coconut\"}\x00"
    );
}

// ---------------------------------------------------------------------------
// TestPrinterRootUnwrap
// ---------------------------------------------------------------------------

#[test]
fn printer_root_unwrap() {
    let mut printer = Printer::new(StaticEncoder { bytes: b"a\n" }, RecordingWriter::default());
    let mut ctx = Context::default();
    let (store, ids) = build_store(&[(0, 0, "")]);

    printer.print_results(&mut ctx, &store, &[ids[0]]).unwrap();

    assert_eq!(printer.into_writer().bytes, b"a\n");
}
