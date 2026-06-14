use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CliCommandId {
    Root,
    Help,
    Operators,
    OperatorsList,
    OperatorsGet,
    OperatorsSearch,
    Formats,
    FormatsList,
    FormatsGet,
    Examples,
    ExamplesList,
    ExamplesGet,
    ExamplesSearch,
    Doctor,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CliOptionScope {
    Global,
    CommandLocal,
    HelpOnly,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CliCommandSpec {
    pub id: CliCommandId,
    pub name: String,
    pub segments: Vec<String>,
    pub path: String,
    pub usage: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<CliArgumentSpec>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<CliOptionSpec>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<CliExampleSpec>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subcommands: Vec<CliCommandSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CliOptionSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    pub long: String,
    pub takes_value: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_name: Option<String>,
    pub multiple: bool,
    pub scope: CliOptionScope,
    pub help: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CliArgumentSpec {
    pub name: String,
    pub required: bool,
    pub repeated: bool,
    pub multiple: bool,
    pub help: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CliExampleSpec {
    pub command: String,
    pub description: String,
}

pub(super) fn root_command_spec() -> CliCommandSpec {
    command_spec(
        CliCommandId::Root,
        &["treease"],
        "treease [OPTIONS] [EXPRESSION] [FILE]...",
        "Evaluate Treease expressions over structured documents.",
        vec![
            CliArgumentSpec {
                name: "expression".to_string(),
                required: false,
                repeated: false,
                multiple: false,
                help: "Expression to evaluate. Defaults to `.`.".to_string(),
            },
            CliArgumentSpec {
                name: "file".to_string(),
                required: false,
                repeated: true,
                multiple: true,
                help: "Input files. Reads stdin when omitted.".to_string(),
            },
        ],
        {
            let mut options = execution_options();
            options.push(help_option());
            options
        },
        vec![
            CliExampleSpec {
                command: "treease '.foo' data.yaml".to_string(),
                description: "Read a field from a structured document.".to_string(),
            },
            CliExampleSpec {
                command: "treease --null-input '{hello:\"world\"}'".to_string(),
                description: "Evaluate an expression without input files.".to_string(),
            },
        ],
        vec![
            command_spec(
                CliCommandId::Help,
                &["treease", "help"],
                "treease help [COMMAND] [--format text|json]",
                "Show CLI help in text or machine-readable JSON.",
                vec![CliArgumentSpec {
                    name: "command".to_string(),
                    required: false,
                    repeated: false,
                    multiple: false,
                    help: "Optional command path to inspect.".to_string(),
                }],
                metadata_options(),
                vec![CliExampleSpec {
                    command: "treease help --format json".to_string(),
                    description: "Emit the root command schema as JSON.".to_string(),
                }],
                Vec::new(),
            ),
            command_spec(
                CliCommandId::Operators,
                &["treease", "operators"],
                "treease operators <COMMAND>",
                "Inspect available operators.",
                Vec::new(),
                Vec::new(),
                vec![CliExampleSpec {
                    command: "treease operators list".to_string(),
                    description: "Inspect supported operators.".to_string(),
                }],
                vec![
                    command_spec(
                        CliCommandId::OperatorsList,
                        &["treease", "operators", "list"],
                        "treease operators list [--category CATEGORY] [--format text|json]",
                        "List available operators.",
                        Vec::new(),
                        {
                            let mut options = vec![category_option()];
                            options.extend(metadata_options());
                            options
                        },
                        Vec::new(),
                        Vec::new(),
                    ),
                    command_spec(
                        CliCommandId::OperatorsGet,
                        &["treease", "operators", "get"],
                        "treease operators get <name> [--format text|json]",
                        "Show a single operator.",
                        vec![required_argument("name", "Operator name.")],
                        metadata_options(),
                        Vec::new(),
                        Vec::new(),
                    ),
                    command_spec(
                        CliCommandId::OperatorsSearch,
                        &["treease", "operators", "search"],
                        "treease operators search <query>",
                        "Search operators.",
                        vec![required_argument("query", "Search query.")],
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    ),
                ],
            ),
            command_spec(
                CliCommandId::Formats,
                &["treease", "formats"],
                "treease formats <COMMAND>",
                "Inspect available formats.",
                Vec::new(),
                Vec::new(),
                vec![CliExampleSpec {
                    command: "treease formats list".to_string(),
                    description: "Inspect supported serialization formats.".to_string(),
                }],
                vec![
                    command_spec(
                        CliCommandId::FormatsList,
                        &["treease", "formats", "list"],
                        "treease formats list [--format text|json]",
                        "List available input and output formats.",
                        Vec::new(),
                        metadata_options(),
                        Vec::new(),
                        Vec::new(),
                    ),
                    command_spec(
                        CliCommandId::FormatsGet,
                        &["treease", "formats", "get"],
                        "treease formats get <name> [--format text|json]",
                        "Show a single format.",
                        vec![required_argument("name", "Format name.")],
                        metadata_options(),
                        Vec::new(),
                        Vec::new(),
                    ),
                ],
            ),
            command_spec(
                CliCommandId::Examples,
                &["treease", "examples"],
                "treease examples <COMMAND>",
                "Inspect bundled examples.",
                Vec::new(),
                Vec::new(),
                vec![CliExampleSpec {
                    command: "treease examples list".to_string(),
                    description: "Show example invocations.".to_string(),
                }],
                vec![
                    command_spec(
                        CliCommandId::ExamplesList,
                        &["treease", "examples", "list"],
                        "treease examples list [--format text|json]",
                        "List runnable examples.",
                        Vec::new(),
                        metadata_options(),
                        Vec::new(),
                        Vec::new(),
                    ),
                    command_spec(
                        CliCommandId::ExamplesGet,
                        &["treease", "examples", "get"],
                        "treease examples get <name> [--format text|json]",
                        "Show a single example.",
                        vec![required_argument("name", "Example name.")],
                        metadata_options(),
                        Vec::new(),
                        Vec::new(),
                    ),
                    command_spec(
                        CliCommandId::ExamplesSearch,
                        &["treease", "examples", "search"],
                        "treease examples search <query>",
                        "Search examples.",
                        vec![required_argument("query", "Search query.")],
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    ),
                ],
            ),
            command_spec(
                CliCommandId::Doctor,
                &["treease", "doctor"],
                "treease doctor [--format text|json]",
                "Run CLI diagnostics.",
                Vec::new(),
                metadata_options(),
                vec![CliExampleSpec {
                    command: "treease doctor".to_string(),
                    description: "Run local CLI diagnostics.".to_string(),
                }],
                Vec::new(),
            ),
        ],
    )
}

pub(super) fn root_help_json_value() -> serde_json::Value {
    serde_json::to_value(root_command_spec()).expect("root CLI help should serialize")
}

pub(super) fn render_root_text_help() -> String {
    render_command_text_help(&root_command_spec())
}

pub(super) fn render_command_text_help(command: &CliCommandSpec) -> String {
    if command.id == CliCommandId::Root {
        return render_root_command_text_help(command);
    }

    let options = command
        .options
        .iter()
        .map(render_option_line)
        .collect::<Vec<_>>()
        .join("\n");
    let commands = command
        .subcommands
        .iter()
        .map(|command| format!("  {:<27} {}", command.path, command.summary))
        .collect::<Vec<_>>()
        .join("\n");
    let arguments = command
        .arguments
        .iter()
        .map(render_argument_line)
        .collect::<Vec<_>>()
        .join("\n");
    let examples = command
        .examples
        .iter()
        .map(|example| format!("  {}", example.command))
        .collect::<Vec<_>>()
        .join("\n");

    let mut sections = vec![format!(
        "{}\n\nUsage:\n  {}",
        command.summary, command.usage
    )];

    if !commands.is_empty() {
        sections.push(format!("Commands:\n{}", commands));
    }
    if !arguments.is_empty() {
        sections.push(format!("Arguments:\n{}", arguments));
    }
    if !options.is_empty() {
        sections.push(format!("Options:\n{}", options));
    }
    if !examples.is_empty() {
        sections.push(format!("Examples:\n{}", examples));
    }

    sections.join("\n\n") + "\n"
}

pub(super) fn find_command_spec(path_segments: &[&str]) -> Option<CliCommandSpec> {
    let root = root_command_spec();
    if path_segments.is_empty() {
        return Some(root);
    }
    find_command_spec_in(&root, path_segments).cloned()
}

pub(super) fn execution_options() -> Vec<CliOptionSpec> {
    vec![
        option_spec(
            "null-input",
            Some("-n"),
            "--null-input",
            false,
            None,
            false,
            CliOptionScope::Global,
            "Evaluate without reading input.",
        ),
        option_spec(
            "exit-status",
            Some("-e"),
            "--exit-status",
            false,
            None,
            false,
            CliOptionScope::Global,
            "Exit with status 1 if result is empty, null, or false.",
        ),
        option_spec(
            "input-format",
            Some("-p"),
            "--input-format",
            true,
            Some("FORMAT"),
            false,
            CliOptionScope::Global,
            "Set input format.",
        ),
        option_spec(
            "output-format",
            Some("-o"),
            "--output-format",
            true,
            Some("FORMAT"),
            false,
            CliOptionScope::Global,
            "Set output format.",
        ),
        option_spec(
            "pretty-print",
            Some("-P"),
            "--prettyPrint",
            false,
            None,
            false,
            CliOptionScope::Global,
            "Pretty-print output.",
        ),
        option_spec(
            "indent",
            Some("-I"),
            "--indent",
            true,
            Some("INDENT"),
            false,
            CliOptionScope::Global,
            "Set indentation width.",
        ),
        option_spec(
            "unwrap-scalar",
            Some("-r"),
            "--unwrapScalar",
            false,
            None,
            false,
            CliOptionScope::Global,
            "Unwrap scalar output.",
        ),
        option_spec(
            "no-doc",
            Some("-N"),
            "--no-doc",
            false,
            None,
            false,
            CliOptionScope::Global,
            "Disable YAML document separators.",
        ),
        option_spec(
            "inplace",
            Some("-i"),
            "--inplace",
            false,
            None,
            false,
            CliOptionScope::Global,
            "Write result back to the input file.",
        ),
    ]
}

pub(super) fn metadata_options() -> Vec<CliOptionSpec> {
    vec![format_option()]
}

fn command_spec(
    id: CliCommandId,
    segments: &[&str],
    usage: &str,
    summary: &str,
    arguments: Vec<CliArgumentSpec>,
    options: Vec<CliOptionSpec>,
    examples: Vec<CliExampleSpec>,
    subcommands: Vec<CliCommandSpec>,
) -> CliCommandSpec {
    let segments = segments
        .iter()
        .map(|segment| (*segment).to_string())
        .collect::<Vec<_>>();
    let name = segments
        .last()
        .cloned()
        .expect("CLI command path should contain at least one segment");
    let path = segments.join(" ");

    CliCommandSpec {
        id,
        name,
        segments,
        path,
        usage: usage.to_string(),
        summary: summary.to_string(),
        arguments,
        options,
        examples,
        subcommands,
    }
}

fn help_option() -> CliOptionSpec {
    option_spec(
        "help",
        Some("-h"),
        "--help",
        false,
        None,
        false,
        CliOptionScope::Global,
        "Show this help message.",
    )
}

fn format_option() -> CliOptionSpec {
    option_spec(
        "format",
        None,
        "--format",
        true,
        Some("FORMAT"),
        false,
        CliOptionScope::HelpOnly,
        "Select help output format.",
    )
}

fn category_option() -> CliOptionSpec {
    option_spec(
        "category",
        None,
        "--category",
        true,
        Some("CATEGORY"),
        false,
        CliOptionScope::CommandLocal,
        "Filter operators by category.",
    )
}

fn required_argument(name: &str, help: &str) -> CliArgumentSpec {
    CliArgumentSpec {
        name: name.to_string(),
        required: true,
        repeated: false,
        multiple: false,
        help: help.to_string(),
    }
}

fn option_spec(
    name: &str,
    short: Option<&str>,
    long: &str,
    takes_value: bool,
    value_name: Option<&str>,
    multiple: bool,
    scope: CliOptionScope,
    help: &str,
) -> CliOptionSpec {
    CliOptionSpec {
        name: name.to_string(),
        short: short.map(str::to_string),
        long: long.to_string(),
        takes_value,
        value_name: value_name.map(str::to_string),
        multiple,
        scope,
        help: help.to_string(),
    }
}

fn find_command_spec_in<'a>(
    command: &'a CliCommandSpec,
    path_segments: &[&str],
) -> Option<&'a CliCommandSpec> {
    if command.segments.len() == path_segments.len()
        && command
            .segments
            .iter()
            .map(String::as_str)
            .eq(path_segments.iter().copied())
    {
        return Some(command);
    }

    command
        .subcommands
        .iter()
        .find_map(|subcommand| find_command_spec_in(subcommand, path_segments))
}

fn render_option_line(option: &CliOptionSpec) -> String {
    let flags = match (&option.short, option.takes_value, &option.value_name) {
        (Some(short), true, Some(value_name)) => format!("{short}, {} <{value_name}>", option.long),
        (Some(short), false, _) => format!("{short}, {}", option.long),
        (None, true, Some(value_name)) => format!("{} <{value_name}>", option.long),
        (None, false, _) => option.long.clone(),
        _ => option.long.clone(),
    };

    format!("  {:<28} {}", flags, option.help)
}

fn render_root_command_text_help(root: &CliCommandSpec) -> String {
    let options = root
        .options
        .iter()
        .filter(|option| option.scope != CliOptionScope::HelpOnly)
        .map(render_option_line)
        .collect::<Vec<_>>()
        .join("\n");
    let commands = root
        .subcommands
        .iter()
        .map(|command| format!("  {:<27} {}", command.path, command.summary))
        .collect::<Vec<_>>()
        .join("\n");
    let discovery = root
        .subcommands
        .iter()
        .filter_map(|command| command.examples.first())
        .map(|example| format!("  {}", example.command))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Treease CLI\n\nUsage:\n  {}\n\nCommands:\n{}\n\nDiscovery:\n{}\n\nOptions:\n{}\n",
        root.usage, commands, discovery, options
    )
}

fn render_argument_line(argument: &CliArgumentSpec) -> String {
    let required = if argument.required {
        "required"
    } else {
        "optional"
    };
    format!("  {:<28} {} ({required})", argument.name, argument.help)
}
