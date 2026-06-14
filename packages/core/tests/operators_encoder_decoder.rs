use treease_core::core::OperationPreferences as CoreOperationPreferences;
use treease_core::core::expression::OperationId;
use treease_core::operators::encoder_decoder::{op_decode, op_encode};
use treease_core::operators::{
    Context, CoreError, DECODE_OP_TYPE, Diagnostics, ENCODE_OP_TYPE, ExpressionNode, NodeId,
    NodeKind, Operation, OperationPreference, SemType, TreeEngine, TreeNode,
};
use treease_core::operators::{
    DecoderPreferences as CompatDecoderPreferences, EncoderPreferences as CompatEncoderPreferences,
};
use treease_core::parser::{TokenKind, lex_participle};

fn encode_expression(format: &str, indent: i32) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &ENCODE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Encoder(
                CompatEncoderPreferences {
                    format: format.to_owned(),
                    indent,
                    unwrap_scalar: false,
                },
            ))),
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

fn decode_expression(format: &str) -> ExpressionNode {
    ExpressionNode {
        operation: Box::new(Operation {
            operation_type: &DECODE_OP_TYPE,
            value: None,
            string_value: String::new(),
            tree_node: None,
            preferences: Some(Box::new(OperationPreference::Decoder(
                CompatDecoderPreferences {
                    format: format.to_owned(),
                },
            ))),
            update_assign: false,
        }),
        lhs: None,
        rhs: None,
    }
}

fn scalar(sem_type: SemType, value: &str) -> TreeNode {
    TreeNode {
        kind: NodeKind::Scalar,
        sem_type: Some(sem_type),
        tag: sem_type.tag().to_owned(),
        value: value.to_owned(),
        ..TreeNode::default()
    }
}

fn mapping(entries: Vec<(&str, TreeNode)>) -> TreeNode {
    let mut content = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        content.push(scalar(SemType::Str, key));
        content.push(value);
    }
    TreeNode {
        kind: NodeKind::Mapping,
        sem_type: Some(SemType::Map),
        tag: SemType::Map.tag().to_owned(),
        content,
        ..TreeNode::default()
    }
}

#[test]
fn encoder_decoder_encode_operator_emits_compact_json_without_trailing_newline() {
    let ctx = Context {
        matching_nodes: vec![mapping(vec![("name", scalar(SemType::Str, "Ada"))])],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = encode_expression("json", 0);

    let out = op_encode(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Scalar);
    assert_eq!(out.matching_nodes[0].value, "{\"name\":\"Ada\"}");
}

#[test]
fn encoder_decoder_decode_operator_decodes_yaml_and_preserves_metadata() {
    let candidate = TreeNode {
        key: Some(NodeId(7)),
        parent: Some(NodeId(9)),
        sequence_index: Some(3),
        ..scalar(SemType::Str, "name: Ada\n")
    };
    let ctx = Context {
        matching_nodes: vec![candidate],
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut expr = decode_expression("yaml");

    let out = op_decode(ctx, &mut engine, &mut expr).unwrap();

    assert_eq!(out.matching_nodes.len(), 1);
    assert_eq!(out.matching_nodes[0].kind, NodeKind::Mapping);
    assert_eq!(out.matching_nodes[0].parent, Some(NodeId(9)));
    assert_eq!(out.matching_nodes[0].key, Some(NodeId(7)));
    assert_eq!(out.matching_nodes[0].sequence_index, Some(3));
    assert_eq!(out.matching_nodes[0].content[0].value, "name");
    assert_eq!(out.matching_nodes[0].content[1].value, "Ada");
}

#[test]
fn encoder_decoder_unknown_format_returns_error_and_sets_diagnostic() {
    let ctx = Context {
        matching_nodes: vec![scalar(SemType::Str, "Ada")],
        diagnostics: Some(Box::new(Diagnostics)),
        ..Context::default()
    };
    let mut engine = TreeEngine::default();
    let mut encode_expr = encode_expression("unknown-format", 0);
    let mut decode_expr = decode_expression("unknown-format");

    let encode_err = op_encode(ctx.clone(), &mut engine, &mut encode_expr).unwrap_err();
    let decode_err = op_decode(ctx, &mut engine, &mut decode_expr).unwrap_err();

    assert!(matches!(
        encode_err,
        CoreError::Format(treease_core::operators::FormatError::UnknownFormat)
    ));
    assert!(matches!(
        decode_err,
        CoreError::Format(treease_core::operators::FormatError::UnknownFormat)
    ));
}

#[test]
fn lexer_parses_encode_decode_shorthand_forms() {
    // @json → encode, format=json, indent=0
    let tokens = lex_participle("@json").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Encode);
            assert_eq!(op.string_value, "@json");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Encoder(prefs)) => {
                    assert_eq!(prefs.format, "json");
                    assert_eq!(prefs.indent, 0);
                }
                other => panic!("expected encoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }

    // @yamld → decode, format=yaml
    let tokens = lex_participle("@yamld").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Decode);
            assert_eq!(op.string_value, "@yamld");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Decoder(prefs)) => {
                    assert_eq!(prefs.format, "yaml");
                }
                other => panic!("expected decoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }

    // @toml → encode, format=toml, indent=2
    let tokens = lex_participle("@toml").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Encode);
            assert_eq!(op.string_value, "@toml");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Encoder(prefs)) => {
                    assert_eq!(prefs.format, "toml");
                    assert_eq!(prefs.indent, 2);
                }
                other => panic!("expected encoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }

    // @tomld → decode, format=toml
    let tokens = lex_participle("@tomld").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Decode);
            assert_eq!(op.string_value, "@tomld");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Decoder(prefs)) => {
                    assert_eq!(prefs.format, "toml");
                }
                other => panic!("expected decoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }

    // @urid → decode, format=uri
    let tokens = lex_participle("@urid").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Decode);
            assert_eq!(op.string_value, "@urid");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Decoder(prefs)) => {
                    assert_eq!(prefs.format, "uri");
                }
                other => panic!("expected decoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }
}

#[test]
fn lexer_parses_encode_decode_function_forms() {
    // to_yaml → encode, format=yaml, indent=2
    let tokens = lex_participle("to_yaml").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Encode);
            assert_eq!(op.string_value, "to_yaml");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Encoder(prefs)) => {
                    assert_eq!(prefs.format, "yaml");
                    assert_eq!(prefs.indent, 2);
                }
                other => panic!("expected encoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }

    // to_json(0) → encode, format=json, indent=0
    // Zig keeps the operation string_value as the base function name.
    let tokens = lex_participle("to_json(0)").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Encode);
            assert_eq!(op.string_value, "to_json");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Encoder(prefs)) => {
                    assert_eq!(prefs.format, "json");
                    assert_eq!(prefs.indent, 0);
                }
                other => panic!("expected encoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }

    // to_toml → encode, format=toml, indent=2
    let tokens = lex_participle("to_toml").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Encode);
            assert_eq!(op.string_value, "to_toml");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Encoder(prefs)) => {
                    assert_eq!(prefs.format, "toml");
                    assert_eq!(prefs.indent, 2);
                }
                other => panic!("expected encoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }

    // to_toml(0) → encode, format=toml, indent=0
    // Zig keeps the operation string_value as the base function name.
    let tokens = lex_participle("to_toml(0)").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Encode);
            assert_eq!(op.string_value, "to_toml");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Encoder(prefs)) => {
                    assert_eq!(prefs.format, "toml");
                    assert_eq!(prefs.indent, 0);
                }
                other => panic!("expected encoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }

    // from_json → decode, format=yaml
    let tokens = lex_participle("from_json").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Decode);
            assert_eq!(op.string_value, "from_json");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Decoder(prefs)) => {
                    assert_eq!(prefs.format, "yaml");
                }
                other => panic!("expected decoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }

    // from_csv → decode, format=csv
    let tokens = lex_participle("from_csv").unwrap();
    assert_eq!(tokens.len(), 1);
    match &tokens[0].kind {
        TokenKind::Operation(op) => {
            assert_eq!(op.operation_type.id, OperationId::Decode);
            assert_eq!(op.string_value, "from_csv");
            match op.preferences.as_deref() {
                Some(CoreOperationPreferences::Decoder(prefs)) => {
                    assert_eq!(prefs.format, "csv");
                }
                other => panic!("expected decoder prefs, got {other:?}"),
            }
        }
        _ => panic!("expected operation token"),
    }
}
