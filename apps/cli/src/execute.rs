use std::fs;
use std::io::{self, Write};

use treease_core::core::{
    Context, CoreError, Encoder, IoPrinterWriter, NodeId, Printer, Reader, SystemError,
    VecPrinterWriter,
};
use treease_core::evaluator::StreamEvaluator;
use treease_core::formats::{
    CsvEncoder, Encode, FormatPreferences, JavascriptEncoder, JsonEncoder, PythonEncoder,
    TomlEncoder, YamlEncoder,
};

use crate::args::{CliError, InputPayload, ParsedArgs, StreamingInput, StreamingInputSource};

struct CliPrinterEncoder {
    inner: Box<dyn Encode>,
}

impl Encoder for CliPrinterEncoder {
    fn encode(
        &self,
        ctx: &mut Context,
        node: NodeId,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), CoreError> {
        let store = ctx
            .current_print_store()
            .ok_or(CoreError::System(SystemError::Error))?;
        self.inner.encode(store, node, writer)
    }

    fn encode_evaluated_value(
        &self,
        value: &treease_core::evaluator::Value,
        writer: &mut dyn std::io::Write,
    ) -> Result<bool, CoreError> {
        self.inner.encode_evaluated_value(value, writer)
    }
}

#[derive(Debug, Clone)]
pub(super) struct ConfiguredFormats {
    pub(super) output: String,
}

pub(super) fn canonical_cli_format(value: &str) -> Result<String, CliError> {
    treease_core::core::find_cli_format_spec(value)
        .map(|spec| spec.name.to_string())
        .ok_or_else(|| CliError::UnsupportedFormat(value.to_string()))
}

pub(super) fn configured_formats(
    parsed: &ParsedArgs,
    first_input: Option<&InputPayload>,
) -> Result<ConfiguredFormats, CliError> {
    let input = first_input
        .map(|payload| super::cli_io::input::resolve_input_format(parsed, payload))
        .transpose()?;
    configured_formats_from_input_format(parsed, input.as_deref())
}

pub(super) fn execute_command(
    parsed: &ParsedArgs,
    inputs: &[InputPayload],
) -> Result<Vec<u8>, CliError> {
    let mut registry = treease_core::init()?;
    let result = (|| {
        let mut ctx = Context::empty(registry.handle());
        let configured = configured_formats(parsed, inputs.first())?;
        let prefs = configured_preferences(parsed, &configured.output)?;
        let encoder = CliPrinterEncoder {
            inner: make_value_encoder(&configured.output, prefs.clone())?,
        };
        let writer = VecPrinterWriter::new();
        let mut printer = Printer::new(encoder, writer);

        let mut evaluator = StreamEvaluator::new();
        if parsed.null_input {
            evaluator
                .evaluate_new(&mut ctx, &parsed.expression, &mut printer)
                .map_err(|err| CliError::Eval(format!("{err:?}")))?;
        } else {
            let parsed_expression = parse_expression(&parsed.expression)?;
            let mut total_processed_docs = 0_u32;
            for payload in inputs {
                let input_format = super::cli_io::input::resolve_input_format(parsed, payload)?;
                let mut cursor = io::Cursor::new(payload.bytes.as_slice());
                let mut reader = Reader::new(&mut cursor);
                total_processed_docs += evaluator
                    .evaluate_with_format(
                        &mut ctx,
                        payload.display_name(),
                        &mut reader,
                        parsed_expression.as_deref(),
                        &mut printer,
                        Some(input_format.as_str()),
                    )
                    .map_err(|err| CliError::Eval(format!("{err:?}")))?;
            }
            if total_processed_docs == 0 {
                evaluator
                    .evaluate_new(&mut ctx, &parsed.expression, &mut printer)
                    .map_err(|err| CliError::Eval(format!("{err:?}")))?;
            }
        }

        Ok(printer.into_writer().into_bytes())
    })();
    treease_core::deinit(&mut registry);
    result
}

pub(super) fn execute_command_to_writer<W: Write>(
    parsed: &ParsedArgs,
    inputs: &[StreamingInput],
    output: &mut W,
) -> Result<bool, CliError> {
    let mut registry = treease_core::init()?;
    let result = (|| {
        let mut ctx = Context::empty(registry.handle());
        let configured = configured_formats_from_input_format(
            parsed,
            inputs.first().map(|input| input.input_format.as_str()),
        )?;
        let prefs = configured_preferences(parsed, &configured.output)?;
        let encoder = CliPrinterEncoder {
            inner: make_value_encoder(&configured.output, prefs.clone())?,
        };
        let writer = IoPrinterWriter::new(output);
        let mut printer = Printer::new(encoder, writer);

        let mut evaluator = StreamEvaluator::new();
        if parsed.null_input {
            evaluator
                .evaluate_new(&mut ctx, &parsed.expression, &mut printer)
                .map_err(|err| CliError::Eval(format!("{err:?}")))?;
        } else {
            let parsed_expression = parse_expression(&parsed.expression)?;
            let mut total_processed_docs = 0_u32;
            for input in inputs {
                total_processed_docs += match &input.source {
                    StreamingInputSource::Stdin(bytes) => {
                        let mut cursor = io::Cursor::new(bytes.as_slice());
                        let mut reader = Reader::new(&mut cursor);
                        evaluator
                            .evaluate_with_format(
                                &mut ctx,
                                input.display_name(),
                                &mut reader,
                                parsed_expression.as_deref(),
                                &mut printer,
                                Some(input.input_format.as_str()),
                            )
                            .map_err(|err| CliError::Eval(format!("{err:?}")))?
                    }
                    StreamingInputSource::FilePath(path) => {
                        let mut file = fs::File::open(path)?;
                        let mut reader = Reader::new(&mut file);
                        evaluator
                            .evaluate_with_format(
                                &mut ctx,
                                input.display_name(),
                                &mut reader,
                                parsed_expression.as_deref(),
                                &mut printer,
                                Some(input.input_format.as_str()),
                            )
                            .map_err(|err| CliError::Eval(format!("{err:?}")))?
                    }
                };
            }
            if total_processed_docs == 0 {
                evaluator
                    .evaluate_new(&mut ctx, &parsed.expression, &mut printer)
                    .map_err(|err| CliError::Eval(format!("{err:?}")))?;
            }
        }

        Ok(printer.printed_anything())
    })();
    treease_core::deinit(&mut registry);
    result
}

pub(super) fn configured_preferences(
    parsed: &ParsedArgs,
    output_format: &str,
) -> Result<FormatPreferences, CliError> {
    let spec = treease_core::core::find_cli_format_spec(output_format)
        .ok_or_else(|| CliError::UnsupportedFormat(output_format.to_string()))?;
    let language = spec
        .format_language
        .ok_or_else(|| CliError::UnsupportedFormat(output_format.to_string()))?;
    let mut prefs = treease_core::formats::configured_language_preferences().effective(language);
    let pretty_print = parsed.pretty_print.unwrap_or(spec.default_pretty_print);
    prefs.indent = if pretty_print {
        parsed.indent.unwrap_or(prefs.indent)
    } else {
        0
    };
    prefs.unwrap_scalar = parsed.unwrap_scalar;
    if parsed.no_doc {
        prefs.print_doc_separators = false;
    }
    Ok(prefs)
}

pub(super) fn make_value_encoder(
    format_name: &str,
    prefs: FormatPreferences,
) -> Result<Box<dyn Encode>, CliError> {
    match format_name {
        "json" => Ok(Box::new(JsonEncoder::new(prefs))),
        "yaml" => Ok(Box::new(YamlEncoder::new(prefs))),
        "toml" => Ok(Box::new(TomlEncoder::new(prefs))),
        "csv" => Ok(Box::new(CsvEncoder::new(prefs))),
        "python" => Ok(Box::new(PythonEncoder::new(prefs))),
        "javascript" => Ok(Box::new(JavascriptEncoder::new(prefs))),
        _ => Err(CliError::UnsupportedFormat(format_name.to_string())),
    }
}

pub(super) fn parse_expression(
    expression: &str,
) -> Result<Option<Box<treease_core::core::ExpressionNode>>, CliError> {
    treease_core::parser::parse_expression(expression)
        .map_err(|error| CliError::Eval(format!("{error:?}")))
}

fn configured_formats_from_input_format(
    parsed: &ParsedArgs,
    first_input_format: Option<&str>,
) -> Result<ConfiguredFormats, CliError> {
    let input = first_input_format.unwrap_or("json").to_string();
    let output = match parsed.output_format.as_deref() {
        Some(value) => canonical_cli_format(value)?,
        None if first_input_format.is_some() => input.clone(),
        None => "yaml".to_string(),
    };
    Ok(ConfiguredFormats { output })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn compute_exit_status(enabled: bool, output: &[u8]) -> u8 {
    let trimmed = std::str::from_utf8(output)
        .unwrap_or("")
        .trim_matches(|ch| matches!(ch, ' ' | '\t' | '\r' | '\n' | '\0'));
    let printed = !trimmed.is_empty() && trimmed != "null" && trimmed != "false";
    compute_exit_status_from_printed(enabled, printed)
}

pub(super) fn compute_exit_status_from_printed(enabled: bool, printed: bool) -> u8 {
    if enabled && !printed { 1 } else { 0 }
}
