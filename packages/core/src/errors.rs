use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemError {
    Error,
    EndOfStream,
    StreamTooLong,
    InvalidUtf8,
    NulInNulSeparatedOutput,
    Io(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    InvalidSyntax,
    InvalidYaml,
    InvalidJson,
    InvalidPython,
    InvalidJavaScript,
    InvalidGo,
    BadParameter,
    BadCsv,
    BadTomlStringEscape,
    BadTomlKey,
    BadTomlBoolean,
    UnsupportedTomlValue,
    BadTomlPair,
    BadTomlDocument,
    BadTomlTable,
    BadTomlArrayTable,
    UnsupportedTomlTopLevel,
    TreeSitterSetLanguageFailed,
    TreeSitterParseFailed,
    CodepointTooLarge,
    Utf8CannotEncodeSurrogateHalf,
    NegativeIndex,
    InvalidPadding,
    InvalidCharacter,
    UnknownToken,
    UnterminatedString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    UnknownFormat,
    TomlRequiresMap,
    TomlEmptyPath,
    TomlNoAliases,
    TomlUnsupportedKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    UnknownOperator,
    MissingRhs,
    MissingLhs,
    MissingTreeNode,
    MustUseVariableWithPipe,
    InvalidVariableRhs,
    KeysOnlyWorksForMapsAndArrays,
    CannotConvertValueToNumber,
    CannotConvertNodeToNumber,
    NodeIsNotArray,
    ExpectedSingleNumber,
    UniqueOnlySupportsArrays,
    CannotModuloByZero,
    CannotModuloTypes,
    CannotModuloNull,
    CannotModuloNonScalars,
    CannotDivideTypes,
    CannotDivideNull,
    CannotDivideNonScalars,
    StringsCannotBeSubtracted,
    CannotSubtractTypes,
    MapsNotSupportedForSubtraction,
    CannotSubtractNonSequence,
    CannotSubtractNonScalar,
    Unsupported,
    ExpectedMap,
    NoKeys,
    FromEntriesOnlyRunsAgainstArrays,
    CannotIndexArray,
    CannotPickIndicesFromType,
    NegativeRepeat,
    RepeatTooLarge,
    CannotMultiplyTypes,
    CannotAddTypes,
    CannotAddNonMapToMap,
    CannotAddNonScalarToScalar,
    IndexOutOfRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    OutOfMemory,
    System(SystemError),
    Parse(ParseError),
    Format(FormatError),
    Eval(EvalError),
    Io(String),
    ParseMessage {
        line: usize,
        column: usize,
        message: String,
    },
    OperatorMessage {
        op: String,
        message: String,
    },
    WasmProtocol {
        code: i32,
    },
    CapabilityMissing {
        language: String,
        capability: &'static str,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::OutOfMemory => write!(f, "out of memory"),
            CoreError::System(error) => write!(f, "system error: {error}"),
            CoreError::Parse(error) => write!(f, "parse error: {error}"),
            CoreError::Format(error) => write!(f, "format error: {error}"),
            CoreError::Eval(error) => write!(f, "eval error: {error}"),
            CoreError::Io(message) => write!(f, "I/O error: {message}"),
            CoreError::ParseMessage {
                line,
                column,
                message,
            } => write!(f, "parse error at {line}:{column}: {message}"),
            CoreError::OperatorMessage { op, message } => write!(f, "operator {op}: {message}"),
            CoreError::WasmProtocol { code } => write!(f, "WASM protocol error: {code}"),
            CoreError::CapabilityMissing {
                language,
                capability,
            } => write!(f, "language capability missing: {language}.{capability}"),
        }
    }
}

impl fmt::Display for SystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemError::Io(message) => write!(f, "I/O error: {message}"),
            other => write!(f, "system error: {other:?}"),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "format error: {self:?}")
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "eval error: {self:?}")
    }
}

impl std::error::Error for SystemError {}
impl std::error::Error for ParseError {}
impl std::error::Error for FormatError {}
impl std::error::Error for EvalError {}
impl std::error::Error for CoreError {}

impl From<SystemError> for CoreError {
    fn from(value: SystemError) -> Self {
        CoreError::System(value)
    }
}

impl From<ParseError> for CoreError {
    fn from(value: ParseError) -> Self {
        CoreError::Parse(value)
    }
}

impl From<FormatError> for CoreError {
    fn from(value: FormatError) -> Self {
        CoreError::Format(value)
    }
}

impl From<EvalError> for CoreError {
    fn from(value: EvalError) -> Self {
        CoreError::Eval(value)
    }
}

impl From<std::io::Error> for CoreError {
    fn from(value: std::io::Error) -> Self {
        CoreError::Io(value.to_string())
    }
}
