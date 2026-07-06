use crate::args::{CliError, CommandKind, ParsedArgs};
use crate::{catalog, spec};

pub(crate) fn render_help(parsed: &ParsedArgs) -> String {
    let format = parsed.metadata_format.as_deref().unwrap_or("text");
    let target = resolve_help_target(parsed.metadata_target.as_deref());

    if format == "json" {
        return if let Some(command) = target {
            serde_json::to_string_pretty(&command).expect("CLI command spec JSON should serialize")
                + "\n"
        } else {
            serde_json::to_string_pretty(&spec::root_help_json_value())
                .expect("root CLI help JSON should serialize")
                + "\n"
        };
    }

    match target {
        Some(command) => spec::render_command_text_help(&command),
        None => spec::render_root_text_help(),
    }
}

pub(crate) fn resolve_help_target(target: Option<&str>) -> Option<spec::CliCommandSpec> {
    let target = target?;
    let segments = target.split_whitespace().collect::<Vec<_>>();
    if segments.is_empty() {
        return None;
    }

    let mut path = vec!["treease"];
    path.extend(segments);
    spec::find_command_spec(&path)
}

pub(crate) fn execute_metadata_command(parsed: &ParsedArgs) -> Result<Vec<u8>, CliError> {
    let json = matches!(parsed.metadata_format.as_deref(), Some("json"));
    let output = match parsed.command {
        CommandKind::Help => render_help(parsed),
        CommandKind::OperatorsList => {
            let operators = catalog::operators()
                .into_iter()
                .filter(|operator| {
                    parsed
                        .metadata_category
                        .as_deref()
                        .is_none_or(|category| operator.category == category)
                })
                .collect::<Vec<_>>();
            render_metadata_value(&operators, json)?
        }
        CommandKind::OperatorsGet => {
            let name = parsed
                .metadata_target
                .as_deref()
                .ok_or(CliError::MissingValue("operator name"))?;
            let operator = catalog::find_operator(name)
                .ok_or_else(|| CliError::UnsupportedOperator(name.to_string()))?;
            render_metadata_value(&operator, json)?
        }
        CommandKind::OperatorsSearch => {
            let query = parsed
                .metadata_target
                .as_deref()
                .ok_or(CliError::MissingValue("operator search query"))?;
            render_metadata_value(&catalog::search_operators(query), json)?
        }
        CommandKind::FormatsList => render_metadata_value(&catalog::formats(), json)?,
        CommandKind::FormatsGet => {
            let name = parsed
                .metadata_target
                .as_deref()
                .ok_or(CliError::MissingValue("format name"))?;
            let format = catalog::find_format(name)
                .ok_or_else(|| CliError::UnsupportedFormat(name.to_string()))?;
            render_metadata_value(&format, json)?
        }
        CommandKind::Web => return Err(CliError::InvalidFlagCombination),
        CommandKind::Run => return Err(CliError::InvalidFlagCombination),
    };

    Ok(output.into_bytes())
}

fn render_metadata_value<T: serde::Serialize>(value: &T, _json: bool) -> Result<String, CliError> {
    serde_json::to_string_pretty(value)
        .map(|mut output| {
            output.push('\n');
            output
        })
        .map_err(|error| CliError::Eval(error.to_string()))
}
