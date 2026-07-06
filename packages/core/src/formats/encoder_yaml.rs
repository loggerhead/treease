use std::io::Write;

use crate::errors::CoreError;
use crate::language::SemType;
use crate::tree::{NodeId, TreeNode, TreeNodeKind, TreeStore};

use super::encoder::{Encode, print_yaml_document_separator, print_yaml_leading_content};
use super::node;
use super::preferences::FormatPreferences;

#[derive(Debug, Clone, Default)]
pub struct YamlEncoder {
    pub prefs: FormatPreferences,
}

impl YamlEncoder {
    pub fn new(prefs: FormatPreferences) -> Self {
        Self { prefs }
    }
}

impl Encode for YamlEncoder {
    fn encode(
        &self,
        store: &TreeStore,
        node_id: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        let root = node(store, node_id)?;
        let mut out = String::new();
        let root_leading_content = store.leading_content_for(node_id).unwrap_or_default();
        if root.kind == TreeNodeKind::Scalar && self.prefs.unwrap_scalar {
            write_yaml_scalar(store, node_id, &mut out)?;
            out.push('\n');
            writer.write_all(out.as_bytes())?;
            return Ok(());
        }
        if self.prefs.print_doc_separators && root.document > 0 && root_leading_content.is_empty() {
            print_yaml_document_separator(&mut out, "\n", true);
        }
        if !root_leading_content.is_empty() {
            print_yaml_leading_content(
                &mut out,
                root_leading_content,
                self.prefs.print_doc_separators,
            );
        }
        write_yaml_node(
            store,
            node_id,
            self.prefs.indent.max(2) as usize,
            0,
            true,
            &mut out,
        )?;
        if !out.ends_with('\n') {
            out.push('\n');
        }
        writer.write_all(out.as_bytes())?;
        Ok(())
    }
}

const TIMESTAMP_TAG: &str = "!!timestamp";

fn push_indent(out: &mut String, count: usize) {
    for _ in 0..count {
        out.push(' ');
    }
}

fn push_comment_block(out: &mut String, indent: usize, comment: &str) {
    for line in comment.lines() {
        let trimmed = line.trim_matches([' ', '\t', '\r']);
        if trimmed.is_empty() {
            continue;
        }
        push_indent(out, indent);
        if trimmed.starts_with('#') {
            out.push_str(trimmed);
        } else {
            out.push_str("# ");
            out.push_str(trimmed);
        }
        out.push('\n');
    }
}

fn needs_double_quotes(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let bytes = value.as_bytes();
    if matches!(bytes[0], b'-' | b'?' | b':' | b'#' | b'!' | b'@')
        || bytes[0] == b' '
        || bytes[value.len() - 1] == b' '
    {
        return true;
    }
    value.chars().any(|ch| {
        matches!(
            ch,
            '\n' | '\r'
                | '\t'
                | ':'
                | '{'
                | '}'
                | '['
                | ']'
                | ','
                | '#'
                | '&'
                | '*'
                | '|'
                | '>'
                | '\\'
                | '"'
        )
    })
}

fn is_ambiguous_plain_scalar(value: &str) -> bool {
    matches!(value, "null" | "true" | "false") || is_number_like(value)
}

fn is_number_like(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn single_quote_yaml(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

fn double_quote_yaml(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn should_emit_tag(tag: &str) -> bool {
    !tag.is_empty() && (!SemType::has_tag_prefix(tag) || SemType::from_string(tag).is_none())
}

fn quoted_yaml_key(value: &str) -> String {
    if is_ambiguous_plain_scalar(value) {
        single_quote_yaml(value)
    } else if needs_double_quotes(value) {
        double_quote_yaml(value)
    } else {
        value.to_string()
    }
}

fn node_tag(node: &TreeNode) -> &str {
    node.tag_str()
}

fn yaml_scalar_value_text(store: &TreeStore, node_id: NodeId) -> Result<String, CoreError> {
    let node = node(store, node_id)?;
    if node_tag(node) == TIMESTAMP_TAG {
        return Ok(store.value_string_for(node_id)?);
    }

    Ok(match SemType::from_string(node_tag(node)) {
        Some(SemType::Nil) => {
            if store.value_for(node_id)?.is_empty() {
                "null".to_string()
            } else {
                store.value_string_for(node_id)?
            }
        }
        Some(SemType::Boolean | SemType::Int | SemType::Float) => {
            store.value_string_for(node_id)?
        }
        _ => {
            if (node_tag(node).is_empty()
                || SemType::from_string(node_tag(node)) == Some(SemType::Str))
                && is_ambiguous_plain_scalar(store.value_for(node_id)?)
            {
                return Ok(single_quote_yaml(store.value_for(node_id)?));
            }
            if needs_double_quotes(store.value_for(node_id)?) {
                return Ok(double_quote_yaml(store.value_for(node_id)?));
            }
            store.value_string_for(node_id)?
        }
    })
}

fn write_yaml_scalar(
    store: &TreeStore,
    node_id: NodeId,
    out: &mut String,
) -> Result<(), CoreError> {
    let current = node(store, node_id)?;
    if current.kind == TreeNodeKind::Alias {
        let current_value = store.value_for(node_id)?;
        if !current_value.is_empty() {
            out.push('*');
            out.push_str(current_value);
            return Ok(());
        }
        if let Some(alias_id) = current.alias() {
            let alias_anchor = store.anchor_for(alias_id).unwrap_or_default();
            if !alias_anchor.is_empty() {
                out.push('*');
                out.push_str(alias_anchor);
                return Ok(());
            }
        }
    }

    let current_anchor = store.anchor_for(node_id).unwrap_or_default();
    if !current_anchor.is_empty() {
        out.push('&');
        out.push_str(current_anchor);
        out.push(' ');
    }
    if should_emit_tag(node_tag(current)) {
        out.push_str(node_tag(current));
        out.push(' ');
    }
    out.push_str(&yaml_scalar_value_text(store, node_id)?);
    Ok(())
}

fn map_is_all_scalar(store: &TreeStore, node_id: NodeId) -> Result<bool, CoreError> {
    let current = node(store, node_id)?;
    if current.kind != TreeNodeKind::Mapping {
        return Ok(false);
    }
    for pair in current.content.chunks_exact(2) {
        let value = node(store, pair[1])?;
        if matches!(value.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn write_yaml_flow_node(
    store: &TreeStore,
    node_id: NodeId,
    out: &mut String,
) -> Result<(), CoreError> {
    let current = node(store, node_id)?;
    let current_anchor = store.anchor_for(node_id).unwrap_or_default();
    if !current_anchor.is_empty() {
        out.push('&');
        out.push_str(current_anchor);
        out.push(' ');
    }
    if should_emit_tag(node_tag(current)) {
        out.push_str(node_tag(current));
        out.push(' ');
    }

    match current.kind {
        TreeNodeKind::Mapping => {
            out.push('{');
            for (index, pair) in current.content.chunks_exact(2).enumerate() {
                if index != 0 {
                    out.push_str(", ");
                }
                out.push_str(&yaml_scalar_value_text(store, pair[0])?);
                out.push_str(": ");
                let value = node(store, pair[1])?;
                if matches!(value.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence) {
                    write_yaml_flow_node(store, pair[1], out)?;
                } else {
                    out.push_str(&yaml_scalar_value_text(store, pair[1])?);
                }
            }
            out.push('}');
        }
        TreeNodeKind::Sequence => {
            out.push('[');
            for (index, child) in current.content.iter().enumerate() {
                if index != 0 {
                    out.push_str(", ");
                }
                let child_node = node(store, *child)?;
                if matches!(
                    child_node.kind,
                    TreeNodeKind::Mapping | TreeNodeKind::Sequence
                ) {
                    write_yaml_flow_node(store, *child, out)?;
                } else {
                    out.push_str(&yaml_scalar_value_text(store, *child)?);
                }
            }
            out.push(']');
        }
        _ => write_yaml_scalar(store, node_id, out)?,
    }

    Ok(())
}

fn write_yaml_node(
    store: &TreeStore,
    node_id: NodeId,
    indent_step: usize,
    indent: usize,
    emit_anchor: bool,
    out: &mut String,
) -> Result<(), CoreError> {
    let current = node(store, node_id)?;
    let current_head_comment = store.head_comment_for(node_id).unwrap_or_default();
    let current_foot_comment = store.foot_comment_for(node_id).unwrap_or_default();
    let current_line_comment = store.line_comment_for(node_id).unwrap_or_default();
    let current_anchor = store.anchor_for(node_id).unwrap_or_default();
    if !current_head_comment.is_empty() {
        push_comment_block(out, indent, current_head_comment);
    }

    if emit_anchor
        && !current_anchor.is_empty()
        && matches!(current.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence)
    {
        push_indent(out, indent);
        write_yaml_flow_node(store, node_id, out)?;
        out.push('\n');
        if !current_foot_comment.is_empty() {
            push_comment_block(out, indent, current_foot_comment);
        }
        return Ok(());
    }

    match current.kind {
        TreeNodeKind::Mapping => {
            if current.content.is_empty() {
                push_indent(out, indent);
                out.push_str("{}\n");
                if !current_foot_comment.is_empty() {
                    push_comment_block(out, indent, current_foot_comment);
                }
                return Ok(());
            }

            let mut wrote_tag = false;
            for pair in current.content.chunks_exact(2) {
                let _key = node(store, pair[0])?;
                let value = node(store, pair[1])?;
                let key_head_comment = store.head_comment_for(pair[0]).unwrap_or_default();
                let value_anchor = store.anchor_for(pair[1]).unwrap_or_default();
                let value_line_comment = store.line_comment_for(pair[1]).unwrap_or_default();

                if !key_head_comment.is_empty() {
                    push_comment_block(out, indent, key_head_comment);
                }

                push_indent(out, indent);
                if !wrote_tag
                    && !node_tag(current).is_empty()
                    && !SemType::has_tag_prefix(node_tag(current))
                {
                    out.push_str(node_tag(current));
                    out.push(' ');
                    wrote_tag = true;
                }
                out.push_str(&quoted_yaml_key(store.value_for(pair[0])?));
                out.push(':');

                if value.kind == TreeNodeKind::Mapping && value.content.is_empty() {
                    if !value_anchor.is_empty() {
                        out.push_str(" &");
                        out.push_str(value_anchor);
                    }
                    out.push_str(" {}\n");
                    continue;
                }
                if value.kind == TreeNodeKind::Sequence && value.content.is_empty() {
                    if !value_anchor.is_empty() {
                        out.push_str(" &");
                        out.push_str(value_anchor);
                    }
                    out.push_str(" []\n");
                    continue;
                }

                if matches!(value.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence) {
                    if !value_anchor.is_empty() {
                        out.push_str(" &");
                        out.push_str(value_anchor);
                    }
                    out.push('\n');
                    write_yaml_node(
                        store,
                        pair[1],
                        indent_step,
                        indent + indent_step,
                        value_anchor.is_empty(),
                        out,
                    )?;
                } else {
                    out.push(' ');
                    write_yaml_scalar(store, pair[1], out)?;
                    if !value_line_comment.trim().is_empty() {
                        out.push(' ');
                        out.push_str(value_line_comment.trim());
                    }
                    out.push('\n');
                }
            }

            if !current_foot_comment.is_empty() {
                push_comment_block(out, indent, current_foot_comment);
            }
        }
        TreeNodeKind::Sequence => {
            if !node_tag(current).is_empty() && !SemType::has_tag_prefix(node_tag(current)) {
                push_indent(out, indent);
                out.push_str(node_tag(current));
                out.push('\n');
            }

            if current.content.is_empty() {
                push_indent(out, indent);
                out.push_str("[]\n");
                if !current_foot_comment.is_empty() {
                    push_comment_block(out, indent, current_foot_comment);
                }
                return Ok(());
            }

            for child in &current.content {
                let child_node = node(store, *child)?;
                let child_head_comment = store.head_comment_for(*child).unwrap_or_default();
                let child_anchor = store.anchor_for(*child).unwrap_or_default();
                let child_line_comment = store.line_comment_for(*child).unwrap_or_default();
                if !child_head_comment.is_empty() {
                    push_comment_block(out, indent, child_head_comment);
                }

                push_indent(out, indent);
                if child_node.kind == TreeNodeKind::Mapping && map_is_all_scalar(store, *child)? {
                    if !child_anchor.is_empty() || should_emit_tag(node_tag(child_node)) {
                        out.push_str("- ");
                        write_yaml_flow_node(store, *child, out)?;
                        out.push('\n');
                        continue;
                    }

                    out.push_str("- ");
                    for (index, pair) in child_node.content.chunks_exact(2).enumerate() {
                        if index != 0 {
                            out.push('\n');
                            push_indent(out, indent + 2);
                        }
                        let _key = node(store, pair[0])?;
                        out.push_str(&quoted_yaml_key(store.value_for(pair[0])?));
                        out.push_str(": ");
                        write_yaml_scalar(store, pair[1], out)?;
                    }
                    out.push('\n');
                    continue;
                }

                if !child_anchor.is_empty() || should_emit_tag(node_tag(child_node)) {
                    out.push_str("- ");
                    write_yaml_flow_node(store, *child, out)?;
                    out.push('\n');
                    continue;
                }

                if matches!(
                    child_node.kind,
                    TreeNodeKind::Mapping | TreeNodeKind::Sequence
                ) {
                    out.push_str("- \n");
                    write_yaml_node(store, *child, indent_step, indent + indent_step, true, out)?;
                    continue;
                }

                out.push_str("- ");
                write_yaml_scalar(store, *child, out)?;
                if !child_line_comment.trim().is_empty() {
                    out.push(' ');
                    out.push_str(child_line_comment.trim());
                }
                out.push('\n');
            }

            if !current_foot_comment.is_empty() {
                push_comment_block(out, indent, current_foot_comment);
            }
        }
        _ => {
            push_indent(out, indent);
            write_yaml_scalar(store, node_id, out)?;
            if !current_line_comment.trim().is_empty() {
                out.push(' ');
                out.push_str(current_line_comment.trim());
            }
            if !current_foot_comment.is_empty() {
                out.push('\n');
                push_comment_block(out, indent, current_foot_comment);
            } else {
                out.push('\n');
            }
        }
    }

    Ok(())
}

pub fn encode_yaml(store: &TreeStore, node: NodeId) -> Result<String, CoreError> {
    YamlEncoder::default().encode_to_string(store, node)
}
