use std::collections::BTreeMap;
use std::io::Cursor;

use treease_core::core::{
    CodecService, Context, CoreError, Encoder, NodeId, ParseError, Printer, PrinterWriter,
    RegistryHandle, format_string_from_filename, io_adapters::reader_from_pointer,
};
use treease_core::evaluator::{
    AllAtOnceEvaluator, EvaluationError, ReaderInput, StreamEvaluator, Value,
};
use treease_core::parser::parse_expression;

#[derive(Default)]
struct NullWriter;

impl PrinterWriter for NullWriter {
    fn write_for_node(
        &mut self,
        _ctx: &mut Context,
        _node: Option<NodeId>,
        _bytes: &[u8],
    ) -> Result<(), CoreError> {
        Ok(())
    }
}

struct NullEncoder;

impl Encoder for NullEncoder {
    fn encode(
        &self,
        _ctx: &mut Context,
        _node: NodeId,
        _writer: &mut dyn std::io::Write,
    ) -> Result<(), CoreError> {
        Ok(())
    }
}

fn evaluate_readers_values(
    expression_source: Option<&str>,
    inputs: &mut [ReaderInput<'_>],
) -> Result<Vec<Value>, EvaluationError> {
    let codec = CodecService::new();
    let evaluator = AllAtOnceEvaluator::new();
    let expression = expression_source
        .map(parse_expression)
        .transpose()
        .map_err(|error| EvaluationError::UnsupportedOperation(format!("{error:?}")))?
        .flatten();
    let expression = expression.as_deref();
    let mut results = Vec::new();

    for input in inputs {
        let bytes = input.reader.read_all()?;
        let source = String::from_utf8(bytes).map_err(|_| {
            EvaluationError::Core(CoreError::System(
                treease_core::core::SystemError::InvalidUtf8,
            ))
        })?;
        if source.trim().is_empty() {
            continue;
        }

        let decoded = codec.decode(format_string_from_filename(input.name), &source)?;
        let mut values = evaluator.evaluate_nodes(
            &decoded.store,
            expression_source.unwrap_or("."),
            &[decoded.root],
        )?;
        results.append(&mut values);
    }

    if results.is_empty() {
        return evaluator.evaluate_many(&[Value::Null], expression);
    }

    Ok(results)
}

#[test]
fn stream_evaluator_should_fallback_to_evaluate_new_when_inputs_are_empty() {
    let mut inputs: [ReaderInput<'_>; 0] = [];

    let results = evaluate_readers_values(Some("self // 42"), &mut inputs)
        .expect("empty inputs should fall back to null evaluation");

    assert_eq!(results, vec![Value::Number(42.0)]);
}

#[test]
fn stream_evaluator_should_treat_empty_sources_as_zero_documents() {
    let mut empty_reader = Cursor::new(Vec::<u8>::new());
    let mut inputs = [ReaderInput::new(
        "empty.yaml",
        reader_from_pointer(&mut empty_reader),
    )];

    let results = evaluate_readers_values(Some("self // 42"), &mut inputs)
        .expect("empty source should fall back to null evaluation");

    assert_eq!(results, vec![Value::Number(42.0)]);
}

#[test]
fn stream_evaluator_should_skip_empty_sources_when_other_documents_exist() {
    let mut empty_reader = Cursor::new(Vec::<u8>::new());
    let mut json_reader = Cursor::new(br#"{"foo": 7}"#.to_vec());
    let mut inputs = [
        ReaderInput::new("empty.json", reader_from_pointer(&mut empty_reader)),
        ReaderInput::new("in.json", reader_from_pointer(&mut json_reader)),
    ];

    let results =
        evaluate_readers_values(Some(".foo"), &mut inputs).expect("empty source should be skipped");

    assert_eq!(results, vec![Value::Number(7.0)]);
}

#[test]
fn stream_evaluator_should_preserve_input_order_across_formats() {
    let mut yaml_reader = Cursor::new(b"a: 1\n".to_vec());
    let mut json_reader = Cursor::new(br#"{"a":2}"#.to_vec());
    let mut inputs = [
        ReaderInput::new("first.yaml", reader_from_pointer(&mut yaml_reader)),
        ReaderInput::new("second.json", reader_from_pointer(&mut json_reader)),
    ];

    let results = evaluate_readers_values(Some(".a"), &mut inputs)
        .expect("mixed formats should evaluate in order");

    assert_eq!(results, vec![Value::Number(1.0), Value::Number(2.0)]);
}

#[test]
fn stream_evaluator_should_propagate_decode_errors_from_reader_inputs() {
    let mut invalid_reader = Cursor::new(b"{".to_vec());
    let mut inputs = [ReaderInput::new(
        "broken.json",
        reader_from_pointer(&mut invalid_reader),
    )];

    let error = evaluate_readers_values(None, &mut inputs).expect_err("invalid json should fail");

    assert_eq!(
        error,
        EvaluationError::Core(CoreError::Parse(ParseError::InvalidJson))
    );
}

#[test]
fn stream_evaluator_should_use_real_reader_pipeline_for_empty_inputs() {
    let mut evaluator = StreamEvaluator::new();
    let mut ctx = Context::empty(RegistryHandle::default());
    let mut printer = Printer::new(NullEncoder, NullWriter);
    let mut inputs: [ReaderInput<'_>; 0] = [];

    evaluator
        .evaluate_readers(&mut ctx, "self // 42", &mut inputs, &mut printer)
        .expect("empty inputs should fall back through the real stream evaluator path");

    assert!(printer.printed_anything());
}

#[test]
fn stream_evaluator_no_input_keeps_empty_object_as_object() {
    let mut inputs: [ReaderInput<'_>; 0] = [];

    let results =
        evaluate_readers_values(Some("{}"), &mut inputs).expect("empty object should evaluate");

    assert_eq!(results, vec![Value::Object(BTreeMap::new())]);
}

#[test]
fn stream_evaluator_no_input_supports_object_literals() {
    let mut inputs: [ReaderInput<'_>; 0] = [];
    let mut expected = BTreeMap::new();
    expected.insert("wrap".to_string(), Value::String("frog".to_string()));

    let results = evaluate_readers_values(Some(r#"{"wrap": "frog"}"#), &mut inputs)
        .expect("object literal should evaluate");

    assert_eq!(results, vec![Value::Object(expected)]);
}

#[test]
fn stream_evaluator_no_input_assignment_pipeline_returns_object_not_array() {
    let mut inputs: [ReaderInput<'_>; 0] = [];
    let mut inner_a = BTreeMap::new();
    inner_a.insert("b".to_string(), Value::String("foo".to_string()));
    let mut inner_d = BTreeMap::new();
    inner_d.insert("e".to_string(), Value::String("bar".to_string()));
    let mut expected = BTreeMap::new();
    expected.insert("a".to_string(), Value::Object(inner_a));
    expected.insert("d".to_string(), Value::Object(inner_d));

    let results = evaluate_readers_values(Some(r#"(.a.b = "foo") | (.d.e = "bar")"#), &mut inputs)
        .expect("assignment pipeline should evaluate");

    assert_eq!(results, vec![Value::Object(expected)]);
}

#[test]
fn stream_evaluator_should_process_real_reader_inputs_without_leaking_state() {
    let mut evaluator = StreamEvaluator::new();
    let mut ctx = Context::empty(RegistryHandle::default());
    let mut printer = Printer::new(NullEncoder, NullWriter);
    let mut json_reader = Cursor::new(br#"{"foo":7}"#.to_vec());
    let mut inputs = [ReaderInput::new(
        "in.json",
        reader_from_pointer(&mut json_reader),
    )];

    evaluator
        .evaluate_readers(&mut ctx, ".foo", &mut inputs, &mut printer)
        .expect("real stream evaluator reader path should succeed on JSON input");

    assert!(printer.printed_anything());
    assert_eq!(evaluator.file_index, 1);
}

#[test]
fn stream_evaluator_should_isolate_per_file_mutations_and_observable_output() {
    let mut first_reader = Cursor::new(br#"[{"id":1}]"#.to_vec());
    let mut second_reader = Cursor::new(br#"[{"id":2}]"#.to_vec());
    let mut inputs = [
        ReaderInput::new("first.json", reader_from_pointer(&mut first_reader)),
        ReaderInput::new("second.json", reader_from_pointer(&mut second_reader)),
    ];

    assert_eq!(
        evaluate_readers_values(Some(".[] |= (.foo = \"bar\")"), &mut inputs)
            .expect("per-file mutation should succeed without leaking state across inputs"),
        vec![
            Value::Object(std::collections::BTreeMap::from([
                ("foo".to_string(), Value::String("bar".to_string())),
                ("id".to_string(), Value::Number(1.0)),
            ])),
            Value::Object(std::collections::BTreeMap::from([
                ("foo".to_string(), Value::String("bar".to_string())),
                ("id".to_string(), Value::Number(2.0)),
            ])),
        ]
    );
}

#[test]
fn stream_evaluator_should_accept_single_reader_mutation_pipeline_from_zig_port() {
    let mut evaluator = StreamEvaluator::new();
    let mut ctx = Context::empty(RegistryHandle::default());
    let mut printer = Printer::new(NullEncoder, NullWriter);
    let mut json_reader = Cursor::new(br#"["foo","baz"]"#.to_vec());
    let mut inputs = [ReaderInput::new(
        "in.json",
        reader_from_pointer(&mut json_reader),
    )];

    evaluator
        .evaluate_readers(
            &mut ctx,
            ".[] |= (.foo = \"bar\")",
            &mut inputs,
            &mut printer,
        )
        .expect("single-reader mutation pipeline should evaluate successfully");

    assert!(printer.printed_anything());
    assert_eq!(evaluator.file_index, 1);
}
