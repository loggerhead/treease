use crate::args::{CliError, CommandKind, InputPayload};
use crate::cli_io::input::{guess_input_format, prepare_streaming_inputs, resolve_input_format};
use crate::commands::metadata::execute_metadata_command;
use crate::commands::run::should_render_root_help_on_empty_interactive_invocation;
use crate::execute::execute_command;
use crate::{errors, parser, web_payload, web_server};
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
    web_server::WebServerState {
        token: "test-token".to_string(),
        result: web_server::WebServerResult::text(
            "input.json".to_string(),
            ".".to_string(),
            "json".to_string(),
            r#"{"ok":true}"#.to_string(),
        ),
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
fn web_server_serves_result_only_with_matching_token() {
    let missing = request_web_server_once(test_web_server_state(), "/cli/result");
    assert_response_contains(&missing, "HTTP/1.1 403 Forbidden");

    let wrong = request_web_server_once(test_web_server_state(), "/cli/result?token=wrong");
    assert_response_contains(&wrong, "HTTP/1.1 403 Forbidden");

    let matching = request_web_server_once(test_web_server_state(), "/cli/result?token=test-token");
    assert_response_contains(&matching, "HTTP/1.1 200 OK");
    assert_response_contains(&matching, "Content-Type: application/json; charset=utf-8");
    assert_response_contains(&matching, r#""source_label":"input.json""#);
    assert_response_contains(&matching, r#""text":"{\"ok\":true}""#);
}

#[test]
fn web_server_serves_cli_metadata_and_source_separately() {
    let missing_meta = request_web_server_once(test_web_server_state(), "/cli/meta");
    assert_response_contains(&missing_meta, "HTTP/1.1 403 Forbidden");

    let wrong_source = request_web_server_once(test_web_server_state(), "/cli/source?token=wrong");
    assert_response_contains(&wrong_source, "HTTP/1.1 403 Forbidden");

    let meta = request_web_server_once(test_web_server_state(), "/cli/meta?token=test-token");
    assert_response_contains(&meta, "HTTP/1.1 200 OK");
    assert_response_contains(&meta, "Content-Type: application/json; charset=utf-8");
    assert_response_contains(&meta, "Access-Control-Allow-Origin: *");
    assert_response_contains(&meta, r#""source_url":"/cli/source?token=test-token""#);

    let source = request_web_server_once(test_web_server_state(), "/cli/source?token=test-token");
    assert_response_contains(&source, "HTTP/1.1 200 OK");
    assert_response_contains(&source, "Content-Type: application/json; charset=utf-8");
    assert_eq!(response_body(&source), br#"{"ok":true}"#);
}

#[test]
fn web_server_streams_file_source() {
    let source_dir = test_asset_dir(&[("input.json", br#"{"streamed":true}"#.as_slice())]);
    let source_path = source_dir.join("input.json");
    let state = web_server::WebServerState {
        token: "test-token".to_string(),
        result: web_server::WebServerResult::file(
            "input.json".to_string(),
            ".".to_string(),
            "json".to_string(),
            source_path,
        ),
    };

    let source = request_web_server_once(state, "/cli/source?token=test-token");

    assert_response_contains(&source, "HTTP/1.1 200 OK");
    assert_response_contains(&source, "Content-Type: application/json; charset=utf-8");
    assert_eq!(response_body(&source), br#"{"streamed":true}"#);
}

#[test]
fn web_server_graph_url_reuses_remote_editor_and_local_source() {
    let server =
        web_server::WebServer::bind_for_test(test_web_server_state()).expect("server should bind");
    let editor_url = server.editor_url();
    assert!(editor_url.starts_with("https://treease.com/editor?textUrl=http%3A%2F%2F127.0.0.1%3A"));
    assert!(editor_url.contains("%2Fcli%2Fsource%3Ftoken%3Dtest-token"));
    assert!(editor_url.ends_with("&lang=json&ui=editor%2Cviewer"));
}

#[test]
fn default_invocation_parses_expression_and_files() {
    let raw = vec![
        "treease".to_string(),
        ".foo".to_string(),
        "a.json".to_string(),
    ];
    let parsed = parser::parse_cli_args(&raw).expect("parse should succeed");
    assert_eq!(parsed.command, CommandKind::Run);
    assert_eq!(parsed.expression, ".foo");
    assert_eq!(parsed.files, vec!["a.json"]);
}

#[test]
fn empty_interactive_invocation_prefers_help_over_stdin() {
    assert!(should_render_root_help_on_empty_interactive_invocation(
        &["treease".to_string()],
        true
    ));
    assert!(!should_render_root_help_on_empty_interactive_invocation(
        &["treease".to_string()],
        false
    ));
    assert!(!should_render_root_help_on_empty_interactive_invocation(
        &["treease".to_string(), ".foo".to_string()],
        true
    ));
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
    let parsed = parser::parse_cli_args(&raw).expect("parse should succeed");
    assert_eq!(parsed.input_format.as_deref(), Some("yaml"));
    assert_eq!(parsed.output_format.as_deref(), Some("json"));
    assert_eq!(parsed.indent, Some(2));
    assert_eq!(parsed.expression, ".foo");
}

#[test]
fn input_format_guess_prefers_explicit_override() {
    let parsed = parser::parse_cli_args(&[
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
    let parsed = parser::parse_cli_args(&["treease".to_string(), ".foo".to_string()])
        .expect("parse should succeed");
    let payload = InputPayload {
        name: "README".to_string(),
        bytes: b"plain text without recognizable structure".to_vec(),
    };

    let input = resolve_input_format(&parsed, &payload).expect("input format should resolve");

    assert_eq!(input, "json");
}

#[test]
fn suffixless_yaml_input_executes_without_explicit_input_format() {
    let parsed = parser::parse_cli_args(&["treease".to_string(), ".foo".to_string()])
        .expect("parse should succeed");
    let inputs = vec![InputPayload {
        name: "sample".to_string(),
        bytes: b"foo: 1\n".to_vec(),
    }];

    let output = execute_command(&parsed, &inputs).expect("command should succeed");

    assert_eq!(String::from_utf8(output).unwrap(), "1\n");
}

#[test]
fn streaming_run_path_reads_file_contents_at_execution_time() {
    let root = test_asset_dir(&[("input.yaml", b"foo: 1\n")]);
    let path = root.join("input.yaml");
    let parsed = parser::parse_cli_args(&[
        "treease".to_string(),
        ".foo".to_string(),
        path.to_string_lossy().into_owned(),
    ])
    .expect("parse should succeed");

    let inputs = prepare_streaming_inputs(&parsed).expect("streaming inputs should prepare");
    fs::write(&path, b"foo: 2\n").expect("test input should update");

    let mut output = Vec::new();
    let printed = crate::execute::execute_command_to_writer(&parsed, &inputs, &mut output)
        .expect("streaming command should succeed");

    assert!(printed);
    assert_eq!(String::from_utf8(output).unwrap(), "2\n");
}

#[test]
fn oversized_json_integer_does_not_block_unrelated_field_query() {
    let parsed =
        parser::parse_cli_args(&["treease".to_string(), ".BaseResp.StatusCode".to_string()])
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
    let parsed =
        parser::parse_cli_args(&["treease".to_string(), "-n".to_string(), "1".to_string()])
            .expect("parse should succeed");
    let output = execute_command(&parsed, &[]).expect("command should succeed");
    assert_eq!(String::from_utf8(output).unwrap(), "1\n");
}

#[test]
fn null_input_empty_object_renders_object_not_array() {
    let parsed =
        parser::parse_cli_args(&["treease".to_string(), "-n".to_string(), "{}".to_string()])
            .expect("parse should succeed");

    let output = execute_command(&parsed, &[]).expect("command should succeed");

    assert_eq!(String::from_utf8(output).unwrap(), "{}\n");
}

#[test]
fn null_input_object_literal_renders_object() {
    let parsed = parser::parse_cli_args(&[
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
    let parsed = parser::parse_cli_args(&[
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
    let parsed = parser::parse_cli_args(&[
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
    assert_eq!(
        crate::execute::compute_exit_status(parsed.exit_status, &output),
        1
    );
}

#[test]
fn discovery_parent_command_requires_leaf_subcommand() {
    let error = parser::parse_cli_args(&["treease".to_string(), "operators".to_string()])
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
    let parsed = parser::parse_cli_args(&[
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
    let parsed = parser::parse_cli_args(&[
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
        let error = parser::parse_cli_args(&["treease".to_string(), command.to_string()])
            .expect_err("legacy eval command should fail");

        match error {
            CliError::UnknownCommand(name) => assert_eq!(name, command),
            other => panic!("unexpected error for {command}: {other:?}"),
        }
    }
}

#[test]
fn web_command_parses_expression_file_and_format_options() {
    let parsed = parser::parse_cli_args(&[
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
    let parsed = parser::parse_cli_args(&[
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
    let error = parser::parse_cli_args(&[
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
    let parsed =
        parser::parse_cli_args(&["treease".to_string(), ".".to_string(), "web".to_string()])
            .expect("root invocation should parse");

    assert_eq!(parsed.command, CommandKind::Run);
    assert_eq!(parsed.expression, ".");
    assert_eq!(parsed.files, vec!["web"]);
}

#[test]
fn web_command_rejects_root_execution_flags_that_do_not_apply() {
    let error = parser::parse_cli_args(&[
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
    let error = parser::parse_cli_args(&[
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
    let error = parser::parse_cli_args(&[
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
    let parsed = parser::parse_cli_args(&[
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
fn web_payload_maps_missing_expression_result_to_miss_scalar() {
    let parsed = parser::parse_cli_args(&[
        "treease".to_string(),
        "web".to_string(),
        ".missing".to_string(),
        "input.json".to_string(),
    ])
    .expect("web should parse");
    let inputs = vec![InputPayload {
        name: "input.json".to_string(),
        bytes: br#"{"foo":1}"#.to_vec(),
    }];

    let payload = web_payload::build_cli_graph_result_payload(&parsed, &inputs)
        .expect("web payload should be produced");

    assert_eq!(payload.language, "json");
    assert_eq!(payload.text, "\"miss\"");
}

#[test]
fn web_payload_supports_stdin_and_defaults_to_yaml() {
    let parsed = parser::parse_cli_args(&[
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
        let error = parser::parse_cli_args(&[
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
