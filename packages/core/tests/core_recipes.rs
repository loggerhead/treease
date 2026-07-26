// core_recipes.rs — recipes test aligned with tests/lib/recipes.zig
//
// The Zig test uses ExpressionScenario to test high-level recipe scenarios:
//   1. Find items in array: .[] | select(.name == "Foo")
//   2. Sort array by field: .myArray |= sort_by(.numBuckets)
//   3. Filter, flatten, sort and unique
//   4. Export as environment variables (simplified)
//
// In Rust, we use the expression_pipeline module which provides
// parse + execute in one step via `expression_pipeline::evaluate`.

use std::collections::BTreeMap;

use treease_core::evaluator::{Numeric, Value};
use treease_core::expression_pipeline;

const BASH_ENV_SCRIPT: &str = r#".[] |(
    ( select(kind == "scalar") | key + "='" + . + "'"),
    ( select(kind == "seq") | key + "=(" + (map("'" + . + "'") | join(",")) + ")")
)"#;

const NESTED_BASH_ENV_SCRIPT: &str = r#".. |(
    ( select(kind == "scalar" and parent | kind != "seq") | (path | join("_")) + "='" + . + "'"),
    ( select(kind == "seq") | (path | join("_")) + "=(" + (map("'" + . + "'") | join(",")) + ")")
)"#;

fn array_strings(value: Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .into_iter()
            .map(|item| match item {
                Value::String(value) => value,
                other => panic!("expected string item, got {other:?}"),
            })
            .collect::<Vec<_>>(),
        other => panic!("expected array result, got {other:?}"),
    }
}

fn sorted_strings(value: Value) -> Vec<String> {
    let mut out = array_strings(value);
    out.sort();
    out
}

// ── Test 1: Find items in an array ───────────────────────────────

#[test]
fn find_items_in_array() {
    // Document: [{name: Foo, numBuckets: 0}, {name: Bar, numBuckets: 0}]
    // Expression: .[] | select(.name == "Foo")
    // Expected: the item with name == "Foo"
    let input = Value::Array(vec![
        Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String("Foo".to_string())),
            ("numBuckets".to_string(), Value::Number(Numeric::Float(0.0))),
        ])),
        Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String("Bar".to_string())),
            ("numBuckets".to_string(), Value::Number(Numeric::Float(0.0))),
        ])),
    ]);

    let result = expression_pipeline::evaluate(&input, ".[] | select(.name == \"Foo\")")
        .expect("evaluation should succeed");

    assert_eq!(
        result,
        Value::Array(vec![Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String("Foo".to_string())),
            ("numBuckets".to_string(), Value::Number(Numeric::Float(0.0))),
        ]))])
    );
}

// ── Test 2: Sort array by field ──────────────────────────────────

#[test]
fn sort_array_by_field() {
    // Document: myArray: [{name: Foo, numBuckets: 1}, {name: Bar, numBuckets: 0}]
    // Expression: .myArray |= sort_by(.numBuckets)
    // Expected: myArray sorted by numBuckets ascending
    let input = Value::Object(BTreeMap::from([(
        "myArray".to_string(),
        Value::Array(vec![
            Value::Object(BTreeMap::from([
                ("name".to_string(), Value::String("Foo".to_string())),
                ("numBuckets".to_string(), Value::Number(Numeric::Float(1.0))),
            ])),
            Value::Object(BTreeMap::from([
                ("name".to_string(), Value::String("Bar".to_string())),
                ("numBuckets".to_string(), Value::Number(Numeric::Float(0.0))),
            ])),
        ]),
    )]));

    let result = expression_pipeline::evaluate(&input, ".myArray |= sort_by(.numBuckets)")
        .expect("evaluation should succeed");

    let expected = Value::Object(BTreeMap::from([(
        "myArray".to_string(),
        Value::Array(vec![
            Value::Object(BTreeMap::from([
                ("name".to_string(), Value::String("Bar".to_string())),
                ("numBuckets".to_string(), Value::Number(Numeric::Float(0.0))),
            ])),
            Value::Object(BTreeMap::from([
                ("name".to_string(), Value::String("Foo".to_string())),
                ("numBuckets".to_string(), Value::Number(Numeric::Float(1.0))),
            ])),
        ]),
    )]));

    assert_eq!(result, expected);
}

#[test]
fn update_items_in_array_increments_matching_bucket_count() {
    let input = Value::Array(vec![
        Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String("Foo".to_string())),
            ("numBuckets".to_string(), Value::Number(Numeric::Float(0.0))),
        ])),
        Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String("Bar".to_string())),
            ("numBuckets".to_string(), Value::Number(Numeric::Float(0.0))),
        ])),
    ]);

    let result = expression_pipeline::evaluate(
        &input,
        "(.[] | select(.name == \"Foo\") | .numBuckets) |= . + 1",
    )
    .expect("evaluation should succeed");

    assert_eq!(
        result,
        Value::Array(vec![
            Value::Object(BTreeMap::from([
                ("name".to_string(), Value::String("Foo".to_string())),
                ("numBuckets".to_string(), Value::Number(Numeric::Float(1.0))),
            ])),
            Value::Object(BTreeMap::from([
                ("name".to_string(), Value::String("Bar".to_string())),
                ("numBuckets".to_string(), Value::Number(Numeric::Float(0.0))),
            ])),
        ])
    );
}

#[test]
fn with_expression_updates_each_item_using_peer_fields() {
    let input = Value::Object(BTreeMap::from([(
        "myArray".to_string(),
        Value::Array(vec![
            Value::Object(BTreeMap::from([
                ("name".to_string(), Value::String("Foo".to_string())),
                ("type".to_string(), Value::String("cat".to_string())),
            ])),
            Value::Object(BTreeMap::from([
                ("name".to_string(), Value::String("Bar".to_string())),
                ("type".to_string(), Value::String("dog".to_string())),
            ])),
        ]),
    )]));

    let result =
        expression_pipeline::evaluate(&input, "with(.myArray[]; .name = .name + \" - \" + .type)")
            .expect("evaluation should succeed");

    assert_eq!(
        result,
        Value::Object(BTreeMap::from([(
            "myArray".to_string(),
            Value::Array(vec![
                Value::Object(BTreeMap::from([
                    ("name".to_string(), Value::String("Foo - cat".to_string())),
                    ("type".to_string(), Value::String("cat".to_string())),
                ])),
                Value::Object(BTreeMap::from([
                    ("name".to_string(), Value::String("Bar - dog".to_string())),
                    ("type".to_string(), Value::String("dog".to_string())),
                ])),
            ]),
        )]))
    );
}

// ── Test 3: Filter, flatten, sort and unique ─────────────────────

#[test]
fn filter_flatten_sort_and_unique() {
    // Document: [{type: foo, names: [Fred, Catherine]}, {type: bar, names: [Zelda]},
    //            {type: foo, names: Fred}, {type: foo, names: Ava}]
    // Expression: [.[] | select(.type == "foo") | .names] | flatten | sort | unique
    // Expected: [Ava, Catherine, Fred]
    let input = Value::Array(vec![
        Value::Object(BTreeMap::from([
            ("type".to_string(), Value::String("foo".to_string())),
            (
                "names".to_string(),
                Value::Array(vec![
                    Value::String("Fred".to_string()),
                    Value::String("Catherine".to_string()),
                ]),
            ),
        ])),
        Value::Object(BTreeMap::from([
            ("type".to_string(), Value::String("bar".to_string())),
            (
                "names".to_string(),
                Value::Array(vec![Value::String("Zelda".to_string())]),
            ),
        ])),
        Value::Object(BTreeMap::from([
            ("type".to_string(), Value::String("foo".to_string())),
            ("names".to_string(), Value::String("Fred".to_string())),
        ])),
        Value::Object(BTreeMap::from([
            ("type".to_string(), Value::String("foo".to_string())),
            ("names".to_string(), Value::String("Ava".to_string())),
        ])),
    ]);

    let result = expression_pipeline::evaluate(
        &input,
        "[.[] | select(.type == \"foo\") | .names] | flatten | sort | unique",
    )
    .expect("evaluation should succeed");

    let expected = Value::Array(vec![
        Value::String("Ava".to_string()),
        Value::String("Catherine".to_string()),
        Value::String("Fred".to_string()),
    ]);

    assert_eq!(result, expected);
}

// ── Test 4: Export as environment variables ───────────────────────

#[test]
fn export_as_environment_variables_script_formats_scalar_entries_exactly() {
    let input = Value::Object(BTreeMap::from([
        ("var0".to_string(), Value::String("string0".to_string())),
        ("var1".to_string(), Value::String("string1".to_string())),
        (
            "fruit".to_string(),
            Value::Array(vec![
                Value::String("apple".to_string()),
                Value::String("banana".to_string()),
                Value::String("peach".to_string()),
            ]),
        ),
    ]));

    let result = expression_pipeline::evaluate(
        &input,
        ".[] | select(kind == \"scalar\") | key + \"='\" + . + \"'\"",
    )
    .expect("scalar bash env formatting should evaluate");

    assert_eq!(
        array_strings(result),
        vec!["var0='string0'".to_string(), "var1='string1'".to_string()]
    );
}

#[test]
fn export_as_environment_variables_script_formats_sequence_entries_exactly() {
    let input = Value::Object(BTreeMap::from([
        ("var0".to_string(), Value::String("string0".to_string())),
        ("var1".to_string(), Value::String("string1".to_string())),
        (
            "fruit".to_string(),
            Value::Array(vec![
                Value::String("apple".to_string()),
                Value::String("banana".to_string()),
                Value::String("peach".to_string()),
            ]),
        ),
    ]));

    let result = expression_pipeline::evaluate(
        &input,
        ".[] | select(kind == \"seq\") | key + \"=(\" + (map(\"'\" + . + \"'\") | join(\",\")) + \")\"",
    )
    .expect("sequence bash env formatting should evaluate");

    assert_eq!(
        array_strings(result),
        vec!["fruit=('apple','banana','peach')".to_string()]
    );
}

#[test]
fn export_as_environment_variables_script_matches_recipe_outputs() {
    let input = Value::Object(BTreeMap::from([
        ("var0".to_string(), Value::String("string0".to_string())),
        ("var1".to_string(), Value::String("string1".to_string())),
        (
            "fruit".to_string(),
            Value::Array(vec![
                Value::String("apple".to_string()),
                Value::String("banana".to_string()),
                Value::String("peach".to_string()),
            ]),
        ),
    ]));

    let result = expression_pipeline::evaluate(&input, BASH_ENV_SCRIPT)
        .expect("bash env script should evaluate");

    assert_eq!(
        sorted_strings(result),
        vec![
            "fruit=('apple','banana','peach')".to_string(),
            "var0='string0'".to_string(),
            "var1='string1'".to_string(),
        ]
    );
}

#[test]
fn nested_custom_format_formats_scalar_paths_exactly() {
    let input = Value::Object(BTreeMap::from([
        ("simple".to_string(), Value::String("string0".to_string())),
        (
            "simpleArray".to_string(),
            Value::Array(vec![
                Value::String("apple".to_string()),
                Value::String("banana".to_string()),
                Value::String("peach".to_string()),
            ]),
        ),
        (
            "deep".to_string(),
            Value::Object(BTreeMap::from([
                ("property".to_string(), Value::String("value".to_string())),
                (
                    "array".to_string(),
                    Value::Array(vec![Value::String("cat".to_string())]),
                ),
            ])),
        ),
    ]));

    let result = expression_pipeline::evaluate(
        &input,
        ".. | select(kind == \"scalar\" and parent | kind != \"seq\") | (path | join(\"_\")) + \"='\" + . + \"'\"",
    )
    .expect("nested scalar custom formatting should evaluate");

    assert_eq!(
        sorted_strings(result),
        vec![
            "deep_property='value'".to_string(),
            "simple='string0'".to_string(),
        ]
    );
}

#[test]
fn nested_custom_format_formats_sequence_paths_exactly() {
    let input = Value::Object(BTreeMap::from([
        ("simple".to_string(), Value::String("string0".to_string())),
        (
            "simpleArray".to_string(),
            Value::Array(vec![
                Value::String("apple".to_string()),
                Value::String("banana".to_string()),
                Value::String("peach".to_string()),
            ]),
        ),
        (
            "deep".to_string(),
            Value::Object(BTreeMap::from([
                ("property".to_string(), Value::String("value".to_string())),
                (
                    "array".to_string(),
                    Value::Array(vec![Value::String("cat".to_string())]),
                ),
            ])),
        ),
    ]));

    let result = expression_pipeline::evaluate(
        &input,
        ".. | select(kind == \"seq\") | (path | join(\"_\")) + \"=(\" + (map(\"'\" + . + \"'\") | join(\",\")) + \")\"",
    )
    .expect("nested sequence custom formatting should evaluate");

    assert_eq!(
        sorted_strings(result),
        vec![
            "deep_array=('cat')".to_string(),
            "simpleArray=('apple','banana','peach')".to_string(),
        ]
    );
}

#[test]
fn nested_custom_format_matches_recipe_outputs() {
    let input = Value::Object(BTreeMap::from([
        ("simple".to_string(), Value::String("string0".to_string())),
        (
            "simpleArray".to_string(),
            Value::Array(vec![
                Value::String("apple".to_string()),
                Value::String("banana".to_string()),
                Value::String("peach".to_string()),
            ]),
        ),
        (
            "deep".to_string(),
            Value::Object(BTreeMap::from([
                ("property".to_string(), Value::String("value".to_string())),
                (
                    "array".to_string(),
                    Value::Array(vec![Value::String("cat".to_string())]),
                ),
            ])),
        ),
    ]));

    let result = expression_pipeline::evaluate(&input, NESTED_BASH_ENV_SCRIPT)
        .expect("nested custom format should evaluate");

    assert_eq!(
        sorted_strings(result),
        vec![
            "deep_array=('cat')".to_string(),
            "deep_property='value'".to_string(),
            "simple='string0'".to_string(),
            "simpleArray=('apple','banana','peach')".to_string(),
        ]
    );
}

// ── Test 5: Simple arithmetic expression ─────────────────────────

#[test]
fn simple_arithmetic_expression() {
    let result =
        expression_pipeline::evaluate(&Value::Null, "1 + 2").expect("evaluation should succeed");
    assert_eq!(result, Value::Number(Numeric::Int(3)));

    let result =
        expression_pipeline::evaluate(&Value::Null, "10 - 3").expect("evaluation should succeed");
    assert_eq!(result, Value::Number(Numeric::Int(7)));

    let result =
        expression_pipeline::evaluate(&Value::Null, "4 * 5").expect("evaluation should succeed");
    assert_eq!(result, Value::Number(Numeric::Int(20)));

    let result =
        expression_pipeline::evaluate(&Value::Null, "10 / 2").expect("evaluation should succeed");
    assert_eq!(result, Value::Number(Numeric::Int(5)));
}

// ── Test 6: String concatenation ─────────────────────────────────

#[test]
fn string_concatenation() {
    let input = Value::Object(BTreeMap::from([
        ("firstName".to_string(), Value::String("Ada".to_string())),
        (
            "lastName".to_string(),
            Value::String("Lovelace".to_string()),
        ),
    ]));

    let result = expression_pipeline::evaluate(&input, ".firstName + \" \" + .lastName")
        .expect("evaluation should succeed");

    assert_eq!(result, Value::String("Ada Lovelace".to_string()));
}

// ── Test 7: Map values transformation ────────────────────────────

#[test]
fn map_values_transformation() {
    let input = Value::Array(vec![
        Value::Number(Numeric::Float(1.0)),
        Value::Number(Numeric::Float(2.0)),
        Value::Number(Numeric::Float(3.0)),
    ]);

    let result =
        expression_pipeline::evaluate(&input, "map(. + 1)").expect("evaluation should succeed");

    assert_eq!(
        result,
        Value::Array(vec![
            Value::Number(Numeric::Float(2.0)),
            Value::Number(Numeric::Float(3.0)),
            Value::Number(Numeric::Float(4.0)),
        ])
    );
}

// ── Test 8: Self-reference (identity) ────────────────────────────

#[test]
fn self_reference_identity() {
    let input = Value::String("hello".to_string());

    let result = expression_pipeline::evaluate(&input, ".").expect("evaluation should succeed");

    assert_eq!(result, Value::String("hello".to_string()));
}
