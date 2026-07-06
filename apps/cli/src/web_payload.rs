use serde::Serialize;

use super::execute::ConfiguredFormats;
use crate::args::{CliError, CommandKind, InputPayload, ParsedArgs};

// Used by follow-up web server wiring.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct CliGraphResultPayload {
    pub source_label: String,
    pub expression: String,
    pub language: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct CliGraphMetadataPayload {
    pub source_label: String,
    pub expression: String,
    pub language: String,
    pub source_url: String,
    pub byte_length: usize,
}

// Used by follow-up web server wiring.
#[allow(dead_code)]
pub(super) fn build_cli_graph_result_payload(
    parsed: &ParsedArgs,
    inputs: &[InputPayload],
) -> Result<CliGraphResultPayload, CliError> {
    if parsed.command != CommandKind::Web {
        return Err(CliError::InvalidFlagCombination);
    }
    if inputs.len() != 1 {
        return Err(CliError::InvalidWebInputCount);
    }

    let input = &inputs[0];
    if should_delegate_identity_to_web(parsed) {
        let language = super::cli_io::input::resolve_input_format(parsed, input)?;
        let text = String::from_utf8(input.bytes.clone()).map_err(|error| {
            CliError::Eval(format!("source document is not valid UTF-8: {error}"))
        })?;
        return Ok(CliGraphResultPayload {
            source_label: input.display_name().to_string(),
            expression: parsed.expression.clone(),
            language,
            text,
        });
    }

    let eval_parsed = ParsedArgs {
        command: CommandKind::Run,
        inplace: false,
        exit_status: false,
        null_input: false,
        files: vec![input.name.clone()],
        ..parsed.clone()
    };
    let ConfiguredFormats { output, .. } =
        super::execute::configured_formats(&eval_parsed, Some(input))?;

    let output_bytes = super::execute::execute_command(&eval_parsed, inputs)?;

    if let Some(text) = build_missing_result_placeholder(&eval_parsed, input)? {
        return Ok(CliGraphResultPayload {
            source_label: input.display_name().to_string(),
            expression: parsed.expression.clone(),
            language: "json".to_string(),
            text,
        });
    }

    let text = String::from_utf8(output_bytes).map_err(|error| {
        CliError::Eval(format!("expression result is not valid UTF-8: {error}"))
    })?;

    Ok(CliGraphResultPayload {
        source_label: input.display_name().to_string(),
        expression: parsed.expression.clone(),
        language: output,
        text,
    })
}

pub(super) fn should_delegate_identity_to_web(parsed: &ParsedArgs) -> bool {
    parsed.expression.trim() == "."
        && parsed.output_format.is_none()
        && parsed.pretty_print.is_none()
        && parsed.indent.is_none()
        && !parsed.unwrap_scalar
        && !parsed.no_doc
}

fn build_missing_result_placeholder(
    parsed: &ParsedArgs,
    input: &InputPayload,
) -> Result<Option<String>, CliError> {
    let input_format = super::cli_io::input::resolve_input_format(parsed, input)?;
    let source_text = String::from_utf8(input.bytes.clone())
        .map_err(|error| CliError::Eval(format!("source document is not valid UTF-8: {error}")))?;
    let codec = treease_core::core::CodecService::new();
    let decoded = codec
        .decode(&input_format, &source_text)
        .map_err(|error| CliError::Eval(format!("{error:?}")))?;
    let json_text = codec
        .minify_to_string("json", &decoded.store, decoded.root)
        .map_err(|error| CliError::Eval(format!("{error:?}")))?;
    let input_value = serde_json::from_str::<serde_json::Value>(&json_text)
        .map_err(|error| CliError::Eval(format!("failed to decode JSON bridge value: {error}")))?;

    Ok(simple_path_is_missing(&input_value, &parsed.expression).then(|| "\"miss\"".to_string()))
}

fn simple_path_is_missing(root: &serde_json::Value, expression: &str) -> bool {
    let Some(segments) = parse_simple_path_expression(expression) else {
        return false;
    };

    let mut current = root;
    for segment in segments {
        match segment {
            SimplePathSegment::Key(key) => {
                let Some(next) = current.as_object().and_then(|value| value.get(key)) else {
                    return true;
                };
                current = next;
            }
            SimplePathSegment::Index(index) => {
                let Some(next) = current.as_array().and_then(|value| value.get(index)) else {
                    return true;
                };
                current = next;
            }
        }
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SimplePathSegment<'a> {
    Key(&'a str),
    Index(usize),
}

fn parse_simple_path_expression(expression: &str) -> Option<Vec<SimplePathSegment<'_>>> {
    let trimmed = expression.trim();
    if !trimmed.starts_with('.') || trimmed == "." {
        return None;
    }

    let bytes = trimmed.as_bytes();
    let mut cursor = 1usize;
    let mut segments = Vec::new();

    while cursor < bytes.len() {
        if bytes[cursor] == b'[' {
            let start = cursor + 1;
            let end = trimmed[start..].find(']')? + start;
            let index = trimmed[start..end].parse::<usize>().ok()?;
            segments.push(SimplePathSegment::Index(index));
            cursor = end + 1;
            if cursor < bytes.len() && bytes[cursor] == b'.' {
                cursor += 1;
            }
            continue;
        }

        let start = cursor;
        while cursor < bytes.len() && bytes[cursor] != b'.' && bytes[cursor] != b'[' {
            cursor += 1;
        }
        if start == cursor {
            return None;
        }
        segments.push(SimplePathSegment::Key(&trimmed[start..cursor]));
        if cursor < bytes.len() && bytes[cursor] == b'.' {
            cursor += 1;
        }
    }

    Some(segments)
}
