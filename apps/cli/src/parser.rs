#[cfg(not(target_arch = "wasm32"))]
use clap::{Arg, ArgAction, ArgMatches, Command, error::ErrorKind};

use super::{CliError, CommandKind, ParsedArgs, spec};

pub(super) fn parse_args_with_clap(argv: &[String]) -> Result<ParsedArgs, CliError> {
    reject_removed_legacy_eval_commands(argv)?;

    #[cfg(not(target_arch = "wasm32"))]
    {
        parse_args_with_clap_impl(argv)
    }
    #[cfg(target_arch = "wasm32")]
    {
        parse_args_fallback(argv)
    }
}

fn reject_removed_legacy_eval_commands(argv: &[String]) -> Result<(), CliError> {
    let Some(first_arg) = argv.get(1) else {
        return Ok(());
    };

    if matches!(first_arg.as_str(), "e" | "eval-all" | "ea") {
        return Err(CliError::UnknownCommand(first_arg.clone()));
    }

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_args_with_clap_impl(argv: &[String]) -> Result<ParsedArgs, CliError> {
    let root_spec = spec::root_command_spec();
    let matches = build_command(root_spec.clone())
        .try_get_matches_from(argv)
        .map_err(map_clap_error)?;

    let mut parsed = ParsedArgs::default();
    parsed.null_input = matches.get_flag("null-input");
    parsed.exit_status = matches.get_flag("exit-status");
    parsed.input_format = matches.get_one::<String>("input-format").cloned();
    parsed.output_format = matches.get_one::<String>("output-format").cloned();
    parsed.pretty_print = matches.get_flag("pretty-print").then_some(true);
    parsed.indent = matches.get_one::<i32>("indent").copied();
    parsed.unwrap_scalar = matches.get_flag("unwrap-scalar");
    parsed.no_doc = matches.get_flag("no-doc");
    parsed.inplace = matches.get_flag("inplace");

    if matches.get_flag("help") {
        parsed.command = CommandKind::Help;
        parsed.help = true;
    }

    if let Some((subcommand_name, submatches)) = matches.subcommand() {
        let subcommand_spec = root_spec
            .subcommands
            .iter()
            .find(|subcommand| subcommand.name == subcommand_name)
            .ok_or_else(|| CliError::Eval(format!("unknown command: {subcommand_name}")))?;
        apply_matches(&mut parsed, subcommand_spec, submatches)?;
    } else {
        if let Some(expression) = matches.get_one::<String>("expression") {
            parsed.expression = expression.clone();
        }
        parsed.files = matches
            .get_many::<String>("file")
            .into_iter()
            .flatten()
            .cloned()
            .collect();
    }

    Ok(parsed)
}

#[cfg(not(target_arch = "wasm32"))]
fn build_command(spec: spec::CliCommandSpec) -> Command {
    let name = leak(spec.name);
    let requires_leaf_subcommand =
        spec.id != spec::CliCommandId::Root && !spec.subcommands.is_empty();
    let mut command = Command::new(name)
        .disable_help_flag(true)
        .disable_help_subcommand(true)
        .subcommand_required(requires_leaf_subcommand)
        .arg_required_else_help(false)
        .about(spec.summary.clone());

    if !spec.usage.is_empty() {
        command = command.override_usage(spec.usage.clone());
    }

    for option in spec.options {
        command = command.arg(build_option(option));
    }

    for argument in spec.arguments {
        command = command.arg(build_argument(argument));
    }

    for subcommand in spec.subcommands {
        command = command.subcommand(build_command(subcommand));
    }

    command
}

#[cfg(not(target_arch = "wasm32"))]
fn build_option(option: spec::CliOptionSpec) -> Arg {
    let option_name = option.name.clone();
    let option_long = leak(option.long.trim_start_matches("--").to_string());
    let mut arg = Arg::new(leak(option_name.clone()))
        .long(option_long)
        .help(option.help.clone());

    if let Some(short) = option
        .short
        .as_deref()
        .and_then(|short| short.chars().nth(1))
    {
        arg = arg.short(short);
    }

    if option.takes_value {
        arg = arg.action(ArgAction::Set);
        if let Some(value_name) = option.value_name {
            arg = arg.value_name(leak(value_name));
        }
        if option_name == "indent" {
            arg = arg.value_parser(clap::value_parser!(i32));
        }
    } else {
        arg = arg.action(ArgAction::SetTrue);
    }

    arg
}

#[cfg(not(target_arch = "wasm32"))]
fn build_argument(argument: spec::CliArgumentSpec) -> Arg {
    let mut arg = Arg::new(leak(argument.name)).help(argument.help.clone());

    if !argument.required {
        arg = arg.required(false);
    }

    if argument.multiple || argument.repeated {
        arg = arg.num_args(0..).allow_hyphen_values(true);
    }

    arg
}

#[cfg(not(target_arch = "wasm32"))]
fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_matches(
    parsed: &mut ParsedArgs,
    command_spec: &spec::CliCommandSpec,
    matches: &ArgMatches,
) -> Result<(), CliError> {
    if let Some(command_kind) = command_kind_for_id(&command_spec.id) {
        parsed.command = command_kind;
    }

    match command_spec.id {
        spec::CliCommandId::Help => {
            parsed.help = true;
            parsed.metadata_target = matches.get_one::<String>("command").cloned();
            parsed.metadata_format = matches.get_one::<String>("format").cloned();
        }
        spec::CliCommandId::OperatorsList => {
            parsed.metadata_category = matches.get_one::<String>("category").cloned();
            parsed.metadata_format = matches.get_one::<String>("format").cloned();
        }
        spec::CliCommandId::OperatorsGet
        | spec::CliCommandId::FormatsGet
        | spec::CliCommandId::ExamplesGet => {
            parsed.metadata_target = matches.get_one::<String>("name").cloned();
            parsed.metadata_format = matches.get_one::<String>("format").cloned();
        }
        spec::CliCommandId::FormatsList | spec::CliCommandId::ExamplesList => {
            parsed.metadata_format = matches.get_one::<String>("format").cloned();
        }
        spec::CliCommandId::OperatorsSearch | spec::CliCommandId::ExamplesSearch => {
            parsed.metadata_target = matches.get_one::<String>("query").cloned();
        }
        spec::CliCommandId::Doctor => {
            parsed.metadata_format = matches.get_one::<String>("format").cloned();
        }
        _ => {}
    }

    if let Some((subcommand_name, submatches)) = matches.subcommand() {
        let child_spec = command_spec
            .subcommands
            .iter()
            .find(|subcommand| subcommand.name == subcommand_name)
            .ok_or_else(|| CliError::Eval(format!("unknown command: {subcommand_name}")))?;
        apply_matches(parsed, child_spec, submatches)?;
    }

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn command_kind_for_id(command_id: &spec::CliCommandId) -> Option<CommandKind> {
    match command_id {
        spec::CliCommandId::Root => Some(CommandKind::Run),
        spec::CliCommandId::Help => Some(CommandKind::Help),
        spec::CliCommandId::OperatorsList => Some(CommandKind::OperatorsList),
        spec::CliCommandId::OperatorsGet => Some(CommandKind::OperatorsGet),
        spec::CliCommandId::OperatorsSearch => Some(CommandKind::OperatorsSearch),
        spec::CliCommandId::FormatsList => Some(CommandKind::FormatsList),
        spec::CliCommandId::FormatsGet => Some(CommandKind::FormatsGet),
        spec::CliCommandId::ExamplesList => Some(CommandKind::ExamplesList),
        spec::CliCommandId::ExamplesGet => Some(CommandKind::ExamplesGet),
        spec::CliCommandId::ExamplesSearch => Some(CommandKind::ExamplesSearch),
        spec::CliCommandId::Doctor => Some(CommandKind::Doctor),
        spec::CliCommandId::Operators
        | spec::CliCommandId::Formats
        | spec::CliCommandId::Examples => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn map_clap_error(error: clap::Error) -> CliError {
    match error.kind() {
        ErrorKind::UnknownArgument => {
            let rendered = error.render().to_string();
            let flag = rendered
                .split_whitespace()
                .find(|token| token.starts_with('-'))
                .unwrap_or("--unknown");
            CliError::UnknownFlag(flag.trim_matches('\'').to_string())
        }
        ErrorKind::InvalidValue => {
            let rendered = error.render().to_string();
            if rendered.contains("--indent") {
                CliError::InvalidIndent(rendered)
            } else {
                CliError::Eval(rendered)
            }
        }
        _ => CliError::Eval(error.render().to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
fn parse_args_fallback(argv: &[String]) -> Result<ParsedArgs, CliError> {
    let mut parsed = ParsedArgs::default();
    let mut index = 1;

    let mut expression: Option<String> = None;
    while index < argv.len() {
        let arg = &argv[index];
        if expression.is_some() {
            parsed.files.push(arg.clone());
            index += 1;
            continue;
        }

        match arg.as_str() {
            "-h" | "--help" => {
                parsed.help = true;
                parsed.command = CommandKind::Help;
            }
            "-n" | "--null-input" => parsed.null_input = true,
            "-e" | "--exit-status" => parsed.exit_status = true,
            "-P" | "--prettyPrint" => parsed.pretty_print = Some(true),
            "-r" | "--unwrapScalar" => parsed.unwrap_scalar = true,
            "-N" | "--no-doc" => parsed.no_doc = true,
            "-i" | "--inplace" => parsed.inplace = true,
            "-p" | "--input-format" => {
                index += 1;
                parsed.input_format = Some(
                    argv.get(index)
                        .ok_or(CliError::MissingValue("--input-format"))?
                        .clone(),
                );
            }
            "-o" | "--output-format" => {
                index += 1;
                parsed.output_format = Some(
                    argv.get(index)
                        .ok_or(CliError::MissingValue("--output-format"))?
                        .clone(),
                );
            }
            "-I" | "--indent" => {
                index += 1;
                let value = argv
                    .get(index)
                    .ok_or(CliError::MissingValue("--indent"))?
                    .clone();
                parsed.indent = Some(
                    value
                        .parse::<i32>()
                        .map_err(|_| CliError::InvalidIndent(value.clone()))?,
                );
            }
            _ if arg.starts_with('-') => return Err(CliError::UnknownFlag(arg.clone())),
            _ => expression = Some(arg.clone()),
        }
        index += 1;
    }

    if let Some(expression) = expression {
        parsed.expression = expression;
    }

    Ok(parsed)
}
