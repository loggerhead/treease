use serde::Serialize;

use treease_core::core::LANG_SPECS;
use treease_core::formats::configured_language_preferences;

#[cfg(not(feature = "lite"))]
use treease_core::operators::{OpFlags, append_ops};

const NO_ALIASES: &[&str] = &[];
const ANY_INPUT_KINDS: &[&str] = &["any"];
const NO_EXAMPLES: &[&str] = &[];
const NO_RELATED: &[&str] = &[];
const DOCS_LIMITATION: &[&str] = &[
    "Detailed operator guidance is still being expanded; consult docs/operators/ for full semantics.",
];
const SELECT_INPUT_KINDS: &[&str] = &["array", "map", "scalar"];
const SELECT_EXAMPLES: &[&str] = &["treease '.[] | select(.enabled)' sample.yml"];
const SELECT_RELATED: &[&str] = &["equals", "not_equals", "relational", "test", "filter"];
const SELECT_LIMITATIONS: &[&str] =
    &["Regular-expression behavior follows Treease string operators."];
const LENGTH_INPUT_KINDS: &[&str] = &["string", "array", "map", "null"];
const LENGTH_EXAMPLES: &[&str] = &["treease '.items | length' sample.yml"];
const LENGTH_RELATED: &[&str] = &["keys", "map", "flatten"];
const LENGTH_LIMITATIONS: &[&str] =
    &["Length semantics follow Treease node kinds and may differ from yq edge cases."];

#[derive(Debug, Clone, Serialize)]
pub(super) struct OperatorInfo {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub category: &'static str,
    pub summary: &'static str,
    pub syntax: &'static str,
    pub input_kinds: &'static [&'static str],
    pub output_kind: &'static str,
    pub examples: &'static [&'static str],
    pub related: &'static [&'static str],
    pub yq_compat: &'static str,
    pub limitations: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct FormatInfo {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub can_decode: bool,
    pub can_encode: bool,
    pub default_pretty_print: bool,
    pub preferences: FormatPreferenceInfo,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct FormatPreferenceInfo {
    pub indent: i32,
    pub unwrap_scalar: bool,
    pub print_doc_separators: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ExampleInfo {
    pub name: &'static str,
    pub title: &'static str,
    pub input: &'static str,
    pub command: &'static str,
    pub output: &'static str,
    pub operators: &'static [&'static str],
    pub formats: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct DoctorInfo {
    pub binary: &'static str,
    pub version: &'static str,
    pub supported_formats: Vec<&'static str>,
    pub supported_operator_count: usize,
    pub notes: Vec<&'static str>,
}

#[cfg(not(feature = "lite"))]
pub(super) fn operators() -> Vec<OperatorInfo> {
    let mut entries = Vec::new();
    append_ops(&mut entries, &OpFlags::default());

    let mut operators = entries
        .into_iter()
        .map(|entry| canonical_operator_name(entry.id.name()))
        .filter(|name| !matches!(*name, "expression" | "value"))
        .map(operator_info)
        .collect::<Vec<_>>();

    operators.sort_by(|left, right| left.name.cmp(right.name));
    operators.dedup_by(|left, right| left.name == right.name);
    operators
}

#[cfg(feature = "lite")]
pub(super) fn operators() -> Vec<OperatorInfo> {
    Vec::new()
}

pub(super) fn find_operator(name: &str) -> Option<OperatorInfo> {
    let normalized = name.trim().to_ascii_lowercase();
    operators().into_iter().find(|operator| {
        operator.name.eq_ignore_ascii_case(&normalized)
            || operator
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(&normalized))
    })
}

pub(super) fn search_operators(query: &str) -> Vec<OperatorInfo> {
    operators()
        .into_iter()
        .filter(|operator| {
            let haystack = format!(
                "{} {} {} {}",
                operator.name,
                operator.category,
                operator.summary,
                operator.aliases.join(" ")
            );
            matches_query(&haystack, query)
        })
        .collect()
}

pub(super) fn formats() -> Vec<FormatInfo> {
    let preferences = configured_language_preferences();
    let mut formats = LANG_SPECS
        .iter()
        .filter(|spec| spec.enabled && spec.is_format)
        .filter_map(|spec| {
            let language = spec.format_language?;
            let effective = preferences.effective(language);
            Some(FormatInfo {
                name: spec.name,
                extensions: spec.extensions,
                can_decode: true,
                can_encode: true,
                default_pretty_print: spec.default_pretty_print,
                preferences: FormatPreferenceInfo {
                    indent: effective.indent,
                    unwrap_scalar: effective.unwrap_scalar,
                    print_doc_separators: effective.print_doc_separators,
                },
            })
        })
        .collect::<Vec<_>>();

    formats.sort_by(|left, right| left.name.cmp(right.name));
    formats
}

pub(super) fn find_format(name: &str) -> Option<FormatInfo> {
    let canonical = treease_core::core::find_cli_format_spec(name)?.name;
    formats()
        .into_iter()
        .find(|format| format.name == canonical)
}

pub(super) fn examples() -> Vec<ExampleInfo> {
    vec![
        ExampleInfo {
            name: "filter-array",
            title: "Filter enabled array items",
            input: "- name: a\n  enabled: true\n- name: b\n  enabled: false\n",
            command: "treease '.[] | select(.enabled)' sample.yml",
            output: "name: a\nenabled: true\n",
            operators: &["select"],
            formats: &["yaml"],
        },
        ExampleInfo {
            name: "read-field",
            title: "Read a nested field from a document",
            input: "a:\n  b: value\n",
            command: "treease '.a.b' file.yaml",
            output: "value\n",
            operators: &["self", "traverse_path"],
            formats: &["yaml"],
        },
        ExampleInfo {
            name: "transcode-json",
            title: "Transcode YAML input to JSON output",
            input: "name: treease\nversion: 1\n",
            command: "treease -p yaml -o json '.' file.yaml",
            output: "{\n  \"name\": \"treease\",\n  \"version\": 1\n}\n",
            operators: &["self"],
            formats: &["yaml", "json"],
        },
    ]
}

pub(super) fn find_example(name: &str) -> Option<ExampleInfo> {
    let normalized = name.trim().to_ascii_lowercase();
    examples()
        .into_iter()
        .find(|example| example.name.eq_ignore_ascii_case(&normalized))
}

pub(super) fn search_examples(query: &str) -> Vec<ExampleInfo> {
    examples()
        .into_iter()
        .filter(|example| {
            let haystack = format!(
                "{} {} {} {} {}",
                example.name,
                example.title,
                example.command,
                example.operators.join(" "),
                example.formats.join(" ")
            );
            matches_query(&haystack, query)
        })
        .collect()
}

pub(super) fn doctor_info() -> DoctorInfo {
    let supported_formats = formats()
        .into_iter()
        .map(|format| format.name)
        .collect::<Vec<_>>();
    let supported_operator_count = operators().len();

    DoctorInfo {
        binary: "treease",
        version: env!("CARGO_PKG_VERSION"),
        supported_formats,
        supported_operator_count,
        notes: vec![
            "Default execution keeps stdout reserved for data output.",
            "Use `treease help --format json` for machine-readable command discovery.",
        ],
    }
}

fn operator_info(name: &'static str) -> OperatorInfo {
    match name {
        "select" => OperatorInfo {
            name,
            aliases: NO_ALIASES,
            category: "special",
            summary: "Filter arrays and maps by a boolean expression.",
            syntax: "select(EXPR)",
            input_kinds: SELECT_INPUT_KINDS,
            output_kind: "matching values",
            examples: SELECT_EXAMPLES,
            related: SELECT_RELATED,
            yq_compat: "partial",
            limitations: SELECT_LIMITATIONS,
        },
        "length" => OperatorInfo {
            name,
            aliases: NO_ALIASES,
            category: "collection",
            summary: "Return the length of strings, arrays, maps, and null.",
            syntax: "length",
            input_kinds: LENGTH_INPUT_KINDS,
            output_kind: "number",
            examples: LENGTH_EXAMPLES,
            related: LENGTH_RELATED,
            yq_compat: "partial",
            limitations: LENGTH_LIMITATIONS,
        },
        _ => OperatorInfo {
            name,
            aliases: operator_aliases(name),
            category: operator_category(name),
            summary: operator_summary(name),
            syntax: operator_syntax(name),
            input_kinds: ANY_INPUT_KINDS,
            output_kind: "varies",
            examples: NO_EXAMPLES,
            related: NO_RELATED,
            yq_compat: "unknown",
            limitations: DOCS_LIMITATION,
        },
    }
}

fn operator_aliases(name: &str) -> &'static [&'static str] {
    match name {
        "get_tag" => &["tag", "type"],
        "get_key" => &["key"],
        "get_kind" => &["kind"],
        "get_parent" => &["parent"],
        "get_parents" => &["parents"],
        "get_path" => &["path"],
        "set_path" => &["setpath"],
        "del_paths" => &["delpaths"],
        "change_case" => &["upcase", "downcase"],
        "sub" => &["substr", "substitute"],
        _ => NO_ALIASES,
    }
}

fn operator_category(name: &str) -> &'static str {
    match name {
        "pipe" | "short_pipe" | "self" | "traverse_path" | "traverse_array"
        | "recursive_descent" | "get_path" | "set_path" | "del_paths" | "delete" => "traversal",
        "or" | "and" | "not" | "alternative" | "any" | "all" | "any_condition"
        | "all_condition" => "logic",
        "assign" | "add_assign" | "subtract_assign" | "multiply_assign" | "assign_variable" => {
            "assign"
        }
        "add" | "subtract" | "multiply" | "divide" | "modulo" => "math",
        "equals" | "not_equals" | "relational" | "min" | "max" => "relational",
        "create_map" | "collect" | "collect_object" | "map" | "map_values" | "pick" | "omit"
        | "union" | "unique" | "unique_by" | "group_by" | "flatten" | "length" => "collection",
        "encode" | "decode" | "to_entries" | "from_entries" | "with_entries" | "to_number" => {
            "codec"
        }
        "join"
        | "sub"
        | "match"
        | "capture"
        | "test"
        | "split"
        | "change_case"
        | "trim"
        | "to_string"
        | "string_interpolation" => "strings",
        "sort_by" | "sort" | "sort_keys" | "reverse" | "shuffle" => "sort",
        "get_variable" | "get_tag" | "get_kind" | "get_key" | "is_key" | "keys" | "get_parent"
        | "get_parents" | "contains" | "has" => "meta",
        "reduce" | "block" | "empty" | "with" | "first" | "select" | "filter" => "special",
        _ => "misc",
    }
}

fn operator_summary(name: &str) -> &'static str {
    match name {
        "pipe" => "Pass the left-hand result into the next expression.",
        "short_pipe" => "Evaluate multiple expressions in sequence.",
        "self" => "Return the current node unchanged.",
        "traverse_path" => "Traverse object keys by path segment.",
        "traverse_array" => "Traverse array or collection elements.",
        "recursive_descent" => "Walk descendant nodes recursively.",
        "get_path" => "Return the current node path.",
        "set_path" => "Set a value at a path.",
        "del_paths" => "Delete values at one or more paths.",
        "delete" => "Delete matching child values.",
        "or" => "Boolean OR over the current expression context.",
        "and" => "Boolean AND over the current expression context.",
        "not" => "Invert truthiness.",
        "alternative" => "Return the right-hand value when the left-hand side is empty.",
        "any" => "Return true when any value is truthy.",
        "all" => "Return true when all values are truthy.",
        "assign" => "Assign the right-hand value to matching nodes.",
        "add_assign" | "subtract_assign" | "multiply_assign" => {
            "Update matching nodes in place with an arithmetic operation."
        }
        "assign_variable" => "Bind a value to a variable for later use.",
        "add" | "subtract" | "multiply" | "divide" | "modulo" => {
            "Apply arithmetic to numeric values."
        }
        "equals" | "not_equals" | "relational" => "Compare values.",
        "min" => "Return the minimum value from a sequence.",
        "max" => "Return the maximum value from a sequence.",
        "create_map" => "Construct an object value.",
        "collect" => "Collect values into an array.",
        "collect_object" => "Collect key-value pairs into an object.",
        "map" => "Apply an expression to each sequence item.",
        "map_values" => "Apply an expression to each object value.",
        "pick" => "Select a subset of object keys or sequence indexes.",
        "omit" => "Drop object keys or sequence indexes.",
        "union" => "Concatenate or combine multiple result streams.",
        "unique" => "Remove duplicate values.",
        "unique_by" => "Remove duplicates based on a derived key.",
        "group_by" => "Group values by a derived key.",
        "flatten" => "Flatten nested arrays.",
        "encode" => "Encode values into a target format.",
        "decode" => "Decode formatted text into structured values.",
        "to_entries" => "Convert objects or arrays into entry objects.",
        "from_entries" => "Build an object from entry objects.",
        "with_entries" => "Rewrite entries with an expression.",
        "to_number" => "Convert scalar values to numbers.",
        "join" => "Join strings with a delimiter.",
        "sub" => "Replace substring or regex matches.",
        "match" => "Return regex match details.",
        "capture" => "Return regex capture groups.",
        "test" => "Return whether a regex matches.",
        "split" => "Split strings by a delimiter.",
        "change_case" => "Change string case.",
        "trim" => "Trim string whitespace.",
        "to_string" => "Convert values to strings.",
        "string_interpolation" => "Interpolate expressions into strings.",
        "sort_by" => "Sort values by a derived key.",
        "sort" => "Sort values.",
        "sort_keys" => "Sort object keys.",
        "reverse" => "Reverse sequence order.",
        "shuffle" => "Shuffle sequence order.",
        "get_variable" => "Read a bound variable.",
        "get_tag" => "Return the semantic tag of a value.",
        "get_kind" => "Return the Treease kind of a value.",
        "get_key" => "Return the key for an object entry.",
        "is_key" => "Return whether the current node is a key.",
        "keys" => "Return object keys or array indexes.",
        "contains" => "Return whether a value contains another value.",
        "has" => "Return whether a key or index exists.",
        "reduce" => "Fold a stream of values into an accumulator.",
        "block" => "Group expressions into a block.",
        "empty" => "Produce no output.",
        "with" => "Update a subtree with a scoped expression.",
        "first" => "Return the first matching value.",
        "filter" => "Keep values that satisfy a predicate.",
        _ => "Built-in Treease operator.",
    }
}

fn operator_syntax(name: &str) -> &'static str {
    match name {
        "pipe" => "LHS | RHS",
        "short_pipe" => "EXPR ; EXPR",
        "self" => ".",
        "traverse_path" => ".name",
        "traverse_array" => ".[]",
        "recursive_descent" => "..",
        "get_path" => "path",
        "set_path" => "setpath(PATH; VALUE)",
        "del_paths" => "delpaths(PATHS)",
        "delete" => "del(EXPR)",
        "or" => "LHS or RHS",
        "and" => "LHS and RHS",
        "not" => "not",
        "alternative" => "LHS // RHS",
        "any" => "any",
        "all" => "all",
        "any_condition" => "any(EXPR)",
        "all_condition" => "all(EXPR)",
        "assign" => "LHS = RHS | LHS |= RHS",
        "add_assign" => "LHS += RHS",
        "subtract_assign" => "LHS -= RHS",
        "multiply_assign" => "LHS *= RHS",
        "assign_variable" => "EXPR as $name",
        "add" => "LHS + RHS",
        "subtract" => "LHS - RHS",
        "multiply" => "LHS * RHS",
        "divide" => "LHS / RHS",
        "modulo" => "LHS % RHS",
        "equals" => "LHS == RHS",
        "not_equals" => "LHS != RHS",
        "relational" => "LHS < RHS | <= | > | >=",
        "min" => "min",
        "max" => "max",
        "create_map" => "{ key: value }",
        "collect" => "[EXPR]",
        "collect_object" => "{EXPR}",
        "map" => "map(EXPR)",
        "map_values" => "map_values(EXPR)",
        "pick" => "pick(PATHS)",
        "omit" => "omit(PATHS)",
        "union" => "EXPR, EXPR",
        "unique" => "unique",
        "unique_by" => "unique_by(EXPR)",
        "group_by" => "group_by(EXPR)",
        "flatten" => "flatten(DEPTH?)",
        "encode" => "@json | to_json",
        "decode" => "@jsond | from_json",
        "to_entries" => "to_entries",
        "from_entries" => "from_entries",
        "with_entries" => "with_entries(EXPR)",
        "to_number" => "to_number",
        "join" => "join(DELIM)",
        "sub" => "sub(REGEX; REPLACEMENT)",
        "match" => "match(REGEX)",
        "capture" => "capture(REGEX)",
        "test" => "test(REGEX)",
        "split" => "split(DELIM)",
        "change_case" => "upcase | downcase",
        "trim" => "trim",
        "to_string" => "to_string",
        "string_interpolation" => "\"... \\(...) ...\"",
        "sort_by" => "sort_by(EXPR)",
        "sort" => "sort",
        "sort_keys" => "sort_keys",
        "reverse" => "reverse",
        "shuffle" => "shuffle",
        "get_variable" => "$name",
        "get_tag" => "tag | type",
        "get_kind" => "kind",
        "get_key" => "key",
        "is_key" => "is_key",
        "keys" => "keys",
        "contains" => "contains(EXPR)",
        "has" => "has(KEY)",
        "reduce" => "reduce EXPR as $item (...)",
        "block" => "(EXPR)",
        "empty" => "empty",
        "with" => "with(PATH; EXPR)",
        "first" => "first",
        "filter" => "filter(EXPR)",
        _ => "See operator docs for syntax.",
    }
}

fn matches_query(haystack: &str, query: &str) -> bool {
    let haystack = haystack.to_ascii_lowercase();
    let terms = query
        .split_whitespace()
        .map(|term| term.to_ascii_lowercase())
        .collect::<Vec<_>>();

    !terms.is_empty() && terms.into_iter().all(|term| haystack.contains(&term))
}

fn canonical_operator_name(name: &'static str) -> &'static str {
    match name {
        "self_reference" => "self",
        "join_string" => "join",
        "sub_string" => "sub",
        "split_string" => "split",
        other => other,
    }
}
