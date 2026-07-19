use std::collections::BTreeMap;

use crate::evaluator::{EvaluationError, Value};
use crate::expression_pipeline;
use crate::operators::operator_helpers::*;
/// String operators: join, match, capture, test, sub, split, trim,
/// to_string, string_interpolation, change_case.
///
use crate::operators::*;
// ── Regex wrapper ──────────────────────────────────────────────────
//
// Uses the workspace's regex implementation for regular expression support.

struct RegexStub {
    inner: regex::Regex,
}

struct RegexMatch {
    start: usize,
    end: usize,
    captures: Vec<RegexCapture>,
}

struct RegexCapture {
    index: usize,
    name: Option<String>,
    text: Option<String>,
    start: Option<usize>,
    end: Option<usize>,
}

impl RegexStub {
    fn new(pattern: &str) -> Result<Self, CoreError> {
        let inner = regex::Regex::new(pattern)
            .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
        Ok(Self { inner })
    }

    fn captures_at(&self, input: &str, start: usize) -> Option<RegexMatch> {
        let captures = self.inner.captures_at(input, start)?;
        let full_match = captures.get(0)?;
        let capture_names = self.inner.capture_names().collect::<Vec<_>>();
        let groups = (1..captures.len())
            .map(|index| {
                let name = capture_names
                    .get(index)
                    .and_then(|name| *name)
                    .map(str::to_owned);
                match captures.get(index) {
                    Some(matched) => {
                        let text = Some(matched.as_str().to_owned());
                        RegexCapture {
                            index,
                            name,
                            text,
                            start: Some(matched.start()),
                            end: Some(matched.end()),
                        }
                    }
                    None => RegexCapture {
                        index,
                        name,
                        text: None,
                        start: None,
                        end: None,
                    },
                }
            })
            .collect();

        Some(RegexMatch {
            start: full_match.start(),
            end: full_match.end(),
            captures: groups,
        })
    }

    fn find_at(&self, input: &str, start: usize) -> Option<RegexMatch> {
        self.captures_at(input, start).map(|m| RegexMatch {
            start: m.start,
            end: m.end,
            captures: m.captures,
        })
    }

    fn is_match(&self, input: &str) -> bool {
        self.inner.is_match(input)
    }
}

/// Advance past a regex match for the next search iteration.
///
/// For non-zero-width matches, returns `match_end`.
/// For zero-width matches, advances to the next UTF-8 boundary because
/// The regex crate searches `&str` and requires valid string offsets.
fn next_search_start(input: &str, match_start: usize, match_end: usize) -> usize {
    if match_end > match_start {
        return match_end;
    }
    match input[match_start..].chars().next() {
        Some(ch) => match_start + ch.len_utf8(),
        None => input.len() + 1,
    }
}

// ── Helper: evaluate RHS to get a string ───────────────────────────

fn rhs_string(
    d: &mut TreeEngine,
    ctx: &Context,
    rhs: Option<&mut ExpressionNode>,
) -> Result<String, CoreError> {
    let exp = match rhs {
        Some(e) => e,
        None => return Ok(String::new()),
    };
    let ro = ctx.read_only_clone()?;
    let got = get_matching_nodes(d, &ro, Some(exp))
        .unwrap_or_else(|_| ctx.read_only_clone().unwrap_or_default());
    if got.matching_nodes.is_empty() {
        return Ok(String::new());
    }
    Ok(got.matching_nodes[0].value.clone())
}

// ── Join operator ──────────────────────────────────────────────────

/// join operator: concatenate array elements with a separator string.
pub fn join_string_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let join_str = rhs_string(d, &ctx, expression_node.rhs.as_deref_mut())?;

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        if candidate.kind != NodeKind::Sequence {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!(
                        "cannot join with {}, can only join arrays of scalars",
                        candidate.tag
                    ),
                )?;
            }
            return Err(CoreError::OperatorMessage {
                op: "join".to_string(),
                message: format!("cannot join with {}", candidate.tag),
            });
        }

        let mut parts: Vec<String> = Vec::new();
        for (idx, child) in candidate.content.iter().enumerate() {
            if idx != 0 {
                parts.push(join_str.clone());
            }
            let s = if child.sem_type == Some(SemType::Nil) {
                String::new()
            } else {
                child.value.clone()
            };
            parts.push(s);
        }
        let joined: String = parts.concat();
        let repl =
            candidate.create_replacement(NodeKind::Scalar, SemType::Str.to_string(), &joined)?;
        results.push((*repl).clone());
    }

    ctx.child_context(results)
}

// ── Match argument extraction ──────────────────────────────────────

struct MatchPreferences {
    global: bool,
}

fn extract_match_arguments(
    d: &mut TreeEngine,
    ctx: &Context,
    expression_node: &mut ExpressionNode,
) -> Result<(String, MatchPreferences), CoreError> {
    let mut regex_exp_node: Option<&mut ExpressionNode> = expression_node.rhs.as_deref_mut();
    let mut prefs = MatchPreferences { global: false };

    if regex_exp_node
        .as_ref()
        .is_some_and(|exp_node| exp_node.operation.operation_type.id == BLOCK_OP_TYPE.id)
    {
        let exp_node = regex_exp_node
            .take()
            .ok_or(CoreError::Eval(EvalError::MissingRhs))?;
        let ro = ctx.read_only_clone()?;
        let rhs_nodes = get_matching_nodes(d, &ro, exp_node.rhs.as_deref_mut())?;
        let param_text = rhs_nodes
            .matching_nodes
            .first()
            .map_or(String::new(), |n| n.value.clone());

        if param_text.contains('g') {
            prefs.global = true;
            let remaining: String = param_text.chars().filter(|&ch| ch != 'g').collect();
            let remaining = remaining.trim_matches(|c: char| " \t\r\n".contains(c));

            if remaining.contains('i') {
                if let Some(ref d2) = ctx.diagnostics {
                    d2.set_messagef("eval", "'i' is not a valid option for match. To ignore case, use an expression like match(\"(?i)cat\")")?;
                }
                return Err(CoreError::Parse(ParseError::InvalidSyntax));
            }
            if !remaining.is_empty() {
                if let Some(ref d2) = ctx.diagnostics {
                    d2.set_messagef(
                        "eval",
                        &format!("unrecognised match params '{}'", remaining),
                    )?;
                }
                return Err(CoreError::Parse(ParseError::InvalidSyntax));
            }
        } else {
            let remaining = param_text.trim_matches(|c: char| " \t\r\n".contains(c));
            if remaining.contains('i') {
                if let Some(ref d2) = ctx.diagnostics {
                    d2.set_messagef("eval", "'i' is not a valid option for match. To ignore case, use an expression like match(\"(?i)cat\")")?;
                }
                return Err(CoreError::Parse(ParseError::InvalidSyntax));
            }
            if !remaining.is_empty() {
                if let Some(ref d2) = ctx.diagnostics {
                    d2.set_messagef(
                        "eval",
                        &format!("unrecognised match params '{}'", remaining),
                    )?;
                }
                return Err(CoreError::Parse(ParseError::InvalidSyntax));
            }
        }

        regex_exp_node = exp_node.lhs.as_deref_mut();
    }

    let regex_nodes = get_matching_nodes(d, ctx, regex_exp_node)?;
    let regex_str = regex_nodes
        .matching_nodes
        .first()
        .map_or(String::new(), |n| n.value.clone());

    Ok((regex_str, prefs))
}

// ── Match info helper ──────────────────────────────────────────────

fn append_match_info(
    map_node: &mut TreeNode,
    match_value: Option<&str>,
    offset: i32,
    length: i32,
    name: &str,
) -> Result<(), CoreError> {
    let k_string = create_string_scalar_node("string")?;
    let v_string = if let Some(mv) = match_value {
        create_string_scalar_node(mv)?
    } else {
        create_scalar_node_null()?
    };
    map_node.add_key_value_child(&k_string, &v_string)?;

    let k_offset = create_string_scalar_node("offset")?;
    let v_offset = create_scalar_node_i64(offset as i64)?;
    map_node.add_key_value_child(&k_offset, &v_offset)?;

    let k_length = create_string_scalar_node("length")?;
    let v_length = create_scalar_node_i64(length as i64)?;
    map_node.add_key_value_child(&k_length, &v_length)?;

    if !name.is_empty() {
        let k_name = create_string_scalar_node("name")?;
        let v_name = create_string_scalar_node(name)?;
        map_node.add_key_value_child(&k_name, &v_name)?;
    }

    Ok(())
}

// ── Match operator ─────────────────────────────────────────────────

/// match operator: perform regex matching and return match metadata.
pub fn match_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let (regex_str, prefs) = extract_match_arguments(d, &ctx, expression_node)?;
    let cr = RegexStub::new(&regex_str)?;

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let inferred = SemType::from_string(&candidate.guess_tag_from_custom_type());
        if inferred != Some(SemType::Str) {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!("cannot match with {}, can only match strings. Hint: Most often you'll want to use '|=' over '=' for this operation", candidate.tag),
                )?;
            }
            return Err(CoreError::OperatorMessage {
                op: "match".to_string(),
                message: "can only match strings".to_string(),
            });
        }

        let mut start_index = 0;
        while start_index <= candidate.value.len() {
            let Some(m) = cr.captures_at(&candidate.value, start_index) else {
                break;
            };
            let full_match = &candidate.value[m.start..m.end];

            let mut captures_list = (*candidate.create_replacement(
                NodeKind::Sequence,
                SemType::Seq.to_string(),
                "",
            )?)
            .clone();
            captures_list.content.clear();

            let mut out_map =
                (*candidate.create_replacement(NodeKind::Mapping, SemType::Map.to_string(), "")?)
                    .clone();
            append_match_info(
                &mut out_map,
                Some(full_match),
                m.start as i32,
                (m.end - m.start) as i32,
                "",
            )?;

            for capture in &m.captures {
                let mut capture_map = (*candidate.create_replacement(
                    NodeKind::Mapping,
                    SemType::Map.to_string(),
                    "",
                )?)
                .clone();
                let offset = capture.start.map(|value| value as i32).unwrap_or(-1);
                let length = match (capture.start, capture.end) {
                    (Some(start), Some(end)) => (end - start) as i32,
                    _ => 0,
                };
                append_match_info(
                    &mut capture_map,
                    capture.text.as_deref(),
                    offset,
                    length,
                    capture.name.as_deref().unwrap_or(""),
                )?;
                captures_list.add_child(&capture_map)?;
            }

            let k_captures = create_string_scalar_node("captures")?;
            out_map.add_key_value_child(&k_captures, &captures_list)?;
            results.push(out_map);

            if !prefs.global {
                break;
            }
            start_index = next_search_start(&candidate.value, m.start, m.end);
        }
    }
    ctx.child_context(results)
}

// ── Capture operator ───────────────────────────────────────────────

/// capture operator: return the matched substring(s).
pub fn capture_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let (regex_str, prefs) = extract_match_arguments(d, &ctx, expression_node)?;
    let cr = RegexStub::new(&regex_str)?;

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let inferred = SemType::from_string(&candidate.guess_tag_from_custom_type());
        if inferred != Some(SemType::Str) {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!("cannot match with {}, can only match strings. Hint: Most often you'll want to use '|=' over '=' for this operation", candidate.tag),
                )?;
            }
            return Err(CoreError::OperatorMessage {
                op: "capture".to_string(),
                message: "can only match strings".to_string(),
            });
        }

        let mut start_index = 0;
        while start_index <= candidate.value.len() {
            let Some(m) = cr.captures_at(&candidate.value, start_index) else {
                break;
            };

            let mut out_map =
                (*candidate.create_replacement(NodeKind::Mapping, SemType::Map.to_string(), "")?)
                    .clone();
            let key = create_string_scalar_node("0")?;
            let value = create_string_scalar_node(&candidate.value[m.start..m.end])?;
            out_map.add_key_value_child(&key, &value)?;

            for capture in &m.captures {
                let numbered_key = create_string_scalar_node(&capture.index.to_string())?;
                let numbered_value = match capture.text.as_deref() {
                    Some(text) => create_string_scalar_node(text)?,
                    None => create_scalar_node_null()?,
                };
                out_map.add_key_value_child(&numbered_key, &numbered_value)?;

                if let Some(name) = capture.name.as_deref() {
                    let named_key = create_string_scalar_node(name)?;
                    let named_value = match capture.text.as_deref() {
                        Some(text) => create_string_scalar_node(text)?,
                        None => create_scalar_node_null()?,
                    };
                    out_map.add_key_value_child(&named_key, &named_value)?;
                }
            }
            results.push(out_map);

            if !prefs.global {
                break;
            }
            start_index = next_search_start(&candidate.value, m.start, m.end);
        }
    }
    ctx.child_context(results)
}

// ── Test operator ──────────────────────────────────────────────────

/// test operator: return true/false if the string matches the regex.
pub fn test_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let (regex_str, _prefs) = extract_match_arguments(d, &ctx, expression_node)?;
    let cr = RegexStub::new(&regex_str)?;

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let inferred = SemType::from_string(&candidate.guess_tag_from_custom_type());
        if inferred != Some(SemType::Str) {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!("cannot match with {}, can only match strings. Hint: Most often you'll want to use '|=' over '=' for this operation", candidate.tag),
                )?;
            }
            return Err(CoreError::OperatorMessage {
                op: "test".to_string(),
                message: "can only match strings".to_string(),
            });
        }

        let ok = cr.is_match(&candidate.value);
        let result = create_boolean_candidate(candidate, ok)?;
        results.push((*result).clone());
    }
    ctx.child_context(results)
}

// ── String interpolation ───────────────────────────────────────────

/// Evaluate a string interpolation expression and stringify the result.
///
/// Avoids unnecessary compat-to-core TreeNode-to-Value-to-TreeNode
/// round-trips: when the pipeline result is already a string or null,
/// it is returned directly without converting to TreeNode.
fn evaluate_to_string(
    d: &mut TreeEngine,
    ctx: &Context,
    exp_str: &str,
) -> Result<String, CoreError> {
    let _ = d;
    let input = ctx
        .matching_nodes
        .first()
        .map(tree_to_value)
        .transpose()?
        .unwrap_or(Value::Null);
    let parsed = expression_pipeline::parse(exp_str).map_err(map_pipeline_error)?;
    let output =
        expression_pipeline::execute(&input, parsed.as_deref()).map_err(map_pipeline_error)?;

    // Fast path: avoid TreeNode conversion for already-string results.
    match &output {
        Value::String(s) => return Ok(s.clone()),
        Value::Null => return Ok(String::new()),
        _ => {}
    }

    // Only convert to TreeNode when we need to encode (non-scalar results).
    let node = value_to_tree(&output)?;
    if node.kind == NodeKind::Scalar {
        return Ok(node.value);
    }

    let rendered = crate::operators::encoder_decoder::encode_node_to_string(ctx, &node, "yaml", 2)?;
    Ok(rendered.trim_end_matches('\n').to_string())
}

fn tree_to_value(node: &TreeNode) -> Result<Value, CoreError> {
    match node.kind {
        NodeKind::Sequence => node
            .content
            .iter()
            .map(tree_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        NodeKind::Mapping => {
            let mut out = BTreeMap::new();
            for pair in node.content.chunks(2) {
                if pair.len() != 2 {
                    return Err(CoreError::Parse(ParseError::InvalidSyntax));
                }
                out.insert(pair[0].value.clone(), tree_to_value(&pair[1])?);
            }
            Ok(Value::Object(out))
        }
        NodeKind::Scalar => match node.resolved_sem_type() {
            Some(SemType::Nil) => Ok(Value::Null),
            Some(SemType::Boolean) => Ok(Value::Bool(matches!(
                node.value.to_ascii_lowercase().as_str(),
                "y" | "yes" | "on" | "true"
            ))),
            Some(SemType::Int) => node
                .value
                .parse::<i64>()
                .map(|value| Value::Number(value as f64))
                .map_err(|_| CoreError::Eval(EvalError::CannotConvertNodeToNumber)),
            Some(SemType::Float) => node
                .value
                .parse::<f64>()
                .map(Value::Number)
                .map_err(|_| CoreError::Eval(EvalError::CannotConvertNodeToNumber)),
            _ => Ok(Value::String(node.value.clone())),
        },
        NodeKind::Alias => Err(CoreError::Eval(EvalError::UnsupportedFlat)),
        NodeKind::Unknown => Err(CoreError::Eval(EvalError::UnsupportedFlat)),
    }
}

fn value_to_tree(value: &Value) -> Result<TreeNode, CoreError> {
    match value {
        Value::Null => Ok(TreeNode::default()),
        Value::Bool(value) => {
            let mut node = TreeNode::default();
            node.kind = NodeKind::Scalar;
            node.sem_type = Some(SemType::Boolean);
            node.tag = SemType::Boolean.to_string().into();
            node.value = value.to_string();
            Ok(node)
        }
        Value::Number(value) => {
            let mut node = TreeNode::default();
            node.kind = NodeKind::Scalar;
            if value.fract() == 0.0 {
                node.sem_type = Some(SemType::Int);
                node.tag = SemType::Int.to_string().into();
                node.value = (*value as i64).to_string();
            } else {
                node.sem_type = Some(SemType::Float);
                node.tag = SemType::Float.to_string().into();
                node.value = float_to_string(*value);
            }
            Ok(node)
        }
        Value::String(value) => {
            let mut node = TreeNode::default();
            node.kind = NodeKind::Scalar;
            node.sem_type = Some(SemType::Str);
            node.tag = SemType::Str.to_string().into();
            node.value = value.clone();
            Ok(node)
        }
        Value::Array(values) => {
            let mut node = TreeNode::default();
            node.kind = NodeKind::Sequence;
            node.sem_type = Some(SemType::Seq);
            node.tag = SemType::Seq.to_string().into();
            for value in values {
                node.add_child(&value_to_tree(value)?)?;
            }
            Ok(node)
        }
        Value::Object(values) => {
            let mut node = TreeNode::default();
            node.kind = NodeKind::Mapping;
            node.sem_type = Some(SemType::Map);
            node.tag = SemType::Map.to_string().into();
            for (key, value) in values {
                let key_node = create_string_scalar_node(key)?;
                let value_node = value_to_tree(value)?;
                node.add_key_value_child(&key_node, &value_node)?;
            }
            Ok(node)
        }
    }
}

fn map_pipeline_error(error: expression_pipeline::PipelineError) -> CoreError {
    match error {
        expression_pipeline::PipelineError::Parse(_) => CoreError::Parse(ParseError::InvalidSyntax),
        expression_pipeline::PipelineError::Evaluate(eval) => match eval {
            EvaluationError::Core(_) => CoreError::Eval(EvalError::UnsupportedFlat),
            EvaluationError::DivisionByZero => CoreError::Eval(EvalError::CannotDivideTypes),
            EvaluationError::MissingOperand(_) | EvaluationError::TypeMismatch { .. } => {
                CoreError::Eval(EvalError::UnsupportedFlat)
            }
            EvaluationError::UnsupportedOperation(_) => CoreError::Eval(EvalError::UnsupportedFlat),
        },
        expression_pipeline::PipelineError::Compat(compat) => compat,
    }
}

/// Interpolate expressions embedded in a string as \(...) patterns.
fn interpolate(d: &mut TreeEngine, ctx: &Context, input: &str) -> Result<String, CoreError> {
    if !ctx.string_interpolation_enabled {
        return Ok(input.to_string());
    }

    let mut out = String::new();
    let mut exp = String::new();
    let mut in_expr = false;
    let mut nested: i32 = 0;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if !in_expr {
            if ch == '\\' && i + 1 < chars.len() {
                let next = chars[i + 1];
                if next == '(' {
                    in_expr = true;
                    i += 2;
                    continue;
                }
                if next == '\\' {
                    out.push('\\');
                    i += 2;
                    continue;
                }
            }
            out.push(ch);
            i += 1;
            continue;
        }

        // Inside expression
        if ch == ')' {
            if nested == 0 {
                let value = evaluate_to_string(d, ctx, &exp)?;
                out.push_str(&value);
                exp.clear();
                in_expr = false;
                i += 1;
                continue;
            }
            nested -= 1;
        } else if ch == '(' {
            nested += 1;
        } else if ch == '\\' && i + 1 < chars.len() {
            let esc = chars[i + 1];
            if esc == ')' || esc == '\\' {
                exp.push(esc);
                i += 2;
                continue;
            }
        }
        exp.push(ch);
        i += 1;
    }

    if in_expr {
        return Ok(input.to_string());
    }

    Ok(out)
}

/// string_interpolation operator: process \(...) patterns within strings.
pub fn string_interpolation_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let s = expression_node.operation.string_value.clone();

    if !ctx.string_interpolation_enabled {
        let node = create_string_scalar_node(&s)?;
        return ctx.single_child_context(&node);
    }

    if ctx.matching_nodes.is_empty() {
        let value = interpolate(d, &ctx, &s)?;
        let node = create_string_scalar_node(&value)?;
        return ctx.single_child_context(&node);
    }

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let inner_ctx = ctx.single_child_context(candidate)?;
        let value = interpolate(d, &inner_ctx, &s)?;
        let node = create_string_scalar_node(&value)?;
        results.push((*node).clone());
    }
    ctx.child_context(results)
}

// ── Substitute operator ────────────────────────────────────────────

/// Expand $0 in replacement strings to the full match.
fn replacement_expand(
    replacement: &str,
    source: &str,
    match_start: usize,
    match_end: usize,
) -> Result<String, CoreError> {
    let mut out = String::new();
    let chars: Vec<char> = replacement.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch != '$' || i + 1 >= chars.len() {
            out.push(ch);
            i += 1;
            continue;
        }
        let next = chars[i + 1];
        if next == '$' {
            out.push('$');
            i += 2;
            continue;
        }
        if next == '0' {
            out.push_str(&source[match_start..match_end]);
            i += 2;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    Ok(out)
}

/// Replace all regex matches in input with replacement string.
fn regex_replace_all(cr: &RegexStub, input: &str, replacement: &str) -> Result<String, CoreError> {
    let mut out = String::new();
    let mut cursor = 0;

    while cursor <= input.len() {
        let Some(m) = cr.find_at(input, cursor) else {
            break;
        };

        out.push_str(&input[cursor..m.start]);
        let repl = replacement_expand(replacement, input, m.start, m.end)?;
        out.push_str(&repl);

        cursor = next_search_start(input, m.start, m.end);
    }

    if cursor <= input.len() {
        out.push_str(&input[cursor..]);
    }

    Ok(out)
}

/// Extract regex and replacement parameters from a block expression.
fn substitute_parameters(
    d: &mut TreeEngine,
    ctx: &Context,
    block: &mut ExpressionNode,
) -> Result<(String, String), CoreError> {
    let mut regex = String::new();
    let mut replacement = String::new();

    let ro = ctx.read_only_clone()?;
    let lhs_nodes = get_matching_nodes(d, &ro, block.lhs.as_deref_mut())?;
    if !lhs_nodes.matching_nodes.is_empty() {
        regex = lhs_nodes.matching_nodes[0].value.clone();
    }

    let rhs_nodes = get_matching_nodes(d, ctx, block.rhs.as_deref_mut())?;
    if !rhs_nodes.matching_nodes.is_empty() {
        replacement = rhs_nodes.matching_nodes[0].value.clone();
    }

    Ok((regex, replacement))
}

/// sub operator: regex substitution within strings.
pub fn substitute_string_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let block = match expression_node.rhs.as_mut() {
        Some(b) => b,
        None => {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef("eval", "sub() missing block")?;
            }
            return Err(CoreError::Eval(EvalError::MissingRhs));
        }
    };

    let (regex_str, replacement_str) = substitute_parameters(d, &ctx, block)?;
    let cr = RegexStub::new(&regex_str)?;

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let inferred = SemType::from_string(&candidate.guess_tag_from_custom_type());
        if inferred != Some(SemType::Str) {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!("cannot substitute with {}, can only substitute strings. Hint: Most often you'll want to use '|=' over '=' for this operation", candidate.tag),
                )?;
            }
            return Err(CoreError::OperatorMessage {
                op: "sub".to_string(),
                message: "can only substitute strings".to_string(),
            });
        }
        let replaced = regex_replace_all(&cr, &candidate.value, &replacement_str)?;
        let result =
            candidate.create_replacement(NodeKind::Scalar, SemType::Str.to_string(), &replaced)?;
        results.push((*result).clone());
    }
    ctx.child_context(results)
}

// ── Split operator ─────────────────────────────────────────────────

/// split operator: split strings by a delimiter into a sequence.
pub fn split_string_operator(
    ctx: Context,
    d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let split_str = rhs_string(d, &ctx, expression_node.rhs.as_deref_mut())?;

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        if candidate.sem_type == Some(SemType::Nil) {
            continue;
        }
        let inferred = SemType::from_string(&candidate.guess_tag_from_custom_type());
        if inferred != Some(SemType::Str) {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!("cannot split {}, can only split strings", candidate.tag),
                )?;
            }
            return Err(CoreError::OperatorMessage {
                op: "split".to_string(),
                message: "can only split strings".to_string(),
            });
        }

        let mut repl =
            candidate.create_replacement(NodeKind::Sequence, SemType::Seq.to_string(), "")?;

        if !candidate.value.is_empty() {
            for part in candidate.value.split(&split_str) {
                let child = create_string_scalar_node(part)?;
                repl.add_child(&child)?;
            }
        }
        results.push((*repl).clone());
    }
    ctx.child_context(results)
}

// ── Trim operator ──────────────────────────────────────────────────

/// trim operator: remove leading/trailing whitespace.
pub fn trim_space_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let inferred = SemType::from_string(&candidate.guess_tag_from_custom_type());
        if inferred != Some(SemType::Str) {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!(
                        "cannot trim {}, can only operate on strings.",
                        candidate.tag
                    ),
                )?;
            }
            return Err(CoreError::OperatorMessage {
                op: "trim".to_string(),
                message: "can only trim strings".to_string(),
            });
        }

        let trimmed = candidate
            .value
            .trim_matches(|c: char| " \t\r\n".contains(c));
        let repl = candidate.create_replacement(NodeKind::Scalar, &candidate.tag, trimmed)?;
        results.push((*repl).clone());
    }
    ctx.child_context(results)
}

// ── To string operator ─────────────────────────────────────────────

/// to_string operator: convert nodes to their string representation.
pub fn to_string_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    _expr: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let mut value = candidate.value.clone();
        if candidate.sem_type != Some(SemType::Str) && candidate.kind != NodeKind::Scalar {
            value = candidate.tag.clone();
        }
        let repl =
            candidate.create_replacement(NodeKind::Scalar, SemType::Str.to_string(), &value)?;
        results.push((*repl).clone());
    }
    ctx.child_context(results)
}

// ── Change case operator ───────────────────────────────────────────

/// change_case operator: convert strings to uppercase or lowercase.
pub fn change_case_operator(
    ctx: Context,
    _d: &mut TreeEngine,
    expression_node: &mut ExpressionNode,
) -> Result<Context, CoreError> {
    let to_upper_case = if let Some(ref prefs) = expression_node.operation.preferences {
        match prefs.as_ref() {
            OperationPreference::ChangeCase(p) => p.to_upper_case,
            _ => false,
        }
    } else {
        false
    };

    let mut results = Vec::new();
    for candidate in &ctx.matching_nodes {
        let inferred = SemType::from_string(&candidate.guess_tag_from_custom_type());
        if inferred != Some(SemType::Str) {
            if let Some(ref d2) = ctx.diagnostics {
                d2.set_messagef(
                    "eval",
                    &format!(
                        "cannot change case with {}, can only operate on strings.",
                        candidate.tag
                    ),
                )?;
            }
            return Err(CoreError::OperatorMessage {
                op: "change_case".to_string(),
                message: "can only change case of strings".to_string(),
            });
        }

        let out = if to_upper_case {
            candidate.value.to_uppercase()
        } else {
            candidate.value.to_lowercase()
        };
        let repl = candidate.create_replacement(NodeKind::Scalar, &candidate.tag, &out)?;
        results.push((*repl).clone());
    }
    ctx.child_context(results)
}
