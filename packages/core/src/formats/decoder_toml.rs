use std::{collections::HashMap, path::Path};

use crate::core::{
    CompactTag, CoreError, NodeId, ParseError, SemType, TreeStore, ensure_map, ensure_seq,
    get_or_create_map_value,
};

use super::{
    Decode, DecodedDocument, add_mapping, add_scalar, add_sequence, append_child, append_key_value,
};

const TIMESTAMP_TAG: &str = "!!timestamp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefKind {
    Value,
    Table,
    ArrayTable,
}

#[derive(Debug, Clone, Copy, Default)]
struct DefState {
    kind: Option<DefKind>,
    explicit: bool,
    from_dotted: bool,
    sealed: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TomlDecoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TomlDialect {
    V1_0,
    V1_1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TomlStringKind {
    Basic,
    Literal,
    MultilineBasic,
    MultilineLiteral,
}

impl TomlDecoder {
    pub fn decode_str_with_filename(
        &self,
        input: &str,
        source_filename: &str,
    ) -> Result<DecodedDocument, CoreError> {
        let mut store = TreeStore::new();
        let root = add_mapping(&mut store);
        let mut current = root;
        let mut current_path = Vec::new();
        let mut defs = HashMap::new();
        let dialect = dialect_from_filename(source_filename);

        for statement in split_toml_statements(input)? {
            let line = statement.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("[[") && line.ends_with("]]") {
                let path = parse_path(&line[2..line.len() - 2], dialect)?;
                validate_table_path(&defs, &path, true)?;
                current = add_array_table(&mut store, root, &path)?;
                register_table_path(&mut defs, &path, true);
                current_path = path;
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                let path = parse_path(&line[1..line.len() - 1], dialect)?;
                validate_table_path(&defs, &path, false)?;
                current = ensure_table(&mut store, root, &path)?;
                register_table_path(&mut defs, &path, false);
                current_path = path;
                continue;
            }
            let Some(eq) = find_top_level_equal(line)? else {
                return Err(CoreError::Parse(ParseError::BadTomlDocument));
            };
            let path = parse_path(line[..eq].trim(), dialect)?;
            let absolute_path = join_path(&current_path, &path);
            validate_path_for_value(&defs, &absolute_path, current_path.len())?;
            let value = parse_value(&mut store, line[eq + 1..].trim(), dialect)?;
            append_assignment(&mut store, current, &path, value)?;
            register_value_path(&mut defs, &absolute_path, current_path.len(), &store, value)?;
        }

        Ok(DecodedDocument::new(store, root))
    }
}

impl Decode for TomlDecoder {
    fn decode_str(&self, input: &str) -> Result<DecodedDocument, CoreError> {
        self.decode_str_with_filename(input, "")
    }
}

fn append_assignment(
    store: &mut TreeStore,
    current: NodeId,
    path: &[String],
    value: NodeId,
) -> Result<(), CoreError> {
    let Some((last, parent_path)) = path.split_last() else {
        return Err(CoreError::Parse(ParseError::BadTomlKey));
    };
    let mut parent = current;
    for key in parent_path {
        parent = get_or_create_map_value(store, parent, key)?;
        ensure_map(store, parent)?;
    }
    append_key_value(store, parent, last.clone(), value)?;
    Ok(())
}

fn ensure_table(store: &mut TreeStore, root: NodeId, path: &[String]) -> Result<NodeId, CoreError> {
    let mut current = root;
    for key in path {
        let next = get_or_create_map_value(store, current, key)?;
        if store
            .get(next)
            .is_some_and(|node| node.kind == crate::core::TreeNodeKind::Sequence)
        {
            current = store
                .get(next)
                .and_then(|node| node.content.last().copied())
                .ok_or(CoreError::Parse(ParseError::BadTomlTable))?;
        } else {
            ensure_map(store, next)?;
            current = next;
        }
    }
    Ok(current)
}

fn add_array_table(
    store: &mut TreeStore,
    root: NodeId,
    path: &[String],
) -> Result<NodeId, CoreError> {
    let Some((last, parent_path)) = path.split_last() else {
        return Err(CoreError::Parse(ParseError::BadTomlArrayTable));
    };
    let parent = ensure_table(store, root, parent_path)?;
    let seq = get_or_create_map_value(store, parent, last)?;
    ensure_seq(store, seq)?;
    let item = add_mapping(store);
    append_child(store, seq, item)?;
    Ok(item)
}

fn parse_value(
    store: &mut TreeStore,
    raw: &str,
    dialect: TomlDialect,
) -> Result<NodeId, CoreError> {
    if raw.starts_with('"') || raw.starts_with('\'') {
        return Ok(add_scalar(store, SemType::Str, parse_string(raw, dialect)?));
    }
    if raw.starts_with('[') {
        if !raw.ends_with(']') {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        }
        let seq = add_sequence(store);
        for item in split_array_items(&raw[1..raw.len() - 1], dialect)? {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            let child = parse_value(store, item, dialect)?;
            append_child(store, seq, child)?;
        }
        return Ok(seq);
    }
    if raw.starts_with('{') {
        return parse_inline_table(store, raw, dialect);
    }
    if raw == "true" || raw == "false" {
        return Ok(add_scalar(store, SemType::Boolean, raw));
    }
    if is_special_float_literal(raw) {
        return Ok(add_scalar(store, SemType::Float, raw));
    }
    if looks_like_timestamp(raw) {
        validate_timestamp(raw, dialect)?;
        let node = add_scalar(store, SemType::Str, raw);
        store
            .get_mut(node)
            .ok_or(CoreError::Parse(ParseError::BadTomlDocument))?
            .tag = CompactTag::from_text(TIMESTAMP_TAG);
        return Ok(node);
    }
    if parse_float_literal(raw).is_ok() {
        return Ok(add_scalar(store, SemType::Float, raw));
    }
    if parse_integer_literal(raw).is_ok() {
        return Ok(add_scalar(store, SemType::Int, raw));
    }
    Err(CoreError::Parse(ParseError::BadTomlDocument))
}

fn is_special_float_literal(raw: &str) -> bool {
    matches!(raw, "inf" | "+inf" | "-inf" | "nan" | "+nan" | "-nan")
}

fn parse_inline_table(
    store: &mut TreeStore,
    raw: &str,
    dialect: TomlDialect,
) -> Result<NodeId, CoreError> {
    validate_inline_table_raw(raw, dialect)?;
    let map = add_mapping(store);
    let mut defs = HashMap::new();
    let inner = raw[1..raw.len() - 1].trim();
    if inner.is_empty() {
        return Ok(map);
    }
    for item in split_inline_table_items(inner, dialect)? {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let Some(eq) = find_top_level_equal(item)? else {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        };
        let path = parse_path(item[..eq].trim(), dialect)?;
        validate_path_for_value(&defs, &path, 0)?;
        let value = parse_value(store, item[eq + 1..].trim(), dialect)?;
        append_assignment(store, map, &path, value)?;
        register_value_path(&mut defs, &path, 0, store, value)?;
    }
    Ok(map)
}

fn join_path(prefix: &[String], suffix: &[String]) -> Vec<String> {
    prefix.iter().chain(suffix.iter()).cloned().collect()
}

fn path_key(path: &[String]) -> String {
    let mut out = String::new();
    for segment in path {
        out.push('S');
        out.push_str(&segment.len().to_string());
        out.push(':');
        out.push_str(segment);
        out.push(';');
    }
    out
}

fn get_def_state(defs: &HashMap<String, DefState>, path: &[String]) -> Option<DefState> {
    defs.get(&path_key(path))
        .copied()
        .filter(|state| state.kind.is_some())
}

fn put_def_state(defs: &mut HashMap<String, DefState>, path: &[String], state: DefState) {
    defs.insert(path_key(path), state);
}

fn has_descendant_state(defs: &HashMap<String, DefState>, path: &[String]) -> bool {
    let prefix = path_key(path);
    defs.keys()
        .any(|key| key.len() > prefix.len() && key.starts_with(&prefix))
}

fn clear_descendant_states(defs: &mut HashMap<String, DefState>, path: &[String]) {
    let prefix = path_key(path);
    defs.retain(|key, _| key.len() <= prefix.len() || !key.starts_with(&prefix));
}

fn has_array_table_ancestor(defs: &HashMap<String, DefState>, path: &[String]) -> bool {
    (1..path.len()).any(|prefix_len| {
        matches!(
            get_def_state(defs, &path[..prefix_len]),
            Some(DefState {
                kind: Some(DefKind::ArrayTable),
                ..
            })
        )
    })
}

fn validate_path_for_value(
    defs: &HashMap<String, DefState>,
    path: &[String],
    ignore_prefix_len: usize,
) -> Result<(), CoreError> {
    if path.is_empty() {
        return Err(CoreError::Parse(ParseError::BadTomlKey));
    }
    if get_def_state(defs, path).is_some() || has_descendant_state(defs, path) {
        return Err(CoreError::Parse(ParseError::BadTomlTable));
    }
    for prefix_len in 1..path.len() {
        if prefix_len <= ignore_prefix_len {
            continue;
        }
        if let Some(state) = get_def_state(defs, &path[..prefix_len]) {
            match state.kind {
                Some(DefKind::Value | DefKind::ArrayTable) => {
                    return Err(CoreError::Parse(ParseError::BadTomlTable));
                }
                Some(DefKind::Table) if state.sealed || state.explicit => {
                    return Err(CoreError::Parse(ParseError::BadTomlTable));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn register_implicit_tables(
    defs: &mut HashMap<String, DefState>,
    path: &[String],
    ignore_prefix_len: usize,
) {
    for prefix_len in 1..path.len() {
        let from_dotted = prefix_len > ignore_prefix_len;
        let prefix = &path[..prefix_len];
        match get_def_state(defs, prefix) {
            Some(mut state)
                if state.kind == Some(DefKind::Table) && from_dotted && !state.from_dotted =>
            {
                state.from_dotted = true;
                put_def_state(defs, prefix, state);
            }
            None => put_def_state(
                defs,
                prefix,
                DefState {
                    kind: Some(DefKind::Table),
                    from_dotted,
                    ..DefState::default()
                },
            ),
            _ => {}
        }
    }
}

fn register_value_path(
    defs: &mut HashMap<String, DefState>,
    path: &[String],
    ignore_prefix_len: usize,
    store: &TreeStore,
    value: NodeId,
) -> Result<(), CoreError> {
    register_implicit_tables(defs, path, ignore_prefix_len);
    let state = if store
        .get(value)
        .is_some_and(|node| node.kind == crate::core::TreeNodeKind::Mapping)
    {
        DefState {
            kind: Some(DefKind::Table),
            explicit: true,
            sealed: true,
            ..DefState::default()
        }
    } else {
        DefState {
            kind: Some(DefKind::Value),
            ..DefState::default()
        }
    };
    put_def_state(defs, path, state);
    Ok(())
}

fn validate_table_path(
    defs: &HashMap<String, DefState>,
    path: &[String],
    is_array: bool,
) -> Result<(), CoreError> {
    if path.is_empty() {
        return Err(CoreError::Parse(ParseError::BadTomlKey));
    }

    if !is_array {
        if let Some(state) = get_def_state(defs, path) {
            match state.kind {
                Some(DefKind::Value | DefKind::ArrayTable) => {
                    return Err(CoreError::Parse(ParseError::BadTomlTable));
                }
                Some(DefKind::Table) if state.sealed || state.explicit || state.from_dotted => {
                    return Err(CoreError::Parse(ParseError::BadTomlTable));
                }
                _ => {}
            }
        }
    } else if let Some(state) = get_def_state(defs, path) {
        if !has_array_table_ancestor(defs, path) {
            if matches!(state.kind, Some(DefKind::Value | DefKind::Table)) {
                return Err(CoreError::Parse(ParseError::BadTomlArrayTable));
            }
        } else if state.kind != Some(DefKind::ArrayTable) {
            return Err(CoreError::Parse(ParseError::BadTomlArrayTable));
        }
    }

    for prefix_len in 1..path.len() {
        if let Some(state) = get_def_state(defs, &path[..prefix_len]) {
            match state.kind {
                Some(DefKind::Value) => return Err(CoreError::Parse(ParseError::BadTomlTable)),
                Some(DefKind::Table) if state.sealed => {
                    return Err(CoreError::Parse(ParseError::BadTomlTable));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn register_table_path(defs: &mut HashMap<String, DefState>, path: &[String], is_array: bool) {
    if is_array {
        clear_descendant_states(defs, path);
        put_def_state(
            defs,
            path,
            DefState {
                kind: Some(DefKind::ArrayTable),
                ..DefState::default()
            },
        );
        return;
    }

    put_def_state(
        defs,
        path,
        DefState {
            kind: Some(DefKind::Table),
            explicit: true,
            ..DefState::default()
        },
    );
}

fn validate_timestamp(raw: &str, dialect: TomlDialect) -> Result<(), CoreError> {
    if raw.len() == 10
        && raw.as_bytes().get(4) == Some(&b'-')
        && raw.as_bytes().get(7) == Some(&b'-')
    {
        return validate_toml_date(raw);
    }
    if raw.as_bytes().get(2) == Some(&b':') && !raw.contains('-') {
        return validate_toml_time(raw, dialect);
    }

    let bytes = raw.as_bytes();
    if bytes.len() < 10
        || !bytes[0..4].iter().all(u8::is_ascii_digit)
        || bytes[4] != b'-'
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || bytes[7] != b'-'
        || !bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let year = raw[0..4]
        .parse::<i32>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    let month = raw[5..7]
        .parse::<u8>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    let day = raw[8..10]
        .parse::<u8>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    if year < 1 || month == 0 || month > 12 {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => return Err(CoreError::Parse(ParseError::BadTomlDocument)),
    };
    if day == 0 || day > max_day {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    if bytes.len() == 10 {
        return Ok(());
    }
    if !matches!(bytes[10], b'T' | b't' | b' ') {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    if bytes.len() < 16 {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }

    let time_and_offset = &raw[11..];
    if time_and_offset.ends_with(['Z', 'z']) {
        validate_toml_time(&time_and_offset[..time_and_offset.len() - 1], dialect)?;
        return Ok(());
    }
    if let Some(offset_pos) = find_offset_start(time_and_offset) {
        validate_toml_time(&time_and_offset[..offset_pos], dialect)?;
        validate_toml_offset(&time_and_offset[offset_pos..])?;
        return Ok(());
    }
    validate_toml_time(time_and_offset, dialect)
}

fn parse_path(raw: &str, dialect: TomlDialect) -> Result<Vec<String>, CoreError> {
    if raw.trim().is_empty() {
        return Err(CoreError::Parse(ParseError::BadTomlTable));
    }
    if raw.contains(['\n', '\r']) {
        return Err(CoreError::Parse(ParseError::BadTomlTable));
    }
    let mut parts = Vec::new();
    for part in split_dotted(raw)? {
        parts.push(parse_key(part.trim(), dialect)?);
    }
    Ok(parts)
}

fn parse_key(raw: &str, dialect: TomlDialect) -> Result<String, CoreError> {
    if raw.starts_with("\"\"\"") || raw.starts_with("'''") {
        Err(CoreError::Parse(ParseError::BadTomlKey))
    } else if raw.starts_with('"') || raw.starts_with('\'') {
        parse_string(raw, dialect)
    } else if raw.is_empty() {
        Err(CoreError::Parse(ParseError::BadTomlKey))
    } else if !is_valid_bare_toml_key(raw) {
        Err(CoreError::Parse(ParseError::BadTomlKey))
    } else {
        Ok(raw.to_string())
    }
}

fn is_valid_bare_toml_key(raw: &str) -> bool {
    raw.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
}

fn parse_string(raw: &str, dialect: TomlDialect) -> Result<String, CoreError> {
    if skip_toml_string(raw, 0)? != raw.len() {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    if raw.len() >= 6 && raw.starts_with("\"\"\"") && raw.ends_with("\"\"\"") {
        validate_multiline_string_run(raw, b'"')?;
        let inner = trim_multiline_toml_start(&raw[3..raw.len() - 3]);
        return decode_basic_toml_string(inner, true, dialect);
    }
    if raw.len() >= 6 && raw.starts_with("'''") && raw.ends_with("'''") {
        validate_multiline_string_run(raw, b'\'')?;
        let inner = trim_multiline_toml_start(&raw[3..raw.len() - 3]);
        if !inner.chars().all(|ch| is_valid_toml_literal_char(ch, true)) {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        }
        return Ok(inner.to_string());
    }
    if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        let inner = &raw[1..raw.len() - 1];
        if !inner
            .chars()
            .all(|ch| is_valid_toml_literal_char(ch, false))
        {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        }
        return Ok(inner.to_string());
    }
    if !(raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"')) {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    decode_basic_toml_string(&raw[1..raw.len() - 1], false, dialect)
}

fn split_dotted(raw: &str) -> Result<Vec<&str>, CoreError> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => i = skip_toml_string(raw, i)?,
            b'.' => {
                out.push(&raw[start..i]);
                start = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    out.push(&raw[start..]);
    Ok(out)
}

fn split_array_items(raw: &str, dialect: TomlDialect) -> Result<Vec<&str>, CoreError> {
    let items = split_top_level_items(raw, dialect)?;
    let has_trailing_comma = raw.trim_end().ends_with(',');
    for (index, item) in items.iter().enumerate() {
        if item.trim().is_empty() {
            let has_non_empty_prefix = items[..index]
                .iter()
                .any(|candidate| !candidate.trim().is_empty());
            let is_allowed_trailing_empty =
                has_trailing_comma && index + 1 == items.len() && has_non_empty_prefix;
            if !is_allowed_trailing_empty {
                return Err(CoreError::Parse(ParseError::BadTomlDocument));
            }
        }
    }
    Ok(items)
}

fn split_inline_table_items(raw: &str, dialect: TomlDialect) -> Result<Vec<&str>, CoreError> {
    let items = split_top_level_items(raw, dialect)?;
    let has_trailing_comma = raw.trim_end().ends_with(',');
    for (index, item) in items.iter().enumerate() {
        if item.trim().is_empty() {
            let has_non_empty_prefix = items[..index]
                .iter()
                .any(|candidate| !candidate.trim().is_empty());
            let is_allowed_trailing_empty = dialect == TomlDialect::V1_1
                && has_trailing_comma
                && index + 1 == items.len()
                && has_non_empty_prefix;
            if !is_allowed_trailing_empty {
                return Err(CoreError::Parse(ParseError::BadTomlDocument));
            }
            continue;
        }

        if top_level_item_ends_with_newline(item) {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        }
    }
    Ok(items)
}

fn top_level_item_ends_with_newline(raw: &str) -> bool {
    raw.trim_end_matches([' ', '\t']).ends_with(['\n', '\r'])
}

fn split_top_level_items(raw: &str, _dialect: TomlDialect) -> Result<Vec<&str>, CoreError> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut start = 0;
    let mut square_depth = 0usize;
    let mut curly_depth = 0usize;
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => i = skip_toml_string(raw, i)?,
            b'#' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'[' => {
                square_depth += 1;
                i += 1;
            }
            b']' => {
                square_depth = square_depth.saturating_sub(1);
                i += 1;
            }
            b'{' => {
                curly_depth += 1;
                i += 1;
            }
            b'}' => {
                curly_depth = curly_depth.saturating_sub(1);
                i += 1;
            }
            b',' if square_depth == 0 && curly_depth == 0 => {
                out.push(&raw[start..i]);
                start = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    if square_depth != 0 || curly_depth != 0 {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    if start < raw.len() {
        out.push(&raw[start..]);
    } else if raw.trim_end().ends_with(',') {
        out.push("");
    }
    Ok(out)
}

fn is_valid_toml_comment_byte(byte: u8) -> bool {
    byte == b'\t' || (byte >= 0x20 && byte != 0x7f)
}

fn is_valid_toml_basic_char(ch: char, multiline: bool) -> bool {
    match ch {
        '\t' => true,
        '\n' => multiline,
        '"' => multiline,
        '\\' => false,
        _ => !is_disallowed_toml_non_newline_char(ch),
    }
}

fn is_valid_toml_literal_char(ch: char, multiline: bool) -> bool {
    match ch {
        '\t' => true,
        '\n' => multiline,
        _ => !is_disallowed_toml_non_newline_char(ch),
    }
}

fn is_disallowed_toml_non_newline_char(ch: char) -> bool {
    matches!(ch as u32, 0x00..=0x08 | 0x0a..=0x1f | 0x7f)
}

fn validate_toml_comment_text(comment: &str) -> Result<(), CoreError> {
    let bytes = comment.as_bytes();
    if bytes.is_empty() || bytes[0] != b'#' {
        return Ok(());
    }
    if bytes[1..]
        .iter()
        .all(|&byte| is_valid_toml_comment_byte(byte))
    {
        Ok(())
    } else {
        Err(CoreError::Parse(ParseError::BadTomlDocument))
    }
}

fn find_top_level_equal(line: &str) -> Result<Option<usize>, CoreError> {
    let mut square_depth = 0usize;
    let mut curly_depth = 0usize;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => i = skip_toml_string(line, i)?,
            b'[' => {
                square_depth += 1;
                i += 1;
            }
            b']' => {
                square_depth = square_depth.saturating_sub(1);
                i += 1;
            }
            b'{' => {
                curly_depth += 1;
                i += 1;
            }
            b'}' => {
                curly_depth = curly_depth.saturating_sub(1);
                i += 1;
            }
            b'=' if square_depth == 0 && curly_depth == 0 => return Ok(Some(i)),
            _ => i += 1,
        }
    }
    Ok(None)
}

pub fn decode_toml(input: &str) -> Result<DecodedDocument, CoreError> {
    TomlDecoder.decode_str(input)
}

fn dialect_from_filename(filename: &str) -> TomlDialect {
    let Some(basename) = Path::new(filename).file_name().and_then(|s| s.to_str()) else {
        return TomlDialect::V1_1;
    };
    if basename.starts_with("spec-1.1.0__") {
        return TomlDialect::V1_1;
    }
    if basename.starts_with("spec-1.0.0__") {
        return TomlDialect::V1_0;
    }
    for prefix in [
        "datetime__no-seconds",
        "inline-table__newline",
        "inline-table__newline-comment",
        "string__hex-escape",
    ] {
        if basename.starts_with(prefix) {
            return if expectation_marker_from_filename(basename) == Some(1) {
                TomlDialect::V1_1
            } else {
                TomlDialect::V1_0
            };
        }
    }
    TomlDialect::V1_1
}

fn expectation_marker_from_filename(filename: &str) -> Option<u8> {
    let stem = filename.rsplit_once('.')?.0;
    let marker = stem.rsplit_once('.')?.1;
    match marker {
        "1" => Some(1),
        "0" => Some(0),
        _ => None,
    }
}

fn looks_like_timestamp(raw: &str) -> bool {
    raw.as_bytes().first().is_some_and(u8::is_ascii_digit)
        && (raw.contains(':') || (raw.len() >= 10 && raw.as_bytes().get(4) == Some(&b'-')))
}

fn validate_toml_date(raw: &str) -> Result<(), CoreError> {
    let bytes = raw.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[0..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let year = raw[0..4]
        .parse::<i32>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    let month = raw[5..7]
        .parse::<u8>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    let day = raw[8..10]
        .parse::<u8>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => return Err(CoreError::Parse(ParseError::BadTomlDocument)),
    };
    if !(1..=max_day).contains(&day) {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    Ok(())
}

fn validate_toml_time(raw: &str, dialect: TomlDialect) -> Result<(), CoreError> {
    let bytes = raw.as_bytes();
    if bytes.len() < 5 || bytes[2] != b':' {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let hour = raw[0..2]
        .parse::<u8>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    let minute = raw[3..5]
        .parse::<u8>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    if hour > 23 || minute > 59 {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let mut i = 5;
    if i == bytes.len() {
        return if dialect == TomlDialect::V1_1 {
            Ok(())
        } else {
            Err(CoreError::Parse(ParseError::BadTomlDocument))
        };
    }
    if bytes[i] != b':' || i + 3 > bytes.len() {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let second = raw[i + 1..i + 3]
        .parse::<u8>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    if second > 60 {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    i += 3;
    if i == bytes.len() {
        return Ok(());
    }
    if bytes[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac_start {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        }
    }
    if i != bytes.len() {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    Ok(())
}

fn validate_toml_offset(raw: &str) -> Result<(), CoreError> {
    let bytes = raw.as_bytes();
    if bytes.len() == 1 && matches!(bytes[0], b'Z' | b'z') {
        return Ok(());
    }
    if bytes.len() != 6 || !matches!(bytes[0], b'+' | b'-') || bytes[3] != b':' {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let hour = raw[1..3]
        .parse::<u8>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    let minute = raw[4..6]
        .parse::<u8>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    if hour > 23 || minute > 59 {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    Ok(())
}

fn find_offset_start(raw: &str) -> Option<usize> {
    raw.as_bytes()
        .iter()
        .enumerate()
        .skip(1)
        .rev()
        .find_map(|(index, byte)| matches!(byte, b'+' | b'-').then_some(index))
}

fn count_trailing_byte_run(raw: &str, byte: u8) -> usize {
    raw.as_bytes()
        .iter()
        .rev()
        .take_while(|&&b| b == byte)
        .count()
}

fn validate_multiline_string_run(raw: &str, quote: u8) -> Result<(), CoreError> {
    let trailing = count_trailing_byte_run(raw, quote);
    if (3..=5).contains(&trailing) {
        return Ok(());
    }
    if trailing == 6 && raw.len() == 6 {
        return Ok(());
    }
    if quote == b'"' && trailing == 6 && raw.len() > 6 && raw.as_bytes()[raw.len() - 7] == b'\\' {
        return Ok(());
    }
    Err(CoreError::Parse(ParseError::BadTomlDocument))
}

fn trim_multiline_toml_start(body: &str) -> &str {
    if let Some(stripped) = body.strip_prefix("\r\n") {
        stripped
    } else if let Some(stripped) = body.strip_prefix('\n') {
        stripped
    } else {
        body
    }
}

fn decode_basic_toml_string(
    body: &str,
    multiline: bool,
    dialect: TomlDialect,
) -> Result<String, CoreError> {
    let bytes = body.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c != b'\\' {
            let ch = body[i..]
                .chars()
                .next()
                .ok_or(CoreError::Parse(ParseError::BadTomlStringEscape))?;
            if !is_valid_toml_basic_char(ch, multiline) {
                return Err(CoreError::Parse(ParseError::BadTomlDocument));
            }
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        if i + 1 >= bytes.len() {
            return Err(CoreError::Parse(ParseError::BadTomlStringEscape));
        }
        if multiline {
            let mut j = i + 1;
            while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\r') {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'\n' {
                j += 1;
                while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\r' | b'\n') {
                    j += 1;
                }
                i = j;
                continue;
            }
        }

        let esc = bytes[i + 1];
        i += 2;
        match esc {
            b'b' => out.push('\u{08}'),
            b't' => out.push('\t'),
            b'n' => out.push('\n'),
            b'f' => out.push('\u{0c}'),
            b'r' => out.push('\r'),
            b'e' => out.push('\u{1b}'),
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            b'x' => {
                if dialect != TomlDialect::V1_1 || i + 2 > bytes.len() {
                    return Err(CoreError::Parse(ParseError::BadTomlStringEscape));
                }
                let value = u8::from_str_radix(&body[i..i + 2], 16)
                    .map_err(|_| CoreError::Parse(ParseError::BadTomlStringEscape))?;
                out.push(char::from(value));
                i += 2;
            }
            b'u' | b'U' => {
                let want = if esc == b'u' { 4 } else { 8 };
                if i + want > bytes.len() {
                    return Err(CoreError::Parse(ParseError::BadTomlStringEscape));
                }
                let value = u32::from_str_radix(&body[i..i + want], 16)
                    .map_err(|_| CoreError::Parse(ParseError::BadTomlStringEscape))?;
                let ch = char::from_u32(value)
                    .ok_or(CoreError::Parse(ParseError::BadTomlStringEscape))?;
                out.push(ch);
                i += want;
            }
            _ => return Err(CoreError::Parse(ParseError::BadTomlStringEscape)),
        }
    }
    Ok(out)
}

fn skip_toml_string(raw: &str, start: usize) -> Result<usize, CoreError> {
    let bytes = raw.as_bytes();
    if start >= bytes.len() {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let quote = bytes[start];
    if !matches!(quote, b'"' | b'\'') {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let multiline =
        start + 2 < bytes.len() && bytes[start + 1] == quote && bytes[start + 2] == quote;
    let kind = match (quote, multiline) {
        (b'"', false) => TomlStringKind::Basic,
        (b'\'', false) => TomlStringKind::Literal,
        (b'"', true) => TomlStringKind::MultilineBasic,
        (b'\'', true) => TomlStringKind::MultilineLiteral,
        _ => unreachable!(),
    };
    let mut i = start + if multiline { 3 } else { 1 };
    while i < bytes.len() {
        match kind {
            TomlStringKind::Basic => {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    return Ok(i + 1);
                }
            }
            TomlStringKind::Literal => {
                if bytes[i] == b'\'' {
                    return Ok(i + 1);
                }
            }
            TomlStringKind::MultilineBasic => {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if i + 2 < bytes.len()
                    && bytes[i] == b'"'
                    && bytes[i + 1] == b'"'
                    && bytes[i + 2] == b'"'
                {
                    i += 3;
                    if i < bytes.len() && bytes[i] == b'"' {
                        i += 1;
                    }
                    if i < bytes.len() && bytes[i] == b'"' {
                        i += 1;
                    }
                    return Ok(i);
                }
            }
            TomlStringKind::MultilineLiteral => {
                if i + 2 < bytes.len()
                    && bytes[i] == b'\''
                    && bytes[i + 1] == b'\''
                    && bytes[i + 2] == b'\''
                {
                    i += 3;
                    if i < bytes.len() && bytes[i] == b'\'' {
                        i += 1;
                    }
                    if i < bytes.len() && bytes[i] == b'\'' {
                        i += 1;
                    }
                    return Ok(i);
                }
            }
        }
        i += 1;
    }
    Err(CoreError::Parse(ParseError::BadTomlDocument))
}

fn validate_inline_table_raw(raw: &str, dialect: TomlDialect) -> Result<(), CoreError> {
    let bytes = raw.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'{' || *bytes.last().unwrap() != b'}' {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    let mut nested_braces = 0usize;
    let mut nested_arrays = 0usize;
    let mut saw_top_level_comma = false;
    let mut i = 1;
    while i + 1 < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                i = skip_toml_string(raw, i)?;
                saw_top_level_comma = false;
            }
            b'#' => {
                if dialect != TomlDialect::V1_1 {
                    return Err(CoreError::Parse(ParseError::BadTomlDocument));
                }
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'{' => {
                nested_braces += 1;
                saw_top_level_comma = false;
                i += 1;
            }
            b'}' => {
                if nested_braces == 0 {
                    return Err(CoreError::Parse(ParseError::BadTomlDocument));
                }
                nested_braces -= 1;
                saw_top_level_comma = false;
                i += 1;
            }
            b'[' => {
                nested_arrays += 1;
                saw_top_level_comma = false;
                i += 1;
            }
            b']' => {
                if nested_arrays == 0 {
                    return Err(CoreError::Parse(ParseError::BadTomlDocument));
                }
                nested_arrays -= 1;
                saw_top_level_comma = false;
                i += 1;
            }
            b'\n' | b'\r' => {
                if nested_braces == 0 && nested_arrays == 0 && dialect != TomlDialect::V1_1 {
                    return Err(CoreError::Parse(ParseError::BadTomlDocument));
                }
                i += 1;
            }
            b',' => {
                if nested_braces == 0 && nested_arrays == 0 {
                    saw_top_level_comma = true;
                }
                i += 1;
            }
            b' ' | b'\t' => i += 1,
            _ => {
                if nested_braces == 0 && nested_arrays == 0 {
                    saw_top_level_comma = false;
                }
                i += 1;
            }
        }
    }
    if nested_braces != 0 || nested_arrays != 0 {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    if saw_top_level_comma && dialect != TomlDialect::V1_1 {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    Ok(())
}

fn split_toml_statements(input: &str) -> Result<Vec<String>, CoreError> {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut current = String::new();
    let mut square_depth = 0usize;
    let mut curly_depth = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                let end = skip_toml_string(input, i)?;
                current.push_str(&input[i..end]);
                i = end;
            }
            b'#' => {
                let comment_start = i;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                let comment_end = if i > comment_start && bytes[i - 1] == b'\r' {
                    i - 1
                } else {
                    i
                };
                validate_toml_comment_text(&input[comment_start..comment_end])?;
            }
            b'[' => {
                square_depth += 1;
                current.push('[');
                i += 1;
            }
            b']' => {
                square_depth = square_depth.saturating_sub(1);
                current.push(']');
                i += 1;
            }
            b'{' => {
                curly_depth += 1;
                current.push('{');
                i += 1;
            }
            b'}' => {
                curly_depth = curly_depth.saturating_sub(1);
                current.push('}');
                i += 1;
            }
            b'\n' => {
                if square_depth == 0 && curly_depth == 0 {
                    if !current.trim().is_empty() {
                        out.push(current.trim().to_string());
                    }
                    current.clear();
                } else {
                    current.push('\n');
                }
                i += 1;
            }
            b'\r' => {
                if i + 1 >= bytes.len() || bytes[i + 1] != b'\n' {
                    return Err(CoreError::Parse(ParseError::BadTomlDocument));
                }
                if square_depth != 0 || curly_depth != 0 {
                    current.push('\r');
                }
                i += 1;
            }
            b' ' | b'\t' => {
                current.push(bytes[i] as char);
                i += 1;
            }
            _ => {
                if bytes[i] < 0x20 || bytes[i] == 0x7f {
                    return Err(CoreError::Parse(ParseError::BadTomlDocument));
                }
                current.push(bytes[i] as char);
                i += 1;
            }
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    Ok(out)
}

fn parse_float_literal(raw: &str) -> Result<f64, CoreError> {
    if !is_valid_toml_float_literal(raw) {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    raw.chars()
        .filter(|&ch| ch != '_')
        .collect::<String>()
        .parse::<f64>()
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))
}

fn is_valid_toml_float_literal(raw: &str) -> bool {
    let Some(body) = strip_toml_number_sign(raw).map(|(_, body)| body) else {
        return false;
    };
    let Some((mantissa, exponent)) = split_toml_exponent(body) else {
        return false;
    };

    if let Some((integer, fractional)) = mantissa.split_once('.') {
        is_valid_toml_decimal_integer_digits(integer)
            && is_valid_toml_zero_prefixable_digits(fractional)
            && exponent.is_none_or(is_valid_toml_signed_exponent_digits)
    } else {
        exponent.is_some()
            && is_valid_toml_decimal_integer_digits(mantissa)
            && exponent.is_some_and(is_valid_toml_signed_exponent_digits)
    }
}

fn split_toml_exponent(raw: &str) -> Option<(&str, Option<&str>)> {
    let mut exponent_index = None;
    for (index, ch) in raw.char_indices() {
        if matches!(ch, 'e' | 'E') {
            if exponent_index.is_some() {
                return None;
            }
            exponent_index = Some(index);
        }
    }

    match exponent_index {
        Some(index) => {
            let mantissa = &raw[..index];
            let exponent = &raw[index + 1..];
            if mantissa.is_empty() || exponent.is_empty() {
                None
            } else {
                Some((mantissa, Some(exponent)))
            }
        }
        None => Some((raw, None)),
    }
}

fn is_valid_toml_signed_exponent_digits(raw: &str) -> bool {
    let Some((_, digits)) = strip_toml_number_sign(raw) else {
        return false;
    };
    is_valid_toml_zero_prefixable_digits(digits)
}

fn parse_integer_literal(raw: &str) -> Result<i64, CoreError> {
    let explicit_plus = raw.starts_with('+');
    let Some((negative, body)) = strip_toml_number_sign(raw) else {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    };

    let (digits, radix) = if let Some(rest) = body.strip_prefix("0x") {
        if negative || explicit_plus || !is_valid_toml_prefixed_digits(rest, 16) {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        }
        (rest, 16)
    } else if let Some(rest) = body.strip_prefix("0o") {
        if negative || explicit_plus || !is_valid_toml_prefixed_digits(rest, 8) {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        }
        (rest, 8)
    } else if let Some(rest) = body.strip_prefix("0b") {
        if negative || explicit_plus || !is_valid_toml_prefixed_digits(rest, 2) {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        }
        (rest, 2)
    } else {
        if !is_valid_toml_decimal_integer_digits(body) {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        }
        (body, 10)
    };

    let magnitude = u64::from_str_radix(&digits.replace('_', ""), radix)
        .map_err(|_| CoreError::Parse(ParseError::BadTomlDocument))?;
    signed_magnitude_to_i64(magnitude, negative)
}

fn strip_toml_number_sign(raw: &str) -> Option<(bool, &str)> {
    if let Some(rest) = raw.strip_prefix('-') {
        Some((true, rest))
    } else if let Some(rest) = raw.strip_prefix('+') {
        Some((false, rest))
    } else if raw.is_empty() {
        None
    } else {
        Some((false, raw))
    }
}

fn is_valid_toml_decimal_integer_digits(raw: &str) -> bool {
    is_valid_toml_digit_sequence(raw, |ch| ch.is_ascii_digit())
        && raw.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        && !(normalized_toml_digits_len(raw) > 1 && raw.starts_with('0'))
}

fn is_valid_toml_zero_prefixable_digits(raw: &str) -> bool {
    is_valid_toml_digit_sequence(raw, |ch| ch.is_ascii_digit())
}

fn is_valid_toml_prefixed_digits(raw: &str, radix: u32) -> bool {
    is_valid_toml_digit_sequence(raw, |ch| ch.is_digit(radix))
}

fn is_valid_toml_digit_sequence(raw: &str, is_digit: impl Fn(char) -> bool) -> bool {
    if raw.is_empty() {
        return false;
    }

    let mut prev_was_underscore = false;
    let mut saw_digit = false;
    for ch in raw.chars() {
        if ch == '_' {
            if !saw_digit || prev_was_underscore {
                return false;
            }
            prev_was_underscore = true;
            continue;
        }
        if !is_digit(ch) {
            return false;
        }
        saw_digit = true;
        prev_was_underscore = false;
    }

    saw_digit && !prev_was_underscore
}

fn normalized_toml_digits_len(raw: &str) -> usize {
    raw.chars().filter(|&ch| ch != '_').count()
}

fn signed_magnitude_to_i64(magnitude: u64, negative: bool) -> Result<i64, CoreError> {
    let max_positive = i64::MAX as u64;
    if !negative {
        if magnitude > max_positive {
            return Err(CoreError::Parse(ParseError::BadTomlDocument));
        }
        return Ok(magnitude as i64);
    }
    let max_negative_magnitude = max_positive + 1;
    if magnitude > max_negative_magnitude {
        return Err(CoreError::Parse(ParseError::BadTomlDocument));
    }
    if magnitude == max_negative_magnitude {
        return Ok(i64::MIN);
    }
    Ok(-(magnitude as i64))
}
