use std::io::Write;

use crate::core::{CoreError, NodeId, SemType, TreeNode, TreeNodeKind, TreeStore};

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
        if root.kind == TreeNodeKind::Scalar && self.prefs.unwrap_scalar {
            write_yaml_scalar(store, node_id, &mut out)?;
            out.push('\n');
            writer.write_all(out.as_bytes())?;
            return Ok(());
        }
        if self.prefs.print_doc_separators && root.document > 0 && root.leading_content.is_empty() {
            print_yaml_document_separator(&mut out, "\n", true);
        }
        if !root.leading_content.is_empty() {
            print_yaml_leading_content(
                &mut out,
                &root.leading_content,
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

fn yaml_scalar_value_text(node: &TreeNode) -> String {
    if node.tag == TIMESTAMP_TAG {
        return node.value.clone();
    }

    match SemType::from_string(&node.tag) {
        Some(SemType::Nil) => {
            if node.value.is_empty() {
                "null".to_string()
            } else {
                node.value.clone()
            }
        }
        Some(SemType::Boolean | SemType::Int | SemType::Float) => node.value.clone(),
        _ => {
            if (node.tag.is_empty() || SemType::from_string(&node.tag) == Some(SemType::Str))
                && is_ambiguous_plain_scalar(&node.value)
            {
                return single_quote_yaml(&node.value);
            }
            if needs_double_quotes(&node.value) {
                return double_quote_yaml(&node.value);
            }
            node.value.clone()
        }
    }
}

fn write_yaml_scalar(
    store: &TreeStore,
    node_id: NodeId,
    out: &mut String,
) -> Result<(), CoreError> {
    let current = node(store, node_id)?;
    if current.kind == TreeNodeKind::Alias {
        if !current.value.is_empty() {
            out.push('*');
            out.push_str(&current.value);
            return Ok(());
        }
        if let Some(alias_id) = current.alias {
            let alias = node(store, alias_id)?;
            if !alias.anchor.is_empty() {
                out.push('*');
                out.push_str(&alias.anchor);
                return Ok(());
            }
        }
    }

    if !current.anchor.is_empty() {
        out.push('&');
        out.push_str(&current.anchor);
        out.push(' ');
    }
    if should_emit_tag(&current.tag) {
        out.push_str(&current.tag);
        out.push(' ');
    }
    out.push_str(&yaml_scalar_value_text(current));
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
    if !current.anchor.is_empty() {
        out.push('&');
        out.push_str(&current.anchor);
        out.push(' ');
    }
    if should_emit_tag(&current.tag) {
        out.push_str(&current.tag);
        out.push(' ');
    }

    match current.kind {
        TreeNodeKind::Mapping => {
            out.push('{');
            for (index, pair) in current.content.chunks_exact(2).enumerate() {
                if index != 0 {
                    out.push_str(", ");
                }
                let key = node(store, pair[0])?;
                out.push_str(&yaml_scalar_value_text(key));
                out.push_str(": ");
                let value = node(store, pair[1])?;
                if matches!(value.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence) {
                    write_yaml_flow_node(store, pair[1], out)?;
                } else {
                    out.push_str(&yaml_scalar_value_text(value));
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
                    out.push_str(&yaml_scalar_value_text(child_node));
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
    if !current.head_comment.is_empty() {
        push_comment_block(out, indent, &current.head_comment);
    }

    if emit_anchor
        && !current.anchor.is_empty()
        && matches!(current.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence)
    {
        push_indent(out, indent);
        write_yaml_flow_node(store, node_id, out)?;
        out.push('\n');
        if !current.foot_comment.is_empty() {
            push_comment_block(out, indent, &current.foot_comment);
        }
        return Ok(());
    }

    match current.kind {
        TreeNodeKind::Mapping => {
            if current.content.is_empty() {
                push_indent(out, indent);
                out.push_str("{}\n");
                if !current.foot_comment.is_empty() {
                    push_comment_block(out, indent, &current.foot_comment);
                }
                return Ok(());
            }

            let mut wrote_tag = false;
            for pair in current.content.chunks_exact(2) {
                let key = node(store, pair[0])?;
                let value = node(store, pair[1])?;

                if !key.head_comment.is_empty() {
                    push_comment_block(out, indent, &key.head_comment);
                }

                push_indent(out, indent);
                if !wrote_tag && !current.tag.is_empty() && !SemType::has_tag_prefix(&current.tag) {
                    out.push_str(&current.tag);
                    out.push(' ');
                    wrote_tag = true;
                }
                out.push_str(&quoted_yaml_key(&key.value));
                out.push(':');

                if value.kind == TreeNodeKind::Mapping && value.content.is_empty() {
                    if !value.anchor.is_empty() {
                        out.push_str(" &");
                        out.push_str(&value.anchor);
                    }
                    out.push_str(" {}\n");
                    continue;
                }
                if value.kind == TreeNodeKind::Sequence && value.content.is_empty() {
                    if !value.anchor.is_empty() {
                        out.push_str(" &");
                        out.push_str(&value.anchor);
                    }
                    out.push_str(" []\n");
                    continue;
                }

                if matches!(value.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence) {
                    if !value.anchor.is_empty() {
                        out.push_str(" &");
                        out.push_str(&value.anchor);
                    }
                    out.push('\n');
                    write_yaml_node(
                        store,
                        pair[1],
                        indent_step,
                        indent + indent_step,
                        value.anchor.is_empty(),
                        out,
                    )?;
                } else {
                    out.push(' ');
                    write_yaml_scalar(store, pair[1], out)?;
                    if !value.line_comment.trim().is_empty() {
                        out.push(' ');
                        out.push_str(value.line_comment.trim());
                    }
                    out.push('\n');
                }
            }

            if !current.foot_comment.is_empty() {
                push_comment_block(out, indent, &current.foot_comment);
            }
        }
        TreeNodeKind::Sequence => {
            if !current.tag.is_empty() && !SemType::has_tag_prefix(&current.tag) {
                push_indent(out, indent);
                out.push_str(&current.tag);
                out.push('\n');
            }

            if current.content.is_empty() {
                push_indent(out, indent);
                out.push_str("[]\n");
                if !current.foot_comment.is_empty() {
                    push_comment_block(out, indent, &current.foot_comment);
                }
                return Ok(());
            }

            for child in &current.content {
                let child_node = node(store, *child)?;
                if !child_node.head_comment.is_empty() {
                    push_comment_block(out, indent, &child_node.head_comment);
                }

                push_indent(out, indent);
                if child_node.kind == TreeNodeKind::Mapping && map_is_all_scalar(store, *child)? {
                    if !child_node.anchor.is_empty() || should_emit_tag(&child_node.tag) {
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
                        let key = node(store, pair[0])?;
                        out.push_str(&quoted_yaml_key(&key.value));
                        out.push_str(": ");
                        write_yaml_scalar(store, pair[1], out)?;
                    }
                    out.push('\n');
                    continue;
                }

                if !child_node.anchor.is_empty() || should_emit_tag(&child_node.tag) {
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
                if !child_node.line_comment.trim().is_empty() {
                    out.push(' ');
                    out.push_str(child_node.line_comment.trim());
                }
                out.push('\n');
            }

            if !current.foot_comment.is_empty() {
                push_comment_block(out, indent, &current.foot_comment);
            }
        }
        _ => {
            push_indent(out, indent);
            write_yaml_scalar(store, node_id, out)?;
            if !current.line_comment.trim().is_empty() {
                out.push(' ');
                out.push_str(current.line_comment.trim());
            }
            if !current.foot_comment.is_empty() {
                out.push('\n');
                push_comment_block(out, indent, &current.foot_comment);
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
