use std::collections::HashMap;

use crate::core::{DocumentTextEdit, SemType, TreeNode};
use crate::document::protocol::GraphPathSeg;
use crate::wasm_types::{PathSeg, PathSegTag, PathSpan};

use super::quote_json_string;
use super::scalar::{ScalarGraphValueEditPlanner, ScalarGraphValueEditRules};

pub(super) static PLANNER: ScalarGraphValueEditPlanner<TomlGraphValueEditRules> =
    ScalarGraphValueEditPlanner::new(TomlGraphValueEditRules);

pub(super) struct TomlGraphValueEditRules;

impl ScalarGraphValueEditRules for TomlGraphValueEditRules {
    fn recover_span(
        &self,
        source: &str,
        path: &[PathSeg<'_>],
        prefer_key: bool,
    ) -> Option<PathSpan> {
        recover_toml_assignment_span(source, path, prefer_key)
    }

    fn build_missing_target_edit(
        &self,
        source: &str,
        path: &[PathSeg<'_>],
        prefer_key: bool,
        replacement: &str,
    ) -> Option<DocumentTextEdit> {
        build_toml_missing_target_edit(source, path, prefer_key, replacement)
    }

    fn format_key(&self, key: &str) -> Option<String> {
        Some(format_toml_key(key))
    }

    fn format_value(&self, value: &serde_json::Value, target: Option<&TreeNode>) -> Option<String> {
        format_toml_value(value, target)
    }
}

fn recover_toml_assignment_span(
    source: &str,
    path: &[PathSeg<'_>],
    prefer_key: bool,
) -> Option<PathSpan> {
    let assignment = find_toml_assignment_line(source, path)?;
    let (start, end) = if prefer_key {
        assignment.key_span
    } else {
        assignment.value_span
    };
    (end > start).then_some(PathSpan {
        start_byte: i32::try_from(start).ok()?,
        end_byte: i32::try_from(end).ok()?,
        row: 0,
        column: 0,
    })
}

fn find_toml_assignment_line(source: &str, path: &[PathSeg<'_>]) -> Option<TomlAssignmentLine> {
    let mut current_context: Vec<GraphPathSeg> = Vec::new();
    let mut array_counts: HashMap<String, usize> = HashMap::new();
    let mut offset = 0usize;

    for line in source.split_inclusive('\n') {
        let line_body = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = line_body.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            offset += line.len();
            continue;
        }
        if let Some((is_array, header_parts)) = parse_toml_header_parts(trimmed) {
            current_context = build_toml_header_context(
                &current_context,
                &header_parts,
                is_array,
                &mut array_counts,
            );
            offset += line.len();
            continue;
        }
        if let Some(pair) = parse_toml_assignment_line(line_body, offset) {
            let mut full_path = current_context.clone();
            full_path.extend(pair.key_parts.iter().cloned().map(|key| GraphPathSeg {
                tag: 0,
                key,
                index: 0,
            }));
            if path_matches_graph_segments(path, &full_path) {
                return Some(pair);
            }
        }
        offset += line.len();
    }

    None
}

#[derive(Debug)]
struct TomlAssignmentLine {
    key_parts: Vec<String>,
    raw_key_text: String,
    key_span: (usize, usize),
    value_span: (usize, usize),
}

fn build_toml_missing_target_edit(
    source: &str,
    path: &[PathSeg<'_>],
    prefer_key: bool,
    replacement: &str,
) -> Option<DocumentTextEdit> {
    let mut normalized_source = normalize_toml_source_keys(source);
    let assignment = find_toml_assignment_line(&normalized_source, path)?;

    if prefer_key {
        normalized_source.replace_range(
            assignment.key_span.0..assignment.key_span.1,
            &format_toml_key(path.last()?.key),
        );
    } else {
        normalized_source.replace_range(
            assignment.value_span.0..assignment.value_span.1,
            replacement,
        );
    }

    Some(DocumentTextEdit {
        start_byte: 0,
        old_end_byte: u32::try_from(source.len()).ok()?,
        new_end_byte: u32::try_from(normalized_source.len()).ok()?,
        replacement: normalized_source,
    })
}

fn normalize_toml_source_keys(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.split_inclusive('\n') {
        let line_body = line.strip_suffix('\n').unwrap_or(line);
        if let Some(pair) = parse_toml_assignment_line(line_body, 0) {
            out.push_str(&line_body[..pair.key_span.0]);
            out.push_str(&normalize_graph_match_key(&pair.raw_key_text));
            out.push_str(&line_body[pair.key_span.1..]);
        } else {
            out.push_str(line_body);
        }
        if line.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

fn parse_toml_header_parts(line: &str) -> Option<(bool, Vec<String>)> {
    let (inner, is_array) = if line.starts_with("[[") && line.ends_with("]]") {
        (&line[2..line.len() - 2], true)
    } else if line.starts_with('[') && line.ends_with(']') {
        (&line[1..line.len() - 1], false)
    } else {
        return None;
    };
    let parts = split_toml_key_parts(inner)?;
    (!parts.is_empty()).then_some((is_array, parts))
}

fn build_toml_header_context(
    current_context: &[GraphPathSeg],
    header_parts: &[String],
    is_array: bool,
    array_counts: &mut HashMap<String, usize>,
) -> Vec<GraphPathSeg> {
    if header_parts.is_empty() {
        return Vec::new();
    }
    let mut context = Vec::new();
    let mut prefix_keys = Vec::new();

    for part in &header_parts[..header_parts.len().saturating_sub(1)] {
        prefix_keys.push(part.clone());
        context.push(GraphPathSeg {
            tag: 0,
            key: part.clone(),
            index: 0,
        });
        if let Some(index) = active_toml_array_index(current_context, &prefix_keys) {
            context.push(GraphPathSeg {
                tag: 1,
                key: String::new(),
                index,
            });
        }
    }

    let last = header_parts.last().cloned().unwrap_or_default();
    let parent_signature = toml_graph_path_signature(&context);
    context.push(GraphPathSeg {
        tag: 0,
        key: last.clone(),
        index: 0,
    });
    if is_array {
        let counter_key = format!("{parent_signature}|{last}");
        let next_index = array_counts.entry(counter_key).or_insert(0usize);
        context.push(GraphPathSeg {
            tag: 1,
            key: String::new(),
            index: i32::try_from(*next_index).unwrap_or(0),
        });
        *next_index += 1;
    }
    context
}

fn active_toml_array_index(
    current_context: &[GraphPathSeg],
    prefix_keys: &[String],
) -> Option<i32> {
    if prefix_keys.is_empty() {
        return None;
    }
    let mut cursor = 0usize;
    for (index, expected) in prefix_keys.iter().enumerate() {
        let segment = current_context.get(cursor)?;
        if segment.tag != 0 || segment.key != *expected {
            return None;
        }
        cursor += 1;
        let maybe_index = current_context.get(cursor);
        if index + 1 == prefix_keys.len() {
            return maybe_index
                .filter(|segment| segment.tag == 1)
                .map(|segment| segment.index);
        }
        if maybe_index.is_some_and(|segment| segment.tag == 1) {
            cursor += 1;
        }
    }
    None
}

fn parse_toml_assignment_line(line: &str, line_offset: usize) -> Option<TomlAssignmentLine> {
    let eq = find_toml_char_outside_quotes(line, '=')?;
    let key_region = &line[..eq];
    let value_region = &line[eq + 1..];
    let key_trimmed = key_region.trim();
    if key_trimmed.is_empty() {
        return None;
    }
    let key_parts = split_toml_key_parts(key_trimmed)?;
    let key_leading_ws = key_region
        .len()
        .saturating_sub(key_region.trim_start().len());
    let key_span = (
        line_offset + key_leading_ws,
        line_offset + key_region.trim_end().len(),
    );

    let value_leading_ws = value_region
        .len()
        .saturating_sub(value_region.trim_start().len());
    let value_start = eq + 1 + value_leading_ws;
    let value_end = line.trim_end().len();
    if value_end <= value_start {
        return None;
    }

    Some(TomlAssignmentLine {
        key_parts,
        raw_key_text: key_trimmed.to_owned(),
        key_span,
        value_span: (line_offset + value_start, line_offset + value_end),
    })
}

fn split_toml_key_parts(raw: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for (index, ch) in raw.char_indices() {
        if in_double {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_double = false,
                _ => {}
            }
            continue;
        }
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        match ch {
            '"' => in_double = true,
            '\'' => in_single = true,
            '.' => {
                parts.push(crate::core::normalize_key_text(&raw[start..index]));
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    if in_single || in_double {
        return None;
    }
    parts.push(crate::core::normalize_key_text(&raw[start..]));
    Some(parts)
}

fn find_toml_char_outside_quotes(text: &str, needle: char) -> Option<usize> {
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for (index, ch) in text.char_indices() {
        if in_double {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_double = false,
                _ => {}
            }
            continue;
        }
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        match ch {
            '"' => in_double = true,
            '\'' => in_single = true,
            ch if ch == needle => return Some(index),
            _ => {}
        }
    }
    None
}

fn path_matches_graph_segments(expected: &[PathSeg<'_>], actual: &[GraphPathSeg]) -> bool {
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(lhs, rhs)| match lhs.tag {
            PathSegTag::Key => {
                rhs.tag == 0
                    && normalize_graph_match_key(lhs.key) == normalize_graph_match_key(&rhs.key)
            }
            PathSegTag::Index => rhs.tag == 1 && lhs.index == rhs.index,
        })
}

fn normalize_graph_match_key(key: &str) -> String {
    key.chars().filter(|ch| !ch.is_control()).collect()
}

fn toml_graph_path_signature(path: &[GraphPathSeg]) -> String {
    let mut out = String::new();
    for segment in path {
        match segment.tag {
            0 => {
                out.push('k');
                out.push_str(&segment.key);
            }
            1 => {
                out.push('i');
                out.push_str(&segment.index.to_string());
            }
            _ => {}
        }
        out.push('|');
    }
    out
}

fn format_toml_value(value: &serde_json::Value, target: Option<&TreeNode>) -> Option<String> {
    match value {
        serde_json::Value::Bool(value) => Some(if *value { "true" } else { "false" }.to_owned()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::String(value) => format_toml_string_for_target(value, target),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            None
        }
    }
}

fn format_toml_string_for_target(value: &str, target: Option<&TreeNode>) -> Option<String> {
    match target.and_then(TreeNode::resolved_sem_type) {
        Some(SemType::Int) => value
            .trim()
            .parse::<i64>()
            .ok()
            .map(|parsed| parsed.to_string()),
        Some(SemType::Float) => value
            .trim()
            .parse::<f64>()
            .ok()
            .map(|parsed| parsed.to_string()),
        Some(SemType::Boolean) => {
            let lowered = value.trim().to_ascii_lowercase();
            match lowered.as_str() {
                "true" | "yes" | "y" => Some("true".to_owned()),
                "false" | "no" | "n" => Some("false".to_owned()),
                _ => Some(quote_json_string(value)),
            }
        }
        _ => Some(quote_json_string(value)),
    }
}

fn format_toml_key(key: &str) -> String {
    if is_bare_toml_key(key) {
        key.to_owned()
    } else {
        quote_json_string(key)
    }
}

fn is_bare_toml_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}
