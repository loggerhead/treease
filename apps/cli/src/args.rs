use std::io;

use treease_core::core::CoreError;

#[derive(Debug, Clone)]
pub(crate) struct ParsedArgs {
    pub(crate) command: CommandKind,
    pub(crate) expression: String,
    pub(crate) files: Vec<String>,
    pub(crate) help: bool,
    pub(crate) null_input: bool,
    pub(crate) exit_status: bool,
    pub(crate) input_format: Option<String>,
    pub(crate) output_format: Option<String>,
    pub(crate) pretty_print: Option<bool>,
    pub(crate) indent: Option<i32>,
    pub(crate) unwrap_scalar: bool,
    pub(crate) no_doc: bool,
    pub(crate) inplace: bool,
    pub(crate) metadata_format: Option<String>,
    pub(crate) metadata_target: Option<String>,
    pub(crate) metadata_category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommandKind {
    Run,
    Web,
    Help,
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
pub(crate) enum CliError {
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
    #[allow(dead_code)]
    WebServer(String),
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
pub(crate) struct InputPayload {
    pub(crate) name: String,
    pub(crate) bytes: Vec<u8>,
}

impl InputPayload {
    pub(crate) fn display_name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StreamingInput {
    pub(crate) name: String,
    pub(crate) input_format: String,
    pub(crate) source: StreamingInputSource,
}

#[derive(Debug, Clone)]
pub(crate) enum StreamingInputSource {
    Stdin(Vec<u8>),
    FilePath(String),
}

impl StreamingInput {
    pub(crate) fn display_name(&self) -> &str {
        &self.name
    }
}
