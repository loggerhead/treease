use std::io::Write;
use std::ptr::NonNull;

use super::context::Context;
use super::errors::{CoreError, SystemError};
use super::printer_writer::PrinterWriter;
use super::tree_node::{NodeId, TreeNodeKind};
use crate::core::TreeStore;

pub trait Encoder {
    fn encode(
        &self,
        ctx: &mut Context,
        node: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError>;

    fn encode_evaluated_value(
        &self,
        _value: &crate::evaluator::Value,
        _writer: &mut dyn Write,
    ) -> Result<bool, CoreError> {
        Ok(false)
    }

    fn print_document_separator(&self, writer: &mut dyn Write) -> Result<(), CoreError> {
        writer.write_all(b"\n---\n")?;
        Ok(())
    }

    fn print_leading_content(
        &self,
        writer: &mut dyn Write,
        content: &str,
    ) -> Result<(), CoreError> {
        if content.is_empty() {
            return Ok(());
        }
        // Output leading content as-is (comments, blank lines, doc separators).
        // Lines containing $DocSeparator$ are replaced with a YAML document separator.
        for segment in content.split_inclusive('\n') {
            if segment.contains("$DocSeparator$") {
                self.print_document_separator(writer)?;
                continue;
            }
            writer.write_all(segment.as_bytes())?;
        }
        Ok(())
    }

    fn can_handle_aliases(&self) -> bool {
        true
    }
}

pub struct Printer<E, W> {
    encoder: E,
    writer: W,
    first_time_printing: bool,
    previous_doc_index: u32,
    previous_file_index: i32,
    printed_matches: bool,
    appendix: Option<Vec<u8>>,
    nul_sep_output: bool,
}

struct PrintStoreGuard<'a> {
    ctx: *mut Context,
    previous: Option<NonNull<TreeStore>>,
    _marker: std::marker::PhantomData<&'a mut Context>,
}

struct PrinterWriteAdapter<'a, W> {
    ctx: *mut Context,
    writer: &'a mut W,
    node_id: Option<NodeId>,
}

impl<W: PrinterWriter> Write for PrinterWriteAdapter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // SAFETY: `ctx` originates from the live `&mut Context` passed into
        // `print_results`, and this adapter never outlives that call.
        let ctx = unsafe { &mut *self.ctx };
        self.writer
            .write_for_node(ctx, self.node_id, buf)
            .map_err(std::io::Error::other)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for PrintStoreGuard<'_> {
    fn drop(&mut self) {
        // SAFETY: `ctx` originates from the live `&mut Context` passed into
        // `print_results`, and the guard never outlives that call.
        unsafe {
            (*self.ctx).print_store = self.previous;
        }
    }
}

impl<E, W> Printer<E, W>
where
    E: Encoder,
    W: PrinterWriter,
{
    pub fn new(encoder: E, writer: W) -> Self {
        Self {
            encoder,
            writer,
            first_time_printing: true,
            previous_doc_index: 0,
            previous_file_index: 0,
            printed_matches: false,
            appendix: None,
            nul_sep_output: false,
        }
    }

    pub fn set_nul_sep_output(&mut self, enabled: bool) {
        self.nul_sep_output = enabled;
    }

    pub fn set_appendix(&mut self, appendix: Option<Vec<u8>>) {
        self.appendix = appendix;
    }

    pub fn printed_anything(&self) -> bool {
        self.printed_matches
    }

    pub fn into_writer(self) -> W {
        self.writer
    }

    pub fn remove_last_eol(buf: &mut Vec<u8>) {
        if buf.ends_with(b"\r\n") {
            buf.truncate(buf.len() - 2);
        } else if matches!(buf.last(), Some(b'\r' | b'\n')) {
            buf.pop();
        }
    }

    pub fn print_results(
        &mut self,
        ctx: &mut Context,
        store: &TreeStore,
        matching_nodes: &[NodeId],
    ) -> Result<(), CoreError> {
        let previous_store = ctx.print_store;
        ctx.print_store = Some(NonNull::from(store));
        let _guard = PrintStoreGuard {
            ctx,
            previous: previous_store,
            _marker: std::marker::PhantomData,
        };
        if matching_nodes.is_empty() {
            if let Some(appendix) = self.appendix.clone() {
                self.writer.write_for_node(ctx, None, &appendix)?;
            }
            return Ok(());
        }

        if self.first_time_printing {
            if let Some(&first_id) = matching_nodes.first() {
                if let Some(node) = store.get(first_id) {
                    self.previous_doc_index = node.document;
                    self.previous_file_index = store.file_index_for(first_id).unwrap_or_default();
                }
            }
            self.first_time_printing = false;
        }

        for &node_id in matching_nodes {
            let node = store.get(node_id);

            // Alias handling: if node is an alias and encoder can't handle it, resolve alias
            let target_id = if let Some(node_ref) = node {
                if node_ref.kind == TreeNodeKind::Alias && !self.encoder.can_handle_aliases() {
                    node_ref.alias().unwrap_or(node_id)
                } else {
                    node_id
                }
            } else {
                node_id
            };

            // printed_matches: mark true unless the value is nil or boolean false
            if let Some(node_ref) = node {
                let is_nil = node_ref.sem_type == Some(super::sem_type::SemType::Nil);
                let is_false = node_ref.sem_type == Some(super::sem_type::SemType::Boolean)
                    && store
                        .value_for(node_id)
                        .is_ok_and(|value| value.eq_ignore_ascii_case("false"));
                if !is_nil && !is_false {
                    self.printed_matches = true;
                }
            } else {
                self.printed_matches = true;
            }

            if self.nul_sep_output {
                let mut encoded = Vec::new();
                self.encoder.encode(ctx, target_id, &mut encoded)?;
                Self::remove_last_eol(&mut encoded);
                if encoded.contains(&0) {
                    return Err(SystemError::NulInNulSeparatedOutput.into());
                }
                encoded.push(0);
                self.writer.write_for_node(ctx, Some(node_id), &encoded)?;
            } else {
                let mut writer = PrinterWriteAdapter {
                    ctx,
                    writer: &mut self.writer,
                    node_id: Some(node_id),
                };
                self.encoder.encode(ctx, target_id, &mut writer)?;
            }

            if let Some(node_ref) = node {
                self.previous_doc_index = node_ref.document;
                self.previous_file_index = store.file_index_for(node_id).unwrap_or_default();
            }
        }

        if let Some(appendix) = self.appendix.clone() {
            self.writer.write_for_node(ctx, None, &appendix)?;
        }
        Ok(())
    }

    pub fn can_print_evaluated_value(&self) -> bool {
        self.encoder
            .encode_evaluated_value(&crate::evaluator::Value::Null, &mut std::io::sink())
            .is_ok_and(|handled| handled)
    }

    pub fn print_evaluated_value(
        &mut self,
        ctx: &mut Context,
        value: &crate::evaluator::Value,
    ) -> Result<(), CoreError> {
        self.first_time_printing = false;
        match value {
            crate::evaluator::Value::Null | crate::evaluator::Value::Bool(false) => {}
            _ => self.printed_matches = true,
        }

        if self.nul_sep_output {
            let mut encoded = Vec::new();
            if !self.encoder.encode_evaluated_value(value, &mut encoded)? {
                return Err(SystemError::Error.into());
            }
            Self::remove_last_eol(&mut encoded);
            if encoded.contains(&0) {
                return Err(SystemError::NulInNulSeparatedOutput.into());
            }
            encoded.push(0);
            self.writer.write_for_node(ctx, None, &encoded)?;
            if let Some(appendix) = self.appendix.clone() {
                self.writer.write_for_node(ctx, None, &appendix)?;
            }
            return Ok(());
        }

        let mut writer = PrinterWriteAdapter {
            ctx,
            writer: &mut self.writer,
            node_id: None,
        };
        if !self.encoder.encode_evaluated_value(value, &mut writer)? {
            return Err(SystemError::Error.into());
        }
        if let Some(appendix) = self.appendix.clone() {
            self.writer.write_for_node(ctx, None, &appendix)?;
        }
        Ok(())
    }

    pub fn previous_doc_index(&self) -> u32 {
        self.previous_doc_index
    }

    pub fn previous_file_index(&self) -> i32 {
        self.previous_file_index
    }
}
