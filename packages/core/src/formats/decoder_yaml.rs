use crate::core::{
    CoreError, NodeId, ParseError, SemType, TreeNode, TreeNodeKind, TreeStore, tree_sitter_support,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use super::{
    Decode, DecodedDocument, add_mapping, add_scalar, add_sequence, append_child,
    append_existing_key_value,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct YamlDecoder;

#[derive(Default)]
struct YamlDecodeContext {
    anchors: HashMap<String, NodeId>,
}

impl Decode for YamlDecoder {
    fn decode_str(&self, input: &str) -> Result<DecodedDocument, CoreError> {
        let mut store = TreeStore::new();
        if input.trim().is_empty() {
            let root = add_scalar(&mut store, SemType::Nil, "");
            return Ok(DecodedDocument::new(store, root));
        }
        validate_yaml_directives_and_document_starts(input)?;
        let parser_source = prepare_yaml_source_for_parser(input);

        let language = tree_sitter_support::tree_sitter_language("yaml")
            .ok_or(CoreError::Parse(ParseError::InvalidYaml))?;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .map_err(|_| CoreError::Parse(ParseError::InvalidYaml))?;
        let tree = parser
            .parse(parser_source.as_bytes(), None)
            .ok_or(CoreError::Parse(ParseError::InvalidYaml))?;
        if tree.root_node().has_error() {
            return Err(CoreError::Parse(ParseError::InvalidYaml));
        }

        let documents: Vec<_> = named_children(tree.root_node())
            .filter(|child| child.kind() == "document")
            .collect();
        if documents.is_empty() {
            let root = add_scalar(&mut store, SemType::Nil, "");
            return Ok(DecodedDocument::new(store, root));
        }
        for document in &documents {
            validate_document_tags(*document, input)?;
        }
        let document = documents[0];
        let mut ctx = YamlDecodeContext::default();
        let root = match doc_value_node(document) {
            Some(node) => add_yaml_node(&mut store, &mut ctx, input, node, false)?,
            None => add_scalar(&mut store, SemType::Nil, ""),
        };
        Ok(DecodedDocument::new(store, root))
    }

    /// Decode all YAML documents (separated by `---`) from the input.
    ///

    fn decode_all_str(&self, input: &str) -> Result<Vec<DecodedDocument>, CoreError> {
        if input.trim().is_empty() {
            let mut store = TreeStore::new();
            let root = add_scalar(&mut store, SemType::Nil, "");
            return Ok(vec![DecodedDocument::new(store, root)]);
        }
        validate_yaml_directives_and_document_starts(input)?;
        let parser_source = prepare_yaml_source_for_parser(input);

        let language = tree_sitter_support::tree_sitter_language("yaml")
            .ok_or(CoreError::Parse(ParseError::InvalidYaml))?;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .map_err(|_| CoreError::Parse(ParseError::InvalidYaml))?;
        let tree = parser
            .parse(parser_source.as_bytes(), None)
            .ok_or(CoreError::Parse(ParseError::InvalidYaml))?;
        if tree.root_node().has_error() {
            return Err(CoreError::Parse(ParseError::InvalidYaml));
        }

        let documents: Vec<_> = named_children(tree.root_node())
            .filter(|child| child.kind() == "document")
            .collect();

        if documents.is_empty() {
            let mut store = TreeStore::new();
            let root = add_scalar(&mut store, SemType::Nil, "");
            return Ok(vec![DecodedDocument::new(store, root)]);
        }

        for document in &documents {
            validate_document_tags(*document, input)?;
        }

        let mut out = Vec::with_capacity(documents.len());
        for document in &documents {
            let mut store = TreeStore::new();
            let mut ctx = YamlDecodeContext::default();
            let root = match doc_value_node(*document) {
                Some(node) => add_yaml_node(&mut store, &mut ctx, input, node, false)?,
                None => add_scalar(&mut store, SemType::Nil, ""),
            };
            out.push(DecodedDocument::new(store, root));
        }
        Ok(out)
    }
}

fn validate_yaml_directives_and_document_starts(input: &str) -> Result<(), CoreError> {
    let mut directive_window_open = true;
    let mut saw_yaml_directive = false;

    for line in input.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if directive_window_open {
            if let Some(rest) = yaml_directive_body(trimmed) {
                if saw_yaml_directive {
                    return Err(CoreError::Parse(ParseError::InvalidSyntax));
                }
                let directive_body = rest.split_once('#').map(|(body, _)| body).unwrap_or(rest);
                let mut parts = directive_body.split_ascii_whitespace();
                let Some(version) = parts.next() else {
                    return Err(CoreError::Parse(ParseError::InvalidSyntax));
                };
                if parts.next().is_some() || !is_valid_yaml_version(version) {
                    return Err(CoreError::Parse(ParseError::InvalidSyntax));
                }
                saw_yaml_directive = true;
                continue;
            }

            if trimmed.starts_with('%') {
                continue;
            }

            if trimmed.starts_with("---") {
                directive_window_open = false;
                saw_yaml_directive = false;
                continue;
            }
        }

        if trimmed == "..." {
            directive_window_open = true;
            saw_yaml_directive = false;
            continue;
        }

        directive_window_open = false;
    }

    Ok(())
}
fn prepare_yaml_source_for_parser(input: &str) -> Cow<'_, str> {
    let mut directive_window_open = true;
    let mut changed = false;
    let mut out = String::new();
    let mut prefix_len = 0usize;

    for line in input.split_inclusive('\n') {
        let content_len = line.trim_end_matches(['\r', '\n']).len();
        let (content, _) = line.split_at(content_len);
        let trimmed = content.trim_start();
        let in_directive_window = directive_window_open;
        let is_unknown_directive = in_directive_window
            && trimmed.starts_with('%')
            && yaml_directive_body(trimmed).is_none()
            && !is_tag_directive(trimmed);

        if is_unknown_directive {
            if !changed {
                out = String::with_capacity(input.len());
                out.push_str(&input[..prefix_len]);
                changed = true;
            }
            let directive_offset = content.len() - trimmed.len();
            out.push_str(&content[..directive_offset]);
            out.push('#');
            out.push_str(&content[directive_offset + 1..]);
            out.push_str(&line[content_len..]);
        } else if changed {
            out.push_str(line);
        }

        if in_directive_window {
            if yaml_directive_body(trimmed).is_some() || is_tag_directive(trimmed) {
                prefix_len += line.len();
                continue;
            }
            if trimmed.starts_with("---") {
                directive_window_open = false;
                prefix_len += line.len();
                continue;
            }
        }
        if trimmed == "..." {
            directive_window_open = true;
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            directive_window_open = false;
        }
        prefix_len += line.len();
    }

    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(input)
    }
}

fn yaml_directive_body(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("%YAML")?;
    match rest.chars().next() {
        None => Some(rest),
        Some(ch) if ch.is_whitespace() || ch == '#' => Some(rest),
        _ => None,
    }
}

fn is_tag_directive(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("%TAG") else {
        return false;
    };
    match rest.chars().next() {
        None => true,
        Some(ch) => ch.is_whitespace() || ch == '#',
    }
}

fn is_valid_yaml_version(version: &str) -> bool {
    let mut parts = version.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !major.is_empty()
        && !minor.is_empty()
        && major.chars().all(|ch| ch.is_ascii_digit())
        && minor.chars().all(|ch| ch.is_ascii_digit())
}

/// Set byte/line/column span on a TreeStore node from a tree-sitter node.
fn set_ts_node_span(store: &mut TreeStore, id: NodeId, node: tree_sitter::Node) {
    if let Some(tn) = store.get_mut(id) {
        tn.start_byte = node.start_byte() as u32;
        tn.end_byte = node.end_byte() as u32;
        tn.line = (node.start_position().row + 1) as i32;
        tn.column = (node.start_position().column + 1) as i32;
    }
}

fn add_yaml_node(
    store: &mut TreeStore,
    ctx: &mut YamlDecodeContext,
    source: &str,
    node: tree_sitter::Node,
    is_key: bool,
) -> Result<NodeId, CoreError> {
    match node.kind() {
        "block_node" | "flow_node" => {
            let tag_node = first_named_child_of_kind(node, "tag");
            let anchor_node = first_named_child_of_kind(node, "anchor");
            let id = match first_named_child_skip(node, &["tag", "anchor", "comment"]) {
                Some(payload) => add_yaml_node(store, ctx, source, payload, is_key)?,
                None => {
                    let id = add_scalar(store, SemType::Nil, "");
                    set_ts_node_span(store, id, node);
                    id
                }
            };
            if let Some(tag_node) = tag_node {
                apply_yaml_tag(store, id, node_text(source, tag_node).trim());
            }
            if let Some(anchor_node) = anchor_node {
                let raw_anchor = node_text(source, anchor_node);
                let anchor = parse_anchor_name(raw_anchor);
                let anchor_suffix = raw_anchor
                    .trim()
                    .strip_prefix('&')
                    .and_then(|rest| rest.strip_prefix(anchor))
                    .map(str::trim)
                    .unwrap_or("");
                if !anchor_suffix.is_empty() {
                    if let Some(decoded) = store.get_mut(id) {
                        if decoded.kind == TreeNodeKind::Scalar
                            && decoded.resolved_sem_type() == Some(SemType::Str)
                        {
                            decoded.value = if decoded.value.is_empty() {
                                anchor_suffix.to_owned()
                            } else {
                                format!("{anchor_suffix} {}", decoded.value)
                            };
                        }
                    }
                }
                if !anchor.is_empty() {
                    if let Some(decoded) = store.get_mut(id) {
                        decoded.anchor = anchor.to_owned();
                    }
                    ctx.anchors.insert(anchor.to_owned(), id);
                }
            }
            Ok(id)
        }
        "flow_pair" | "block_mapping_pair" => {
            add_yaml_single_pair_mapping(store, ctx, source, node)
        }
        "block_mapping" | "flow_mapping" => add_yaml_mapping(store, ctx, source, node),
        "block_sequence" | "flow_sequence" => add_yaml_sequence(store, ctx, source, node),
        "alias" => Ok(add_yaml_alias(store, ctx, source, node)),
        "single_quote_scalar" | "double_quote_scalar" => {
            validate_yaml_quoted_scalar_line_prefixes(source, node)?;
            let id = add_scalar(
                store,
                SemType::Str,
                parse_yaml_quoted_scalar(node_text(source, node)),
            );
            set_ts_node_span(store, id, node);
            Ok(id)
        }
        "block_scalar" => {
            let id = add_scalar(store, SemType::Str, block_scalar_text(source, node));
            set_ts_node_span(store, id, node);
            Ok(id)
        }
        "plain_scalar" => {
            let scalar_node = first_named_child_skip(node, &[]).unwrap_or(node);
            let raw = node_text(source, scalar_node);
            let id = if is_key {
                add_scalar(store, SemType::Str, fold_yaml_plain_scalar_lines(raw))
            } else {
                add_plain_scalar(store, &fold_yaml_plain_scalar_lines(raw))
            };
            set_ts_node_span(store, id, scalar_node);
            Ok(id)
        }
        _ => {
            let id = add_scalar(store, SemType::Str, node_text(source, node).trim());
            set_ts_node_span(store, id, node);
            Ok(id)
        }
    }
}

fn add_yaml_single_pair_mapping(
    store: &mut TreeStore,
    ctx: &mut YamlDecodeContext,
    source: &str,
    node: tree_sitter::Node,
) -> Result<NodeId, CoreError> {
    let map = add_mapping(store);
    append_yaml_pair(store, ctx, source, map, node)?;
    set_ts_node_span(store, map, node);
    Ok(map)
}

fn add_yaml_mapping(
    store: &mut TreeStore,
    ctx: &mut YamlDecodeContext,
    source: &str,
    node: tree_sitter::Node,
) -> Result<NodeId, CoreError> {
    let map = add_mapping(store);
    for child in named_children(node) {
        match child.kind() {
            "block_mapping_pair" | "flow_pair" => {
                append_yaml_pair(store, ctx, source, map, child)?;
            }
            "flow_node" if node.kind() == "flow_mapping" => {
                let key = add_yaml_node(store, ctx, source, child, true)?;
                let value = add_scalar(store, SemType::Str, "");
                if let Some(child_node) = store.get_mut(value) {
                    child_node.start_byte = child.end_byte() as u32;
                    child_node.end_byte = child.end_byte() as u32;
                    child_node.line = (child.end_position().row + 1) as i32;
                    child_node.column = (child.end_position().column + 1) as i32;
                }
                append_existing_key_value(store, map, key, value)?;
            }
            _ => continue,
        }
    }
    set_ts_node_span(store, map, node);
    Ok(map)
}

fn append_yaml_pair(
    store: &mut TreeStore,
    ctx: &mut YamlDecodeContext,
    source: &str,
    map: NodeId,
    pair: tree_sitter::Node,
) -> Result<(), CoreError> {
    let (key_node, value_node) = yaml_pair_fields(pair);
    let mut key = match key_node {
        Some(key_node) => add_yaml_node(store, ctx, source, key_node, true)?,
        None => add_scalar(store, SemType::Str, ""),
    };
    if pair.kind() == "flow_pair" && flow_pair_key_should_collapse_to_empty_string(store, key) {
        key = add_scalar(store, SemType::Str, "");
    }
    if flow_pair_missing_value_keeps_trailing_colon(pair, key_node, value_node, source) {
        preserve_flow_pair_trailing_colon_in_key(store, key, source, pair);
    }
    let value = match value_node {
        Some(value_node) => add_yaml_node(store, ctx, source, value_node, false)?,
        None => add_scalar(store, SemType::Str, ""),
    };
    append_existing_key_value(store, map, key, value)
}

fn flow_pair_missing_value_keeps_trailing_colon(
    pair: tree_sitter::Node,
    key_node: Option<tree_sitter::Node>,
    value_node: Option<tree_sitter::Node>,
    source: &str,
) -> bool {
    if pair.kind() != "flow_pair" || value_node.is_some() {
        return false;
    }
    let Some(key_node) = key_node else {
        return false;
    };
    let raw_key = node_text(source, key_node).trim_start();
    !matches!(
        raw_key.as_bytes().first(),
        Some(b'"' | b'\'' | b'[' | b'{' | b'*' | b'&')
    )
}

fn preserve_flow_pair_trailing_colon_in_key(
    store: &mut TreeStore,
    key: NodeId,
    source: &str,
    pair: tree_sitter::Node,
) {
    let raw = node_text(source, pair).trim_end();
    if !raw.ends_with(':') {
        return;
    }
    let Some(key_node) = store.get_mut(key) else {
        return;
    };
    if key_node.kind == TreeNodeKind::Scalar
        && key_node.resolved_sem_type() == Some(SemType::Str)
        && !key_node.value.ends_with(':')
    {
        key_node.value.push(':');
    }
}

fn flow_pair_key_should_collapse_to_empty_string(store: &TreeStore, key: NodeId) -> bool {
    let Some(key_node) = store.get(key) else {
        return false;
    };
    key_node.kind != TreeNodeKind::Scalar || key_node.resolved_sem_type() != Some(SemType::Str)
}

fn yaml_pair_fields(
    pair: tree_sitter::Node,
) -> (Option<tree_sitter::Node>, Option<tree_sitter::Node>) {
    let key = pair.child_by_field_name("key");
    let value = pair.child_by_field_name("value");
    if key.is_some() || value.is_some() {
        return (key, value);
    }

    let mut children = named_children(pair).filter(|child| child.kind() != "comment");
    let key = children.next();
    let value = children.next();
    (key, value)
}

fn add_yaml_sequence(
    store: &mut TreeStore,
    ctx: &mut YamlDecodeContext,
    source: &str,
    node: tree_sitter::Node,
) -> Result<NodeId, CoreError> {
    let seq = add_sequence(store);
    for child in named_children(node) {
        if child.kind() == "comment" {
            continue;
        }
        let item_node = if child.kind() == "block_sequence_item" {
            match first_named_child_skip(child, &["comment"]) {
                Some(item) => item,
                None => {
                    let item = add_scalar(store, SemType::Nil, "");
                    set_ts_node_span(store, item, child);
                    append_child(store, seq, item)?;
                    continue;
                }
            }
        } else {
            child
        };
        let item = add_yaml_node(store, ctx, source, item_node, false)?;
        append_child(store, seq, item)?;
    }
    set_ts_node_span(store, seq, node);
    Ok(seq)
}
fn add_yaml_alias(
    store: &mut TreeStore,
    ctx: &YamlDecodeContext,
    source: &str,
    node: tree_sitter::Node,
) -> NodeId {
    let alias_name = parse_alias_name(node_text(source, node));
    let id = store.add(TreeNode {
        kind: TreeNodeKind::Alias,
        value: alias_name.to_owned(),
        alias: ctx.anchors.get(alias_name).copied(),
        ..TreeNode::default()
    });
    set_ts_node_span(store, id, node);
    id
}

fn add_plain_scalar(store: &mut TreeStore, value: &str) -> NodeId {
    if value.eq_ignore_ascii_case("null") || value == "~" {
        return add_scalar(store, SemType::Nil, "");
    }
    if value.eq_ignore_ascii_case("true") {
        return add_scalar(store, SemType::Boolean, "true");
    }
    if value.eq_ignore_ascii_case("false") {
        return add_scalar(store, SemType::Boolean, "false");
    }
    if value.contains(['.', 'e', 'E']) && value.parse::<f64>().is_ok() {
        return add_scalar(store, SemType::Float, value);
    }
    if value.parse::<i64>().is_ok() {
        return add_scalar(store, SemType::Int, value);
    }
    add_scalar(store, SemType::Str, value)
}

fn fold_yaml_plain_scalar_lines(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.contains('\n') {
        return normalized.trim().to_string();
    }

    let lines: Vec<&str> = normalized.split('\n').collect();
    let continuation_indent = lines
        .iter()
        .skip(1)
        .filter_map(|line| {
            if line.trim().is_empty() {
                None
            } else {
                Some(line.len() - line.trim_start_matches([' ', '\t']).len())
            }
        })
        .min()
        .unwrap_or(0);

    let mut out = String::new();
    let mut pending_line_break = false;

    for (index, line) in lines.iter().enumerate() {
        let content = if index == 0 {
            line.trim()
        } else {
            line.get(continuation_indent..).unwrap_or("").trim()
        };

        if content.trim().is_empty() {
            pending_line_break = true;
            continue;
        }

        if !out.is_empty() {
            out.push(if pending_line_break { '\n' } else { ' ' });
        }
        out.push_str(content);
        pending_line_break = false;
    }

    out
}

fn doc_value_node(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    first_named_child_of_kind(node, "block_node")
        .or_else(|| first_named_child_of_kind(node, "flow_node"))
        .or_else(|| first_named_child_of_kind(node, "block_mapping"))
        .or_else(|| first_named_child_of_kind(node, "flow_mapping"))
        .or_else(|| first_named_child_of_kind(node, "block_sequence"))
        .or_else(|| first_named_child_of_kind(node, "flow_sequence"))
        .or_else(|| first_named_child_of_kind(node, "block_mapping_pair"))
        .or_else(|| first_named_child_of_kind(node, "flow_pair"))
        .or_else(|| first_named_child_of_kind(node, "plain_scalar"))
        .or_else(|| first_named_child_of_kind(node, "single_quote_scalar"))
        .or_else(|| first_named_child_of_kind(node, "double_quote_scalar"))
        .or_else(|| first_named_child_of_kind(node, "block_scalar"))
        .or_else(|| first_named_child_of_kind(node, "alias"))
}

fn validate_document_tags(document: tree_sitter::Node, source: &str) -> Result<(), CoreError> {
    let declared_handles: HashSet<String> = named_children(document)
        .filter(|child| child.kind() == "tag_directive")
        .filter_map(|directive| first_named_child_of_kind(directive, "tag_handle"))
        .map(|handle| node_text(source, handle).trim().to_string())
        .collect();

    if let Some(value_node) = doc_value_node(document) {
        validate_node_tags(value_node, source, &declared_handles)?;
    }

    Ok(())
}

fn validate_node_tags(
    node: tree_sitter::Node,
    source: &str,
    declared_handles: &HashSet<String>,
) -> Result<(), CoreError> {
    if node.kind() == "tag" {
        let text = node_text(source, node).trim();
        if let Some(handle) = shorthand_tag_handle(text) {
            if handle != "!!" && !declared_handles.contains(handle) {
                return Err(CoreError::Parse(ParseError::InvalidSyntax));
            }
        }
    }

    for child in named_children(node) {
        validate_node_tags(child, source, declared_handles)?;
    }

    Ok(())
}

fn shorthand_tag_handle(text: &str) -> Option<&str> {
    if !text.starts_with('!') || text.starts_with("!<") {
        return None;
    }

    let rest = &text[1..];
    let second_bang = rest.find('!')?;
    Some(&text[..second_bang + 2])
}

fn parse_anchor_name(raw: &str) -> &str {
    yaml_anchor_or_alias_name(
        raw.trim()
            .strip_prefix('&')
            .map(str::trim)
            .unwrap_or_else(|| raw.trim()),
    )
}

fn parse_alias_name(raw: &str) -> &str {
    yaml_anchor_or_alias_name(
        raw.trim()
            .strip_prefix('*')
            .map(str::trim)
            .unwrap_or_else(|| raw.trim()),
    )
}

fn yaml_anchor_or_alias_name(raw: &str) -> &str {
    let end = raw
        .find(|ch: char| ch.is_whitespace() || matches!(ch, '[' | ']' | '{' | '}' | ',' | ':'))
        .unwrap_or(raw.len());
    &raw[..end]
}

fn apply_yaml_tag(store: &mut TreeStore, id: NodeId, raw_tag: &str) {
    let tag = raw_tag
        .split_ascii_whitespace()
        .next()
        .unwrap_or(raw_tag)
        .trim();
    if tag.is_empty() {
        return;
    }
    if let Some(node) = store.get_mut(id) {
        node.tag = tag.to_owned();
        if tag == "!!timestamp" && node.kind == TreeNodeKind::Scalar {
            node.sem_type = Some(SemType::Str);
        }
    }
}

fn first_named_child_of_kind<'tree>(
    node: tree_sitter::Node<'tree>,
    kind: &str,
) -> Option<tree_sitter::Node<'tree>> {
    named_children(node).find(|child| child.kind() == kind)
}

fn first_named_child_skip<'tree>(
    node: tree_sitter::Node<'tree>,
    skipped: &[&str],
) -> Option<tree_sitter::Node<'tree>> {
    named_children(node).find(|child| !skipped.contains(&child.kind()))
}

fn named_children<'tree>(
    node: tree_sitter::Node<'tree>,
) -> impl Iterator<Item = tree_sitter::Node<'tree>> {
    (0..node.named_child_count()).filter_map(move |index| node.named_child(index as u32))
}

fn node_text<'a>(source: &'a str, node: tree_sitter::Node) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn parse_yaml_quoted_scalar(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    if trimmed.len() < 2 || trimmed.as_bytes().first() != trimmed.as_bytes().last() {
        return trimmed.to_string();
    }

    let body = &trimmed[1..trimmed.len() - 1];
    match trimmed.as_bytes()[0] {
        b'\'' => fold_yaml_quoted_scalar_lines(body).replace("''", "'"),
        b'"' => {
            let continued = remove_yaml_double_quoted_line_continuations(body);
            let canonical = canonicalize_yaml_double_quoted_whitespace_escapes(&continued);
            let folded = fold_yaml_quoted_scalar_lines(&canonical);
            unescape_yaml_double_quoted_scalar(&folded)
        }
        _ => trimmed.to_string(),
    }
}

fn validate_yaml_quoted_scalar_line_prefixes(
    source: &str,
    node: tree_sitter::Node,
) -> Result<(), CoreError> {
    let raw = node_text(source, node);
    if !raw.contains(['\n', '\r']) {
        return Ok(());
    }

    let required_indent = quoted_scalar_required_line_prefix(node);
    if required_indent == 0 {
        return Ok(());
    }

    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    for line in normalized.split('\n').skip(1) {
        if line.trim_matches([' ', '\t']).is_empty() {
            continue;
        }
        let leading_spaces = line.bytes().take_while(|byte| *byte == b' ').count();
        if leading_spaces < required_indent {
            return Err(CoreError::Parse(ParseError::InvalidYaml));
        }
    }

    Ok(())
}

fn quoted_scalar_required_line_prefix(node: tree_sitter::Node) -> usize {
    let mut current = node;
    while let Some(parent) = current.parent() {
        match parent.kind() {
            "block_mapping_pair" | "block_sequence_item" => {
                return parent.start_position().column + 1;
            }
            _ => {
                current = parent;
            }
        }
    }

    0
}

fn canonicalize_yaml_double_quoted_whitespace_escapes(raw: &str) -> String {
    raw.replace("\\\t", "\\t")
}

fn remove_yaml_double_quoted_line_continuations(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(normalized.len());
    let bytes = normalized.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
            i += 2;
            while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n') {
                i += 1;
            }
            continue;
        }

        let ch = normalized[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

fn fold_yaml_quoted_scalar_lines(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.contains('\n') {
        return normalized;
    }

    let lines: Vec<&str> = normalized.split('\n').collect();
    let continuation_indent = lines
        .iter()
        .skip(1)
        .filter_map(|line| {
            if line.trim().is_empty() {
                None
            } else {
                Some(line.len() - line.trim_start_matches([' ', '\t']).len())
            }
        })
        .min()
        .unwrap_or(0);

    let starts_with_newline = normalized.starts_with('\n');
    let ends_with_newline = normalized.ends_with('\n');
    let first_line_had_trailing_ws = lines
        .first()
        .is_some_and(|line| line.ends_with([' ', '\t']));
    let mut out = String::new();
    let mut pending_breaks = 0usize;
    let mut content_lines_emitted = 0usize;

    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            pending_breaks += 1;
        }

        let mut content = if index == 0 {
            *line
        } else {
            line.get(continuation_indent..).unwrap_or("")
        };

        let preserve_leading = index == 0 && !starts_with_newline;
        let preserve_trailing = index + 1 == lines.len() && !ends_with_newline;

        if !preserve_leading {
            content = content.trim_start_matches([' ', '\t']);
        }
        if !preserve_trailing {
            content = content.trim_end_matches([' ', '\t']);
        }

        if content.is_empty() {
            continue;
        }

        if !out.is_empty() || pending_breaks > 0 {
            if pending_breaks >= 2 {
                out.push('\n');
            } else if !(content_lines_emitted == 1
                && continuation_indent == 0
                && !starts_with_newline
                && !first_line_had_trailing_ws)
            {
                out.push(' ');
            } else {
                // yq concatenates the first physical line of a quoted scalar directly
                // with an unindented continuation line.
            }
        }
        out.push_str(content);
        pending_breaks = 0;
        content_lines_emitted += 1;
    }

    if pending_breaks > 0 && !out.is_empty() {
        out.push(if pending_breaks >= 2 { '\n' } else { ' ' });
    } else if pending_breaks > 0 {
        let effective_breaks =
            pending_breaks + usize::from(lines.last().is_some_and(|line| !line.is_empty()));
        match effective_breaks {
            0 | 1 => {}
            2 => out.push(' '),
            n => out.push_str(&"\n".repeat(n - 2)),
        }
    }

    out
}

fn unescape_yaml_double_quoted_scalar(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'\\' || i + 1 >= bytes.len() {
            let ch = raw[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }

        match bytes[i + 1] {
            b'0' => {
                out.push('\0');
                i += 2;
            }
            b'a' => {
                out.push('\u{07}');
                i += 2;
            }
            b'b' => {
                out.push('\u{08}');
                i += 2;
            }
            b't' => {
                out.push('\t');
                i += 2;
            }
            b'n' => {
                out.push('\n');
                i += 2;
            }
            b'v' => {
                out.push('\u{0b}');
                i += 2;
            }
            b'f' => {
                out.push('\u{0c}');
                i += 2;
            }
            b'r' => {
                out.push('\r');
                i += 2;
            }
            b'e' => {
                out.push('\u{1b}');
                i += 2;
            }
            b' ' => {
                out.push(' ');
                i += 2;
            }
            b'"' => {
                out.push('"');
                i += 2;
            }
            b'/' => {
                out.push('/');
                i += 2;
            }
            b'\\' => {
                out.push('\\');
                i += 2;
            }
            b'N' => {
                out.push('\u{85}');
                i += 2;
            }
            b'_' => {
                out.push('\u{a0}');
                i += 2;
            }
            b'L' => {
                out.push('\u{2028}');
                i += 2;
            }
            b'P' => {
                out.push('\u{2029}');
                i += 2;
            }
            b'x' => {
                if let Some((ch, consumed)) = parse_yaml_hex_escape(raw, i + 2, 2) {
                    out.push(ch);
                    i = consumed;
                } else {
                    out.push('\\');
                    out.push('x');
                    i += 2;
                }
            }
            b'u' => {
                if let Some((ch, consumed)) = parse_yaml_hex_escape(raw, i + 2, 4) {
                    out.push(ch);
                    i = consumed;
                } else {
                    out.push('\\');
                    out.push('u');
                    i += 2;
                }
            }
            b'U' => {
                if let Some((ch, consumed)) = parse_yaml_hex_escape(raw, i + 2, 8) {
                    out.push(ch);
                    i = consumed;
                } else {
                    out.push('\\');
                    out.push('U');
                    i += 2;
                }
            }
            _ => {
                out.push('\\');
                let ch = raw[i + 1..].chars().next().unwrap();
                out.push(ch);
                i += 1 + ch.len_utf8();
            }
        }
    }

    out
}

fn parse_yaml_hex_escape(raw: &str, start: usize, width: usize) -> Option<(char, usize)> {
    let end = start.checked_add(width)?;
    let digits = raw.get(start..end)?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    Some((char::from_u32(value)?, end))
}

fn block_scalar_text(source: &str, node: tree_sitter::Node) -> String {
    let raw = node_text(source, node);
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let scalar_has_terminating_line_break = normalized.ends_with('\n')
        || source
            .get(node.end_byte()..)
            .and_then(|suffix| suffix.as_bytes().first().copied())
            .is_some_and(|byte| matches!(byte, b'\n' | b'\r'));
    let Some((header, rest)) = normalized.split_once('\n') else {
        return String::new();
    };

    let (style, chomping, explicit_indent) = parse_yaml_block_scalar_header(header);
    let header_has_prefix_content = yaml_block_scalar_header_has_prefix_content(source, node);
    let detected_indent = rest
        .split('\n')
        .filter(|line| !line.trim().is_empty())
        .map(yaml_line_indent)
        .min();
    let blank_only_indent = rest
        .split('\n')
        .filter(|line| !line.is_empty())
        .map(yaml_line_indent)
        .min();
    let indent = match (explicit_indent, detected_indent) {
        (Some(explicit), _) => yaml_block_scalar_explicit_indent(node, explicit),
        (None, Some(detected)) => detected,
        (None, None) => blank_only_indent.unwrap_or(0),
    };

    let mut lines = Vec::new();
    let mut saw_content = false;
    for line in rest.split('\n') {
        let line_indent = yaml_line_indent(line);
        let is_blank = line.trim().is_empty();
        if !saw_content
            && matches!(style, YamlBlockScalarStyle::Literal)
            && header_has_prefix_content
            && is_blank
            && line_indent < indent
        {
            continue;
        }
        if !saw_content && (!is_blank || line_indent >= indent) {
            saw_content = true;
        }
        lines.push(if line.len() >= indent {
            &line[indent..]
        } else {
            ""
        });
    }

    let mut out = match style {
        YamlBlockScalarStyle::Literal => lines.join("\n"),
        YamlBlockScalarStyle::Folded => fold_yaml_block_scalar_lines(&lines),
    };

    out.push_str(&yaml_block_scalar_trailing_breaks(source, node));
    apply_yaml_block_chomping(&mut out, chomping, scalar_has_terminating_line_break);
    out
}

fn yaml_line_indent(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

fn yaml_block_scalar_explicit_indent(node: tree_sitter::Node, explicit_indent: usize) -> usize {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() == "block_mapping_pair" {
            return parent.start_position().column + explicit_indent;
        }
        current = parent;
    }
    explicit_indent
}

fn yaml_block_scalar_header_has_prefix_content(source: &str, node: tree_sitter::Node) -> bool {
    let start = node.start_byte();
    let before = source.get(..start).unwrap_or("");
    let line_start = before.rfind('\n').map_or(0, |idx| idx + 1);
    !source
        .get(line_start..start)
        .unwrap_or("")
        .trim_matches([' ', '\t'])
        .is_empty()
}

fn yaml_block_scalar_trailing_breaks(source: &str, node: tree_sitter::Node) -> String {
    let suffix = source.get(node.end_byte()..).unwrap_or("");
    let normalized = suffix.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.is_empty() {
        return String::new();
    }
    let mut out = String::new();

    for segment in normalized.split_inclusive('\n') {
        let body = segment.strip_suffix('\n').unwrap_or(segment);
        if body.trim_matches([' ', '\t']).is_empty() {
            out.push('\n');
            continue;
        }
        break;
    }

    out
}

#[derive(Clone, Copy)]
enum YamlBlockScalarStyle {
    Literal,
    Folded,
}

#[derive(Clone, Copy)]
enum YamlBlockScalarChomping {
    Clip,
    Strip,
    Keep,
}

fn parse_yaml_block_scalar_header(
    header: &str,
) -> (YamlBlockScalarStyle, YamlBlockScalarChomping, Option<usize>) {
    let trimmed = header.trim();
    let style = if trimmed.starts_with('>') {
        YamlBlockScalarStyle::Folded
    } else {
        YamlBlockScalarStyle::Literal
    };

    let chomping = if trimmed.contains('+') {
        YamlBlockScalarChomping::Keep
    } else if trimmed.contains('-') {
        YamlBlockScalarChomping::Strip
    } else {
        YamlBlockScalarChomping::Clip
    };

    let explicit_indent = trimmed
        .chars()
        .find_map(|ch| ch.to_digit(10))
        .map(|d| d as usize);
    (style, chomping, explicit_indent)
}

fn fold_yaml_block_scalar_lines(lines: &[&str]) -> String {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum LineKind {
        Normal,
        MoreIndented,
    }

    let mut out = String::new();
    let mut pending_empty_lines = 0usize;
    let mut prev_kind: Option<LineKind> = None;
    let mut saw_content = false;

    for line in lines {
        if line.is_empty() {
            pending_empty_lines += 1;
            continue;
        }

        let kind = if line.starts_with([' ', '\t']) {
            LineKind::MoreIndented
        } else {
            LineKind::Normal
        };

        if !saw_content {
            out.push_str(&"\n".repeat(pending_empty_lines));
        } else if pending_empty_lines > 0 {
            let extra = usize::from(
                prev_kind == Some(LineKind::MoreIndented) || kind == LineKind::MoreIndented,
            );
            out.push_str(&"\n".repeat(pending_empty_lines + extra));
        } else if prev_kind == Some(LineKind::Normal) && kind == LineKind::Normal {
            out.push(' ');
        } else {
            out.push('\n');
        }

        out.push_str(line);
        pending_empty_lines = 0;
        saw_content = true;
        prev_kind = Some(kind);
    }

    out
}

fn apply_yaml_block_chomping(
    out: &mut String,
    chomping: YamlBlockScalarChomping,
    scalar_has_terminating_line_break: bool,
) {
    match chomping {
        YamlBlockScalarChomping::Keep => {}
        YamlBlockScalarChomping::Strip => {
            while out.ends_with('\n') {
                out.pop();
            }
        }
        YamlBlockScalarChomping::Clip => {
            let had_content = !out.is_empty() && out.chars().any(|ch| ch != '\n');
            while out.ends_with('\n') {
                out.pop();
            }
            if had_content && scalar_has_terminating_line_break {
                out.push('\n');
            }
        }
    }
}

pub fn decode_yaml(input: &str) -> Result<DecodedDocument, CoreError> {
    YamlDecoder.decode_str(input)
}

#[cfg(test)]
mod tests {
    use super::YamlDecoder;
    use crate::core::{CoreError, ParseError, TreeNodeKind};
    use crate::evaluator::{AllAtOnceEvaluator, Value};
    use crate::formats::Decode;

    #[test]
    fn yaml_decoder_rejects_duplicate_yaml_directive_before_document_start() {
        assert_eq!(
            YamlDecoder
                .decode_str("%YAML 1.2\n%YAML 1.2\n---\n")
                .unwrap_err(),
            CoreError::Parse(ParseError::InvalidSyntax)
        );
    }

    #[test]
    fn yaml_decoder_rejects_extra_words_on_yaml_directive() {
        assert_eq!(
            YamlDecoder.decode_str("%YAML 1.2 foo\n---\n").unwrap_err(),
            CoreError::Parse(ParseError::InvalidSyntax)
        );
    }

    #[test]
    fn yaml_decoder_decodes_direct_block_mapping_document() {
        let decoded = YamlDecoder.decode_str("foo: 1\n").unwrap();
        let root = decoded.store.get(decoded.root).unwrap();
        assert_eq!(root.kind, TreeNodeKind::Mapping);

        let selected = AllAtOnceEvaluator::new()
            .evaluate_nodes(&decoded.store, ".foo", &[decoded.root])
            .unwrap();
        assert_eq!(selected, vec![Value::Number(1.0)]);
    }

    #[test]
    fn yaml_decoder_allows_anchor_after_document_start_marker() {
        assert!(YamlDecoder.decode_str("--- &sequence\n- a\n").is_ok());
    }

    #[test]
    fn yaml_decoder_allows_comment_after_yaml_directive() {
        assert!(
            YamlDecoder
                .decode_str("%YAML 1.3 # Attempt parsing\n---\n\"foo\"\n")
                .is_ok()
        );
    }

    #[test]
    fn yaml_decoder_allows_yaml_like_scalar_line_in_document_body() {
        assert!(YamlDecoder.decode_str("---\nscalar\n%YAML 1.2\n").is_ok());
    }

    #[test]
    fn yaml_decoder_allows_unknown_directive_before_document_start() {
        assert!(YamlDecoder.decode_str("%YAMLL 1.1\n---\n").is_ok());
    }

    #[test]
    fn yaml_decoder_rejects_misindented_multiline_quoted_mapping_values() {
        for input in [
            "foo: \"bar\n\tbaz\"\n",
            "---\nquoted: \"a\nb\nc\"\n",
            "---\nquoted: 'a\nb\nc'\n",
        ] {
            assert_eq!(
                YamlDecoder.decode_str(input).unwrap_err(),
                CoreError::Parse(ParseError::InvalidYaml)
            );
        }
    }

    #[test]
    fn yaml_decoder_allows_multiline_quoted_mapping_values_with_line_prefixes() {
        assert!(YamlDecoder.decode_str("foo: \"bar\n  \tbaz\"\n").is_ok());
        assert!(
            YamlDecoder
                .decode_str("---\nquoted: \"a\n  b\n  c\"\n")
                .is_ok()
        );
    }
}
