use serde::Serialize;

use super::{CliError, CommandKind, ConfiguredFormats, InputPayload, ParsedArgs};

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
        let language = super::resolve_input_format(parsed, input)?;
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
    let ConfiguredFormats { output, .. } = super::configured_formats(&eval_parsed, Some(input))?;

    let output_bytes = super::execute_command(&eval_parsed, inputs)?;

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
