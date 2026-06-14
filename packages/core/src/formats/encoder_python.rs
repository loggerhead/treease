use std::io::Write;

use crate::core::{CoreError, NodeId, TreeStore};

use super::Encode;
use super::encoder_json::{self, LanguageStyle};
use super::formats_helpers::resolve_alias_for_encode;
use super::preferences::FormatPreferences;
use super::smart_layout::encode_python_node_smart;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonEncoder {
    prefs: FormatPreferences,
}

impl PythonEncoder {
    pub fn new(prefs: FormatPreferences) -> Self {
        Self { prefs }
    }
}

impl Default for PythonEncoder {
    fn default() -> Self {
        Self::new(
            super::default_language_preferences().effective(crate::core::FormatLanguage::Python),
        )
    }
}

impl Encode for PythonEncoder {
    fn encode(
        &self,
        store: &TreeStore,
        node_id: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        let root_id = resolve_alias_for_encode(store, node_id)?.unwrap_or(node_id);
        let mut out = String::new();
        if self.prefs.smart && self.prefs.indent > 0 {
            encode_python_node_smart(store, root_id, &self.prefs, 0, &mut out)?;
        } else {
            encoder_json::encode_node(
                store,
                root_id,
                self.prefs.indent,
                0,
                &mut out,
                LanguageStyle::Python,
            )?;
        }
        out.push('\n');
        writer.write_all(out.as_bytes())?;
        Ok(())
    }
}

use super::formats_helpers::write_quoted_string;
use super::is_truthy_literal;
use crate::core::SemType;

pub(crate) fn write_python_key(out: &mut String, node: &crate::core::TreeNode) {
    match node.resolved_sem_type() {
        Some(SemType::Nil) => out.push_str("None"),
        Some(SemType::Boolean) => out.push_str(if is_truthy_literal(&node.value) {
            "True"
        } else {
            "False"
        }),
        Some(SemType::Int | SemType::Float) => out.push_str(&node.value),
        _ => write_quoted_string(out, &node.value, '\''),
    }
}

#[cfg(test)]
mod tests {
    use crate::formats::{Decode, Encode, JavascriptObjectDecoder, PythonObjectDecoder};

    use super::PythonEncoder;

    #[test]
    fn python_encoder_writes_python_literals() {
        let decoded = PythonObjectDecoder
            .decode_str("{'name': 'Ada', 'active': True, 'none': None}")
            .unwrap();
        let encoded = PythonEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap();
        assert!(encoded.contains("'name': 'Ada'"));
        assert!(encoded.contains("'active': True"));
        assert!(encoded.contains("'none': None"));
    }

    #[test]
    fn shared_encoder_supports_nested_sequences() {
        let decoded = JavascriptObjectDecoder
            .decode_str("{items: [1, true, null]}")
            .unwrap();
        let encoded = PythonEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap();
        assert!(encoded.contains("["));
        assert!(encoded.contains("True"));
        assert!(encoded.contains("None"));
    }
}
