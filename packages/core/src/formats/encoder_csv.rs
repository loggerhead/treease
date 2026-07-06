use std::io::Write;

use crate::errors::{CoreError, ParseError};
use crate::tree::{NodeId, TreeNodeKind, TreeStore, get_map_entry};

use super::preferences::FormatPreferences;
use super::{Encode, missing_tree_node};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvEncoder {
    separator: char,
}

impl CsvEncoder {
    pub fn new(prefs: FormatPreferences) -> Self {
        Self {
            separator: prefs.separator,
        }
    }
}

impl Default for CsvEncoder {
    fn default() -> Self {
        Self::new(
            super::default_language_preferences().effective(crate::language::FormatLanguage::Csv),
        )
    }
}

impl Encode for CsvEncoder {
    fn encode(
        &self,
        store: &TreeStore,
        node_id: NodeId,
        writer: &mut dyn Write,
    ) -> Result<(), CoreError> {
        let node = store.get(node_id).ok_or_else(missing_tree_node)?;
        match node.kind {
            TreeNodeKind::Scalar => {
                writer.write_all(store.value_for(node_id)?.as_bytes())?;
                writer.write_all(b"\n")?;
            }
            TreeNodeKind::Sequence => {
                if node.content.is_empty() {
                    return Ok(());
                }
                let first = store.get(node.content[0]).ok_or_else(missing_tree_node)?;
                match first.kind {
                    TreeNodeKind::Scalar => {
                        write_sequence_row(store, writer, self.separator, &node.content)?;
                    }
                    TreeNodeKind::Mapping => {
                        let headers = extract_headers(store, node.content[0])?;
                        write_row(writer, self.separator, &headers)?;
                        for row_id in &node.content {
                            write_mapping_row(store, writer, self.separator, *row_id, &headers)?;
                        }
                    }
                    TreeNodeKind::Sequence => {
                        for row_id in &node.content {
                            write_sequence_row(
                                store,
                                writer,
                                self.separator,
                                &store.get(*row_id).ok_or_else(missing_tree_node)?.content,
                            )?;
                        }
                    }
                    _ => return Err(CoreError::Parse(ParseError::BadCsv)),
                }
            }
            _ => return Err(CoreError::Parse(ParseError::BadCsv)),
        }
        Ok(())
    }
}

fn write_sequence_row(
    store: &TreeStore,
    writer: &mut dyn Write,
    separator: char,
    nodes: &[NodeId],
) -> Result<(), CoreError> {
    let values = nodes
        .iter()
        .map(|id| scalar_text(store, *id))
        .collect::<Result<Vec<_>, _>>()?;
    write_row(writer, separator, &values)
}

fn write_mapping_row(
    store: &TreeStore,
    writer: &mut dyn Write,
    separator: char,
    row: NodeId,
    headers: &[String],
) -> Result<(), CoreError> {
    let row_node = store.get(row).ok_or_else(missing_tree_node)?;
    if row_node.kind != TreeNodeKind::Mapping {
        return Err(CoreError::Parse(ParseError::BadCsv));
    }
    let values = headers
        .iter()
        .map(|header| {
            let value = match get_map_entry(store, row, header)? {
                Some(entry) => scalar_text(store, entry.value)?,
                None => String::new(),
            };
            Ok(value)
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    write_row(writer, separator, &values)
}

fn extract_headers(store: &TreeStore, row: NodeId) -> Result<Vec<String>, CoreError> {
    let row_node = store.get(row).ok_or_else(missing_tree_node)?;
    if row_node.kind != TreeNodeKind::Mapping {
        return Err(CoreError::Parse(ParseError::BadCsv));
    }
    row_node
        .content
        .chunks_exact(2)
        .map(|pair| scalar_text(store, pair[0]))
        .collect()
}

fn scalar_text(store: &TreeStore, node_id: NodeId) -> Result<String, CoreError> {
    let node = store.get(node_id).ok_or_else(missing_tree_node)?;
    if node.kind != TreeNodeKind::Scalar {
        return Err(CoreError::Parse(ParseError::BadCsv));
    }
    Ok(store.value_string_for(node_id)?)
}

fn write_row(writer: &mut dyn Write, separator: char, values: &[String]) -> Result<(), CoreError> {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            writer.write_all(&(separator as u8).to_ne_bytes())?;
        }
        write_field(writer, separator, value)?;
    }
    writer.write_all(b"\n")?;
    Ok(())
}

fn write_field(writer: &mut dyn Write, separator: char, value: &str) -> Result<(), CoreError> {
    let needs_quote = value
        .chars()
        .any(|ch| ch == separator || matches!(ch, '"' | '\n' | '\r'));
    if !needs_quote {
        writer.write_all(value.as_bytes())?;
        return Ok(());
    }
    writer.write_all(b"\"")?;
    for ch in value.chars() {
        if ch == '"' {
            writer.write_all(b"\"\"")?;
        } else {
            let mut buffer = [0_u8; 4];
            writer.write_all(ch.encode_utf8(&mut buffer).as_bytes())?;
        }
    }
    writer.write_all(b"\"")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::formats::{CsvObjectDecoder, Decode, Encode};

    use super::CsvEncoder;

    #[test]
    fn csv_encoder_roundtrips_object_rows() {
        let decoded = CsvObjectDecoder::default()
            .decode_str("a,b\n1,\"two,three\"\n")
            .unwrap();
        let encoded = CsvEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .unwrap();
        assert_eq!(encoded, "a,b\n1,\"two,three\"\n");
    }
}
