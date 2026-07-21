use std::fs;
use std::io::{self, Write};

use crate::args::{CliError, ParsedArgs};
use crate::{cli_io, execute, web_payload, web_server};

pub(crate) fn run_web_command(parsed: &ParsedArgs) -> Result<i32, CliError> {
    let result = match build_web_file_source_result(parsed)? {
        Some(result) => result,
        None => {
            let inputs = cli_io::input::read_inputs(&parsed.files)?;
            let payload = web_payload::build_cli_graph_result_payload(parsed, &inputs)?;
            web_server::WebServerResult::text(
                payload.source_label,
                parsed.expression.clone(),
                payload.language,
                payload.text,
            )
        }
    };
    let server = web_server::WebServer::bind(result)?;

    let mut stdout = io::stdout();
    writeln!(stdout, "{}", server.editor_url())?;
    stdout.flush()?;

    server.serve_forever()?;
    Ok(0)
}

pub(crate) fn build_web_file_source_result(
    parsed: &ParsedArgs,
) -> Result<Option<web_server::WebServerResult>, CliError> {
    if !web_payload::should_delegate_identity_to_web(parsed) {
        return Ok(None);
    }
    let Some(source) = parsed.files.first() else {
        return Ok(None);
    };
    if source == "-" {
        return Ok(None);
    }
    let metadata = fs::metadata(source).map_err(CliError::Io)?;
    if !metadata.is_file() {
        return Err(CliError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{source} is not a file"),
        )));
    }

    let language = match parsed.input_format.as_deref() {
        Some(value) => execute::canonical_cli_format(value)?,
        None => match cli_io::input::guess_input_format_from_filename(source) {
            Some(format) => format,
            None => return Ok(None),
        },
    };

    Ok(Some(web_server::WebServerResult::file(
        source.clone(),
        parsed.expression.clone(),
        language,
        source.into(),
    )))
}
