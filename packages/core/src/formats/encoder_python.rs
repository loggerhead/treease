use std::io::Write;

use crate::errors::CoreError;
use crate::tree::{NodeId, TreeStore};

use super::Encode;
use super::encoder_json::{
    self, LanguageStyle, eval_scalar_unquoted_text, write_eval_value_into, write_eval_value_smart,
};
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
            super::default_language_preferences()
                .effective(crate::language::FormatLanguage::Python),
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

    fn encode_evaluated_value(
        &self,
        value: &crate::evaluator::Value,
        writer: &mut dyn Write,
    ) -> Result<bool, CoreError> {
        if self.prefs.unwrap_scalar {
            if let Some(text) = eval_scalar_unquoted_text(value) {
                writer.write_all(text.as_bytes())?;
                return Ok(true);
            }
        }

        let mut out = String::new();
        if self.prefs.smart && self.prefs.indent > 0 {
            write_eval_value_smart(value, &self.prefs, 0, LanguageStyle::Python, &mut out)?;
        } else {
            write_eval_value_into(value, self.prefs.indent, 0, LanguageStyle::Python, &mut out)?;
        }
        out.push('\n');
        writer.write_all(out.as_bytes())?;
        Ok(true)
    }
}

use super::formats_helpers::write_quoted_string;
use super::is_truthy_literal;
use crate::language::SemType;

pub(crate) fn write_python_key(store: &TreeStore, node_id: NodeId, out: &mut String) {
    let Some(node) = store.get(node_id) else {
        return;
    };
    let value = store.value_for(node_id).unwrap_or_default();
    match node.resolved_sem_type() {
        Some(SemType::Nil) => out.push_str("None"),
        Some(SemType::Boolean) => out.push_str(if is_truthy_literal(value) {
            "True"
        } else {
            "False"
        }),
        Some(SemType::Int | SemType::Float) => out.push_str(value),
        _ => write_quoted_string(out, value, '\''),
    }
}

#[cfg(test)]
mod tests {
    use crate::evaluator::Value;
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

    #[test]
    fn encode_evaluated_value_matches_store_backed_output() {
        let value = Value::Object(std::collections::BTreeMap::from([
            ("bad-key".to_string(), Value::String("Ada".to_string())),
            (
                "items".to_string(),
                Value::Array(vec![Value::Bool(true), Value::Null]),
            ),
        ]));
        let mut direct = Vec::new();
        PythonEncoder::default()
            .encode_evaluated_value(&value, &mut direct)
            .expect("direct encode should succeed");

        let decoded = PythonObjectDecoder
            .decode_str("{'bad-key': 'Ada', 'items': [True, None]}")
            .unwrap();
        let store_backed = PythonEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap();

        assert_eq!(String::from_utf8(direct).unwrap(), store_backed);
    }
}
