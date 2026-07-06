use std::fs;
use std::io::{self, IsTerminal, Write};

use crate::args::{CliError, CommandKind, ParsedArgs};
use crate::{cli_io, execute, parser};

pub(crate) fn run_root_command(raw_args: &[String]) -> Result<i32, CliError> {
    if should_render_root_help_on_empty_interactive_invocation(raw_args, io::stdin().is_terminal())
    {
        print!("{}", super::metadata::render_help(&ParsedArgs::default()));
        return Ok(0);
    }

    let parsed = parser::parse_cli_args(raw_args)?;

    match parsed.command {
        CommandKind::Help => {
            print!("{}", super::metadata::render_help(&parsed));
            Ok(0)
        }
        CommandKind::Web => super::web::run_web_command(&parsed),
        CommandKind::Run => run_evaluation_command(&parsed),
        _ => {
            let output = super::metadata::execute_metadata_command(&parsed)?;
            io::Write::write_all(&mut io::stdout(), &output)?;
            Ok(0)
        }
    }
}

fn run_evaluation_command(parsed: &ParsedArgs) -> Result<i32, CliError> {
    if parsed.inplace {
        let inputs = if parsed.null_input {
            Vec::new()
        } else {
            cli_io::input::read_inputs(&parsed.files)?
        };
        let output = execute::execute_command(parsed, &inputs)?;
        fs::write(&parsed.files[0], output)?;
        return Ok(0);
    }

    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let streaming_inputs = if parsed.null_input {
        Vec::new()
    } else {
        cli_io::input::prepare_streaming_inputs(parsed)?
    };
    let printed = execute::execute_command_to_writer(parsed, &streaming_inputs, &mut stdout)?;
    stdout.flush()?;
    Ok(execute::compute_exit_status_from_printed(parsed.exit_status, printed) as i32)
}

pub(crate) fn should_render_root_help_on_empty_interactive_invocation(
    raw_args: &[String],
    stdin_is_terminal: bool,
) -> bool {
    stdin_is_terminal && raw_args.len() == 1
}
