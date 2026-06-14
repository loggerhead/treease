use crate::{
    core::{CoreError, ParseError},
    stream::streaming_json,
};

use super::{Decode, DecodedDocument};

#[derive(Debug, Clone, Copy, Default)]
pub struct JsonDecoder;

impl Decode for JsonDecoder {
    fn decode_str(&self, input: &str) -> Result<DecodedDocument, CoreError> {
        streaming_json::decode_slice_to_tree(input)
            .map_err(|_| CoreError::Parse(ParseError::InvalidJson))
    }
}

pub fn decode_json(input: &str) -> Result<DecodedDocument, CoreError> {
    JsonDecoder.decode_str(input)
}
