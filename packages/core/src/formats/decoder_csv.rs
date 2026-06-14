use crate::core::{CoreError, ParseError, SemType, TreeStore};

use super::{Decode, DecodedDocument, add_scalar, add_sequence, append_child};

#[derive(Debug, Clone, Copy, Default)]
pub struct CsvDecoder;

impl Decode for CsvDecoder {
    fn decode_str(&self, input: &str) -> Result<DecodedDocument, CoreError> {
        let rows = decode_csv(input)?;
        let mut store = TreeStore::new();
        let root = add_sequence(&mut store);
        for row in rows {
            let row_node = add_sequence(&mut store);
            for value in row {
                let child = add_scalar(&mut store, SemType::Str, value);
                append_child(&mut store, row_node, child)?;
            }
            append_child(&mut store, root, row_node)?;
        }
        Ok(DecodedDocument::new(store, root))
    }
}

fn decode_csv(input: &str) -> Result<Vec<Vec<String>>, CoreError> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                row.push(std::mem::take(&mut field));
            }
            '\n' if !in_quotes => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' if !in_quotes => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(ch),
        }
    }

    if in_quotes {
        return Err(CoreError::Parse(ParseError::BadCsv));
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}
