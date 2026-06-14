use crate::core::{
    CoreError, LineIndex, ParseError, SemType, TreeNode, TreeNodeKind, TreeStore, infer_scalar_tag,
};

use super::preferences::FormatPreferences;
use super::{
    Decode, DecodedDocument, add_mapping, add_scalar, add_sequence, append_child, append_key_value,
};

#[derive(Debug, Clone)]
struct CsvCell {
    value: String,
    start_byte: u32,
    end_byte: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvObjectDecoder {
    prefs: FormatPreferences,
}

impl CsvObjectDecoder {
    pub fn new(prefs: FormatPreferences) -> Self {
        Self { prefs }
    }
}

impl Default for CsvObjectDecoder {
    fn default() -> Self {
        Self::new(super::default_language_preferences().effective(crate::core::FormatLanguage::Csv))
    }
}

impl Decode for CsvObjectDecoder {
    fn decode_str(&self, input: &str) -> Result<DecodedDocument, CoreError> {
        let rows = parse_csv_object_rows(input, self.prefs.separator)?;
        let line_index = LineIndex::build(input);
        let mut store = TreeStore::new();
        let root = add_sequence(&mut store);
        if rows.is_empty() {
            return Ok(DecodedDocument::new(store, root));
        }
        let headers = &rows[0];
        for row in rows.iter().skip(1) {
            let mapping = add_mapping(&mut store);
            for (index, header) in headers.iter().enumerate() {
                let value = row.get(index);
                let value_node = add_auto_scalar(
                    &mut store,
                    value.map(|cell| cell.value.as_str()).unwrap_or(""),
                    self.prefs.auto_parse,
                );
                if let Some(value) = value {
                    set_csv_node_span(&mut store, value_node, value, &line_index);
                }
                let key_id =
                    append_key_value(&mut store, mapping, header.value.clone(), value_node)?;
                set_csv_node_span(&mut store, key_id, header, &line_index);
            }
            set_csv_container_span(&mut store, mapping, &line_index);
            append_child(&mut store, root, mapping)?;
        }
        set_csv_root_span(&mut store, root, input, &line_index);
        Ok(DecodedDocument::new(store, root))
    }
}

fn parse_csv_object_rows(input: &str, separator: char) -> Result<Vec<Vec<CsvCell>>, CoreError> {
    let (input, base_offset) = input
        .strip_prefix('\u{feff}')
        .map(|stripped| (stripped, '\u{feff}'.len_utf8()))
        .unwrap_or((input, 0));
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut field_start = base_offset;
    let mut chars = input.char_indices().peekable();
    let mut in_quotes = false;
    let separator = separator.max('\0');

    while let Some((index, ch)) = chars.next() {
        let byte_index = base_offset + index;
        match ch {
            '"' if in_quotes && chars.peek().is_some_and(|(_, next)| *next == '"') => {
                let _ = chars.next();
                field.push('"');
            }
            '"' => in_quotes = !in_quotes,
            ch if ch == separator && !in_quotes => {
                row.push(CsvCell {
                    value: std::mem::take(&mut field),
                    start_byte: field_start as u32,
                    end_byte: byte_index as u32,
                });
                field_start = byte_index + ch.len_utf8();
            }
            '\n' if !in_quotes => {
                row.push(CsvCell {
                    value: std::mem::take(&mut field),
                    start_byte: field_start as u32,
                    end_byte: byte_index as u32,
                });
                rows.push(std::mem::take(&mut row));
                field_start = byte_index + ch.len_utf8();
            }
            '\r' if !in_quotes => {
                let mut next_start = byte_index + ch.len_utf8();
                if chars.peek().is_some_and(|(_, next)| *next == '\n') {
                    if let Some((next_index, next_ch)) = chars.next() {
                        next_start = base_offset + next_index + next_ch.len_utf8();
                    }
                }
                row.push(CsvCell {
                    value: std::mem::take(&mut field),
                    start_byte: field_start as u32,
                    end_byte: byte_index as u32,
                });
                rows.push(std::mem::take(&mut row));
                field_start = next_start;
            }
            _ => field.push(ch),
        }
    }

    if in_quotes {
        return Err(CoreError::Parse(ParseError::BadCsv));
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(CsvCell {
            value: field,
            start_byte: field_start as u32,
            end_byte: input.len().saturating_add(base_offset) as u32,
        });
        rows.push(row);
    }
    Ok(rows)
}

fn set_csv_node_span(
    store: &mut TreeStore,
    id: crate::core::NodeId,
    cell: &CsvCell,
    line_index: &LineIndex,
) {
    let Some(node) = store.get_mut(id) else {
        return;
    };
    node.start_byte = cell.start_byte;
    node.end_byte = cell.end_byte;
    let line_column = line_index.offset_to_line_column(cell.start_byte as usize);
    node.line = line_column.line as i32 + 1;
    node.column = line_column.column as i32 + 1;
}

fn set_csv_container_span(store: &mut TreeStore, id: crate::core::NodeId, line_index: &LineIndex) {
    let Some(children) = store.get(id).map(|node| node.content.clone()) else {
        return;
    };
    let mut range: Option<(u32, u32)> = None;
    for child_id in children {
        let Some(child) = store.get(child_id) else {
            continue;
        };
        if child.end_byte <= child.start_byte {
            continue;
        }
        range = Some(match range {
            Some((start, end)) => (start.min(child.start_byte), end.max(child.end_byte)),
            None => (child.start_byte, child.end_byte),
        });
    }
    let Some((start_byte, end_byte)) = range else {
        return;
    };
    if let Some(node) = store.get_mut(id) {
        node.start_byte = start_byte;
        node.end_byte = end_byte;
        let line_column = line_index.offset_to_line_column(start_byte as usize);
        node.line = line_column.line as i32 + 1;
        node.column = line_column.column as i32 + 1;
    }
}

fn set_csv_root_span(
    store: &mut TreeStore,
    root: crate::core::NodeId,
    source: &str,
    line_index: &LineIndex,
) {
    let Some(node) = store.get_mut(root) else {
        return;
    };
    node.start_byte = 0;
    node.end_byte = source.len() as u32;
    let line_column = line_index.offset_to_line_column(0);
    node.line = line_column.line as i32 + 1;
    node.column = line_column.column as i32 + 1;
}

fn add_auto_scalar(store: &mut TreeStore, raw: &str, auto_parse: bool) -> crate::core::NodeId {
    if raw.is_empty() {
        return add_scalar(store, SemType::Nil, "");
    }

    if auto_parse && raw.starts_with('#') {
        return store.add(TreeNode {
            kind: TreeNodeKind::Scalar,
            sem_type: Some(SemType::Nil),
            tag: SemType::Nil.to_string(),
            line_comment: raw.to_string(),
            ..TreeNode::default()
        });
    }

    if raw == "null" {
        return add_scalar(store, SemType::Nil, raw);
    }

    let tag = infer_scalar_tag("", raw);
    if let Some(sem_type) = SemType::from_string(tag) {
        return add_scalar(store, sem_type, raw);
    }
    add_scalar(store, SemType::Str, raw)
}

#[cfg(test)]
mod tests {
    use crate::core::{SemType, TreeNodeKind, get_map_entry};

    use super::CsvObjectDecoder;
    use crate::core::FormatLanguage;
    use crate::formats::{Decode, Encode, YamlEncoder, default_language_preferences};

    #[test]
    fn csv_object_decoder_builds_sequence_of_mappings() {
        let decoded = CsvObjectDecoder::default()
            .decode_str("a,b\n1,2\n")
            .unwrap();
        let row = decoded
            .store
            .get(decoded.store.get(decoded.root).unwrap().content[0])
            .unwrap();
        assert_eq!(row.kind, TreeNodeKind::Mapping);
        let a = get_map_entry(
            &decoded.store,
            decoded.store.get(decoded.root).unwrap().content[0],
            "a",
        )
        .unwrap()
        .unwrap()
        .value;
        assert_eq!(decoded.store.get(a).unwrap().sem_type, Some(SemType::Int));
    }

    #[test]
    fn csv_object_decoder_auto_parse_false_still_infers_scalar_type() {
        let mut prefs = default_language_preferences().effective(FormatLanguage::Csv);
        prefs.auto_parse = false;
        let decoded = CsvObjectDecoder::new(prefs).decode_str("a\n1\n").unwrap();
        let row_id = decoded.store.get(decoded.root).unwrap().content[0];
        let value = get_map_entry(&decoded.store, row_id, "a")
            .unwrap()
            .unwrap()
            .value;
        assert_eq!(
            decoded.store.get(value).unwrap().sem_type,
            Some(SemType::Int)
        );
        let encoded = YamlEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap();
        assert!(encoded.contains("1"));
    }
}
