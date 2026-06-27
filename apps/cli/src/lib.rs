use std::env;
use std::fs;
use std::io::{self, Read, Write};

mod catalog;
mod errors;
mod parser;
mod spec;
mod web_assets;
mod web_payload;
mod web_server;

use treease_core::core::{
    Context, CoreError, Encoder, NodeId, Printer, Reader, SystemError, VecPrinterWriter,
};
use treease_core::evaluator::StreamEvaluator;
use treease_core::formats::{
    CsvEncoder, Encode, FormatPreferences, JavascriptEncoder, JsonEncoder, PythonEncoder,
    TomlEncoder, YamlEncoder,
};

#[derive(Debug, Clone)]
struct ParsedArgs {
    command: CommandKind,
    expression: String,
    files: Vec<String>,
    help: bool,
    version: bool,
    null_input: bool,
    exit_status: bool,
    input_format: Option<String>,
    output_format: Option<String>,
    pretty_print: Option<bool>,
    indent: Option<i32>,
    unwrap_scalar: bool,
    no_doc: bool,
    inplace: bool,
    metadata_format: Option<String>,
    metadata_target: Option<String>,
    metadata_category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommandKind {
    Run,
    Web,
    Help,
    Version,
    OperatorsList,
    OperatorsGet,
    OperatorsSearch,
    FormatsList,
    FormatsGet,
}

impl Default for ParsedArgs {
    fn default() -> Self {
        Self {
            command: CommandKind::Run,
            expression: ".".to_string(),
            files: Vec::new(),
            help: false,
            version: false,
            null_input: false,
            exit_status: false,
            input_format: None,
            output_format: None,
            pretty_print: None,
            indent: None,
            unwrap_scalar: false,
            no_doc: false,
            inplace: false,
            metadata_format: None,
            metadata_target: None,
            metadata_category: None,
        }
    }
}

#[derive(Debug)]
enum CliError {
    #[allow(dead_code)]
    MissingValue(&'static str),
    InvalidIndent(String),
    UnsupportedFormat(String),
    UnsupportedOperator(String),
    InvalidFlagCombination,
    MultipleInputFiles,
    MultipleInputFilesForInplace,
    UnsupportedWebFlag(&'static str),
    InvalidWebInputCount,
    WebAssetDownload(String),
    WebAssetManifest(String),
    WebAssetCache(String),
    #[allow(dead_code)]
    WebServer(String),
    #[allow(dead_code)]
    WebForbidden,
    UnknownCommand(String),
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

    match parsed.command {
        CommandKind::Help => {
            print!("{}", render_help(&parsed));
            return Ok(0);
        }
        CommandKind::Version => {
            print!("{}", render_version());
            return Ok(0);
        }
        CommandKind::Web => return run_web_command(&parsed),
        CommandKind::Run => {}
        _ => {
            let output = execute_metadata_command(&parsed)?;
            io::Write::write_all(&mut io::stdout(), &output)?;
            return Ok(0);
        }
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

fn run_web_command(parsed: &ParsedArgs) -> Result<i32, CliError> {
    let inputs = read_inputs(&parsed.files)?;
    let payload = web_payload::build_cli_graph_result_payload(parsed, &inputs)?;
    let result_json =
        serde_json::to_vec(&payload).map_err(|error| CliError::Eval(error.to_string()))?;
    let assets_dir = web_assets::ensure_available()?;
    let server = web_server::WebServer::bind(result_json, assets_dir)?;

    let mut stdout = io::stdout();
    writeln!(stdout, "{}", server.graph_url())?;
    stdout.flush()?;

    server.serve_forever()?;
    Ok(0)
}

fn execute_command(parsed: &ParsedArgs, inputs: &[InputPayload]) -> Result<Vec<u8>, CliError> {
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
                let input_format = resolve_input_format(parsed, payload)?;
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

fn parse_expression(
    expression: &str,
) -> Result<Option<Box<treease_core::core::ExpressionNode>>, CliError> {
    treease_core::parser::parse_expression(expression)
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
    first_input: Option<&InputPayload>,
) -> Result<ConfiguredFormats, CliError> {
    let input = first_input
        .map(|payload| resolve_input_format(parsed, payload))
        .transpose()?
        .unwrap_or_else(|| "json".to_string());
    let output = match parsed.output_format.as_deref() {
        Some(value) => canonical_cli_format(value)?,
        None if first_input.is_some() => input.clone(),
        None => "yaml".to_string(),
    };
    Ok(ConfiguredFormats { output })
}

fn resolve_input_format(parsed: &ParsedArgs, payload: &InputPayload) -> Result<String, CliError> {
    if let Some(value) = parsed.input_format.as_deref() {
        return canonical_cli_format(value);
    }

    Ok(guess_input_format(payload).unwrap_or_else(|| "json".to_string()))
}

fn guess_input_format(payload: &InputPayload) -> Option<String> {
    guess_input_format_from_filename(payload.display_name())
        .or_else(|| guess_input_format_from_content(&payload.bytes))
}

fn guess_input_format_from_filename(filename: &str) -> Option<String> {
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

fn configured_preferences(
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

fn canonical_cli_format(value: &str) -> Result<String, CliError> {
    treease_core::core::find_cli_format_spec(value)
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
    let parsed = parser::parse_args_with_clap(argv)?;
    validate_args(&parsed)?;
    Ok(parsed)
}

fn validate_args(parsed: &ParsedArgs) -> Result<(), CliError> {
    if parsed.command == CommandKind::Web {
        if parsed.null_input {
            return Err(CliError::UnsupportedWebFlag("--null-input"));
        }
        if parsed.exit_status {
            return Err(CliError::UnsupportedWebFlag("--exit-status"));
        }
        if parsed.inplace {
            return Err(CliError::UnsupportedWebFlag("--inplace"));
        }
        if parsed.files.len() != 1 {
            return Err(CliError::InvalidWebInputCount);
        }
    }

    if parsed.command == CommandKind::Run && parsed.files.len() > 1 {
        return Err(CliError::MultipleInputFiles);
    }

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

fn render_help(parsed: &ParsedArgs) -> String {
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

fn render_version() -> String {
    format!("treease {}\n", env!("CARGO_PKG_VERSION"))
}

fn resolve_help_target(target: Option<&str>) -> Option<spec::CliCommandSpec> {
    let target = target?;
    let segments = target.split_whitespace().collect::<Vec<_>>();
    if segments.is_empty() {
        return None;
    }

    let mut path = vec!["treease"];
    path.extend(segments);
    spec::find_command_spec(&path)
}

fn execute_metadata_command(parsed: &ParsedArgs) -> Result<Vec<u8>, CliError> {
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
        CommandKind::Version => return Err(CliError::InvalidFlagCombination),
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

fn render_runtime_error(err: &CliError) -> String {
    errors::render_text(err)
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
    output: String,
}

pub mod internal_metadata {
    pub fn cli_help_json() -> serde_json::Value {
        super::spec::root_help_json_value()
    }

    pub fn operators_json() -> serde_json::Value {
        serde_json::to_value(super::catalog::operators()).expect("operators should serialize")
    }

    pub fn formats_json() -> serde_json::Value {
        serde_json::to_value(super::catalog::formats()).expect("formats should serialize")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn request_web_server_once(state: web_server::WebServerState, target: &str) -> Vec<u8> {
        let server = web_server::WebServer::bind_for_test(state).expect("server should bind");
        let addr = server.local_addr();
        let handle = std::thread::spawn(move || {
            server
                .serve_once_for_test()
                .expect("server should handle one request");
        });

        let mut stream = TcpStream::connect(addr).expect("client should connect");
        write!(
            stream,
            "GET {target} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        )
        .expect("request should write");
        stream.flush().expect("request should flush");

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .expect("response should read");
        handle.join().expect("server thread should join");
        response
    }

    fn test_web_server_state() -> web_server::WebServerState {
        let assets_dir = test_asset_dir(&[
            ("index.html", b"<html><body>graph</body></html>".as_slice()),
            ("_app/app.js", b"console.log('graph')".as_slice()),
            (
                "_app/immutable/assets/core.testhash.wasm",
                b"\0asmtest".as_slice(),
            ),
        ]);
        web_server::WebServerState {
            token: "test-token".to_string(),
            result_json: br#"{"ok":true}"#.to_vec(),
            assets_dir,
        }
    }

    fn assert_response_contains(response: &[u8], expected: &str) {
        let text = std::str::from_utf8(response).expect("response should be utf8");
        assert!(
            text.contains(expected),
            "response did not contain {expected:?}:\n{text}"
        );
    }

    fn response_body(response: &[u8]) -> &[u8] {
        response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| &response[index + 4..])
            .expect("response should include header terminator")
    }

    #[test]
    fn embedded_assets_lookup_normalizes_graph_route_to_index() {
        let asset_dir = test_asset_dir(&[
            ("index.html", b"<html></html>".as_slice()),
            ("_app/app.js", b"console.log(1)".as_slice()),
        ]);

        let index = web_assets::find_asset(&asset_dir, "/cli/graph")
            .expect("graph route should use index.html");
        assert!(index.path.ends_with("index.html"));

        let graph_with_query = web_assets::find_asset(&asset_dir, "/cli/graph?token=x")
            .expect("graph route with query should use index.html");
        assert!(graph_with_query.path.ends_with("index.html"));

        let script = web_assets::find_asset(&asset_dir, "/_app/app.js?hash=1")
            .expect("static asset should resolve directly");
        assert_eq!(script.content_type, "text/javascript; charset=utf-8");

        assert!(
            web_assets::find_asset(&asset_dir, "/missing").is_none(),
            "unknown extensionless route should not fallback"
        );
        assert!(
            web_assets::find_asset(&asset_dir, "/api/status").is_none(),
            "api-looking route should not fallback"
        );
        assert!(
            web_assets::find_asset(&asset_dir, "/missing.txt").is_none(),
            "missing static asset should not fallback"
        );
        assert!(
            web_assets::find_asset(&asset_dir, "/cli/graph/anything").is_none(),
            "graph fallback should not cover unknown subpaths"
        );
    }

    #[test]
    fn web_server_serves_result_only_with_matching_token() {
        let missing = request_web_server_once(test_web_server_state(), "/cli/result");
        assert_response_contains(&missing, "HTTP/1.1 403 Forbidden");

        let wrong = request_web_server_once(test_web_server_state(), "/cli/result?token=wrong");
        assert_response_contains(&wrong, "HTTP/1.1 403 Forbidden");

        let matching =
            request_web_server_once(test_web_server_state(), "/cli/result?token=test-token");
        assert_response_contains(&matching, "HTTP/1.1 200 OK");
        assert_response_contains(&matching, "Content-Type: application/json; charset=utf-8");
        assert_eq!(response_body(&matching), br#"{"ok":true}"#);
    }

    fn test_asset_dir(files: &[(&str, &[u8])]) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic enough for tests")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("treease-cli-test-assets-{unique}"));
        fs::create_dir_all(&root).expect("test asset root should be creatable");
        for (relative, bytes) in files {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("test asset parent should be creatable");
            }
            fs::write(path, bytes).expect("test asset file should write");
        }
        root
    }

    #[test]
    fn web_server_serves_graph_route_from_index_asset() {
        let missing = request_web_server_once(test_web_server_state(), "/cli/graph");
        assert_response_contains(&missing, "HTTP/1.1 403 Forbidden");

        let wrong = request_web_server_once(test_web_server_state(), "/cli/graph?token=wrong");
        assert_response_contains(&wrong, "HTTP/1.1 403 Forbidden");

        let graph = request_web_server_once(test_web_server_state(), "/cli/graph?token=test-token");
        assert_response_contains(&graph, "HTTP/1.1 200 OK");
        assert_response_contains(&graph, "Content-Type: text/html; charset=utf-8");
        assert_eq!(response_body(&graph), b"<html><body>graph</body></html>");

        let static_asset = request_web_server_once(test_web_server_state(), "/_app/app.js");
        assert_response_contains(&static_asset, "HTTP/1.1 200 OK");
        assert_response_contains(
            &static_asset,
            "Content-Type: text/javascript; charset=utf-8",
        );
        assert_eq!(response_body(&static_asset), b"console.log('graph')");

        let root = request_web_server_once(test_web_server_state(), "/");
        assert_response_contains(&root, "HTTP/1.1 200 OK");
        assert_eq!(response_body(&root), b"<html><body>graph</body></html>");

        let direct_index = request_web_server_once(test_web_server_state(), "/index.html");
        assert_response_contains(&direct_index, "HTTP/1.1 200 OK");
        assert_eq!(
            response_body(&direct_index),
            b"<html><body>graph</body></html>"
        );

        let wasm_asset = request_web_server_once(
            test_web_server_state(),
            "/_app/immutable/assets/core.testhash.wasm",
        );
        assert_response_contains(&wasm_asset, "HTTP/1.1 200 OK");
        assert_response_contains(&wasm_asset, "Content-Type: application/wasm");
        assert_eq!(response_body(&wasm_asset), b"\0asmtest");

        let graph_subpath = request_web_server_once(
            test_web_server_state(),
            "/cli/graph/anything?token=test-token",
        );
        assert_response_contains(&graph_subpath, "HTTP/1.1 404 Not Found");

        let unknown = request_web_server_once(test_web_server_state(), "/missing");
        assert_response_contains(&unknown, "HTTP/1.1 404 Not Found");
    }

    #[test]
    fn web_asset_manifest_requires_matching_index_asset_version() {
        let manifest = serde_json::json!({
            "version": "26062410",
            "assetVersion": "1761465123456",
            "files": [
                { "path": "index.html" },
                { "path": "_app/app.js" }
            ]
        });
        let asset_dir = test_asset_dir(&[
            (
                "index.html",
                br#"<html data-treease-cli-asset-version="1761465123456"></html>"#,
            ),
            ("_app/app.js", b"console.log(1)".as_slice()),
        ]);
        fs::write(
            asset_dir.join("manifest.json"),
            serde_json::to_vec(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should write");

        assert!(
            web_assets::cache_is_complete_for_test(&asset_dir),
            "matching assetVersion should be accepted"
        );

        fs::write(
            asset_dir.join("index.html"),
            br#"<html data-treease-cli-asset-version="1761465999999"></html>"#,
        )
        .expect("index should rewrite");

        assert!(
            !web_assets::cache_is_complete_for_test(&asset_dir),
            "mismatched assetVersion should invalidate cache"
        );
    }

    #[test]
    fn web_asset_index_parser_extracts_asset_version_from_html_attribute() {
        let html = br#"<!doctype html>
<html lang="en" data-treease-cli-asset-version="1761465123456">
  <body></body>
</html>
"#;
        assert_eq!(
            web_assets::read_index_asset_version_for_test(html),
            Some("1761465123456".to_string())
        );
        assert_eq!(
            web_assets::read_index_asset_version_for_test(
                br#"<html data-treease-cli-asset-version='abc-123'></html>"#
            ),
            Some("abc-123".to_string())
        );
        assert_eq!(
            web_assets::read_index_asset_version_for_test(br#"<html lang="en"></html>"#),
            None
        );
    }

    #[test]
    fn default_invocation_parses_expression_and_files() {
        let raw = vec![
            "treease".to_string(),
            ".foo".to_string(),
            "a.json".to_string(),
        ];
        let parsed = parse_args(&raw).expect("parse should succeed");
        assert_eq!(parsed.command, CommandKind::Run);
        assert_eq!(parsed.expression, ".foo");
        assert_eq!(parsed.files, vec!["a.json"]);
    }

    #[test]
    fn root_help_only_documents_default_invocation() {
        let help = render_help(
            &parse_args(&["treease".to_string(), "--help".to_string()]).expect("help should parse"),
        );

        assert!(help.contains("treease [OPTIONS] [EXPRESSION] [FILE]"));
        assert!(!help.contains("treease eval"));
        assert!(!help.contains("eval-all"));
    }

    #[test]
    fn version_flag_parses_without_reading_stdin() {
        let parsed = parse_args(&["treease".to_string(), "--version".to_string()])
            .expect("version should parse");

        assert_eq!(parsed.command, CommandKind::Version);
        assert!(parsed.version);
        assert!(parsed.files.is_empty());
    }

    #[test]
    fn root_help_advertises_discovery_commands() {
        let help = render_help(
            &parse_args(&["treease".to_string(), "--help".to_string()]).expect("help should parse"),
        );

        assert!(help.contains("treease [OPTIONS] [EXPRESSION] [FILE]"));
        assert!(help.contains("treease help --format json"));
        assert!(help.contains("treease operators list"));
        assert!(help.contains("treease formats list"));
        assert!(help.contains("treease --null-input '.hello = \"world\"'"));
        assert!(!help.contains("eval-all"));
    }

    #[test]
    fn root_help_json_contains_machine_readable_schema() {
        let json = spec::root_help_json_value();

        assert_eq!(
            json.get("name").and_then(serde_json::Value::as_str),
            Some("treease")
        );
        assert_eq!(
            json.get("usage").and_then(serde_json::Value::as_str),
            Some("treease [OPTIONS] [EXPRESSION] [FILE]")
        );
        assert!(
            json.get("subcommands")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|subcommands| subcommands.iter().any(|command| {
                    command.get("name").and_then(serde_json::Value::as_str) == Some("operators")
                }))
        );
        assert!(
            json.get("options")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|options| options.iter().any(|option| {
                    option.get("name").and_then(serde_json::Value::as_str) == Some("output-format")
                }))
        );
        assert!(
            json.get("options")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|options| options.iter().all(|option| {
                    option.get("name").and_then(serde_json::Value::as_str) != Some("format")
                }))
        );
    }

    #[test]
    fn command_spec_can_be_found_by_structured_path() {
        let command = spec::find_command_spec(&["treease", "operators", "list"])
            .expect("operators list spec should exist");

        assert_eq!(command.id, spec::CliCommandId::OperatorsList);
        assert_eq!(command.name, "list");
        assert_eq!(
            command.segments,
            vec![
                "treease".to_string(),
                "operators".to_string(),
                "list".to_string()
            ]
        );
        assert_eq!(command.path, "treease operators list");
    }

    #[test]
    fn root_spec_does_not_expose_format_option() {
        let root = spec::root_command_spec();

        assert!(root.options.iter().all(|option| option.name != "format"));
        assert!(root.options.iter().any(|option| option.name == "help"));
    }

    #[test]
    fn help_command_exposes_help_only_format_option() {
        let help_command =
            spec::find_command_spec(&["treease", "help"]).expect("help command should exist");
        let format_option = help_command
            .options
            .iter()
            .find(|option| option.name == "format")
            .expect("format option should exist");

        assert_eq!(help_command.id, spec::CliCommandId::Help);
        assert!(format_option.takes_value);
        assert_eq!(format_option.value_name.as_deref(), Some("FORMAT"));
        assert_eq!(format_option.scope, spec::CliOptionScope::HelpOnly);
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
    fn input_format_guess_prefers_explicit_override() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "--input-format".to_string(),
            "yaml".to_string(),
            ".foo".to_string(),
        ])
        .expect("parse should succeed");
        let payload = InputPayload {
            name: "data.json".to_string(),
            bytes: br#"{"foo":1}"#.to_vec(),
        };

        let input = resolve_input_format(&parsed, &payload).expect("input format should resolve");

        assert_eq!(input, "yaml");
    }

    #[test]
    fn input_format_guess_uses_filename_extension_before_content() {
        let payload = InputPayload {
            name: "data.yaml".to_string(),
            bytes: br#"{"foo":1}"#.to_vec(),
        };

        let guessed = guess_input_format(&payload);

        assert_eq!(guessed.as_deref(), Some("yaml"));
    }

    #[test]
    fn input_format_guess_uses_content_for_stdin() {
        let payload = InputPayload {
            name: "<stdin>".to_string(),
            bytes: b"foo: 1\nbar: 2\n".to_vec(),
        };

        let guessed = guess_input_format(&payload);

        assert_eq!(guessed.as_deref(), Some("yaml"));
    }

    #[test]
    fn input_format_guess_uses_content_for_suffixless_file() {
        let payload = InputPayload {
            name: "config".to_string(),
            bytes: b"foo = 1\nbar = 2\n".to_vec(),
        };

        let guessed = guess_input_format(&payload);

        assert_eq!(guessed.as_deref(), Some("toml"));
    }

    #[test]
    fn input_format_defaults_to_json_when_guess_fails() {
        let parsed =
            parse_args(&["treease".to_string(), ".foo".to_string()]).expect("parse should succeed");
        let payload = InputPayload {
            name: "README".to_string(),
            bytes: b"plain text without recognizable structure".to_vec(),
        };

        let input = resolve_input_format(&parsed, &payload).expect("input format should resolve");

        assert_eq!(input, "json");
    }

    #[test]
    fn suffixless_yaml_input_executes_without_explicit_input_format() {
        let parsed =
            parse_args(&["treease".to_string(), ".foo".to_string()]).expect("parse should succeed");
        let inputs = vec![InputPayload {
            name: "sample".to_string(),
            bytes: b"foo: 1\n".to_vec(),
        }];

        let output = execute_command(&parsed, &inputs).expect("command should succeed");

        assert_eq!(String::from_utf8(output).unwrap(), "1\n");
    }

    #[test]
    fn oversized_json_integer_does_not_block_unrelated_field_query() {
        let parsed = parse_args(&["treease".to_string(), ".BaseResp.StatusCode".to_string()])
            .expect("parse should succeed");
        let inputs = vec![InputPayload {
            name: "base.json".to_string(),
            bytes: br#"{"BaseResp":{"StatusCode":0},"Huge":9999999999999999999}"#.to_vec(),
        }];

        let output = execute_command(&parsed, &inputs).expect("command should succeed");

        assert_eq!(String::from_utf8(output).unwrap(), "0\n");
    }

    #[test]
    fn null_input_evaluates_expression_without_stdin() {
        let parsed = parse_args(&["treease".to_string(), "-n".to_string(), "1".to_string()])
            .expect("parse should succeed");
        let output = execute_command(&parsed, &[]).expect("command should succeed");
        assert_eq!(String::from_utf8(output).unwrap(), "1\n");
    }

    #[test]
    fn null_input_empty_object_renders_object_not_array() {
        let parsed = parse_args(&["treease".to_string(), "-n".to_string(), "{}".to_string()])
            .expect("parse should succeed");

        let output = execute_command(&parsed, &[]).expect("command should succeed");

        assert_eq!(String::from_utf8(output).unwrap(), "{}\n");
    }

    #[test]
    fn null_input_object_literal_renders_object() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "-n".to_string(),
            "{\"wrap\": \"frog\"}".to_string(),
        ])
        .expect("parse should succeed");

        let output = execute_command(&parsed, &[]).expect("command should succeed");

        assert_eq!(String::from_utf8(output).unwrap(), "wrap: frog\n");
    }

    #[test]
    fn null_input_assignment_pipeline_renders_object_without_outer_array() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "-n".to_string(),
            "(.a.b = \"foo\") | (.d.e = \"bar\")".to_string(),
        ])
        .expect("parse should succeed");

        let output = execute_command(&parsed, &[]).expect("command should succeed");

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "a:\n  b: foo\nd:\n  e: bar\n"
        );
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

    #[test]
    fn help_json_command_parses_without_reading_stdin() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "help".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ])
        .expect("help json should parse");

        assert_eq!(parsed.command, CommandKind::Help);
        assert_eq!(parsed.metadata_format.as_deref(), Some("json"));
        assert!(parsed.files.is_empty());
    }

    #[test]
    fn operators_get_command_parses_name_and_json_format() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "operators".to_string(),
            "get".to_string(),
            "map".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ])
        .expect("operators get should parse");

        assert_eq!(parsed.command, CommandKind::OperatorsGet);
        assert_eq!(parsed.metadata_target.as_deref(), Some("map"));
        assert_eq!(parsed.metadata_format.as_deref(), Some("json"));
    }

    #[test]
    fn formats_list_command_parses_json_format() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "formats".to_string(),
            "list".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ])
        .expect("formats list should parse");

        assert_eq!(parsed.command, CommandKind::FormatsList);
        assert_eq!(parsed.metadata_format.as_deref(), Some("json"));
    }

    #[test]
    fn discovery_parent_command_requires_leaf_subcommand() {
        let error = parse_args(&["treease".to_string(), "operators".to_string()])
            .expect_err("parent discovery command should fail");

        match error {
            CliError::Eval(message) => {
                assert!(message.contains("subcommand"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn operators_get_select_returns_json_metadata() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "operators".to_string(),
            "get".to_string(),
            "select".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ])
        .expect("operators get should parse");

        let output = execute_metadata_command(&parsed).expect("metadata should render");
        let value: serde_json::Value = serde_json::from_slice(&output).expect("valid json");

        assert_eq!(value["name"], "select");
        assert_eq!(value["category"], "special");
        assert_eq!(value["syntax"], "select(EXPR)");
    }

    #[test]
    fn formats_get_yaml_returns_json_metadata() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "formats".to_string(),
            "get".to_string(),
            "yaml".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ])
        .expect("formats get should parse");

        let output = execute_metadata_command(&parsed).expect("metadata should render");
        let value: serde_json::Value = serde_json::from_slice(&output).expect("valid json");

        assert_eq!(value["name"], "yaml");
        assert_eq!(value["can_decode"], true);
        assert_eq!(value["can_encode"], true);
    }

    #[test]
    fn unsupported_format_error_has_code_and_hint() {
        let err = CliError::UnsupportedFormat("foo".to_string());
        let report = errors::error_report(&err);

        assert_eq!(report.code, "UNSUPPORTED_FORMAT");
        assert!(report.hint.contains("treease formats list"));
        assert!(errors::render_text(&err).contains("UNSUPPORTED_FORMAT"));
    }

    #[test]
    fn unknown_flag_error_has_code_and_hint() {
        let err = CliError::UnknownFlag("--wat".to_string());
        let report = errors::error_report(&err);

        assert_eq!(report.code, "UNKNOWN_FLAG");
        assert!(report.hint.contains("treease --help"));
    }

    #[test]
    fn unknown_command_error_has_code_and_migration_hint() {
        let err = CliError::UnknownCommand("eval-all".to_string());
        let report = errors::error_report(&err);

        assert_eq!(report.code, "UNKNOWN_COMMAND");
        assert!(report.hint.contains("legacy eval subcommands were removed"));
        assert!(
            report
                .hint
                .contains("treease [OPTIONS] [EXPRESSION] [FILE]")
        );
    }

    #[test]
    fn removed_legacy_eval_commands_are_rejected() {
        for command in ["e", "eval-all", "ea"] {
            let error = parse_args(&["treease".to_string(), command.to_string()])
                .expect_err("legacy eval command should fail");

            match error {
                CliError::UnknownCommand(name) => assert_eq!(name, command),
                other => panic!("unexpected error for {command}: {other:?}"),
            }
        }
    }

    #[test]
    fn help_operators_text_targets_command_specific_help() {
        let help = render_help(
            &parse_args(&[
                "treease".to_string(),
                "help".to_string(),
                "operators".to_string(),
            ])
            .expect("help operators should parse"),
        );

        assert!(help.contains("treease operators <COMMAND>"));
        assert!(help.contains("treease operators list"));
        assert!(!help.contains("treease [OPTIONS] [EXPRESSION] [FILE]"));
    }

    #[test]
    fn web_command_parses_expression_file_and_format_options() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "web".to_string(),
            "-p".to_string(),
            "yaml".to_string(),
            "-o".to_string(),
            "json".to_string(),
            "-I".to_string(),
            "2".to_string(),
            ".service".to_string(),
            "config.yaml".to_string(),
        ])
        .expect("web command should parse");

        assert_eq!(parsed.command, CommandKind::Web);
        assert_eq!(parsed.expression, ".service");
        assert_eq!(parsed.files, vec!["config.yaml"]);
        assert_eq!(parsed.input_format.as_deref(), Some("yaml"));
        assert_eq!(parsed.output_format.as_deref(), Some("json"));
        assert_eq!(parsed.indent, Some(2));
    }

    #[test]
    fn web_command_accepts_stdin_source() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "web".to_string(),
            ".".to_string(),
            "-".to_string(),
        ])
        .expect("web stdin should parse");

        assert_eq!(parsed.command, CommandKind::Web);
        assert_eq!(parsed.files, vec!["-"]);
    }

    #[test]
    fn web_command_rejects_multiple_input_sources() {
        let error = parse_args(&[
            "treease".to_string(),
            "web".to_string(),
            ".".to_string(),
            "a.yaml".to_string(),
            "b.yaml".to_string(),
        ])
        .expect_err("web should reject multiple files");

        match error {
            CliError::InvalidWebInputCount => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn root_invocation_with_web_file_name_is_not_prechecked_as_web_command() {
        let parsed = parse_args(&["treease".to_string(), ".".to_string(), "web".to_string()])
            .expect("root invocation should parse");

        assert_eq!(parsed.command, CommandKind::Run);
        assert_eq!(parsed.expression, ".");
        assert_eq!(parsed.files, vec!["web"]);
    }

    #[test]
    fn web_command_rejects_root_execution_flags_that_do_not_apply() {
        let error = parse_args(&[
            "treease".to_string(),
            "-e".to_string(),
            "web".to_string(),
            ".".to_string(),
            "a.yaml".to_string(),
        ])
        .expect_err("web should reject exit-status");

        match error {
            CliError::UnsupportedWebFlag("--exit-status") => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn web_command_preserves_unknown_flag_error() {
        let error = parse_args(&[
            "treease".to_string(),
            "web".to_string(),
            "--wat".to_string(),
            ".".to_string(),
            "file.yaml".to_string(),
        ])
        .expect_err("unknown web flag should fail");

        match error {
            CliError::UnknownFlag(flag) => assert_eq!(flag, "--wat"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn web_command_preserves_unknown_flag_with_value_error() {
        let error = parse_args(&[
            "treease".to_string(),
            "web".to_string(),
            "--wat".to_string(),
            "value".to_string(),
            ".".to_string(),
            "file.yaml".to_string(),
        ])
        .expect_err("unknown web flag should fail before input count validation");

        match error {
            CliError::UnknownFlag(flag) => assert_eq!(flag, "--wat"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn web_payload_uses_expression_output_and_output_format() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "web".to_string(),
            "-o".to_string(),
            "json".to_string(),
            ".foo".to_string(),
            "input.yaml".to_string(),
        ])
        .expect("web should parse");
        let inputs = vec![InputPayload {
            name: "input.yaml".to_string(),
            bytes: b"foo:\n  bar: 1\n".to_vec(),
        }];

        let payload = web_payload::build_cli_graph_result_payload(&parsed, &inputs)
            .expect("web payload should be produced");

        assert_eq!(payload.source_label, "input.yaml");
        assert_eq!(payload.expression, ".foo");
        assert_eq!(payload.language, "json");
        assert_eq!(payload.text, "{\"bar\": 1}\n");
    }

    #[test]
    fn web_payload_supports_stdin_and_defaults_to_yaml() {
        let parsed = parse_args(&[
            "treease".to_string(),
            "web".to_string(),
            ".".to_string(),
            "-".to_string(),
        ])
        .expect("web stdin should parse");
        let inputs = vec![InputPayload {
            name: "<stdin>".to_string(),
            bytes: b"foo: 1\n".to_vec(),
        }];

        let payload = web_payload::build_cli_graph_result_payload(&parsed, &inputs)
            .expect("web payload should be produced");

        assert_eq!(payload.source_label, "<stdin>");
        assert_eq!(payload.language, "yaml");
        assert_eq!(payload.text, "foo: 1\n");
    }

    #[test]
    fn web_command_rejects_subcommand_position_unsupported_flags() {
        for (flag, expected) in [
            ("--null-input", "--null-input"),
            ("--exit-status", "--exit-status"),
            ("--inplace", "--inplace"),
        ] {
            let error = parse_args(&[
                "treease".to_string(),
                "web".to_string(),
                flag.to_string(),
                ".".to_string(),
                "file.yaml".to_string(),
            ])
            .expect_err("web should reject unsupported execution flag");

            match error {
                CliError::UnsupportedWebFlag(actual) => assert_eq!(actual, expected),
                other => panic!("unexpected error for {flag}: {other:?}"),
            }
        }
    }

    #[test]
    fn help_json_advertises_web_command() {
        let json = spec::root_help_json_value();
        let subcommands = json
            .get("subcommands")
            .and_then(serde_json::Value::as_array)
            .expect("root subcommands should be present");

        let web_command = subcommands
            .iter()
            .find(|command| command.get("name").and_then(serde_json::Value::as_str) == Some("web"))
            .expect("web command should be advertised");

        assert!(subcommands.iter().any(|command| {
            command.get("name").and_then(serde_json::Value::as_str) == Some("web")
                && command.get("usage").and_then(serde_json::Value::as_str)
                    == Some("treease web [OPTIONS] <EXPRESSION> <FILE|->")
        }));

        let file_argument = web_command
            .get("arguments")
            .and_then(serde_json::Value::as_array)
            .and_then(|arguments| {
                arguments.iter().find(|argument| {
                    argument.get("name").and_then(serde_json::Value::as_str) == Some("file")
                })
            })
            .expect("web file argument should be present");

        assert_eq!(
            file_argument
                .get("repeated")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            file_argument
                .get("multiple")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
    }
}
