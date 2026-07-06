use std::fs;
use std::io::{self, Read};

use crate::args::{CliError, InputPayload, ParsedArgs, StreamingInput, StreamingInputSource};
use crate::execute;

pub(crate) fn read_inputs(files: &[String]) -> Result<Vec<InputPayload>, CliError> {
    let sources = if files.is_empty() {
        vec!["-".to_string()]
    } else {
        files.to_vec()
    };
    let mut inputs = Vec::with_capacity(sources.len());
    for source in sources {
        if source == "-" {
            let mut bytes = Vec::new();
            io::stdin().read_to_end(&mut bytes)?;
            inputs.push(InputPayload {
                name: "<stdin>".to_string(),
                bytes,
            });
        } else {
            inputs.push(InputPayload {
                name: source.clone(),
                bytes: fs::read(&source)?,
            });
        }
    }
    Ok(inputs)
}

pub(crate) fn prepare_streaming_inputs(
    parsed: &ParsedArgs,
) -> Result<Vec<StreamingInput>, CliError> {
    let sources = if parsed.files.is_empty() {
        vec!["-".to_string()]
    } else {
        parsed.files.clone()
    };

    let mut inputs = Vec::with_capacity(sources.len());
    for source in sources {
        if source == "-" {
            let mut bytes = Vec::new();
            io::stdin().read_to_end(&mut bytes)?;
            let input_format = parsed
                .input_format
                .as_deref()
                .map(execute::canonical_cli_format)
                .transpose()?
                .unwrap_or_else(|| {
                    guess_input_format_from_content(&bytes).unwrap_or_else(|| "json".to_string())
                });
            inputs.push(StreamingInput {
                name: "<stdin>".to_string(),
                input_format,
                source: StreamingInputSource::Stdin(bytes),
            });
            continue;
        }

        let input_format = resolve_input_format_for_path(parsed, &source)?;
        inputs.push(StreamingInput {
            name: source.clone(),
            input_format,
            source: StreamingInputSource::FilePath(source),
        });
    }
    Ok(inputs)
}

pub(crate) fn resolve_input_format(
    parsed: &ParsedArgs,
    payload: &InputPayload,
) -> Result<String, CliError> {
    if let Some(value) = parsed.input_format.as_deref() {
        return execute::canonical_cli_format(value);
    }

    Ok(guess_input_format(payload).unwrap_or_else(|| "json".to_string()))
}

pub(crate) fn guess_input_format(payload: &InputPayload) -> Option<String> {
    guess_input_format_from_filename(payload.display_name())
        .or_else(|| guess_input_format_from_content(&payload.bytes))
}

fn resolve_input_format_for_path(parsed: &ParsedArgs, path: &str) -> Result<String, CliError> {
    if let Some(value) = parsed.input_format.as_deref() {
        return execute::canonical_cli_format(value);
    }

    if let Some(format) = guess_input_format_from_filename(path) {
        return Ok(format);
    }

    let bytes = fs::read(path)?;
    Ok(guess_input_format_from_content(&bytes).unwrap_or_else(|| "json".to_string()))
}

pub(crate) fn guess_input_format_from_filename(filename: &str) -> Option<String> {
    if matches!(filename, "<stdin>" | "-") {
        return None;
    }

    let last_segment = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let dot_pos = last_segment.rfind('.')?;
    let ext = &last_segment[dot_pos + 1..];
    if ext.is_empty() {
        return None;
    }

    treease_core::core::find_cli_format_spec(ext).map(|spec| spec.name.to_string())
}

fn guess_input_format_from_content(bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let trimmed = text
        .trim_start_matches('\u{feff}')
        .trim_matches(|ch: char| ch.is_ascii_whitespace());
    if trimmed.is_empty() {
        return None;
    }

    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Some("json".to_string());
    }
    if looks_like_toml(trimmed) {
        return Some("toml".to_string());
    }
    if looks_like_yaml(trimmed) {
        return Some("yaml".to_string());
    }
    if looks_like_csv(trimmed) {
        return Some("csv".to_string());
    }

    None
}

fn looks_like_toml(text: &str) -> bool {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .any(|line| {
            if line.starts_with('[') && line.ends_with(']') && line.len() > 2 {
                return true;
            }
            let Some((key, value)) = line.split_once('=') else {
                return false;
            };
            !key.trim().is_empty()
                && !value.trim().is_empty()
                && key.trim().chars().all(|ch| {
                    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '"' | '\'' | ' ')
                })
        })
}

fn looks_like_yaml(text: &str) -> bool {
    if text.starts_with("---") || text.starts_with("- ") {
        return true;
    }

    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .any(|line| {
            if line.starts_with("- ") {
                return true;
            }
            let Some((key, value)) = line.split_once(':') else {
                return false;
            };
            !key.trim().is_empty()
                && !key.contains('{')
                && !key.contains('[')
                && (!value.trim().is_empty() || line.ends_with(':'))
        })
}

fn looks_like_csv(text: &str) -> bool {
    let rows: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .collect();
    if rows.len() < 2 {
        return false;
    }

    let first_count = rows[0].split(',').count();
    first_count > 1
        && rows[1..]
            .iter()
            .all(|row| row.split(',').count() == first_count)
}
