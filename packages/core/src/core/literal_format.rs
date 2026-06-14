use super::{CoreError, TreeNode, ValueRep};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralStyle {
    Json,
    Python,
    Buffer,
}

pub fn format_literal(node: &TreeNode, style: LiteralStyle) -> Result<String, CoreError> {
    let value = node.get_value_rep()?;
    Ok(match (style, value) {
        (LiteralStyle::Json, ValueRep::Nil) => "null".to_string(),
        (LiteralStyle::Python, ValueRep::Nil) => "None".to_string(),
        (_, ValueRep::Nil) => String::new(),
        (_, ValueRep::Boolean(value)) => {
            if style == LiteralStyle::Python {
                if value {
                    "True".to_string()
                } else {
                    "False".to_string()
                }
            } else {
                value.to_string()
            }
        }
        (_, ValueRep::Int(value)) => value.to_string(),
        (_, ValueRep::Float(value)) => value.to_string(),
        (LiteralStyle::Buffer, ValueRep::Str(value)) => format_buffer_literal(value.as_bytes()),
        (LiteralStyle::Python, ValueRep::Str(value)) => format_python_string(&value),
        (_, ValueRep::Str(value)) => format_json_string(&value),
    })
}

pub fn format_json_string(value: &str) -> String {
    crate::formats::escape_json_string(value)
}

pub fn format_python_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        match ch {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('\'');
    out
}

pub fn format_buffer_literal(bytes: &[u8]) -> String {
    let mut out = String::from("0x");
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
