use std::env;
use std::fs;
use std::io::{self, Read};

use crate::core::{
    Context, CoreError, Encoder, NodeId, Printer, Reader, SystemError, VecPrinterWriter,
};
use crate::evaluator::StreamEvaluator;
use crate::formats::{
    CsvEncoder, Encode, FormatPreferences, JavascriptEncoder, JsonEncoder, PythonEncoder,
    TomlEncoder, YamlEncoder,
};

#[derive(Debug, Clone)]
struct ParsedArgs {
    expression: String,
    files: Vec<String>,
    help: bool,
    null_input: bool,
    exit_status: bool,
    input_format: Option<String>,
    output_format: Option<String>,
    pretty_print: Option<bool>,
    indent: Option<i32>,
    unwrap_scalar: bool,
    no_doc: bool,
    inplace: bool,
}

impl Default for ParsedArgs {
    fn default() -> Self {
        Self {
            expression: ".".to_string(),
            files: Vec::new(),
            help: false,
            null_input: false,
            exit_status: false,
            input_format: None,
            output_format: None,
            pretty_print: None,
            indent: None,
            unwrap_scalar: false,
            no_doc: false,
            inplace: false,
        }
    }
}

#[derive(Debug)]
enum CliError {
    MissingValue(&'static str),
    InvalidIndent(String),
    UnsupportedFormat(String),
    InvalidFlagCombination,
    MultipleInputFilesForInplace,
    UnknownFlag(String),
    Eval(String),
    Io(io::Error),
    Core(CoreError),
}

impl From<io::Error> for CliError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<CoreError> for CliError {
    fn from(value: CoreError) -> Self {
        Self::Core(value)
    }
}

#[derive(Debug, Clone)]
struct InputPayload {
    name: String,
    bytes: Vec<u8>,
}

impl InputPayload {
    fn display_name(&self) -> &str {
        &self.name
    }
}

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
}

pub fn main() {
    let args: Vec<String> = env::args().collect();
    let exit_code = match run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprint!("{}", render_runtime_error(&err));
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(raw_args: &[String]) -> Result<i32, CliError> {
    let parsed = parse_args(raw_args)?;

    if parsed.help {
        print!("{}", render_help(raw_args));
        return Ok(0);
    }

    let inputs = if parsed.null_input {
        Vec::new()
    } else {
        read_inputs(&parsed.files)?
    };

    let output = execute_command(&parsed, &inputs)?;

    if parsed.inplace {
        fs::write(&parsed.files[0], output)?;
        return Ok(0);
    }

    io::Write::write_all(&mut io::stdout(), &output)?;
    Ok(compute_exit_status(parsed.exit_status, &output) as i32)
}

fn execute_command(parsed: &ParsedArgs, inputs: &[InputPayload]) -> Result<Vec<u8>, CliError> {
    let mut registry = crate::init()?;
    let result = (|| {
        let mut ctx = Context::empty(registry.handle());
        let configured =
            configured_formats(parsed, inputs.first().map(InputPayload::display_name))?;
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
                let mut cursor = io::Cursor::new(payload.bytes.as_slice());
                let mut reader = Reader::new(&mut cursor);
                total_processed_docs += evaluator
                    .evaluate_with_format(
                        &mut ctx,
                        payload.display_name(),
                        &mut reader,
                        parsed_expression.as_deref(),
                        &mut printer,
                        Some(configured.input.as_str()),
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
    crate::deinit(&mut registry);
    result
}

fn parse_expression(
    expression: &str,
) -> Result<Option<Box<crate::core::ExpressionNode>>, CliError> {
    crate::parser::parse_expression(expression)
        .map_err(|error| CliError::Eval(format!("{error:?}")))
}

fn read_inputs(files: &[String]) -> Result<Vec<InputPayload>, CliError> {
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

fn configured_formats(
    parsed: &ParsedArgs,
    filename: Option<&str>,
) -> Result<ConfiguredFormats, CliError> {
    let input = match parsed.input_format.as_deref() {
        Some(value) => canonical_cli_format(value)?,
        None => match filename {
            Some("<stdin>") | Some("-") => "yaml".to_string(),
            Some(name) => crate::core::find_cli_format_spec_from_filename(name)
                .map(|spec| spec.name.to_string())
                .ok_or_else(|| CliError::UnsupportedFormat(name.to_string()))?,
            None => "yaml".to_string(),
        },
    };
    let output = match parsed.output_format.as_deref() {
        Some(value) => canonical_cli_format(value)?,
        None => input.clone(),
    };
    Ok(ConfiguredFormats { input, output })
}

fn configured_preferences(
    parsed: &ParsedArgs,
    output_format: &str,
) -> Result<FormatPreferences, CliError> {
    let spec = crate::core::find_cli_format_spec(output_format)
        .ok_or_else(|| CliError::UnsupportedFormat(output_format.to_string()))?;
    let language = spec
        .format_language
        .ok_or_else(|| CliError::UnsupportedFormat(output_format.to_string()))?;
    let mut prefs = crate::formats::configured_language_preferences().effective(language);
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

fn canonical_cli_format(value: &str) -> Result<String, CliError> {
    crate::core::find_cli_format_spec(value)
        .map(|spec| spec.name.to_string())
        .ok_or_else(|| CliError::UnsupportedFormat(value.to_string()))
}

fn make_value_encoder(
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

fn parse_args(argv: &[String]) -> Result<ParsedArgs, CliError> {
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
            "-h" | "--help" => parsed.help = true,
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

    validate_args(&parsed)?;
    Ok(parsed)
}

fn validate_args(parsed: &ParsedArgs) -> Result<(), CliError> {
    if parsed.inplace {
        if parsed.null_input {
            return Err(CliError::InvalidFlagCombination);
        }
        if parsed.files.len() != 1 {
            return Err(CliError::MultipleInputFilesForInplace);
        }
        if parsed.files[0] == "-" {
            return Err(CliError::InvalidFlagCombination);
        }
    }
    Ok(())
}

fn render_help(_: &[String]) -> String {
    format!(
        "Treease CLI\n\nUsage:\n  treease [OPTIONS] [EXPRESSION] [FILE]...\n\nOptions:\n{OPTIONS_HELP}"
    )
}

fn render_runtime_error(err: &CliError) -> String {
    match err {
        CliError::InvalidFlagCombination => {
            "The argument '--inplace' cannot be used with --null-input or stdin\n".to_string()
        }
        CliError::MultipleInputFilesForInplace => {
            "The argument '--inplace' requires exactly one input file\n".to_string()
        }
        CliError::MissingValue(flag) => format!("missing value for {flag}\n"),
        CliError::InvalidIndent(value) => format!("invalid indent: {value}\n"),
        CliError::UnsupportedFormat(value) => format!("unsupported format: {value}\n"),
        CliError::UnknownFlag(flag) => format!("unknown flag: {flag}\n"),
        CliError::Eval(message) => format!("{message}\n"),
        CliError::Io(err) => format!("{err}\n"),
        CliError::Core(err) => format!("{err}\n"),
    }
}

fn compute_exit_status(enabled: bool, output: &[u8]) -> u8 {
    let trimmed = std::str::from_utf8(output)
        .unwrap_or("")
        .trim_matches(|ch| matches!(ch, ' ' | '\t' | '\r' | '\n' | '\0'));
    let printed = !trimmed.is_empty() && trimmed != "null" && trimmed != "false";
    if enabled && !printed { 1 } else { 0 }
}

#[derive(Debug, Clone)]
struct ConfiguredFormats {
    input: String,
    output: String,
}

const OPTIONS_HELP: &str = "  -h, --help                  Show this help message.\n  -n, --null-input            Evaluate without reading input.\n  -e, --exit-status           Exit with status 1 if result is empty, null, or false.\n  -p, --input-format <FORMAT> Set input format.\n  -o, --output-format <FORMAT>\n                              Set output format.\n  -P, --prettyPrint           Pretty-print output.\n  -I, --indent <INDENT>       Set indentation width.\n  -r, --unwrapScalar          Unwrap scalar output.\n  -N, --no-doc                Disable YAML document separators.\n  -i, --inplace               Write result back to the input file.\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_invocation_parses_expression_and_files() {
        let raw = vec![
            "treease".to_string(),
            ".foo".to_string(),
            "a.json".to_string(),
        ];
        let parsed = parse_args(&raw).expect("parse should succeed");
        assert_eq!(parsed.expression, ".foo");
        assert_eq!(parsed.files, vec!["a.json"]);
    }

    #[test]
    fn root_help_only_documents_default_invocation() {
        let help = render_help(&["treease".to_string(), "--help".to_string()]);

        assert!(help.contains("treease [OPTIONS] [EXPRESSION] [FILE]..."));
        assert!(!help.contains("treease eval"));
        assert!(!help.contains("eval-all"));
    }

    #[test]
    fn format_and_indent_options_parse() {
        let raw = vec![
            "treease".to_string(),
            "-p".to_string(),
            "yaml".to_string(),
            "-o".to_string(),
            "json".to_string(),
            "-I".to_string(),
            "2".to_string(),
            ".foo".to_string(),
        ];
        let parsed = parse_args(&raw).expect("parse should succeed");
        assert_eq!(parsed.input_format.as_deref(), Some("yaml"));
        assert_eq!(parsed.output_format.as_deref(), Some("json"));
        assert_eq!(parsed.indent, Some(2));
        assert_eq!(parsed.expression, ".foo");
    }

    #[test]
    fn null_input_evaluates_expression_without_stdin() {
        let parsed = parse_args(&["treease".to_string(), "-n".to_string(), "1".to_string()])
            .expect("parse should succeed");
        let output = execute_command(&parsed, &[]).expect("command should succeed");
        assert_eq!(String::from_utf8(output).unwrap(), "1\n");
    }

    #[test]
    fn missing_path_renders_null() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "-e".to_string(),
            ".missing".to_string(),
        ])
        .expect("parse should succeed");
        let inputs = vec![InputPayload {
            name: "<stdin>".to_string(),
            bytes: b"{\"foo\":1}\n".to_vec(),
        }];

        let output = execute_command(&parsed, &inputs).expect("command should succeed");

        assert_eq!(String::from_utf8(output.clone()).unwrap(), "null\n");
        assert_eq!(compute_exit_status(parsed.exit_status, &output), 1);
    }
}
