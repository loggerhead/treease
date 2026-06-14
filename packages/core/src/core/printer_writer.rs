use std::io::Write;

use super::context::Context;
use super::errors::CoreError;
use super::tree_node::NodeId;

pub trait PrinterWriter {
    fn write_for_node(
        &mut self,
        ctx: &mut Context,
        node: Option<NodeId>,
        bytes: &[u8],
    ) -> Result<(), CoreError>;
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VecPrinterWriter {
    bytes: Vec<u8>,
}

impl VecPrinterWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}

impl PrinterWriter for VecPrinterWriter {
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
