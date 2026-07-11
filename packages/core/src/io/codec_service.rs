use crate::formats::{Decode, DecodedDocument, Encode, configured_language_preferences};

use crate::errors::{CoreError, FormatError, ParseError};
use crate::language::tree_sitter_support;
use crate::language::{FormatLanguage, Language};
use crate::registry::format_registry;
use crate::registry::{FormatRegistry, format_from_string};
use crate::tree::{NodeId, TreeStore};

#[derive(Debug, Clone)]
pub struct CodecService {
    prefs: crate::formats::LanguagePreferences,
    registry: Option<FormatRegistry>,
}

impl Default for CodecService {
    fn default() -> Self {
        Self::new()
    }
}

impl CodecService {
    pub fn new() -> Self {
        Self {
            prefs: configured_language_preferences(),
            registry: None,
        }
    }

    pub fn with_registry(registry: FormatRegistry) -> Self {
        Self {
            prefs: configured_language_preferences(),
            registry: Some(registry),
        }
    }

    pub fn decode(&self, format_name: &str, input: &str) -> Result<DecodedDocument, CoreError> {
        let decoder = self.get_decoder(format_name)?;

        decoder.decode_str(input)
    }

    /// Decode all documents from the input.
    ///
    /// For multi-document formats (e.g. YAML with `---` separators), returns
    /// one `DecodedDocument` per document. For single-document formats,
    /// returns a vec with one element.
    ///
    pub fn decode_all(
        &self,
        format_name: &str,
        input: &str,
    ) -> Result<Vec<DecodedDocument>, CoreError> {
        let decoder = self.get_decoder(format_name)?;

        decoder.decode_all_str(input)
    }

    pub fn get_encoder(
        &self,
        format_name: &str,
        indent: i32,
    ) -> Result<Box<dyn Encode>, CoreError> {
        let mut prefs = self.preferences_for(format_name)?;
        prefs.indent = indent;

        if let Some(registry) = &self.registry {
            let registry_prefs = format_registry::FormatPreferences {
                indent,
                pretty: indent != 0,
            };
            let canonical = canonical_format_name(format_name)?;
            return registry
                .create_encoder_by_prefs(canonical, &registry_prefs)
                .ok_or(FormatError::UnknownFormat.into());
        }

        let canonical = canonical_format_name(format_name)?;
        Ok(crate::language::capability::capability_for_format(canonical, "encode")?.encode(prefs))
    }

    pub fn get_decoder(&self, format_name: &str) -> Result<Box<dyn Decode>, CoreError> {
        if let Some(registry) = &self.registry {
            let registry_prefs = format_registry::FormatPreferences::default();
            let canonical = canonical_format_name(format_name)?;
            return registry
                .create_decoder_by_prefs(canonical, &registry_prefs)
                .ok_or(FormatError::UnknownFormat.into());
        }

        let canonical = canonical_format_name(format_name)?;
        Ok(crate::language::capability::capability_for_format(canonical, "decode")?.decode())
    }

    pub fn encode_to_string(
        &self,
        format_name: &str,
        store: &TreeStore,
        root: NodeId,
    ) -> Result<String, CoreError> {
        let indent = self.preferences_for(format_name)?.indent;
        self.get_encoder(format_name, indent)?
            .encode_to_string(store, root)
    }

    pub fn minify_to_string(
        &self,
        format_name: &str,
        store: &TreeStore,
        root: NodeId,
    ) -> Result<String, CoreError> {
        let mut prefs = self.preferences_for(format_name)?;
        prefs.indent = 0;
        prefs.unwrap_scalar = false;
        let canonical = canonical_format_name(format_name)?;
        crate::language::capability::encode_with_capability(canonical, prefs, store, root)
    }

    pub fn convert_string(
        &self,
        source_format: &str,
        target_format: &str,
        input: &str,
    ) -> Result<String, CoreError> {
        let source = canonical_format_name(source_format)?;
        let target = canonical_format_name(target_format)?;
        if source == "yaml" && target == "yaml" {
            if let Some(documents) = split_yaml_documents(input)? {
                let mut out = String::new();
                for (index, document) in documents.iter().enumerate() {
                    let decoded = self.decode(source, document)?;
                    if index != 0 {
                        out.push_str("---\n");
                    }
                    out.push_str(&self.encode_to_string(target, &decoded.store, decoded.root)?);
                }
                return Ok(out);
            }
        }
        let decoded = self.decode(source_format, input)?;
        self.encode_to_string(target_format, &decoded.store, decoded.root)
    }

    pub fn preferences_for(
        &self,
        format_name: &str,
    ) -> Result<crate::formats::FormatPreferences, CoreError> {
        Ok(self.prefs.effective(language_for_format(format_name)?))
    }
}

fn split_yaml_documents(input: &str) -> Result<Option<Vec<&str>>, CoreError> {
    let language = tree_sitter_support::tree_sitter_language("yaml")
        .ok_or(CoreError::Parse(ParseError::InvalidYaml))?;
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|_| CoreError::Parse(ParseError::InvalidYaml))?;
    let tree = parser
        .parse(input.as_bytes(), None)
        .ok_or(CoreError::Parse(ParseError::InvalidYaml))?;
    if tree.root_node().has_error() {
        return Err(CoreError::Parse(ParseError::InvalidYaml));
    }

    let mut documents = Vec::new();
    for index in 0..tree.root_node().named_child_count() {
        let Some(child) = tree.root_node().named_child(index as _) else {
            continue;
        };
        if child.kind() != "document" {
            continue;
        }
        let start = child.start_byte();
        let end = child.end_byte();
        documents.push(&input[start..end]);
    }

    if documents.len() > 1 {
        Ok(Some(documents))
    } else {
        Ok(None)
    }
}

pub fn canonical_format_name(format_name: &str) -> Result<&'static str, CoreError> {
    Ok(format_from_string(format_name)?.formal_name)
}

pub fn language_for_format(format_name: &str) -> Result<FormatLanguage, CoreError> {
    let canonical = canonical_format_name(format_name)?;
    Language::from_name(canonical)
        .as_format_language()
        .ok_or_else(|| FormatError::UnknownFormat.into())
}
