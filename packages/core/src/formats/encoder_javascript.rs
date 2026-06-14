use std::io::Write;

use crate::core::{CoreError, NodeId, TreeNodeKind, TreeStore};

use super::encoder_json::{self, LanguageStyle};
use super::formats_helpers::resolve_alias_for_encode;
use super::preferences::FormatPreferences;
use super::smart_layout::encode_javascript_node_smart;
use super::{Encode, missing_tree_node};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavascriptEncoder {
    prefs: FormatPreferences,
}

impl JavascriptEncoder {
    pub fn new(prefs: FormatPreferences) -> Self {
        Self { prefs }
    }

    fn indent(&self) -> i32 {
        self.prefs.indent.max(0)
    }
}

impl Default for JavascriptEncoder {
    fn default() -> Self {
        Self::new(
            super::default_language_preferences()
                .effective(crate::core::FormatLanguage::Javascript),
        )
    }
}

pub(crate) fn is_safe_integer_literal(text: &str) -> bool {
    if text.is_empty() || text.len() > 32 {
        return false;
    }
    let digits = match text.strip_prefix('-') {
        Some("") => return false,
        Some(rest) => rest,
        None => text,
    };
    if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    const MAX_SAFE_INTEGER: &str = "9007199254740991";
    if digits.len() < MAX_SAFE_INTEGER.len() {
        return true;
    }
    if digits.len() > MAX_SAFE_INTEGER.len() {
        return false;
    }
    digits <= MAX_SAFE_INTEGER
}

pub(crate) fn is_js_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || matches!(first, '_' | '$')) {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$'))
        && !matches!(
            text,
            "break"
                | "case"
                | "catch"
                | "class"
                | "const"
                | "continue"
                | "debugger"
                | "default"
                | "delete"
                | "do"
                | "else"
                | "enum"
                | "export"
                | "extends"
                | "false"
                | "finally"
                | "for"
                | "function"
                | "if"
                | "import"
                | "in"
                | "instanceof"
                | "implements"
                | "interface"
                | "let"
                | "new"
                | "null"
                | "package"
                | "private"
                | "protected"
                | "public"
                | "return"
                | "static"
                | "super"
                | "switch"
                | "this"
                | "throw"
                | "true"
                | "try"
                | "typeof"
                | "var"
                | "void"
                | "while"
                | "with"
                | "yield"
        )
}

impl Encode for JavascriptEncoder {
    fn encode(
        &self,
        store: &TreeStore,
        node_id: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        let root_id = resolve_alias_for_encode(store, node_id)?.unwrap_or(node_id);
        let root = store.get(root_id).ok_or_else(missing_tree_node)?;
        if root.kind == TreeNodeKind::Scalar && self.prefs.unwrap_scalar {
            let mut out = String::new();
            encoder_json::encode_node(
                store,
                root_id,
                self.indent(),
                0,
                &mut out,
                LanguageStyle::Javascript,
            )?;
            writer.write_all(out.as_bytes())?;
            writer.write_all(b"\n")?;
            return Ok(());
        }

        let wrap_root_mapping = root_mapping_needs_wrap(store, root_id)?;
        let mut out = String::new();
        if wrap_root_mapping {
            out.push('(');
        }
        if self.prefs.smart && self.prefs.indent > 0 {
            if encode_javascript_node_smart(store, root_id, &self.prefs, 0, &mut out).is_err() {
                out.clear();
                if wrap_root_mapping {
                    out.push('(');
                }
                encoder_json::encode_node(
                    store,
                    root_id,
                    self.indent(),
                    0,
                    &mut out,
                    LanguageStyle::Javascript,
                )?;
            }
        } else {
            encoder_json::encode_node(
                store,
                root_id,
                self.indent(),
                0,
                &mut out,
                LanguageStyle::Javascript,
            )?;
        }
        if wrap_root_mapping {
            out.push(')');
        }
        out.push('\n');
        writer.write_all(out.as_bytes())?;
        Ok(())
    }
}

fn root_mapping_needs_wrap(store: &TreeStore, node: NodeId) -> Result<bool, CoreError> {
    let node = store.get(node).ok_or_else(missing_tree_node)?;
    if node.kind != TreeNodeKind::Mapping {
        return Ok(false);
    }
    if node.content.is_empty() {
        return Ok(true);
    }

    for pair in node.content.chunks_exact(2) {
        let key = store.get(pair[0]).ok_or_else(missing_tree_node)?;
        if !is_js_identifier(&key.value) {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use crate::formats::{Decode, Encode, JavascriptObjectDecoder, PythonObjectDecoder};

    use super::JavascriptEncoder;

    #[test]
    fn javascript_encoder_writes_js_literals() {
        let decoded = JavascriptObjectDecoder
            .decode_str("{name: 'Ada', active: true, none: null}")
            .unwrap();
        let encoded = JavascriptEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap();
        assert!(encoded.contains("name: 'Ada'"));
        assert!(encoded.contains("active: true"));
        assert!(encoded.contains("none: null"));
    }

    #[test]
    fn javascript_encoder_quotes_unsafe_keys() {
        let decoded = PythonObjectDecoder
            .decode_str("{'bad-key': 'Ada'}")
            .unwrap();
        let encoded = JavascriptEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap();
        assert!(encoded.contains("'bad-key': 'Ada'"));
    }
}
