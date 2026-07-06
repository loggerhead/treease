use serde::Serialize;

use crate::args::CliError;

#[derive(Debug, Clone, Serialize)]
pub(super) struct CliErrorReport {
    pub code: &'static str,
    pub message: String,
    pub hint: &'static str,
    pub docs: &'static str,
    pub examples: Vec<&'static str>,
}

pub(super) fn error_report(err: &CliError) -> CliErrorReport {
    match err {
        CliError::MissingValue(flag) => report(
            "MISSING_VALUE",
            format!("missing value for {flag}"),
            "Provide the value immediately after the flag or command argument.",
            "docs/cli/README.md",
            vec!["treease -p yaml '.a' file.yaml"],
        ),
        CliError::InvalidIndent(value) => report(
            "INVALID_INDENT",
            format!("invalid indent: {value}"),
            "Use an integer indentation width.",
            "docs/cli/README.md",
            vec!["treease -o json -I 2 '.a' file.yaml"],
        ),
        CliError::UnsupportedFormat(value) => report(
            "UNSUPPORTED_FORMAT",
            format!("unsupported format: {value}"),
            "Run `treease formats list` to see supported formats.",
            "docs/cli/README.md",
            vec![
                "treease formats list",
                "treease -p yaml -o json '.a' file.yaml",
            ],
        ),
        CliError::UnsupportedOperator(value) => report(
            "UNSUPPORTED_OPERATOR",
            format!("unsupported operator: {value}"),
            "Run `treease operators list` to see supported operators.",
            "docs/operators/README.md",
            vec![
                "treease operators list",
                "treease operators get select --format json",
            ],
        ),
        CliError::InvalidFlagCombination => report(
            "INVALID_FLAG_COMBINATION",
            "The argument '--inplace' cannot be used with --null-input or stdin".to_string(),
            "Use --inplace with exactly one real input file.",
            "docs/cli/README.md",
            vec!["treease -i '.a = 1' file.yaml"],
        ),
        CliError::MultipleInputFiles => report(
            "INVALID_INPUT_COUNT",
            "This command accepts at most one input file".to_string(),
            "Pass one file path, or omit the file to read stdin.",
            "docs/cli/README.md",
            vec!["treease '.a' file.yaml", "treease '.a'"],
        ),
        CliError::MultipleInputFilesForInplace => report(
            "INVALID_FLAG_COMBINATION",
            "The argument '--inplace' requires exactly one input file".to_string(),
            "Pass one file path when using --inplace.",
            "docs/cli/README.md",
            vec!["treease -i '.a = 1' file.yaml"],
        ),
        CliError::UnsupportedWebFlag(flag) => report(
            "UNSUPPORTED_WEB_FLAG",
            format!("the argument '{flag}' is not supported by `treease web`"),
            "Use `treease web` with format/display options and exactly one input source.",
            "docs/cli/README.md",
            vec!["treease web -p yaml -o json '.service' config.yaml"],
        ),
        CliError::InvalidWebInputCount => report(
            "INVALID_WEB_INPUT_COUNT",
            "`treease web` requires exactly one input source".to_string(),
            "Pass one file path or `-` for stdin.",
            "docs/cli/README.md",
            vec!["treease web '.' file.yaml", "treease web '.' -"],
        ),
        CliError::WebAssetDownload(message) => report(
            "WEB_ASSET_DOWNLOAD_ERROR",
            message.clone(),
            "Check network access to treease.com or override TREEASE_WEB_ASSET_BASE_URL for a different asset origin.",
            "docs/cli/README.md",
            vec!["treease web '.' file.yaml"],
        ),
        CliError::WebAssetManifest(message) => report(
            "WEB_ASSET_MANIFEST_ERROR",
            message.clone(),
            "The downloaded web asset manifest or files are invalid. Rebuild and redeploy the web assets, then retry.",
            "docs/cli/README.md",
            vec!["treease web '.' file.yaml"],
        ),
        CliError::WebAssetCache(message) => report(
            "WEB_ASSET_CACHE_ERROR",
            message.clone(),
            "Check whether the local cache directory is writable, or set TREEASE_WEB_CACHE_DIR to a writable path.",
            "docs/cli/README.md",
            vec!["treease web '.' file.yaml"],
        ),
        CliError::WebServer(message) => report(
            "WEB_SERVER_ERROR",
            message.clone(),
            "Check whether the web server port is available and retry.",
            "docs/cli/README.md",
            vec!["treease web '.' file.yaml"],
        ),
        CliError::WebForbidden => report(
            "WEB_FORBIDDEN",
            "forbidden web request".to_string(),
            "Only local web UI asset and payload requests are allowed.",
            "docs/cli/README.md",
            vec!["treease web '.' file.yaml"],
        ),
        CliError::UnknownCommand(command) => report(
            "UNKNOWN_COMMAND",
            format!("unknown command: {command}"),
            "Use `treease [OPTIONS] [EXPRESSION] [FILE]`; legacy eval subcommands were removed.",
            "docs/cli/README.md",
            vec!["treease '.a.b' file.yaml", "treease --help"],
        ),
        CliError::UnknownFlag(flag) => report(
            "UNKNOWN_FLAG",
            format!("unknown flag: {flag}"),
            "Run `treease --help` for execution flags or `treease help --format json` for schema.",
            "docs/cli/README.md",
            vec!["treease --help", "treease help --format json"],
        ),
        CliError::Eval(message) => report(
            "EXECUTION_ERROR",
            message.clone(),
            "Check the expression syntax and supported operators.",
            "docs/operators/README.md",
            vec!["treease operators list", "treease '.a.b' file.yaml"],
        ),
        CliError::Io(err) => report(
            "IO_ERROR",
            err.to_string(),
            "Check the input file path and permissions.",
            "docs/cli/README.md",
            vec!["treease '.a' file.yaml"],
        ),
        CliError::Core(err) => report(
            "EXECUTION_ERROR",
            err.to_string(),
            "Check the input document, expression, and selected formats.",
            "docs/cli/README.md",
            vec!["treease '.a.b' file.yaml"],
        ),
    }
}

pub(super) fn render_text(err: &CliError) -> String {
    let report = error_report(err);
    format!(
        "{} [{}]\nhint: {}\n",
        report.message, report.code, report.hint
    )
}

#[allow(dead_code)]
pub(super) fn render_json(err: &CliError) -> String {
    let mut output =
        serde_json::to_string_pretty(&error_report(err)).expect("error report should serialize");
    output.push('\n');
    output
}

fn report(
    code: &'static str,
    message: String,
    hint: &'static str,
    docs: &'static str,
    examples: Vec<&'static str>,
) -> CliErrorReport {
    CliErrorReport {
        code,
        message,
        hint,
        docs,
        examples,
    }
}
