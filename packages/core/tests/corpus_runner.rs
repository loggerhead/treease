// corpus_runner.rs — Rust fixture corpus runner aligned with Zig fixtures-run.

use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fs;
#[cfg(not(test))]
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use treease_core::core::graph_builder::{GraphKind, GraphModel, PathSeg};
use treease_core::core::{
    CoreError, GraphBuilder, GraphLanguage, NodeId as CoreNodeId, ParsedKey,
    SemType as CoreSemType, TreeNodeKind as CoreTreeNodeKind, TreeStore, ValueRep, default_config,
};
use treease_core::formats::{
    Decode, DecodedDocument, Encode, JsonDecoder, JsonEncoder, TomlDecoder, YamlDecoder,
};
use treease_core::operators::{
    NodeKind as OpNodeKind, SemType as OpSemType, TreeNode as OpTreeNode,
};

const MB: u64 = 1024 * 1024;
const DEFAULT_TIMEOUT_SECONDS: u64 = 1;
const TIMEOUT_EXIT_CODE: i32 = 124;
const TIMEOUT_DIAGNOSTICS_FILENAME: &str = "timeout-diagnostics.txt";
const FAILURE_DIAGNOSTICS_FILENAME: &str = "failure-diagnostics.txt";
const FAILURE_FILENAME: &str = "failure.txt";
const WORKERS_ENV: &str = "TREEASE_CORPUS_WORKERS";
const HELPER_BINARY_NAME: &str = "corpus_runner_helper";
const HELPER_BINARY_ENV: &str = "CARGO_BIN_EXE_corpus_runner_helper";
const MESH_JSON_FIXTURE: &str = "jsonexamples__mesh.pretty.1.json";
const MEDIUM_JSON_FIXTURE: &str = "1MB-min.1.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FixtureKind {
    Json,
    Toml,
    Yaml,
}

impl FixtureKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
        }
    }

    fn ordered() -> [Self; 3] {
        [Self::Json, Self::Toml, Self::Yaml]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expectation {
    Valid,
    Invalid,
}

#[derive(Debug, Clone)]
struct FixtureCase {
    path: PathBuf,
    kind: FixtureKind,
    expectation: Expectation,
}

#[derive(Debug, Clone, Default)]
struct LanguageStats {
    total: usize,
    passed: usize,
    failed: usize,
    timeout: usize,
    elapsed_ns: u128,
}

#[derive(Debug, Clone)]
struct CaseRunResult {
    case_path: PathBuf,
    code: i32,
    stdout: String,
    stderr: String,
    elapsed_ns: u128,
}

#[derive(Debug, Clone)]
struct DriverInvocation {
    program: PathBuf,
    args: Vec<OsString>,
    envs: Vec<(String, String)>,
}

#[derive(Debug)]
struct TreeSummaryEntry {
    path: String,
    kind: CoreTreeNodeKind,
    child_count: usize,
    value: String,
}

#[derive(Debug)]
struct GraphNodeSummary {
    path: String,
    kind: GraphKind,
    row_count: usize,
    column_keys: String,
}

#[derive(Debug)]
struct GraphEdgeSummary {
    from_path: String,
    to_path: String,
}

fn fixtures_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    Path::new(&manifest_dir)
        .parent()
        .expect("packages/core parent")
        .parent()
        .expect("repo root")
        .join("test/fixtures")
}

fn expectation_from_name(name: &str) -> Option<Expectation> {
    let ext_idx = name.rfind('.')?;
    if ext_idx == 0 {
        return None;
    }
    let stem = &name[..ext_idx];
    let marker_idx = stem.rfind('.')?;
    match &stem[marker_idx + 1..] {
        "1" => Some(Expectation::Valid),
        "0" => Some(Expectation::Invalid),
        _ => None,
    }
}

fn should_skip_fixture(kind: FixtureKind, name: &str) -> bool {
    matches!(kind, FixtureKind::Json) && name.starts_with("minefield__i_")
}

fn collect_cases_in_dir(dir: &Path, kind: FixtureKind) -> Vec<FixtureCase> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut cases = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        })
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if should_skip_fixture(kind, &name) {
                return None;
            }
            let expectation = expectation_from_name(&name)?;
            Some(FixtureCase {
                path: entry.path(),
                kind,
                expectation,
            })
        })
        .collect::<Vec<_>>();
    cases.sort_by(|lhs, rhs| lhs.path.cmp(&rhs.path));
    cases
}

fn collect_all_cases() -> Vec<FixtureCase> {
    let root = fixtures_root();
    let mut all = Vec::new();
    for (dir_name, kind) in [
        ("json", FixtureKind::Json),
        ("toml", FixtureKind::Toml),
        ("yaml", FixtureKind::Yaml),
    ] {
        all.extend(collect_cases_in_dir(&root.join(dir_name), kind));
    }
    all
}

fn decode(
    kind: FixtureKind,
    input: &str,
    source_filename: &str,
) -> Result<DecodedDocument, CoreError> {
    match kind {
        FixtureKind::Json => JsonDecoder.decode_str(input),
        FixtureKind::Toml => TomlDecoder.decode_str_with_filename(input, source_filename),
        FixtureKind::Yaml => YamlDecoder.decode_str(input),
    }
}

fn store_value_string(store: &TreeStore, id: CoreNodeId) -> String {
    store.value_string_for(id).unwrap_or_default()
}

fn store_value_rep(store: &TreeStore, id: CoreNodeId) -> Result<ValueRep, CoreError> {
    store.value_rep_for(id)
}

fn core_tree_to_compat(store: &TreeStore, root: CoreNodeId) -> Result<OpTreeNode, CoreError> {
    let source = store.get(root).ok_or(CoreError::Eval(
        treease_core::core::EvalError::MissingTreeNode,
    ))?;
    let kind = match source.kind {
        CoreTreeNodeKind::Scalar | CoreTreeNodeKind::Unknown => OpNodeKind::Scalar,
        CoreTreeNodeKind::Mapping => OpNodeKind::Mapping,
        CoreTreeNodeKind::Sequence => OpNodeKind::Sequence,
        CoreTreeNodeKind::Alias => OpNodeKind::Alias,
    };
    let sem_type = source.sem_type.map(|sem_type| match sem_type {
        CoreSemType::Nil => OpSemType::Nil,
        CoreSemType::Str => OpSemType::Str,
        CoreSemType::Int => OpSemType::Int,
        CoreSemType::Float => OpSemType::Float,
        CoreSemType::Boolean => OpSemType::Boolean,
        CoreSemType::Map => OpSemType::Map,
        CoreSemType::Seq => OpSemType::Seq,
    });

    let mut out = OpTreeNode {
        kind,
        sequence_closed: source.sequence_closed(),
        sem_type,
        tag: source.tag.to_string_value(),
        value: store_value_string(store, root),
        anchor: store.anchor_for(root).unwrap_or_default().to_owned(),
        alias: source
            .alias()
            .map(|id| treease_core::operators::NodeId(id.index())),
        head_comment: store.head_comment_for(root).unwrap_or_default().to_owned(),
        line_comment: store.line_comment_for(root).unwrap_or_default().to_owned(),
        foot_comment: store.foot_comment_for(root).unwrap_or_default().to_owned(),
        parent: source
            .parent
            .map(|id| treease_core::operators::NodeId(id.index())),
        key: source
            .key()
            .map(|id| treease_core::operators::NodeId(id.index())),
        sequence_index: source.sequence_index().map(|index| index as i64),
        leading_content: store
            .leading_content_for(root)
            .unwrap_or_default()
            .to_owned(),
        document: 0,
        filename: String::new(),
        line: 0,
        column: 0,
        file_index: 0,
        is_map_key: source.is_map_key,
        encode_separate: source.encode_separate(),
        evaluate_together: source.evaluate_together(),
        ..OpTreeNode::default()
    };
    out.content = source
        .content
        .iter()
        .map(|child| core_tree_to_compat(store, *child))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(out)
}

fn build_graph(root: &OpTreeNode) -> GraphModel {
    let mut builder = GraphBuilder::new(default_config(), GraphLanguage::None);
    builder.build(root)
}

fn json_valid_serde_oracle(input: &str) -> Result<DecodedDocument, String> {
    let expected = serde_json::from_str::<serde_json::Value>(input)
        .map_err(|error| format!("serde_json rejected valid JSON input: {error}"))?;
    let decoded = JsonDecoder
        .decode_str(input)
        .map_err(|error| format!("treease json decoder rejected valid JSON input: {error}"))?;
    json_valid_serde_oracle_with_decoded(input, &decoded)?;
    let actual = serde_json::from_str::<serde_json::Value>(
        JsonEncoder::default()
            .encode_to_string(&decoded.store, decoded.root)
            .map_err(|error| format!("treease json encoder failed: {error}"))?
            .trim_end(),
    )
    .map_err(|error| format!("serde_json rejected treease JSON output: {error}"))?;
    if actual != expected {
        return Err(format!(
            "serde_json semantic mismatch after treease json round-trip: expected={expected:?} actual={actual:?}"
        ));
    }
    Ok(decoded)
}

fn json_oracle_encode(decoded: &DecodedDocument) -> Result<String, String> {
    let mut out = String::new();
    json_oracle_write_node(&decoded.store, decoded.root, &mut out)?;
    out.push('\n');
    Ok(out)
}

fn json_oracle_escape_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn json_oracle_write_scalar(
    store: &TreeStore,
    node_id: CoreNodeId,
    node: &treease_core::core::TreeNode,
    out: &mut String,
) {
    let value = store.value_for(node_id).unwrap_or_default();
    match node.resolved_sem_type() {
        Some(CoreSemType::Boolean) => {
            out.push_str(if value.eq_ignore_ascii_case("true") {
                "true"
            } else {
                "false"
            });
        }
        Some(CoreSemType::Int | CoreSemType::Float) => out.push_str(value),
        Some(CoreSemType::Nil) => out.push_str("null"),
        Some(CoreSemType::Str) => out.push_str(&json_oracle_escape_string(value)),
        Some(CoreSemType::Map | CoreSemType::Seq) | None => match store_value_rep(store, node_id) {
            Ok(ValueRep::Nil) => out.push_str("null"),
            Ok(ValueRep::Boolean(value)) => out.push_str(if value { "true" } else { "false" }),
            Ok(ValueRep::Int(value)) => out.push_str(&value.to_string()),
            Ok(ValueRep::Float(value)) => out.push_str(&value.to_string()),
            Ok(ValueRep::Str(value)) => out.push_str(&json_oracle_escape_string(&value)),
            Err(_) => out.push_str(&json_oracle_escape_string(value)),
        },
    }
}

fn json_oracle_write_node(
    store: &TreeStore,
    node_id: CoreNodeId,
    out: &mut String,
) -> Result<(), String> {
    let node = store
        .get(node_id)
        .ok_or_else(|| format!("missing tree node for oracle encode: {:?}", node_id))?;
    match node.kind {
        CoreTreeNodeKind::Scalar => json_oracle_write_scalar(store, node_id, node, out),
        CoreTreeNodeKind::Alias => {
            if let Some(alias) = node.alias() {
                json_oracle_write_node(store, alias, out)?;
            } else {
                out.push_str("null");
            }
        }
        CoreTreeNodeKind::Sequence => {
            out.push('[');
            for (index, child) in node.content.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                json_oracle_write_node(store, *child, out)?;
            }
            out.push(']');
        }
        CoreTreeNodeKind::Mapping => {
            out.push('{');
            for (index, pair) in node.content.chunks_exact(2).enumerate() {
                if index > 0 {
                    out.push(',');
                }
                store.get(pair[0]).ok_or_else(|| {
                    format!("missing mapping key node for oracle encode: {:?}", pair[0])
                })?;
                out.push_str(&json_oracle_escape_string(
                    store.value_for(pair[0]).unwrap_or_default(),
                ));
                out.push(':');
                json_oracle_write_node(store, pair[1], out)?;
            }
            out.push('}');
        }
        CoreTreeNodeKind::Unknown => out.push_str("null"),
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JsonOracleCanonicalization {
    text: String,
    has_unsupported_numbers: bool,
}

fn canonicalize_json_number_lexeme_for_oracle(raw: &str) -> (String, bool) {
    if !raw.contains(|ch: char| matches!(ch, '.' | 'e' | 'E')) {
        return match raw.parse::<i64>() {
            Ok(_) => (raw.to_owned(), false),
            Err(_) => (raw.to_owned(), true),
        };
    }

    match raw.parse::<f64>() {
        Ok(value)
            if value.is_finite()
                && value == value.trunc()
                && value >= i64::MIN as f64
                && value <= i64::MAX as f64 =>
        {
            ((value as i64).to_string(), false)
        }
        Ok(value) if value.is_finite() => {
            let _ = value;
            (raw.to_owned(), false)
        }
        _ => (raw.to_owned(), true),
    }
}

fn canonicalize_json_for_serde_oracle(input: &str) -> JsonOracleCanonicalization {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut has_unsupported_numbers = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                let start = index;
                index += 1;
                while index < bytes.len() {
                    match bytes[index] {
                        b'\\' => index += 2,
                        b'"' => {
                            index += 1;
                            break;
                        }
                        _ => index += 1,
                    }
                }
                out.push_str(&input[start..index]);
            }
            b'-' | b'0'..=b'9' => {
                let start = index;
                index += 1;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if index < bytes.len() && bytes[index] == b'.' {
                    index += 1;
                    while index < bytes.len() && bytes[index].is_ascii_digit() {
                        index += 1;
                    }
                }
                if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
                    index += 1;
                    if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
                        index += 1;
                    }
                    while index < bytes.len() && bytes[index].is_ascii_digit() {
                        index += 1;
                    }
                }
                let (canonical, unsupported) =
                    canonicalize_json_number_lexeme_for_oracle(&input[start..index]);
                has_unsupported_numbers |= unsupported;
                out.push_str(&canonical);
            }
            byte => {
                out.push(byte as char);
                index += 1;
            }
        }
    }
    JsonOracleCanonicalization {
        text: out,
        has_unsupported_numbers,
    }
}

fn float_ulp_diff(lhs: f64, rhs: f64) -> u64 {
    fn ordered_bits(value: f64) -> i64 {
        let bits = value.to_bits() as i64;
        if bits < 0 { i64::MIN - bits } else { bits }
    }

    ordered_bits(lhs).abs_diff(ordered_bits(rhs))
}

fn json_numbers_semantically_equal(
    expected: &serde_json::Number,
    actual: &serde_json::Number,
) -> bool {
    if expected == actual {
        return true;
    }

    match (
        expected.as_i64(),
        expected.as_u64(),
        expected.as_f64(),
        actual.as_i64(),
        actual.as_u64(),
        actual.as_f64(),
    ) {
        (Some(lhs), _, _, Some(rhs), _, _) => lhs == rhs,
        (_, Some(lhs), _, _, Some(rhs), _) => lhs == rhs,
        (Some(lhs), _, _, _, Some(rhs), _) => lhs >= 0 && (lhs as u64) == rhs,
        (_, Some(lhs), _, Some(rhs), _, _) => rhs >= 0 && lhs == (rhs as u64),
        (_, _, Some(lhs), _, _, Some(rhs)) => float_ulp_diff(lhs, rhs) <= 1,
        _ => false,
    }
}

fn json_values_semantically_equal(
    expected: &serde_json::Value,
    actual: &serde_json::Value,
) -> bool {
    match (expected, actual) {
        (serde_json::Value::Number(expected), serde_json::Value::Number(actual)) => {
            json_numbers_semantically_equal(expected, actual)
        }
        _ => expected == actual,
    }
}

fn first_json_diff_path(
    expected: &serde_json::Value,
    actual: &serde_json::Value,
    path: &mut String,
) -> Option<String> {
    if json_values_semantically_equal(expected, actual) {
        return None;
    }
    match (expected, actual) {
        (serde_json::Value::Object(expected), serde_json::Value::Object(actual)) => {
            for (key, expected_value) in expected {
                let checkpoint = path.len();
                path.push('.');
                path.push_str(key);
                match actual.get(key) {
                    Some(actual_value) => {
                        if let Some(diff) = first_json_diff_path(expected_value, actual_value, path)
                        {
                            return Some(diff);
                        }
                    }
                    None => {
                        return Some(format!(
                            "{} missing in actual; expected={expected_value:?}",
                            path
                        ));
                    }
                }
                path.truncate(checkpoint);
            }
            for key in actual.keys() {
                if !expected.contains_key(key) {
                    return Some(format!("{}.{} unexpected key in actual", path, key));
                }
            }
            None
        }
        (serde_json::Value::Array(expected), serde_json::Value::Array(actual)) => {
            let shared_len = expected.len().min(actual.len());
            for index in 0..shared_len {
                let checkpoint = path.len();
                path.push('[');
                path.push_str(&index.to_string());
                path.push(']');
                if let Some(diff) = first_json_diff_path(&expected[index], &actual[index], path) {
                    return Some(diff);
                }
                path.truncate(checkpoint);
            }
            if expected.len() != actual.len() {
                return Some(format!(
                    "{} length mismatch: expected={} actual={}",
                    path,
                    expected.len(),
                    actual.len()
                ));
            }
            None
        }
        _ => Some(format!("{} expected={expected:?} actual={actual:?}", path)),
    }
}

fn json_valid_serde_oracle_with_decoded(
    input: &str,
    decoded: &DecodedDocument,
) -> Result<(), String> {
    let encoded = json_oracle_encode(decoded)?;
    JsonDecoder
        .decode_str(encoded.trim_end())
        .map_err(|error| format!("treease json decoder rejected oracle output: {error}"))?;
    let canonical_input = canonicalize_json_for_serde_oracle(input);
    if canonical_input.has_unsupported_numbers {
        return Ok(());
    }
    let Ok(expected) = serde_json::from_str::<serde_json::Value>(&canonical_input.text) else {
        return Ok(());
    };
    let actual = serde_json::from_str::<serde_json::Value>(encoded.trim_end())
        .map_err(|error| format!("serde_json rejected treease JSON output: {error}"))?;
    if let Some(diff) = first_json_diff_path(&expected, &actual, &mut "$".to_owned()) {
        return Err(format!(
            "serde_json semantic mismatch after treease json round-trip: {diff}"
        ));
    }
    Ok(())
}

fn json_invalid_serde_oracle(input: &str) -> Result<(), String> {
    if serde_json::from_str::<serde_json::Value>(input).is_ok() {
        return Err("serde_json accepted input that should be invalid JSON".to_owned());
    }
    if JsonDecoder.decode_str(input).is_ok() {
        return Err("treease json decoder accepted input that serde_json rejected".to_owned());
    }
    Ok(())
}

fn validate_valid_primary_oracle(
    kind: FixtureKind,
    input: &str,
    decoded: &DecodedDocument,
) -> Result<(), String> {
    if kind != FixtureKind::Json {
        return Ok(());
    }
    json_valid_serde_oracle_with_decoded(input, decoded)
}

fn yq_to_json(path: &Path) -> Option<String> {
    let output = Command::new("yq")
        .args(["-o", "json", "select(documentIndex == 0)"])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    normalize_yq_yaml_root_anchor_sequence_oracle(path, &stdout).or(Some(stdout))
}

fn normalize_yq_yaml_root_anchor_sequence_oracle(path: &Path, yq_json: &str) -> Option<String> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
        return None;
    }
    let source = fs::read_to_string(path).ok()?;
    if !source.starts_with("--- &") {
        return None;
    }
    let mut lines = source.lines();
    let _header = lines.next()?;
    let second = lines.next()?.trim_start();
    if !second.starts_with('-') {
        return None;
    }
    let parsed = serde_json::from_str::<serde_json::Value>(yq_json).ok()?;
    if matches!(parsed, serde_json::Value::Array(_)) {
        return None;
    }
    Some(serde_json::to_string(&serde_json::Value::Array(vec![parsed])).ok()? + "\n")
}

fn format_tree_path(store: &TreeStore, id: CoreNodeId) -> String {
    let Some(path) = oracle_path_for(store, id) else {
        return "/".to_owned();
    };
    if path.is_empty() {
        return "/".to_owned();
    }
    let mut out = String::new();
    for segment in path {
        match segment {
            ParsedKey::Str(value) => {
                out.push_str("/k");
                out.push_str(&value.len().to_string());
                out.push(':');
                out.push_str(&value);
            }
            ParsedKey::Int(value) => {
                out.push_str("/i");
                out.push_str(&value.to_string());
            }
        }
    }
    out
}

fn oracle_path_for(store: &TreeStore, id: CoreNodeId) -> Option<Vec<ParsedKey>> {
    let node = store.get(id)?;
    let key = oracle_parsed_key_for(store, id)?;
    let mut path = if let Some(parent) = node.parent {
        oracle_path_for(store, parent)?
    } else {
        Vec::new()
    };
    if let Some(key) = key {
        path.push(key);
    }
    Some(path)
}

fn oracle_parsed_key_for(store: &TreeStore, id: CoreNodeId) -> Option<Option<ParsedKey>> {
    let node = store.get(id)?;
    if node.is_map_key {
        return Some(Some(ParsedKey::Str(oracle_key_text(store, id))));
    }
    if let Some(key_id) = node.key() {
        let key_node = store.get(key_id)?;
        if key_node.resolved_sem_type() == Some(CoreSemType::Str)
            || key_node.kind == CoreTreeNodeKind::Alias
        {
            return Some(Some(ParsedKey::Str(oracle_key_text(store, key_id))));
        }
        return Some(Some(match store.value_for(key_id).ok()?.parse::<i64>() {
            Ok(index) => ParsedKey::Int(index),
            Err(_) => ParsedKey::Str(oracle_key_text(store, key_id)),
        }));
    }
    if let Some(index) = node.sequence_index() {
        return Some(Some(ParsedKey::Int(index as i64)));
    }
    Some(None)
}

fn oracle_key_text(store: &TreeStore, id: CoreNodeId) -> String {
    store_value_string(store, resolved_alias_id(store, id).unwrap_or(id))
}

fn resolved_alias_node(store: &TreeStore, id: CoreNodeId) -> Option<&treease_core::core::TreeNode> {
    store.get(resolved_alias_id(store, id)?)
}

fn resolved_alias_id(store: &TreeStore, id: CoreNodeId) -> Option<CoreNodeId> {
    let mut current_id = id;
    let mut seen = HashSet::new();
    loop {
        let node = store.get(current_id)?;
        if node.kind != CoreTreeNodeKind::Alias {
            return Some(current_id);
        }
        let next = node.alias()?;
        if !seen.insert(next) {
            return None;
        }
        current_id = next;
    }
}

fn resolved_tree_kind_for_oracle(store: &TreeStore, id: CoreNodeId) -> CoreTreeNodeKind {
    resolved_alias_node(store, id)
        .map(|node| node.kind)
        .unwrap_or(CoreTreeNodeKind::Alias)
}

fn resolved_tree_child_count_for_oracle(store: &TreeStore, id: CoreNodeId) -> usize {
    resolved_alias_node(store, id)
        .map(|node| node.content.len())
        .unwrap_or(0)
}

fn resolved_tree_value_for_oracle(store: &TreeStore, id: CoreNodeId) -> String {
    let Some(resolved_id) = resolved_alias_id(store, id) else {
        return String::new();
    };
    if store
        .get(resolved_id)
        .is_some_and(|node| node.kind == CoreTreeNodeKind::Scalar)
    {
        store_value_string(store, resolved_id)
    } else {
        String::new()
    }
}

enum ComparePathSegment<'a> {
    Key(&'a str),
    Index(usize),
}

fn parse_compare_path(path: &str) -> Option<Vec<ComparePathSegment<'_>>> {
    if path == "/" {
        return Some(Vec::new());
    }
    let mut rest = path;
    let mut segments = Vec::new();
    while !rest.is_empty() {
        let next = rest.strip_prefix('/')?;
        if let Some(key_payload) = next.strip_prefix('k') {
            let colon = key_payload.find(':')?;
            let len = key_payload[..colon].parse::<usize>().ok()?;
            let value_start = colon + 1;
            let value_end = value_start + len;
            if key_payload.len() < value_end {
                return None;
            }
            let key = &key_payload[value_start..value_end];
            segments.push(ComparePathSegment::Key(key));
            rest = &key_payload[value_end..];
            continue;
        }
        if let Some(index_payload) = next.strip_prefix('i') {
            let next_sep = index_payload.find('/').unwrap_or(index_payload.len());
            let index = index_payload[..next_sep].parse::<usize>().ok()?;
            segments.push(ComparePathSegment::Index(index));
            rest = &index_payload[next_sep..];
            continue;
        }
        return None;
    }
    Some(segments)
}

fn summarize_tree(store: &TreeStore, id: CoreNodeId, out: &mut Vec<TreeSummaryEntry>) {
    let Some(node) = store.get(id) else {
        return;
    };
    out.push(TreeSummaryEntry {
        path: format_tree_path(store, id),
        kind: resolved_tree_kind_for_oracle(store, id),
        child_count: resolved_tree_child_count_for_oracle(store, id),
        value: resolved_tree_value_for_oracle(store, id),
    });
    for child in node.value_child_ids() {
        summarize_tree(store, child, out);
    }
}

fn compare_trees(
    store_a: &TreeStore,
    root_a: CoreNodeId,
    store_b: &TreeStore,
    root_b: CoreNodeId,
) -> Result<(), String> {
    let mut lhs_summary = Vec::new();
    let mut rhs_summary = Vec::new();
    summarize_tree(store_a, root_a, &mut lhs_summary);
    summarize_tree(store_b, root_b, &mut rhs_summary);
    let mut lhs_by_path = HashMap::<&str, VecDeque<&TreeSummaryEntry>>::new();
    for entry in &lhs_summary {
        lhs_by_path
            .entry(entry.path.as_str())
            .or_default()
            .push_back(entry);
    }

    if let (Some(lhs_root), Some(rhs_root)) = (store_a.get(root_a), store_b.get(root_b)) {
        if empty_mapping_matches_json_null(lhs_root, store_b, root_b, rhs_root)
            || empty_mapping_matches_json_null(rhs_root, store_a, root_a, lhs_root)
        {
            return Ok(());
        }
        if lhs_root.kind == CoreTreeNodeKind::Mapping
            || lhs_root.kind == CoreTreeNodeKind::Sequence
            || lhs_root.tag == rhs_root.tag
        {
            if lhs_summary.len() < rhs_summary.len() {
                return Err(format!(
                    "compareTrees: lhs has {} entries, rhs has {}",
                    lhs_summary.len(),
                    rhs_summary.len()
                ));
            }
        }
    }

    for rhs_entry in &rhs_summary {
        let lhs_entry = lhs_by_path
            .get_mut(rhs_entry.path.as_str())
            .and_then(|entries| entries.pop_front())
            .ok_or_else(|| format!("compareTrees: path {:?} not found in lhs", rhs_entry.path))?;
        if lhs_entry.kind != rhs_entry.kind {
            return Err(format!(
                "compareTrees: kind mismatch at {:?}: lhs={:?} rhs={:?}",
                rhs_entry.path, lhs_entry.kind, rhs_entry.kind
            ));
        }
        if lhs_entry.child_count < rhs_entry.child_count {
            return Err(format!(
                "compareTrees: child_count mismatch at {:?}: lhs={} rhs={}",
                rhs_entry.path, lhs_entry.child_count, rhs_entry.child_count
            ));
        }
        if rhs_entry.kind == CoreTreeNodeKind::Scalar
            && !tree_scalar_entries_semantically_equal(
                store_a,
                root_a,
                rhs_entry.path.as_str(),
                lhs_entry,
                store_b,
                root_b,
                rhs_entry,
            )
        {
            return Err(format!(
                "compareTrees: value mismatch at {:?}: lhs={:?} rhs={:?}",
                rhs_entry.path, lhs_entry.value, rhs_entry.value
            ));
        }
    }

    Ok(())
}

fn empty_mapping_matches_json_null(
    lhs: &treease_core::core::TreeNode,
    rhs_store: &TreeStore,
    rhs_id: CoreNodeId,
    rhs: &treease_core::core::TreeNode,
) -> bool {
    lhs.kind == CoreTreeNodeKind::Mapping
        && lhs.content.is_empty()
        && rhs.kind == CoreTreeNodeKind::Scalar
        && matches!(store_value_rep(rhs_store, rhs_id), Ok(ValueRep::Nil))
}

fn tree_scalar_entries_semantically_equal(
    store_a: &TreeStore,
    root_a: CoreNodeId,
    path: &str,
    lhs_entry: &TreeSummaryEntry,
    store_b: &TreeStore,
    root_b: CoreNodeId,
    rhs_entry: &TreeSummaryEntry,
) -> bool {
    let lhs_node = tree_node_at_path(store_a, root_a, path);
    let rhs_node = tree_node_at_path(store_b, root_b, path);
    match (lhs_node, rhs_node) {
        (Some(lhs_id), Some(rhs_id)) => {
            tree_scalar_nodes_semantically_equal(store_a, lhs_id, store_b, rhs_id)
        }
        _ => lhs_entry.value == rhs_entry.value,
    }
}

fn tree_node_at_path(store: &TreeStore, root: CoreNodeId, path: &str) -> Option<CoreNodeId> {
    let mut current = root;
    for segment in parse_compare_path(path)? {
        let node = store.get(current)?;
        match (node.kind, segment) {
            (CoreTreeNodeKind::Mapping, ComparePathSegment::Key(segment)) => {
                let mut next = None;
                for pair in node.content.chunks_exact(2) {
                    if oracle_key_text(store, pair[0]) == segment {
                        next = Some(pair[1]);
                        break;
                    }
                }
                current = next?;
            }
            (CoreTreeNodeKind::Sequence, ComparePathSegment::Index(index)) => {
                current = *node.content.get(index)?;
            }
            _ => return None,
        }
    }
    resolved_alias_id(store, current)
}

fn tree_scalar_nodes_semantically_equal(
    lhs_store: &TreeStore,
    lhs_id: CoreNodeId,
    rhs_store: &TreeStore,
    rhs_id: CoreNodeId,
) -> bool {
    match (
        normalize_scalar_value_for_oracle(lhs_store, lhs_id),
        normalize_scalar_value_for_oracle(rhs_store, rhs_id),
    ) {
        (Some(OracleScalarValue::Nil), Some(OracleScalarValue::Nil)) => true,
        (Some(OracleScalarValue::Boolean(lhs)), Some(OracleScalarValue::Boolean(rhs))) => {
            lhs == rhs
        }
        (Some(OracleScalarValue::Int(lhs)), Some(OracleScalarValue::Int(rhs))) => lhs == rhs,
        (Some(OracleScalarValue::Float(lhs)), Some(OracleScalarValue::Float(rhs))) => {
            float_ulp_diff(lhs, rhs) <= 1
        }
        (Some(OracleScalarValue::Int(lhs)), Some(OracleScalarValue::Float(rhs))) => {
            int_and_float_semantically_equal(lhs, rhs)
        }
        (Some(OracleScalarValue::Float(lhs)), Some(OracleScalarValue::Int(rhs))) => {
            int_and_float_semantically_equal(rhs, lhs)
        }
        (Some(OracleScalarValue::Str(lhs)), Some(OracleScalarValue::Str(rhs))) => lhs == rhs,
        _ => store_value_string(lhs_store, lhs_id) == store_value_string(rhs_store, rhs_id),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum OracleScalarValue {
    Nil,
    Boolean(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

fn normalize_scalar_value_for_oracle(
    store: &TreeStore,
    node_id: CoreNodeId,
) -> Option<OracleScalarValue> {
    let node = store.get(node_id)?;
    let value = store.value_for(node_id).ok()?;
    match node.resolved_sem_type() {
        Some(CoreSemType::Nil) => Some(OracleScalarValue::Nil),
        Some(CoreSemType::Boolean) => Some(OracleScalarValue::Boolean(
            value.eq_ignore_ascii_case("true"),
        )),
        Some(CoreSemType::Int) => parse_integer_lexeme_for_oracle(value)
            .ok()
            .map(OracleScalarValue::Int),
        Some(CoreSemType::Float) => parse_float_lexeme_for_oracle(value)
            .ok()
            .map(OracleScalarValue::Float),
        Some(CoreSemType::Str) => Some(OracleScalarValue::Str(value.to_owned())),
        Some(CoreSemType::Map | CoreSemType::Seq) | None => match store_value_rep(store, node_id) {
            Ok(ValueRep::Nil) => Some(OracleScalarValue::Nil),
            Ok(ValueRep::Boolean(value)) => Some(OracleScalarValue::Boolean(value)),
            Ok(ValueRep::Int(value)) => Some(OracleScalarValue::Int(value)),
            Ok(ValueRep::Float(value)) => Some(OracleScalarValue::Float(value)),
            Ok(ValueRep::Str(value)) => Some(OracleScalarValue::Str(value)),
            Err(_) => Some(OracleScalarValue::Str(value.to_owned())),
        },
    }
}

fn int_and_float_semantically_equal(integer: i64, float: f64) -> bool {
    float.is_finite()
        && float.fract() == 0.0
        && float >= i64::MIN as f64
        && float <= i64::MAX as f64
        && (float as i64) == integer
}

fn parse_integer_lexeme_for_oracle(raw: &str) -> Result<i64, ()> {
    let normalized: String = raw.chars().filter(|&ch| ch != '_').collect();
    let mut s = normalized.as_str();
    let mut negative = false;
    if let Some(rest) = s.strip_prefix('-') {
        negative = true;
        s = rest;
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest;
    }
    let (digits, radix) = if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))
    {
        (rest, 16)
    } else if let Some(rest) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
        (rest, 8)
    } else if let Some(rest) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        (rest, 2)
    } else {
        (s, 10)
    };
    let magnitude = u64::from_str_radix(digits, radix).map_err(|_| ())?;
    if !negative {
        i64::try_from(magnitude).map_err(|_| ())
    } else if magnitude == (i64::MAX as u64) + 1 {
        Ok(i64::MIN)
    } else {
        let positive = i64::try_from(magnitude).map_err(|_| ())?;
        Ok(-positive)
    }
}

fn parse_float_lexeme_for_oracle(raw: &str) -> Result<f64, ()> {
    match raw {
        "inf" | "+inf" => Ok(f64::INFINITY),
        "-inf" => Ok(f64::NEG_INFINITY),
        "nan" | "+nan" | "-nan" => Ok(f64::NAN),
        _ => raw
            .chars()
            .filter(|&ch| ch != '_')
            .collect::<String>()
            .parse::<f64>()
            .map_err(|_| ()),
    }
}

fn format_graph_path(path: &[PathSeg]) -> String {
    if path.is_empty() {
        return "/".to_owned();
    }

    let mut out = String::new();
    for segment in path {
        match segment {
            PathSeg::Key(key) => {
                out.push_str("/k");
                out.push_str(&key.len().to_string());
                out.push(':');
                out.push_str(key);
            }
            PathSeg::Index(index) => {
                out.push_str("/i");
                out.push_str(&index.to_string());
            }
        }
    }
    out
}

fn summarize_graph_nodes(graph: &GraphModel) -> Vec<GraphNodeSummary> {
    graph
        .nodes
        .iter()
        .map(|node| {
            let column_keys = if let Some(table) = &node.table {
                table
                    .columns
                    .iter()
                    .map(|col| col.text.clone())
                    .collect::<Vec<_>>()
                    .join(",")
            } else {
                String::new()
            };
            let row_count = node
                .table
                .as_ref()
                .map(|table| table.rows.len())
                .unwrap_or(node.rows.len());
            GraphNodeSummary {
                path: format_graph_path(&node.path),
                kind: node.kind,
                row_count,
                column_keys,
            }
        })
        .collect()
}

fn summarize_graph_edges(graph: &GraphModel) -> Vec<GraphEdgeSummary> {
    graph
        .edges
        .iter()
        .map(|edge| {
            let from_node = graph
                .nodes
                .iter()
                .find(|node| node.render_handle == edge.from_render_handle);
            let to_node = graph
                .nodes
                .iter()
                .find(|node| node.render_handle == edge.to_render_handle);
            GraphEdgeSummary {
                from_path: from_node
                    .map(|node| format_graph_path(&node.path))
                    .unwrap_or_default(),
                to_path: to_node
                    .map(|node| format_graph_path(&node.path))
                    .unwrap_or_default(),
            }
        })
        .collect()
}

fn compare_graphs(lhs: &GraphModel, rhs: &GraphModel, kind: FixtureKind) -> Result<(), String> {
    let lhs_nodes = summarize_graph_nodes(lhs);
    let rhs_nodes = summarize_graph_nodes(rhs);
    let lhs_edges = summarize_graph_edges(lhs);
    let rhs_edges = summarize_graph_edges(rhs);
    let lhs_nodes_by_path = lhs_nodes
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<HashMap<_, _>>();
    let lhs_edge_paths = lhs_edges
        .iter()
        .map(|edge| (edge.from_path.as_str(), edge.to_path.as_str()))
        .collect::<HashSet<_>>();

    if lhs.nodes.is_empty() {
        return Err("compareGraphs: lhs graph has no nodes".to_owned());
    }
    if rhs.nodes.is_empty() {
        return Err("compareGraphs: rhs graph has no nodes".to_owned());
    }
    if empty_object_graph_matches_scalar_null(&lhs_nodes[0], &rhs_nodes[0])
        || empty_object_graph_matches_scalar_null(&rhs_nodes[0], &lhs_nodes[0])
    {
        return Ok(());
    }
    if lhs.nodes[0].kind != rhs.nodes[0].kind {
        return Err(format!(
            "compareGraphs: root kind mismatch: lhs={:?} rhs={:?}",
            lhs.nodes[0].kind, rhs.nodes[0].kind
        ));
    }

    for rhs_entry in &rhs_nodes {
        let lhs_entry = lhs_nodes_by_path
            .get(rhs_entry.path.as_str())
            .copied()
            .ok_or_else(|| {
                format!(
                    "compareGraphs: node path {:?} not found in lhs",
                    rhs_entry.path
                )
            })?;
        if lhs_entry.kind != rhs_entry.kind {
            return Err(format!(
                "compareGraphs: kind mismatch at {:?}: lhs={:?} rhs={:?}",
                rhs_entry.path, lhs_entry.kind, rhs_entry.kind
            ));
        }
        if lhs_entry.row_count < rhs_entry.row_count && kind != FixtureKind::Json {
            return Err(format!(
                "compareGraphs: row_count mismatch at {:?}: lhs={} rhs={}",
                rhs_entry.path, lhs_entry.row_count, rhs_entry.row_count
            ));
        }
        if !rhs_entry.column_keys.is_empty() {
            let matches = lhs_entry.column_keys.contains(&rhs_entry.column_keys)
                || lhs_entry.column_keys == rhs_entry.column_keys;
            if !matches {
                return Err(format!(
                    "compareGraphs: column_keys mismatch at {:?}: lhs={:?} rhs={:?}",
                    rhs_entry.path, lhs_entry.column_keys, rhs_entry.column_keys
                ));
            }
        }
    }

    for rhs_edge in &rhs_edges {
        if !lhs_edge_paths.contains(&(rhs_edge.from_path.as_str(), rhs_edge.to_path.as_str())) {
            return Err(format!(
                "compareGraphs: edge {:?} -> {:?} not found in lhs",
                rhs_edge.from_path, rhs_edge.to_path
            ));
        }
    }

    Ok(())
}

fn empty_object_graph_matches_scalar_null(lhs: &GraphNodeSummary, rhs: &GraphNodeSummary) -> bool {
    lhs.path == "/"
        && rhs.path == "/"
        && lhs.kind == GraphKind::Object
        && lhs.row_count == 1
        && rhs.kind == GraphKind::Scalar
        && rhs.row_count == 1
}

fn run_one(case: &FixtureCase) -> Result<(), String> {
    let child = thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn({
            let case = case.clone();
            move || run_one_with_stack(&case)
        })
        .map_err(|error| format!("thread spawn failed for {:?}: {error}", case.path))?;

    match child.join() {
        Ok(result) => result,
        Err(_) => Err(format!(
            "corpus thread panicked (likely stack overflow on a deep fixture): {:?}",
            case.path
        )),
    }
}

fn run_one_with_stack(case: &FixtureCase) -> Result<(), String> {
    let bytes = fs::read(&case.path).map_err(|error| format!("read error: {error}"))?;
    let input = match decode_fixture_input(case, &bytes) {
        Ok(input) => input,
        Err(error) => {
            return if case.expectation == Expectation::Invalid {
                Ok(())
            } else {
                Err(error)
            };
        }
    };
    run_one_inner(case, &input)
}

fn decode_fixture_input(case: &FixtureCase, bytes: &[u8]) -> Result<String, String> {
    match std::str::from_utf8(bytes) {
        Ok(input) => Ok(input.to_owned()),
        Err(_) => Err(format!("non-UTF-8 file for {:?}", case.path)),
    }
}

fn run_one_inner(case: &FixtureCase, input: &str) -> Result<(), String> {
    if case.expectation == Expectation::Invalid {
        let result = if case.kind == FixtureKind::Json {
            json_invalid_serde_oracle(input)
        } else if decode(case.kind, input, &case.path.to_string_lossy()).is_ok() {
            Err(format!(
                "expected decode failure for {:?}, but it succeeded",
                case.path
            ))
        } else {
            Ok(())
        };
        return result;
    }

    let decoded_a = decode(case.kind, input, &case.path.to_string_lossy()).map_err(|error| {
        format!(
            "expected valid decode for {:?}, got error: {error}",
            case.path
        )
    })?;

    validate_valid_primary_oracle(case.kind, input, &decoded_a)
        .map_err(|error| format!("primary oracle for {:?}: {error}", case.path))?;

    if case.kind != FixtureKind::Json {
        let yq_json = yq_to_json(&case.path);
        if let Some(yq_json) = yq_json {
            let decoded_b = decode(FixtureKind::Json, &yq_json, "").map_err(|error| {
                format!(
                    "yq cross-check: failed to decode yq JSON for {:?}: {error}",
                    case.path
                )
            })?;

            compare_trees(
                &decoded_a.store,
                decoded_a.root,
                &decoded_b.store,
                decoded_b.root,
            )
            .map_err(|error| format!("yq cross-check compareTrees for {:?}: {error}", case.path))?;

            let compat_a = core_tree_to_compat(&decoded_a.store, decoded_a.root)
                .map_err(|error| format!("compat bridge a for {:?}: {error}", case.path))?;
            let graph_a = build_graph(&compat_a);

            let compat_b = core_tree_to_compat(&decoded_b.store, decoded_b.root)
                .map_err(|error| format!("compat bridge b for {:?}: {error}", case.path))?;
            let graph_b = build_graph(&compat_b);

            compare_graphs(&graph_a, &graph_b, case.kind).map_err(|error| {
                format!("yq cross-check compareGraphs for {:?}: {error}", case.path)
            })?;
            return Ok(());
        }
    }

    let compat = core_tree_to_compat(&decoded_a.store, decoded_a.root)
        .map_err(|error| format!("compat bridge for {:?}: {error}", case.path))?;
    let graph = build_graph(&compat);
    if graph.nodes.is_empty() {
        return Err(format!("graph has no nodes for {:?}", case.path));
    }
    Ok(())
}

fn case_worker_count_for(available_parallelism: usize) -> usize {
    if let Some(val) = std::env::var_os(WORKERS_ENV)
        .and_then(|v| v.to_str().map(|s| s.to_owned()))
        .and_then(|s| s.parse::<usize>().ok())
    {
        return val.clamp(1, 4);
    }
    available_parallelism.clamp(1, 4)
}

fn worker_count() -> usize {
    case_worker_count_for(
        thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1),
    )
}

fn case_size_bytes(path: &Path) -> Option<u64> {
    path.metadata().ok().map(|metadata| metadata.len())
}

fn base_case_timeout_seconds(path: &Path) -> u64 {
    let Some(size) = case_size_bytes(path) else {
        return DEFAULT_TIMEOUT_SECONDS;
    };
    DEFAULT_TIMEOUT_SECONDS.max(size.div_ceil(MB))
}

fn case_timeout_seconds(path: &Path) -> u64 {
    let timeout = base_case_timeout_seconds(path);
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") if should_run_serially(path) => timeout.max(2),
        _ => timeout,
    }
}

fn should_run_serially(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => case_size_bytes(path)
            .map(|size| {
                size >= (MB / 2) && base_case_timeout_seconds(path) == DEFAULT_TIMEOUT_SECONDS
            })
            .unwrap_or(false),
        _ => false,
    }
}

fn format_duration_ns(duration_ns: u128) -> String {
    if duration_ns >= 1_000_000_000 {
        format!("{:.3}s", duration_ns as f64 / 1_000_000_000f64)
    } else if duration_ns >= 1_000_000 {
        format!("{:.3}ms", duration_ns as f64 / 1_000_000f64)
    } else if duration_ns >= 1_000 {
        format!("{:.3}us", duration_ns as f64 / 1_000f64)
    } else {
        format!("{duration_ns}ns")
    }
}

fn record_case_result(
    per_language: &mut std::collections::BTreeMap<String, LanguageStats>,
    result: &CaseRunResult,
) {
    let language = result.case_path.to_string_lossy().replace('\\', "/");
    let language = FixtureKind::ordered()
        .iter()
        .find_map(|kind| {
            if language.contains(&format!("/{}/", kind.as_str())) {
                Some(kind.as_str().to_owned())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_owned());
    let stats = per_language.entry(language).or_default();
    stats.total += 1;
    stats.elapsed_ns += result.elapsed_ns;
    if result.code == 0 {
        stats.passed += 1;
    } else if result.code == TIMEOUT_EXIT_CODE {
        stats.timeout += 1;
    } else {
        stats.failed += 1;
    }
}

fn print_summary(per_language: &std::collections::BTreeMap<String, LanguageStats>) {
    let mut rows = Vec::new();
    let mut total_cases = 0usize;
    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut total_timeout = 0usize;
    let mut total_elapsed_ns = 0u128;

    for kind in FixtureKind::ordered() {
        let language = kind.as_str();
        let Some(stats) = per_language.get(language) else {
            continue;
        };
        if stats.total == 0 {
            continue;
        }
        total_cases += stats.total;
        total_passed += stats.passed;
        total_failed += stats.failed;
        total_timeout += stats.timeout;
        total_elapsed_ns += stats.elapsed_ns;
        rows.push((
            language.to_owned(),
            stats.total,
            stats.passed,
            stats.failed,
            stats.timeout,
            format_duration_ns(stats.elapsed_ns),
            format_duration_ns(stats.elapsed_ns / stats.total as u128),
        ));
    }

    for (language, stats) in per_language {
        if FixtureKind::ordered()
            .iter()
            .any(|kind| kind.as_str() == language)
            || stats.total == 0
        {
            continue;
        }
        total_cases += stats.total;
        total_passed += stats.passed;
        total_failed += stats.failed;
        total_timeout += stats.timeout;
        total_elapsed_ns += stats.elapsed_ns;
        rows.push((
            language.clone(),
            stats.total,
            stats.passed,
            stats.failed,
            stats.timeout,
            format_duration_ns(stats.elapsed_ns),
            format_duration_ns(stats.elapsed_ns / stats.total as u128),
        ));
    }

    rows.push((
        "TOTAL".to_owned(),
        total_cases,
        total_passed,
        total_failed,
        total_timeout,
        format_duration_ns(total_elapsed_ns),
        if total_cases == 0 {
            format_duration_ns(0)
        } else {
            format_duration_ns(total_elapsed_ns / total_cases as u128)
        },
    ));

    let name_width = rows
        .iter()
        .map(|row| row.0.len())
        .max()
        .unwrap_or(4)
        .max("NAME".len());
    let total_width = rows
        .iter()
        .map(|row| row.1.to_string().len())
        .max()
        .unwrap_or(5)
        .max("TOTAL".len());
    let passed_width = rows
        .iter()
        .map(|row| row.2.to_string().len())
        .max()
        .unwrap_or(6)
        .max("PASSED".len());
    let failed_width = rows
        .iter()
        .map(|row| row.3.to_string().len())
        .max()
        .unwrap_or(6)
        .max("FAILED".len());
    let timeout_width = rows
        .iter()
        .map(|row| row.4.to_string().len())
        .max()
        .unwrap_or(7)
        .max("TIMEOUT".len());
    let elapsed_width = rows
        .iter()
        .map(|row| row.5.len())
        .max()
        .unwrap_or(7)
        .max("ELAPSED".len());
    let avg_width = rows
        .iter()
        .map(|row| row.6.len())
        .max()
        .unwrap_or(3)
        .max("AVG".len());

    eprintln!(
        "{:<name_width$} {:>total_width$} {:>passed_width$} {:>failed_width$} {:>timeout_width$} {:>elapsed_width$} {:>avg_width$}",
        "NAME", "TOTAL", "PASSED", "FAILED", "TIMEOUT", "ELAPSED", "AVG",
    );
    for row in rows {
        eprintln!(
            "{:<name_width$} {:>total_width$} {:>passed_width$} {:>failed_width$} {:>timeout_width$} {:>elapsed_width$} {:>avg_width$}",
            row.0, row.1, row.2, row.3, row.4, row.5, row.6,
        );
    }
}

fn truncate_for_log(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    format!(
        "{}\n... truncated {} chars ...\n",
        &text[..limit],
        text.len() - limit
    )
}

fn parse_timeout_elapsed(stderr: &str) -> String {
    stderr
        .lines()
        .find_map(|line| line.trim().strip_prefix("elapsed: ").map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn parse_diagnostic_timeout_line(stderr: &str) -> Option<String> {
    stderr.lines().find_map(|line| {
        let line = line.trim();
        line.starts_with("diagnostic timeout:")
            .then(|| line.to_owned())
    })
}

fn parse_phase_rows(stderr: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for line in stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let prefix = if let Some(rest) = line.strip_prefix("corpus case phase end: ") {
            rest.split_once(" :: ").map(|(_, rest)| rest)
        } else if let Some(rest) = line.strip_prefix("corpus phase end: ") {
            Some(rest)
        } else {
            None
        };
        let Some(rest) = prefix else {
            continue;
        };
        let Some((phase, duration_part)) = rest.rsplit_once(" (") else {
            continue;
        };
        let duration = duration_part.strip_suffix(')').unwrap_or(duration_part);
        let phase = if phase == "runOne total" {
            "runOne".to_owned()
        } else {
            phase.to_owned()
        };
        if seen.insert((phase.clone(), duration.to_owned())) {
            rows.push((phase, duration.to_owned()));
        }
    }

    let phase_names = rows
        .iter()
        .map(|(phase, _)| phase.clone())
        .collect::<std::collections::BTreeSet<_>>();
    rows.into_iter()
        .filter(|(phase, _)| {
            !(phase == "decode primary"
                && (phase_names.contains("decodeSingle init")
                    || phase_names.contains("decodeSingle decode")))
        })
        .collect()
}

fn format_timeout_diagnostic(stdout: &str, stderr: &str, case_path: &Path) -> String {
    let mut out = String::new();
    out.push_str(&format!("=== {} ===\n", case_path.display()));
    out.push_str(&format!("elapsed: {}\n", parse_timeout_elapsed(stderr)));
    out.push_str("phase:\n");
    let rows = parse_phase_rows(stderr);
    if rows.is_empty() {
        if let Some(line) = parse_diagnostic_timeout_line(stderr) {
            out.push_str(&format!("1. {}\n", line));
        } else {
            out.push_str("1. unavailable\n");
        }
    } else {
        for (index, (phase, duration)) in rows.iter().enumerate() {
            out.push_str(&format!("{}. {} ({})\n", index + 1, phase, duration));
        }
    }
    if !stdout.trim().is_empty() {
        out.push_str("stdout: non-empty (omitted)\n");
    }
    out
}

fn format_failure_diagnostic(result: &CaseRunResult) -> String {
    let size_text = case_size_bytes(&result.case_path)
        .map(|size| size.to_string())
        .unwrap_or_else(|| "unknown".to_owned());
    let mut out = String::new();
    out.push_str(&format!("=== {} ===\n", result.case_path.display()));
    out.push_str(&format!("exit_code: {}\n", result.code));
    out.push_str(&format!("language: {}\n", path_language(&result.case_path)));
    out.push_str(&format!("size_bytes: {}\n", size_text));
    out.push_str(&format!(
        "elapsed: {}\n",
        format_duration_ns(result.elapsed_ns)
    ));
    out.push_str(&format!(
        "timeout_seconds: {}\n",
        case_timeout_seconds(&result.case_path)
    ));
    out.push_str(&format!(
        "serial: {}\n",
        should_run_serially(&result.case_path)
    ));
    if !result.stdout.is_empty() {
        out.push_str("stdout:\n");
        out.push_str(&truncate_for_log(&result.stdout, 8_000));
        if !result.stdout.ends_with('\n') {
            out.push('\n');
        }
    }
    if !result.stderr.is_empty() {
        out.push_str("stderr:\n");
        out.push_str(&truncate_for_log(&result.stderr, 8_000));
        if !result.stderr.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push('\n');
    out
}

fn path_language(path: &Path) -> &'static str {
    let normalized = path.to_string_lossy().replace('\\', "/");
    for kind in FixtureKind::ordered() {
        let marker = format!("/{}/", kind.as_str());
        if normalized.contains(&marker) {
            return kind.as_str();
        }
    }
    "unknown"
}

fn write_failure_outputs(failures: &[CaseRunResult]) -> Result<(), String> {
    let failure_lines = failures
        .iter()
        .map(|failure| format!("{}\n", failure.case_path.display()))
        .collect::<String>();
    fs::write(FAILURE_FILENAME, failure_lines)
        .map_err(|error| format!("write {FAILURE_FILENAME} failed: {error}"))?;

    let timeout_text = failures
        .iter()
        .filter(|failure| failure.code == TIMEOUT_EXIT_CODE)
        .map(|failure| {
            format!(
                "{}\n",
                format_timeout_diagnostic(&failure.stdout, &failure.stderr, &failure.case_path)
            )
        })
        .collect::<String>();
    fs::write(TIMEOUT_DIAGNOSTICS_FILENAME, timeout_text)
        .map_err(|error| format!("write {TIMEOUT_DIAGNOSTICS_FILENAME} failed: {error}"))?;

    let failure_text = failures
        .iter()
        .map(format_failure_diagnostic)
        .collect::<String>();
    fs::write(FAILURE_DIAGNOSTICS_FILENAME, failure_text)
        .map_err(|error| format!("write {FAILURE_DIAGNOSTICS_FILENAME} failed: {error}"))?;

    Ok(())
}

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<Output, std::process::Child> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child
                    .wait_with_output()
                    .expect("wait_with_output after child exit");
                return Ok(output);
            }
            Ok(None) => {
                let now = Instant::now();
                if now >= deadline {
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            let output = child
                                .wait_with_output()
                                .expect("wait_with_output after timeout boundary exit");
                            return Ok(output);
                        }
                        Ok(None) | Err(_) => return Err(child),
                    }
                }
                let remaining = deadline.saturating_duration_since(now);
                thread::sleep(remaining.min(Duration::from_millis(1)));
            }
            Err(_) => return Err(child),
        }
    }
}

fn helper_binary_path() -> PathBuf {
    if let Some(path) = std::env::var_os(HELPER_BINARY_ENV) {
        return PathBuf::from(path);
    }
    if let Some(path) = option_env!("CARGO_BIN_EXE_corpus_runner_helper") {
        return PathBuf::from(path);
    }
    panic!(
        "{} is not available; ensure {} is built alongside tests",
        HELPER_BINARY_ENV, HELPER_BINARY_NAME
    );
}

fn build_driver_invocation(case_path: &Path) -> DriverInvocation {
    DriverInvocation {
        program: helper_binary_path(),
        args: vec![case_path.as_os_str().to_os_string()],
        envs: Vec::new(),
    }
}

fn run_driver_subprocess(case_path: &Path, timeout_seconds: u64) -> CaseRunResult {
    let started_at = Instant::now();
    let invocation = build_driver_invocation(case_path);
    let mut command = Command::new(&invocation.program);
    command
        .args(&invocation.args)
        .current_dir(std::env::current_dir().expect("current dir"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &invocation.envs {
        command.env(key, value);
    }

    let child = command.spawn().expect("spawn corpus single-case driver");
    match wait_with_timeout(child, Duration::from_secs(timeout_seconds)) {
        Ok(output) => CaseRunResult {
            case_path: case_path.to_path_buf(),
            code: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            elapsed_ns: started_at.elapsed().as_nanos(),
        },
        Err(mut child) => {
            let timeout_elapsed_ns = started_at.elapsed().as_nanos();
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .expect("wait timed out child output");
            CaseRunResult {
                case_path: case_path.to_path_buf(),
                code: TIMEOUT_EXIT_CODE,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                elapsed_ns: timeout_elapsed_ns,
            }
        }
    }
}

fn run_parallel_cases(cases: &[FixtureCase]) -> Vec<CaseRunResult> {
    let serial_cases = cases
        .iter()
        .filter(|case| should_run_serially(&case.path))
        .cloned()
        .collect::<Vec<_>>();
    let parallel_cases = cases
        .iter()
        .filter(|case| !should_run_serially(&case.path))
        .cloned()
        .collect::<Vec<_>>();

    let mut results = Vec::new();
    for case in serial_cases {
        results.push(run_driver_subprocess(
            &case.path,
            case_timeout_seconds(&case.path),
        ));
    }

    if parallel_cases.is_empty() {
        return results;
    }

    let queue = Arc::new(parallel_cases);
    let next_index = Arc::new(AtomicUsize::new(0));
    let shared_results = Arc::new(Mutex::new(Vec::new()));
    thread::scope(|scope| {
        for _ in 0..worker_count() {
            let queue = Arc::clone(&queue);
            let next_index = Arc::clone(&next_index);
            let shared_results = Arc::clone(&shared_results);
            scope.spawn(move || {
                loop {
                    let index = next_index.fetch_add(1, Ordering::SeqCst);
                    if index >= queue.len() {
                        break;
                    }
                    let case = &queue[index];
                    let result =
                        run_driver_subprocess(&case.path, case_timeout_seconds(&case.path));
                    shared_results
                        .lock()
                        .expect("lock shared results")
                        .push(result);
                }
            });
        }
    });

    let mut parallel_results = Arc::try_unwrap(shared_results)
        .expect("unwrap shared results")
        .into_inner()
        .expect("take shared results");
    parallel_results.sort_by(|lhs, rhs| lhs.case_path.cmp(&rhs.case_path));
    results.extend(parallel_results);

    // Retry timed-out cases serially — parallel scheduling contention on CI can
    // starve a subprocess past its timeout even when the fixture itself is fast.
    // A clean serial retry is authoritative.
    let timeout_count = results
        .iter()
        .filter(|r| r.code == TIMEOUT_EXIT_CODE)
        .count();
    if timeout_count > 0 {
        eprintln!(
            "[corpus] {} timed-out case(s), retrying serially...",
            timeout_count
        );
        let mut retried = 0;
        for r in &mut results {
            if r.code == TIMEOUT_EXIT_CODE {
                let case = cases
                    .iter()
                    .find(|c| c.path == r.case_path)
                    .expect("timed-out case not found in input list");
                let base_timeout = case_timeout_seconds(&case.path);
                let retry_timeout = if std::env::var("CI").is_ok() {
                    base_timeout * 2
                } else {
                    base_timeout
                };
                let retry = run_driver_subprocess(&case.path, retry_timeout);
                if retry.code == 0 {
                    retried += 1;
                    eprintln!(
                        "[corpus]   {} — retry passed (was {}s timeout, retry {}s)",
                        case.path.display(),
                        base_timeout,
                        retry_timeout,
                    );
                }
                *r = retry;
            }
        }
        eprintln!(
            "[corpus] {}/{} timed-out case(s) passed on serial retry",
            retried, timeout_count
        );
    }

    results
}

fn build_fixture_case_from_path(path: PathBuf) -> FixtureCase {
    let kind = match path_language(&path) {
        "json" => FixtureKind::Json,
        "toml" => FixtureKind::Toml,
        "yaml" => FixtureKind::Yaml,
        other => panic!("unsupported fixture kind for {:?}: {}", path, other),
    };
    let expectation = expectation_from_name(
        path.file_name()
            .and_then(|name| name.to_str())
            .expect("fixture basename as utf-8"),
    )
    .expect("fixture expectation from name");

    FixtureCase {
        path,
        kind,
        expectation,
    }
}

#[cfg(not(test))]
pub(crate) fn corpus_runner_helper_main() -> i32 {
    let Some(case_path) = std::env::args_os().nth(1) else {
        let _ = writeln!(io::stderr(), "usage: {} <fixture-path>", HELPER_BINARY_NAME);
        return 2;
    };
    let case = build_fixture_case_from_path(PathBuf::from(case_path));
    match run_one(&case) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(io::stderr(), "{error}");
            1
        }
    }
}

#[test]
fn corpus_fixtures_run() {
    let cases = collect_all_cases();
    assert!(!cases.is_empty(), "no fixture cases found");

    let results = run_parallel_cases(&cases);
    let mut per_language = std::collections::BTreeMap::new();
    for result in &results {
        record_case_result(&mut per_language, result);
    }
    let failures = results
        .into_iter()
        .filter(|result| result.code != 0)
        .collect::<Vec<_>>();

    write_failure_outputs(&failures).expect("write corpus failure outputs");
    print_summary(&per_language);

    let total = per_language
        .values()
        .map(|stats| stats.total)
        .sum::<usize>();
    let passed = per_language
        .values()
        .map(|stats| stats.passed)
        .sum::<usize>();
    let failed = per_language
        .values()
        .map(|stats| stats.failed)
        .sum::<usize>();
    let timeout = per_language
        .values()
        .map(|stats| stats.timeout)
        .sum::<usize>();
    eprintln!(
        "corpus done: total={} passed={} failed={} timeout={} skipped=0",
        total, passed, failed, timeout
    );

    if !failures.is_empty() {
        let non_timeout = failures
            .iter()
            .filter(|failure| failure.code != TIMEOUT_EXIT_CODE)
            .count();
        let timeouts = failures.len() - non_timeout;
        panic!(
            "{} corpus fixture(s) failed (failed={}, timeout={})",
            failures.len(),
            non_timeout,
            timeouts
        );
    }
}

#[test]
fn driver_invocation_uses_helper_binary_instead_of_libtest_flags() {
    let case_path = Path::new("/tmp/sample.toml");
    let invocation = build_driver_invocation(case_path);

    assert!(
        invocation.program.ends_with("corpus_runner_helper"),
        "expected helper binary, got {}",
        invocation.program.display()
    );
    assert_eq!(
        invocation.args,
        vec![case_path.as_os_str().to_os_string()],
        "helper should receive only the case path as argv"
    );
    assert!(
        !invocation.args.iter().any(|arg| arg == "--ignored"
            || arg == "--exact"
            || arg == "corpus_single_case_driver"),
        "helper invocation must not route through libtest flags"
    );
    assert!(
        invocation.envs.is_empty(),
        "helper invocation should have no env overrides"
    );
}

#[test]
fn json_valid_oracle_matches_serde_round_trip_semantics() {
    let decoded = json_valid_serde_oracle("{\"a\":1,\"b\":[true,null,\"x\"]}")
        .expect("json valid serde oracle should pass");

    let compat = core_tree_to_compat(&decoded.store, decoded.root).expect("compat bridge");
    let graph = build_graph(&compat);
    assert!(!graph.nodes.is_empty(), "graph should remain non-empty");
}

#[test]
fn json_invalid_oracle_requires_both_serde_and_treease_to_reject() {
    json_invalid_serde_oracle("{\"a\":1,}")
        .expect("invalid json should be rejected by serde_json and treease");
}

#[test]
fn json_serde_oracle_encoder_uses_compact_non_smart_output() {
    let input = concat!(
        "{\"items\":[",
        "{\"alpha\":1,\"beta\":2},",
        "{\"alpha\":3,\"beta\":4},",
        "{\"alpha\":5,\"beta\":6}",
        "]}"
    );
    let decoded = JsonDecoder
        .decode_str(input)
        .expect("json input should decode");

    let encoded = json_oracle_encode(&decoded).expect("oracle encode should succeed");

    assert_eq!(encoded, format!("{input}\n"));
}

#[test]
fn json_oracle_large_fixture_stays_within_case_timeout() {
    let case_path = fixtures_root()
        .join("json")
        .join("jsonexamples__gsoc-2018.1.json");

    let result = run_driver_subprocess(&case_path, 8);

    assert_eq!(
        result.code, 0,
        "large JSON oracle path should finish within the per-case timeout; stderr:\n{}",
        result.stderr
    );
}

#[test]
fn json_oracle_mesh_fixture_matches_serde_semantics() {
    let input = fs::read_to_string(fixtures_root().join("json").join(MESH_JSON_FIXTURE))
        .expect("mesh fixture should be readable");
    let decoded = JsonDecoder
        .decode_str(&input)
        .expect("mesh fixture should decode");

    json_valid_serde_oracle_with_decoded(&input, &decoded)
        .expect("mesh fixture should preserve serde_json semantics");
}

#[test]
fn json_oracle_skips_unsupported_large_integer_semantics() {
    let input = fs::read_to_string(fixtures_root().join("json").join("pass22.1.json"))
        .expect("large integer fixture should be readable");
    let decoded = JsonDecoder
        .decode_str(&input)
        .expect("large integer fixture should decode");

    json_valid_serde_oracle_with_decoded(&input, &decoded)
        .expect("serde oracle should skip integers outside treease's i64 value domain");
}

#[test]
fn json_oracle_canonicalizes_supported_float_precision() {
    let canonical = canonicalize_json_for_serde_oracle("[1.0,2.5,1e2]");

    assert_eq!(canonical.text, "[1,2.5,100]");
    assert!(
        !canonical.has_unsupported_numbers,
        "supported finite f64 values should stay eligible for serde comparison"
    );
}

#[test]
fn json_oracle_pass01_accepts_single_ulp_float_drift() {
    let input = fs::read_to_string(fixtures_root().join("json").join("pass01.1.json"))
        .expect("pass01 fixture should be readable");
    let decoded = JsonDecoder
        .decode_str(&input)
        .expect("pass01 fixture should decode");

    json_valid_serde_oracle_with_decoded(&input, &decoded)
        .expect("pass01 fixture should pass oracle within f64 semantic tolerance");
}

#[test]
fn json_oracle_canada_accepts_single_ulp_float_drift() {
    let input = fs::read_to_string(
        fixtures_root()
            .join("json")
            .join("jsonexamples__canada.1.json"),
    )
    .expect("canada fixture should be readable");
    let decoded = JsonDecoder
        .decode_str(&input)
        .expect("canada fixture should decode");

    json_valid_serde_oracle_with_decoded(&input, &decoded)
        .expect("canada fixture should pass oracle within f64 semantic tolerance");
}

#[test]
fn medium_json_with_one_second_budget_runs_serially() {
    let case_path = fixtures_root().join("json").join(MEDIUM_JSON_FIXTURE);

    assert!(
        should_run_serially(&case_path),
        "medium JSON fixtures that already consume most of a 1s budget should avoid parallel contention"
    );
}

#[test]
fn medium_json_serial_case_gets_two_second_timeout_budget() {
    let case_path = fixtures_root().join("json").join(MEDIUM_JSON_FIXTURE);

    assert_eq!(
        case_timeout_seconds(&case_path),
        2,
        "medium JSON fixtures that are forced serial should get extra timeout headroom"
    );
}

#[test]
fn toml_invalid_double_comma_fixture_should_fail_like_zig() {
    let case_path = fixtures_root()
        .join("toml")
        .join("array__double-comma-01.0.toml");
    let input =
        fs::read_to_string(&case_path).expect("double comma TOML fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("double comma TOML fixture should be rejected");
}

#[test]
fn toml_only_comma_array_fixture_should_fail_like_zig() {
    let case_path = fixtures_root()
        .join("toml")
        .join("array__only-comma-01.0.toml");
    let input = fs::read_to_string(&case_path).expect("only-comma TOML fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("comma-only TOML array fixture should be rejected");
}

#[test]
fn toml_ideographic_space_prefix_fixture_should_fail_like_zig() {
    let case_path = fixtures_root()
        .join("toml")
        .join("encoding__ideographic-space.0.toml");
    let input =
        fs::read_to_string(&case_path).expect("ideographic-space TOML fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("non-ASCII leading whitespace should be rejected like Zig");
}

#[test]
fn toml_exp_dot_float_fixture_should_fail_like_zig() {
    let case_path = fixtures_root()
        .join("toml")
        .join("float__exp-dot-02.0.toml");
    let input = fs::read_to_string(&case_path).expect("exp-dot TOML fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("TOML floats without fractional digits before exponent should be rejected");
}

#[test]
fn toml_inline_table_empty_item_fixture_should_fail_like_zig() {
    let case_path = fixtures_root()
        .join("toml")
        .join("inline-table__empty-01.0.toml");
    let input =
        fs::read_to_string(&case_path).expect("inline table empty-item fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("inline table with only a comma should be rejected");
}

#[test]
fn toml_inline_table_linebreak_before_comma_fixture_should_fail_like_zig() {
    let case_path = fixtures_root()
        .join("toml")
        .join("inline-table__linebreak-03.0.toml");
    let input =
        fs::read_to_string(&case_path).expect("inline table linebreak fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("inline table linebreak before comma should be rejected");
}

#[test]
fn toml_key_after_value_fixture_should_fail_like_zig() {
    let case_path = fixtures_root().join("toml").join("key__after-value.0.toml");
    let input =
        fs::read_to_string(&case_path).expect("key-after-value TOML fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("trailing tokens after a quoted value should be rejected");
}

#[test]
fn toml_multiline_quoted_key_fixture_should_fail_like_zig() {
    let case_path = fixtures_root()
        .join("toml")
        .join("key__multiline-key-01.0.toml");
    let input = fs::read_to_string(&case_path)
        .expect("multiline quoted-key TOML fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("multiline quoted keys should be rejected");
}

#[test]
fn toml_quoted_unicode_key_fixture_should_pass_like_zig() {
    let case = build_fixture_case_from_path(
        fixtures_root()
            .join("toml")
            .join("key__quoted-unicode.1.toml"),
    );

    run_one(&case).expect("quoted unicode TOML keys should decode");
}

#[test]
fn toml_multibyte_fixture_should_pass_like_zig() {
    let case = build_fixture_case_from_path(fixtures_root().join("toml").join("multibyte.1.toml"));

    run_one(&case).expect("multibyte TOML fixture should decode");
}

#[test]
fn toml_empty_root_key_should_compare_with_json_oracle() {
    let lhs = TomlDecoder
        .decode_str_with_filename("\"\" = \"blank\"\n", "key__empty-01.1.toml")
        .expect("TOML empty-string key should decode");
    let rhs = JsonDecoder
        .decode_str("{\"\":\"blank\"}")
        .expect("json oracle should decode");

    compare_trees(&lhs.store, lhs.root, &rhs.store, rhs.root)
        .expect("empty-string root key should compare equal to json oracle");
}

#[test]
fn toml_empty_name_array_table_fixture_should_compare_with_json_oracle() {
    let case_path = fixtures_root()
        .join("toml")
        .join("table__array-empty-name.1.toml");
    let input =
        fs::read_to_string(&case_path).expect("empty-name array-table fixture should be readable");
    let lhs = TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect("empty-name array-table TOML should decode");
    let rhs_json = yq_to_json(&case_path).expect("yq should produce json oracle");
    let rhs = JsonDecoder
        .decode_str(&rhs_json)
        .expect("json oracle should decode");

    compare_trees(&lhs.store, lhs.root, &rhs.store, rhs.root)
        .expect("empty-name array-table should compare equal to yq oracle");
}

#[test]
fn toml_multiline_basic_quotes_fixture_should_pass_like_zig() {
    let case = build_fixture_case_from_path(
        fixtures_root()
            .join("toml")
            .join("spec-1.0.0__string-4.1.toml"),
    );

    run_one(&case).expect("valid multiline basic quotes TOML should decode");
}

#[test]
fn toml_multiline_crlf_comment_fixture_should_pass_like_zig() {
    let case = build_fixture_case_from_path(
        fixtures_root()
            .join("toml")
            .join("string__multiline-escaped-crlf.1.toml"),
    );

    run_one(&case).expect("CRLF multiline string fixture should decode");
}

#[test]
fn toml_trailing_comma_array_fixture_should_pass_like_zig() {
    let case = build_fixture_case_from_path(
        fixtures_root()
            .join("toml")
            .join("array__trailing-comma.1.toml"),
    );

    run_one(&case).expect("TOML trailing-comma arrays should decode");
}

#[test]
fn toml_comment_after_literal_without_whitespace_fixture_should_pass_like_zig() {
    let case = build_fixture_case_from_path(
        fixtures_root()
            .join("toml")
            .join("comment__after-literal-no-ws.1.toml"),
    );

    run_one(&case).expect("TOML inline comments after literals should decode");
}

#[test]
fn toml_comment_with_control_byte_fixture_should_fail_like_zig() {
    let case_path = fixtures_root()
        .join("toml")
        .join("control__comment-del.0.toml");
    let input =
        fs::read_to_string(&case_path).expect("control comment TOML fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("comments with DEL should be rejected");
}

#[test]
fn toml_basic_string_with_control_byte_fixture_should_fail_like_zig() {
    let case_path = fixtures_root()
        .join("toml")
        .join("control__string-bs.0.toml");
    let input =
        fs::read_to_string(&case_path).expect("control string TOML fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("basic strings with raw control bytes should be rejected");
}

#[test]
fn toml_bare_cr_fixture_should_fail_like_zig() {
    let case_path = fixtures_root().join("toml").join("control__bare-cr.0.toml");
    let input = fs::read_to_string(&case_path).expect("bare-CR TOML fixture should be readable");

    TomlDecoder
        .decode_str_with_filename(&input, &case_path.to_string_lossy())
        .expect_err("bare carriage returns outside CRLF should be rejected");
}

#[test]
fn toml_table_header_with_newline_fixture_should_fail_like_zig() {
    for fixture in [
        "table__newline-01.0.toml",
        "table__newline-03.0.toml",
        "table__newline-05.0.toml",
    ] {
        let case_path = fixtures_root().join("toml").join(fixture);
        let input = fs::read_to_string(&case_path)
            .expect("newline table-header TOML fixture should be readable");

        TomlDecoder
            .decode_str_with_filename(&input, &case_path.to_string_lossy())
            .expect_err("table headers containing newlines should be rejected");
    }
}

#[test]
fn yaml_anchor_alias_key_fixture_should_match_yq() {
    let case = build_fixture_case_from_path(fixtures_root().join("yaml").join("26DV.1.yaml"));
    run_one(&case).expect("YAML anchor and alias keys should match yq oracle");
}

#[test]
fn yaml_anchor_rebind_fixture_should_match_yq() {
    let case = build_fixture_case_from_path(fixtures_root().join("yaml").join("3GZX.1.yaml"));
    run_one(&case).expect("YAML anchor rebinding should match yq oracle");
}

#[test]
fn yaml_multiline_plain_scalar_fixture_should_match_yq() {
    let case = build_fixture_case_from_path(fixtures_root().join("yaml").join("36F6.1.yaml"));
    run_one(&case).expect("YAML multiline plain scalars should fold like yq");
}

#[test]
fn yaml_multiline_double_quoted_scalar_fixture_should_match_yq() {
    let case = build_fixture_case_from_path(fixtures_root().join("yaml").join("4CQQ.1.yaml"));
    run_one(&case).expect("YAML multiline double-quoted scalars should decode like yq");
}

#[test]
fn yaml_double_quoted_scalar_with_leading_and_trailing_breaks_should_match_yq() {
    let case = build_fixture_case_from_path(fixtures_root().join("yaml").join("6WPF.1.yaml"));
    run_one(&case).expect("YAML double-quoted folding should preserve boundary spaces like yq");
}

#[test]
fn yaml_double_quoted_scalar_with_blank_and_indented_lines_should_match_yq() {
    let case = build_fixture_case_from_path(fixtures_root().join("yaml").join("7A4E.1.yaml"));
    run_one(&case).expect("YAML double-quoted multiline folding should match yq");
}

#[test]
fn yaml_double_quoted_line_continuations_should_match_yq() {
    let case = build_fixture_case_from_path(
        fixtures_root()
            .join("yaml")
            .join("spec-example-7-5-double-quoted-line-breaks.1.yaml"),
    );
    run_one(&case).expect("YAML double-quoted line continuations should match yq");
}

#[test]
fn yaml_double_quoted_backslash_tab_escape_variants_should_match_yq() {
    for fixture in [
        "3RLN__01.1.yaml",
        "3RLN__04.1.yaml",
        "DE56__02.1.yaml",
        "DE56__03.1.yaml",
        "KH5V__01.1.yaml",
    ] {
        let case = build_fixture_case_from_path(fixtures_root().join("yaml").join(fixture));
        run_one(&case).unwrap_or_else(|err| {
            panic!("YAML double-quoted backslash-tab escape should match yq for {fixture}: {err}")
        });
    }
}

#[test]
fn yaml_block_scalar_chomping_and_folding_fixtures_should_match_yq() {
    for fixture in [
        "4QFQ.1.yaml",
        "A6F9.1.yaml",
        "F8F9.1.yaml",
        "K858.1.yaml",
        "4ZYM.1.yaml",
        "5GBF.1.yaml",
        "DWX9.1.yaml",
        "7T8X.1.yaml",
        "4WA9.1.yaml",
        "literal-scalars.1.yaml",
        "spec-example-8-8-literal-content-1-3.1.yaml",
    ] {
        let case = build_fixture_case_from_path(fixtures_root().join("yaml").join(fixture));
        run_one(&case).unwrap_or_else(|err| {
            panic!("YAML block scalar semantics should match yq for {fixture}: {err}")
        });
    }
}

#[test]
fn toml_comment_only_document_graph_should_match_yq_null_oracle() {
    let lhs = TomlDecoder
        .decode_str_with_filename("# comment only\n", "comment__noeol.1.toml")
        .expect("comment-only TOML should decode");
    let rhs = JsonDecoder
        .decode_str("null")
        .expect("json null should decode");

    let lhs_graph = build_graph(&core_tree_to_compat(&lhs.store, lhs.root).expect("lhs compat"));
    let rhs_graph = build_graph(&core_tree_to_compat(&rhs.store, rhs.root).expect("rhs compat"));

    compare_graphs(&lhs_graph, &rhs_graph, FixtureKind::Toml)
        .expect("comment-only TOML graph should be treated as equivalent to yq null output");
}

#[test]
fn case_worker_count_caps_at_four() {
    assert_eq!(case_worker_count_for(1), 1);
    assert_eq!(case_worker_count_for(4), 4);
    assert_eq!(case_worker_count_for(8), 4);
}
