use crate::core::CoreError;

use super::decoder_python::PythonDecoder;
use super::{Decode, DecodedDocument};

/// Python object-literal decoder.
///
/// Delegates to the tree-sitter-based `PythonDecoder` for real CST parsing
/// instead of the previous normalize-to-JSON fallback.
#[derive(Debug, Clone, Copy, Default)]
pub struct PythonObjectDecoder;

impl Decode for PythonObjectDecoder {
    fn decode_str(&self, input: &str) -> Result<DecodedDocument, CoreError> {
        PythonDecoder.decode_str(input)
    }
}
