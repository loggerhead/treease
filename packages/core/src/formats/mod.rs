use crate::core::{CoreError, NodeId, TreeStore};

pub mod decoder_csv;
pub mod decoder_csv_object;
pub mod decoder_javascript;
pub mod decoder_json;
pub mod decoder_object;
pub mod decoder_python;
pub mod decoder_toml;
pub mod decoder_yaml;
pub mod encoder;
pub mod encoder_csv;
pub mod encoder_javascript;
pub mod encoder_json;
pub mod encoder_python;
pub mod encoder_toml;
pub mod encoder_yaml;
pub mod formats_helpers;
pub mod preferences;
pub mod smart_layout;

pub use decoder_csv::CsvDecoder;
pub use decoder_csv_object::CsvObjectDecoder;
pub use decoder_javascript::JavascriptObjectDecoder;
pub use decoder_json::JsonDecoder;
pub use decoder_object::PythonObjectDecoder;
pub use decoder_toml::TomlDecoder;
pub use decoder_yaml::YamlDecoder;
pub use encoder::Encode;
pub(crate) use encoder::{
    add_mapping, add_scalar, add_sequence, append_child, append_existing_key_value,
    append_key_value, escape_json_string, is_truthy_literal, missing_tree_node, node,
    scalar_json_text, write_indent,
};
pub use encoder_csv::CsvEncoder;
pub use encoder_javascript::JavascriptEncoder;
pub use encoder_json::JsonEncoder;
pub use encoder_python::PythonEncoder;
pub use encoder_toml::TomlEncoder;
pub use encoder_yaml::YamlEncoder;
pub use preferences::{
    DEFAULT_MAX_ARRAY_INLINE_ITEMS, DEFAULT_MAX_INLINE_COMPLEXITY, DEFAULT_MAX_LINE_LENGTH,
    FormatPreferences, LanguagePreferences, configured_language_preferences,
    default_language_preferences,
};

#[derive(Debug, Clone)]
pub struct DecodedDocument {
    pub store: TreeStore,
    pub root: NodeId,
}

impl DecodedDocument {
    pub fn new(store: TreeStore, root: NodeId) -> Self {
        Self { store, root }
    }
}

pub trait Decode {
    fn decode_str(&self, input: &str) -> Result<DecodedDocument, CoreError>;

    /// Decode all documents from the input.
    ///
    /// For single-document formats (JSON, TOML, CSV, Python, JavaScript)
    /// this returns a vec with one element (the same as `decode_str`).
    /// For multi-document formats (YAML with `---` separators) this returns
    fn decode_all_str(&self, input: &str) -> Result<Vec<DecodedDocument>, CoreError> {
        Ok(vec![self.decode_str(input)?])
    }
}
