#[cfg(not(target_arch = "wasm32"))]
use clap::{Arg, ArgAction, ArgMatches, Command, error::ErrorKind};

use super::{CliError, CommandKind, ParsedArgs, spec};

pub(super) fn parse_args_with_clap(argv: &[String]) -> Result<ParsedArgs, CliError> {
    reject_removed_legacy_eval_commands(argv)?;
    reject_removed_discovery_commands(argv)?;
    reject_invalid_web_invocation(argv)?;
    reject_too_many_root_positionals(argv)?;

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

fn reject_removed_discovery_commands(argv: &[String]) -> Result<(), CliError> {
    let Some(first_arg) = argv.get(1) else {
        return Ok(());
    };

    if matches!(first_arg.as_str(), "examples" | "doctor") {
        return Err(CliError::UnknownCommand(first_arg.clone()));
    }

    Ok(())
}

fn reject_invalid_web_invocation(argv: &[String]) -> Result<(), CliError> {
    let Some(web_index) = root_subcommand_index(argv) else {
        return Ok(());
    };
    if argv[web_index] != "web" {
        return Ok(());
    }

    let mut positional_count = 0;
    let mut index = web_index + 1;
    while index < argv.len() {
        match argv[index].as_str() {
            "-n" | "--null-input" => return Err(CliError::UnsupportedWebFlag("--null-input")),
            "-e" | "--exit-status" => return Err(CliError::UnsupportedWebFlag("--exit-status")),
            "-i" | "--inplace" => return Err(CliError::UnsupportedWebFlag("--inplace")),
            "-p" | "--input-format" | "-o" | "--output-format" | "-I" | "--indent" => {
                index += 2;
                continue;
            }
            "-P" | "--prettyPrint" | "-r" | "--unwrapScalar" | "-N" | "--no-doc" => {}
            arg if arg.starts_with('-') => return Ok(()),
            _ => positional_count += 1,
        }

        if positional_count > 2 {
            return Err(CliError::InvalidWebInputCount);
        }

        index += 1;
    }

    Ok(())
}

fn root_subcommand_index(argv: &[String]) -> Option<usize> {
    let mut index = 1;
    while index < argv.len() {
        match argv[index].as_str() {
            "-p" | "--input-format" | "-o" | "--output-format" | "-I" | "--indent" => {
                index += 2;
            }
            "-n" | "--null-input" | "-e" | "--exit-status" | "-P" | "--prettyPrint" | "-r"
            | "--unwrapScalar" | "-N" | "--no-doc" | "-i" | "--inplace" | "-h" | "--help" => {
                index += 1;
            }
            _ if argv[index].starts_with('-') => return None,
            _ => return Some(index),
        }
    }

    None
}

fn reject_too_many_root_positionals(argv: &[String]) -> Result<(), CliError> {
    if !should_parse_as_root_invocation(argv) {
        return Ok(());
    }

    let mut positional_count = 0;
    let mut index = 1;
    while index < argv.len() {
        match argv[index].as_str() {
            "-p" | "--input-format" | "-o" | "--output-format" | "-I" | "--indent" => {
                index += 2;
                continue;
            }
            "-n" | "--null-input" | "-e" | "--exit-status" | "-P" | "--prettyPrint" | "-r"
            | "--unwrapScalar" | "-N" | "--no-doc" | "-i" | "--inplace" | "-h" | "--help" => {
                index += 1;
                continue;
            }
            _ if argv[index].starts_with('-') && argv[index] != "-" => return Ok(()),
            _ => {
                positional_count += 1;
                index += 1;
            }
        }
    }

    if positional_count > 2 {
        if argv.iter().any(|arg| arg == "-i" || arg == "--inplace") {
            return Err(CliError::MultipleInputFilesForInplace);
        }
        return Err(CliError::MultipleInputFiles);
    }

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_args_with_clap_impl(argv: &[String]) -> Result<ParsedArgs, CliError> {
    let root_spec = spec::root_command_spec();
    let parse_spec = if should_parse_as_root_invocation(argv) {
        let mut root_only_spec = root_spec.clone();
        root_only_spec.subcommands.clear();
        root_only_spec
    } else {
        root_spec.clone()
    };
    let matches = build_command(parse_spec)
        .try_get_matches_from(argv)
        .map_err(map_clap_error)?;

    let mut parsed = ParsedArgs::default();
    apply_execution_options(&mut parsed, &matches);

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

fn should_parse_as_root_invocation(argv: &[String]) -> bool {
    root_subcommand_index(argv).is_some_and(|index| !is_root_subcommand(argv[index].as_str()))
}

fn is_root_subcommand(value: &str) -> bool {
    matches!(value, "web" | "help" | "operators" | "formats")
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_execution_options(parsed: &mut ParsedArgs, matches: &ArgMatches) {
    if get_flag_if_present(matches, "null-input") {
        parsed.null_input = true;
    }
    if get_flag_if_present(matches, "exit-status") {
        parsed.exit_status = true;
    }
    if let Some(value) = get_one_if_present::<String>(matches, "input-format") {
        parsed.input_format = Some(value.clone());
    }
    if let Some(value) = get_one_if_present::<String>(matches, "output-format") {
        parsed.output_format = Some(value.clone());
    }
    if get_flag_if_present(matches, "pretty-print") {
        parsed.pretty_print = Some(true);
    }
    if let Some(value) = get_one_if_present::<i32>(matches, "indent") {
        parsed.indent = Some(*value);
    }
    if get_flag_if_present(matches, "unwrap-scalar") {
        parsed.unwrap_scalar = true;
    }
    if get_flag_if_present(matches, "no-doc") {
        parsed.no_doc = true;
    }
    if get_flag_if_present(matches, "inplace") {
        parsed.inplace = true;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn get_flag_if_present(matches: &ArgMatches, id: &str) -> bool {
    matches
        .try_get_one::<bool>(id)
        .ok()
        .flatten()
        .copied()
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn get_one_if_present<'a, T: Clone + Send + Sync + 'static>(
    matches: &'a ArgMatches,
    id: &str,
) -> Option<&'a T> {
    matches.try_get_one::<T>(id).ok().flatten()
}

#[cfg(not(target_arch = "wasm32"))]
fn build_command(spec: spec::CliCommandSpec) -> Command {
    let name = leak(spec.name);
    let requires_leaf_subcommand =
        spec.id != spec::CliCommandId::Root && !spec.subcommands.is_empty();
    let mut command = Command::new(name)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .subcommand_required(requires_leaf_subcommand)
        .arg_required_else_help(false)
        .about(spec.summary.clone())
        .version(env!("CARGO_PKG_VERSION"));

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
    let argument_name = argument.name.clone();
    let mut arg = Arg::new(leak(argument.name)).help(argument.help.clone());

    if !argument.required {
        arg = arg.required(false);
    }

    if argument.required {
        arg = arg.required(true);
    }

    if argument.multiple || argument.repeated {
        if argument.required {
            arg = arg.num_args(1..);
        } else {
            arg = arg.num_args(0..);
        }
        arg = arg.allow_hyphen_values(true);
    } else if argument_name == "file" {
        arg = arg.allow_hyphen_values(true);
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
        spec::CliCommandId::Web => {
            apply_execution_options(parsed, matches);
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
        spec::CliCommandId::Help => {
            parsed.help = true;
            parsed.metadata_target = matches.get_one::<String>("command").cloned();
            parsed.metadata_format = matches.get_one::<String>("format").cloned();
        }
        spec::CliCommandId::OperatorsList => {
            parsed.metadata_category = matches.get_one::<String>("category").cloned();
            parsed.metadata_format = matches.get_one::<String>("format").cloned();
        }
        spec::CliCommandId::OperatorsGet | spec::CliCommandId::FormatsGet => {
            parsed.metadata_target = matches.get_one::<String>("name").cloned();
            parsed.metadata_format = matches.get_one::<String>("format").cloned();
        }
        spec::CliCommandId::FormatsList => {
            parsed.metadata_format = matches.get_one::<String>("format").cloned();
        }
        spec::CliCommandId::OperatorsSearch => {
            parsed.metadata_target = matches.get_one::<String>("query").cloned();
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
        spec::CliCommandId::Web => Some(CommandKind::Web),
        spec::CliCommandId::Help => Some(CommandKind::Help),
        spec::CliCommandId::OperatorsList => Some(CommandKind::OperatorsList),
        spec::CliCommandId::OperatorsGet => Some(CommandKind::OperatorsGet),
        spec::CliCommandId::OperatorsSearch => Some(CommandKind::OperatorsSearch),
        spec::CliCommandId::FormatsList => Some(CommandKind::FormatsList),
        spec::CliCommandId::FormatsGet => Some(CommandKind::FormatsGet),
        spec::CliCommandId::Operators | spec::CliCommandId::Formats => None,
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
