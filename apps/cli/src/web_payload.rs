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
