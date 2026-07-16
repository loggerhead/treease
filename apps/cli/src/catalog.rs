use serde::Serialize;

use treease_core::core::LANG_SPECS;
use treease_core::formats::configured_language_preferences;

use treease_core::operators::{OpFlags, append_ops};

const NO_ALIASES: &[&str] = &[];
const ANY_INPUT_KINDS: &[&str] = &["any"];
const NO_LIMITATIONS: &[&str] = &[];
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

fn operator_info(name: &'static str) -> OperatorInfo {
    OperatorInfo {
        name,
        aliases: operator_aliases(name),
        category: operator_category(name),
        summary: operator_summary(name),
        syntax: operator_syntax(name),
        input_kinds: operator_input_kinds(name),
        output_kind: operator_output_kind(name),
        examples: operator_examples(name),
        related: operator_related(name),
        yq_compat: operator_yq_compat(name),
        limitations: operator_limitations(name),
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
        "any_condition" => "Return true when any array element satisfies the condition.",
        "all_condition" => "Return true when all array elements satisfy the condition.",
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
        "length" => "Return the length of strings, arrays, maps, and null.",
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
        "get_parent" => "Return an ancestor of the current node.",
        "get_parents" => "Return all ancestors of the current node.",
        "contains" => "Return whether a value contains another value.",
        "has" => "Return whether a key or index exists.",
        "reduce" => "Fold a stream of values into an accumulator.",
        "block" => "Group expressions into a block.",
        "empty" => "Produce no output.",
        "with" => "Update a subtree with a scoped expression.",
        "first" => "Return the first matching value.",
        "select" => "Filter arrays and maps by a boolean expression.",
        "filter" => "Keep values that satisfy a predicate.",
        _ => unreachable!("unknown operator summary: {name}"),
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
        "length" => "length",
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
        "get_parent" => "parent(N?)",
        "get_parents" => "parents",
        "contains" => "contains(EXPR)",
        "has" => "has(KEY)",
        "reduce" => "reduce EXPR as $item (...)",
        "block" => "(EXPR)",
        "empty" => "empty",
        "with" => "with(PATH; EXPR)",
        "first" => "first",
        "select" => "select(EXPR)",
        "filter" => "filter(EXPR)",
        _ => unreachable!("unknown operator syntax: {name}"),
    }
}

fn operator_input_kinds(name: &str) -> &'static [&'static str] {
    match name {
        "pipe"
        | "short_pipe"
        | "self"
        | "assign"
        | "add_assign"
        | "subtract_assign"
        | "multiply_assign"
        | "assign_variable"
        | "or"
        | "and"
        | "not"
        | "alternative"
        | "equals"
        | "not_equals"
        | "relational"
        | "create_map"
        | "collect"
        | "collect_object"
        | "encode"
        | "to_string"
        | "string_interpolation"
        | "get_variable"
        | "get_tag"
        | "get_kind"
        | "contains"
        | "get_parent"
        | "get_parents"
        | "get_path"
        | "set_path"
        | "del_paths"
        | "delete"
        | "reduce"
        | "block"
        | "empty"
        | "with"
        | "filter"
        | "first"
        | "union" => ANY_INPUT_KINDS,
        "select" => SELECT_INPUT_KINDS,
        "traverse_path" => &["map"],
        "traverse_array" => &["array", "map"],
        "recursive_descent" => &["map", "array", "scalar"],
        "any" | "all" | "any_condition" | "all_condition" => &["array"],
        "add" => &["number", "string", "array", "map", "null"],
        "subtract" => &["number", "array", "datetime", "null"],
        "multiply" => &["number", "string", "array", "map", "null"],
        "divide" => &["number", "string"],
        "modulo" => &["number"],
        "min" | "max" => &["array"],
        "map" | "group_by" | "flatten" | "unique" | "unique_by" | "join" | "sort_by" | "sort"
        | "reverse" | "shuffle" => &["array"],
        "map_values" | "with_entries" | "sort_keys" => &["map"],
        "pick" | "omit" | "keys" | "has" => &["map", "array"],
        "length" => LENGTH_INPUT_KINDS,
        "decode" => &["string"],
        "to_entries" => &["map", "array", "null"],
        "from_entries" => &["array"],
        "to_number" => &["string", "number"],
        "sub" | "match" | "capture" | "test" | "split" | "change_case" | "trim" => &["string"],
        "get_key" | "is_key" => &["entry"],
        _ => unreachable!("unknown operator input kinds: {name}"),
    }
}

fn operator_output_kind(name: &str) -> &'static str {
    match name {
        "pipe" | "traverse_path" | "traverse_array" | "recursive_descent" | "select" | "filter"
        | "first" => "matching values",
        "short_pipe" | "block" | "union" => "combined result stream",
        "self" => "input value",
        "get_path" | "keys" | "collect" | "flatten" | "split" => "array",
        "set_path" | "del_paths" | "delete" | "assign" | "add_assign" | "subtract_assign"
        | "multiply_assign" | "with" => "updated document",
        "or" | "and" | "not" | "any" | "all" | "any_condition" | "all_condition" | "equals"
        | "not_equals" | "relational" | "contains" | "has" | "is_key" | "test" => "boolean",
        "alternative" => "lhs if present, otherwise rhs",
        "assign_variable" => "original input context",
        "add" | "subtract" | "multiply" => "computed value or merged collection",
        "divide" | "modulo" | "length" | "to_number" => "number",
        "min" | "max" => "extreme value",
        "create_map" => "map entry stream",
        "collect_object" | "from_entries" => "map",
        "map" | "unique" | "unique_by" | "reverse" | "shuffle" | "sort" => "array",
        "map_values" => "map",
        "pick" | "omit" => "same collection shape as input",
        "group_by" => "array of groups",
        "encode" | "join" | "sub" | "get_key" | "get_kind" | "get_tag" | "to_string" | "trim"
        | "change_case" => "string",
        "decode" => "decoded structured value",
        "to_entries" => "entry array",
        "with_entries" => "rewritten map",
        "match" | "capture" => "match detail map or map stream",
        "string_interpolation" => "interpolated string",
        "get_variable" => "bound value",
        "get_parent" => "ancestor node",
        "get_parents" => "ancestor sequence",
        "reduce" => "accumulator result",
        "empty" => "no output",
        "sort_by" => "sorted array",
        "sort_keys" => "key-sorted map",
        _ => unreachable!("unknown operator output kind: {name}"),
    }
}

fn operator_related(name: &str) -> &'static [&'static str] {
    match name {
        "pipe" => &["short_pipe", "self", "union"],
        "short_pipe" => &["pipe", "block", "union"],
        "self" => &["traverse_path", "pipe", "select"],
        "traverse_path" => &["self", "traverse_array", "recursive_descent"],
        "traverse_array" => &["traverse_path", "recursive_descent", "select"],
        "recursive_descent" => &["traverse_path", "traverse_array", "select"],
        "get_path" => &["set_path", "del_paths", "delete"],
        "set_path" => &["get_path", "del_paths", "assign"],
        "del_paths" => &["get_path", "set_path", "delete"],
        "delete" => &["del_paths", "select", "filter"],
        "or" | "and" | "not" => &["any", "all", "select"],
        "alternative" => &["or", "assign", "has"],
        "any" | "all" | "any_condition" | "all_condition" => &["or", "and", "select"],
        "assign" | "add_assign" | "subtract_assign" | "multiply_assign" => {
            &["assign_variable", "with", "set_path"]
        }
        "assign_variable" | "get_variable" => &["pipe", "reduce", "with"],
        "add" | "subtract" | "multiply" | "divide" | "modulo" => &["assign", "to_number", "map"],
        "equals" | "not_equals" | "relational" => &["select", "filter", "contains"],
        "min" | "max" => &["sort", "group_by", "relational"],
        "create_map" | "collect_object" => &["collect", "to_entries", "with_entries"],
        "collect" => &["collect_object", "union", "map"],
        "map" | "map_values" => &["filter", "select", "with_entries"],
        "pick" | "omit" => &["keys", "has", "delete"],
        "union" => &["collect", "pipe", "short_pipe"],
        "unique" | "unique_by" => &["group_by", "sort", "map"],
        "group_by" => &["sort_by", "unique_by", "map"],
        "flatten" => &["map", "collect", "traverse_array"],
        "length" => &LENGTH_RELATED,
        "encode" | "decode" => &["to_string", "to_number", "with_entries"],
        "to_entries" | "from_entries" | "with_entries" => &["create_map", "collect_object", "keys"],
        "to_number" => &["add", "subtract", "to_string"],
        "join" | "split" => &["to_string", "map", "collect"],
        "sub" | "match" | "capture" | "test" => &["split", "change_case", "select"],
        "change_case" | "trim" | "to_string" | "string_interpolation" => &["join", "split", "sub"],
        "get_tag" | "get_kind" | "get_key" | "is_key" | "keys" => {
            &["get_parent", "get_path", "has"]
        }
        "get_parent" | "get_parents" => &["get_path", "recursive_descent", "keys"],
        "contains" | "has" => &["equals", "keys", "select"],
        "reduce" => &["assign_variable", "get_variable", "collect"],
        "block" => &["short_pipe", "pipe", "union"],
        "empty" => &["select", "filter", "delete"],
        "with" => &["assign", "set_path", "map"],
        "first" => &["select", "filter", "sort"],
        "select" => &SELECT_RELATED,
        "filter" => &["select", "map", "first"],
        "sort_by" | "sort" | "sort_keys" | "reverse" | "shuffle" => {
            &["unique", "group_by", "first"]
        }
        _ => unreachable!("unknown operator related set: {name}"),
    }
}

fn operator_yq_compat(name: &str) -> &'static str {
    match name {
        "short_pipe" | "block" | "empty" | "string_interpolation" => "unknown",
        "traverse_path" | "traverse_array" | "recursive_descent" | "multiply_assign" | "add"
        | "multiply" | "equals" | "not_equals" | "relational" | "create_map" | "collect"
        | "map" | "map_values" | "omit" | "union" | "encode" | "decode" | "join" | "sub"
        | "capture" | "split" | "change_case" | "to_string" | "get_kind" | "contains"
        | "reduce" | "sort_keys" => "partial",
        _ => "compatible",
    }
}

fn operator_limitations(name: &str) -> &'static [&'static str] {
    match name {
        "create_map" => &[
            "This is an internal pair-builder; multi-result object construction is surfaced through object syntax and `collect_object`.",
        ],
        "collect" => &[
            "Array collection follows Treease's evaluate-together semantics, so multi-candidate contexts may collect per candidate.",
        ],
        "map" => &[
            "Mapping over an object yields a sequence of transformed values rather than rewriting the object in place.",
        ],
        "map_values" => &[
            "Only map values are rewritten, and when RHS yields multiple values Treease keeps the first one.",
        ],
        "omit" => &["Non-collection inputs are returned unchanged instead of raising an error."],
        "union" => &[
            "Treease avoids duplicating results when both sides resolve to the same passthrough list.",
        ],
        "encode" | "decode" => &[
            "Supported formats are limited to the codecs registered in the current build; unknown codecs return an error.",
        ],
        "traverse_path" => &[
            "Missing paths can synthesize null placeholders, and merge keys are treated as ordinary keys rather than YAML merge semantics.",
        ],
        "traverse_array" => &[
            "Array traversal also powers slices and index-based expansion, including null-padding behavior for some out-of-range lookups.",
        ],
        "join" => &[
            "Join expects an array input and does not serialize nested maps or arrays into YAML text first.",
        ],
        "multiply_assign" => &[
            "Relative multiplication inherits Treease's current multiply semantics, including shallow map merge behavior.",
        ],
        "add" => &[
            "Map addition is shallow and Treease also supports scalar-plus-map combinations that are not emphasized in yq docs.",
        ],
        "multiply" => &[
            "Map multiplication currently behaves as a shallow merge rather than yq's full deep-merge feature set.",
        ],
        "equals" | "not_equals" => &[
            "String equality supports wildcard-style matching, so equality is broader than strict byte-for-byte comparison.",
        ],
        "relational" => &[
            "Current relational comparisons cover scalar ordering but do not expose the broader datetime/documentation surface yq describes.",
        ],
        "sub" => &[
            "Replacement expansion is narrower than yq's full syntax and currently focuses on whole-match and indexed captures.",
        ],
        "capture" => &[
            "Capture results include numeric keys and the full-match key `0` in addition to named groups.",
        ],
        "split" => {
            &["Null inputs are skipped rather than converted into an empty-string split result."]
        }
        "change_case" => &[
            "The CLI exposes `upcase` and `downcase` aliases; `change_case` is the internal canonical name.",
        ],
        "to_string" => &[
            "Maps and arrays stringify to their tag-like representation rather than serialized YAML content.",
        ],
        "get_tag" => &[
            "Current metadata covers reading tags via `tag`/`type`; mutation forms are not surfaced here.",
        ],
        "get_kind" => &[
            "Treease exposes additional kinds such as `alias` and `unknown` beyond the core `map`/`seq`/`scalar` set.",
        ],
        "contains" => &["Type mismatches can surface an error instead of simply returning false."],
        "reduce" => &[
            "Treease documents the current infix `... as $x reduce (...)` form rather than yq's `ireduce` naming.",
        ],
        "block" => &[
            "This is primarily a syntax-carrier for paired expressions like `with(path; update)` rather than a standalone user-facing operator.",
        ],
        "empty" => &[
            "Treease uses `empty` as a real empty-stream operator, but its yq documentation coverage is much thinner than most operators.",
        ],
        "recursive_descent" => &[
            "The `..` form is documented here; key-inclusive recursive descent remains a separate preference-driven variant.",
        ],
        "sort_keys" => &[
            "Sorting keys does not attempt to preserve YAML anchor or merge-key semantics in every case.",
        ],
        "length" => LENGTH_LIMITATIONS,
        "select" => SELECT_LIMITATIONS,
        _ => NO_LIMITATIONS,
    }
}

fn operator_examples(name: &str) -> &'static [&'static str] {
    match name {
        "pipe" => &["treease '.a | .b' sample.yml"],
        "short_pipe" => &["treease '.a.b' sample.yml"],
        "self" => &["treease '.' sample.yml"],
        "traverse_path" => &["treease '.a.b' sample.yml"],
        "traverse_array" => &["treease '.items[]' sample.yml"],
        "recursive_descent" => &["treease '.. | .name' sample.yml"],
        "get_path" => &["treease '.a[] | path' sample.yml"],
        "set_path" => &[r#"treease -n 'setpath(["a", 0]; "x")'"#],
        "del_paths" => &[r#"treease 'delpaths([["a","debug"],["a","tmp"]])' sample.yml"#],
        "delete" => &["treease 'del(.a.debug)' sample.yml"],
        "or" => &["treease -n 'true or false'"],
        "and" => &["treease -n 'true and false'"],
        "not" => &["treease -n 'true | not'"],
        "alternative" => &[r#"treease '.nickname // "anonymous"' sample.yml"#],
        "any" => &["treease '.flags | any' sample.yml"],
        "all" => &["treease '.flags | all' sample.yml"],
        "any_condition" => &["treease '.items | any(.enabled)' sample.yml"],
        "all_condition" => &["treease '.items | all(.enabled)' sample.yml"],
        "assign" => &["treease '.a = .b' sample.yml"],
        "add_assign" => &["treease '.count += 1' sample.yml"],
        "subtract_assign" => &["treease '.count -= 1' sample.yml"],
        "multiply_assign" => &["treease '.count *= 2' sample.yml"],
        "assign_variable" => &["treease '.a as $x | $x' sample.yml"],
        "add" => &["treease '.a + .b' sample.yml"],
        "subtract" => &["treease '.count - 1' sample.yml"],
        "multiply" => &["treease '.count * 2' sample.yml"],
        "divide" => &["treease '.path / \"/\"' sample.yml"],
        "modulo" => &["treease '.count % 2' sample.yml"],
        "equals" => &[r#"treease '.kind == "cat"' sample.yml"#],
        "not_equals" => &[r#"treease '.kind != "cat"' sample.yml"#],
        "relational" => &["treease '.count >= 10' sample.yml"],
        "min" => &["treease '.items | min' sample.yml"],
        "max" => &["treease '.items | max' sample.yml"],
        "create_map" => &[r#"treease '{user: .name}' sample.yml"#],
        "collect" => &["treease '[.a, .b]' sample.yml"],
        "collect_object" => &[r#"treease '{"name": .name, "pet": .pets[]}' sample.yml"#],
        "map" => &["treease '.items | map(. + 1)' sample.yml"],
        "map_values" => &["treease '.labels | map_values(. + \"-x\")' sample.yml"],
        "pick" => &[r#"treease 'pick(["a", "c"])' sample.yml"#],
        "omit" => &[r#"treease 'omit(["debug"])' sample.yml"#],
        "union" => &["treease '.a, .b' sample.yml"],
        "unique" => &["treease '.items | unique' sample.yml"],
        "unique_by" => &["treease '.users | unique_by(.id)' sample.yml"],
        "group_by" => &["treease '.users | group_by(.team)' sample.yml"],
        "flatten" => &["treease '.items | flatten(1)' sample.yml"],
        "length" => LENGTH_EXAMPLES,
        "encode" => &["treease '.value | to_json(0)' sample.yml"],
        "decode" => &["treease '.raw | from_json' sample.yml"],
        "to_entries" => &["treease '.labels | to_entries' sample.yml"],
        "from_entries" => &["treease '.pairs | from_entries' sample.yml"],
        "with_entries" => &["treease '.labels | with_entries(.value |= upcase)' sample.yml"],
        "to_number" => &["treease '.count | to_number' sample.yml"],
        "join" => &["treease '.tags | join(\",\")' sample.yml"],
        "sub" => &[r#"treease '.name | sub("cat"; "dog")' sample.yml"#],
        "match" => &[r#"treease '.name | match("cat")' sample.yml"#],
        "capture" => &[r#"treease '.name | capture("(?P<animal>cat)")' sample.yml"#],
        "test" => &[r#"treease '.name | test("cat")' sample.yml"#],
        "split" => &[r#"treease '.name | split(",")' sample.yml"#],
        "change_case" => &["treease '.name | upcase' sample.yml"],
        "trim" => &["treease '.name | trim' sample.yml"],
        "to_string" => &["treease '.value | to_string' sample.yml"],
        "string_interpolation" => &[r#"treease '"Hello \(.name)"' sample.yml"#],
        "get_variable" => &["treease '.a as $x | $x' sample.yml"],
        "get_tag" => &["treease '.. | tag' sample.yml"],
        "get_kind" => &["treease '.. | kind' sample.yml"],
        "get_key" => &["treease '.a | key' sample.yml"],
        "is_key" => &["treease '... | is_key' sample.yml"],
        "keys" => &["treease '.labels | keys' sample.yml"],
        "get_parent" => &["treease '.a.b | parent' sample.yml"],
        "get_parents" => &["treease '.a.b.c | parents' sample.yml"],
        "contains" => &["treease '.tags | contains([\"prod\"])' sample.yml"],
        "has" => &[r#"treease '.items[] | has("enabled")' sample.yml"#],
        "reduce" => &["treease '.items[] as $item reduce (0; . + $item)' sample.yml"],
        "block" => &[r#"treease 'with(.a; . = "x")' sample.yml"#],
        "empty" => &["treease -n 'empty'"],
        "with" => &[r#"treease 'with(.a; . = "x")' sample.yml"#],
        "first" => &["treease '.items | first(.enabled)' sample.yml"],
        "select" => SELECT_EXAMPLES,
        "filter" => &["treease '.items | filter(.enabled)' sample.yml"],
        "sort_by" => &["treease '.users | sort_by(.name, .age)' sample.yml"],
        "sort" => &["treease '.items | sort' sample.yml"],
        "sort_keys" => &["treease 'sort_keys(..)' sample.yml"],
        "reverse" => &["treease '.items | reverse' sample.yml"],
        "shuffle" => &["treease '.items | shuffle' sample.yml"],
        _ => unreachable!("unknown operator example set: {name}"),
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
