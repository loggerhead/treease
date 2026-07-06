use std::io::Cursor;

use treease_core::core::SemType;
use treease_core::stream::{
    DecodeOptions, StreamKind, StreamingDecodeError, StreamingEvent, decode, decode_from_reader,
    decode_with_options, stream_kind,
};

const JSON_SPLIT_FIXTURE: &str =
    include_str!("../../../test/fixtures/json/graph-table-missing-row.1.json");

#[test]
fn streaming_json_emits_object_array_and_scalar_events() {
    let events = decode("json", r#"{"a":1,"b":[true,false,null]}"#).unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        StreamingEvent::MapKey { value, .. } if value == "b"
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, StreamingEvent::SeqStart(_)))
    );
    assert!(events.iter().any(|event| matches!(
        event,
        StreamingEvent::Scalar {
            meta,
            ..
        } if meta.sem_type == Some(SemType::Nil)
    )));
}

#[test]
fn streaming_json_keeps_nested_object_paths_on_scalar_events() {
    let events = decode_with_options(
        "json",
        r#"{"outer":{"inner":1,"tail":2}}"#,
        DecodeOptions {
            emit_path: true,
            ..DecodeOptions::default()
        },
    )
    .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        StreamingEvent::MapStart(meta) if meta.path == "$.outer"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        StreamingEvent::Scalar { value, meta, .. } if value == "2" && meta.path == "$.outer.tail"
    )));
}

#[test]
fn streaming_json_keeps_array_index_paths_on_scalar_events() {
    let events = decode_with_options(
        "json",
        r#"{"arr":["a","b"]}"#,
        DecodeOptions {
            emit_path: true,
            ..DecodeOptions::default()
        },
    )
    .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        StreamingEvent::Scalar { value, meta, .. } if value == "b" && meta.path == "$.arr[1]"
    )));
}

#[test]
fn streaming_json_singleton_array_keeps_container_events_and_nested_paths() {
    let events = decode_with_options(
        "json",
        r#"{"wrapper":[{"only":1}]}"#,
        DecodeOptions {
            emit_path: true,
            ..DecodeOptions::default()
        },
    )
    .unwrap();

    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingEvent::SeqStart(_)))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingEvent::SeqEnd(_)))
            .count(),
        1
    );
    assert!(events.iter().any(|event| matches!(
        event,
        StreamingEvent::Scalar { value, meta }
            if value == "1" && meta.path == "$.wrapper[0].only"
    )));
}

#[test]
fn streaming_json_rejects_trailing_comma() {
    let error = decode("json", r#"{"a":1,}"#).unwrap_err();

    assert!(matches!(error, StreamingDecodeError::Json(_)));
}

#[test]
fn streaming_json_rejects_malformed_number_forms() {
    for input in ["[2.]", "[-]"] {
        assert!(
            matches!(decode("json", input), Err(StreamingDecodeError::Json(_))),
            "expected malformed JSON number to fail: {input}"
        );
    }
}

#[test]
fn streaming_json_rejects_adjacent_top_level_documents() {
    let error = decode("json", "{\"a\":1}\n[2]").unwrap_err();

    assert!(matches!(error, StreamingDecodeError::Json(_)));
}

#[test]
fn streaming_json_decodes_escaped_strings_and_preserves_scalar_metadata() {
    let events = decode_with_options(
        "json",
        r#"{"msg":"line\nend\/ok"}"#,
        DecodeOptions {
            emit_path: true,
            ..DecodeOptions::default()
        },
    )
    .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        StreamingEvent::Scalar { value, meta }
            if value == "line\nend/ok"
                && meta.path == "$.msg"
                && meta.end_byte > meta.start_byte
                && meta.sem_type == Some(SemType::Str)
    )));
}

#[test]
fn streaming_json_tracks_nested_array_scalar_paths_and_semantic_types() {
    let events = decode_with_options(
        "json",
        r#"{"items":[0,1.5,true]}"#,
        DecodeOptions {
            emit_path: true,
            ..DecodeOptions::default()
        },
    )
    .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        StreamingEvent::Scalar { value, meta }
            if value == "0" && meta.path == "$.items[0]" && meta.sem_type == Some(SemType::Int)
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        StreamingEvent::Scalar { value, meta }
            if value == "1.5" && meta.path == "$.items[1]" && meta.sem_type == Some(SemType::Float)
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        StreamingEvent::Scalar { value, meta }
            if value == "true" && meta.path == "$.items[2]" && meta.sem_type == Some(SemType::Boolean)
    )));
}

#[test]
fn streaming_json_split_documents_accepts_single_fixture_document() {
    let ranges = treease_core::stream::streaming_json::split_documents(JSON_SPLIT_FIXTURE)
        .expect("fixture should be recognized as a single JSON document");

    assert_eq!(ranges.len(), 1);
    let range = ranges[0];
    assert_eq!(
        &JSON_SPLIT_FIXTURE[range.start..range.end],
        JSON_SPLIT_FIXTURE.trim()
    );
}

#[test]
fn streaming_json_token_spans_classify_object_keys_and_strings() {
    let spans = treease_core::stream::streaming_json::token_spans(r#"{"k":"v","arr":[{"x":"y"}]}"#)
        .expect("token span scan should succeed");

    let property_count = spans.iter().filter(|span| span.token_type == 1).count();
    let string_count = spans.iter().filter(|span| span.token_type == 3).count();

    assert_eq!(property_count, 3);
    assert_eq!(string_count, 2);
}

#[test]
fn streaming_json_token_spans_keep_array_strings_as_strings() {
    let spans = treease_core::stream::streaming_json::token_spans(r#"["a",{"b":"c"}]"#)
        .expect("token span scan should succeed");

    let property_count = spans.iter().filter(|span| span.token_type == 1).count();
    let string_count = spans.iter().filter(|span| span.token_type == 3).count();

    assert_eq!(property_count, 1);
    assert_eq!(string_count, 2);
}

#[test]
fn streaming_json_first_parse_failure_reports_offsets() {
    let failure =
        treease_core::stream::streaming_json::first_parse_failure(r#"{"a":1,,"b":2}"#, false)
            .expect("invalid JSON should report first parse failure");

    assert!(failure.start_byte > 0);
    assert!(failure.end_byte >= failure.start_byte);
    assert!(failure.line >= 1);
    assert!(failure.column >= 1);
}

#[test]
fn streaming_json_decode_slice_to_tree_builds_mapping_document() {
    let decoded =
        treease_core::stream::streaming_json::decode_slice_to_tree(r#"{"outer":{"inner":1}}"#)
            .expect("decode_slice_to_tree should produce a document tree");
    let root = decoded.store.get(decoded.root).unwrap();

    assert_eq!(root.kind, treease_core::core::TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 2);
    let outer_value = decoded.store.get(root.content[1]).unwrap();
    assert_eq!(decoded.store.value_for(root.content[0]).unwrap(), "outer");
    assert_eq!(outer_value.kind, treease_core::core::TreeNodeKind::Mapping);
}

#[test]
fn streaming_decoder_routes_supported_languages_and_rejects_others() {
    assert_eq!(stream_kind("json"), StreamKind::Json);
    assert_eq!(stream_kind("yaml"), StreamKind::NonStreaming);

    let error = decode("yaml", "a: 1").unwrap_err();
    assert_eq!(
        error,
        StreamingDecodeError::UnsupportedLanguage("yaml".to_string())
    );
}

#[test]
fn streaming_decoder_reader_wrapper_rejects_non_streaming_languages() {
    let mut reader = Cursor::new(b"a: 1\n".to_vec());

    let error = decode_from_reader("yaml", &mut reader).unwrap_err();

    assert_eq!(
        error,
        StreamingDecodeError::UnsupportedLanguage("yaml".to_string())
    );
}

// =============================================================================
// Streaming JSON chunked-feed / edge-case tests (ported from streaming_json.zig)
// =============================================================================

#[test]
fn streaming_json_builds_tree_for_object_and_array_with_chunked_feed() {
    let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);
    let input = r#"{"a":1,"b":[true,false,null]}"#;
    decoder.feed(&input[..5]).unwrap();
    decoder.feed(&input[5..]).unwrap();
    let doc = decoder
        .finish()
        .expect("chunked decode should produce a document tree");

    let root = doc.store.get(doc.root).unwrap();
    assert_eq!(root.kind, treease_core::core::TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 4);
}

#[test]
fn streaming_json_emits_error_on_invalid_input_via_stream_decoder() {
    let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);
    let input = r#"{"a":1,,"b":2}"#;
    decoder.feed(input).unwrap();
    let result = decoder.finish();
    assert!(result.is_err());
}

#[test]
fn streaming_json_parser_emits_events_for_array() {
    let mut parser = treease_core::stream::streaming_json::StreamingParser::new(false);
    parser.feed("[1,2]").unwrap();
    let events = parser.finish().expect("array parse should succeed");
    assert!(!events.is_empty());
}

#[test]
fn streaming_json_keeps_object_paths_stable_after_scanner_compaction() {
    // Use a large (~140KB) string value in the middle of a JSON object to
    // trigger scanner buffer compaction.  The default scanner compact
    // threshold is 64 KB, so 140 KB forces at least one compaction pass.
    let mut parser =
        treease_core::stream::streaming_json::StreamingParser::with_path_emission(false, true);

    let prefix = r#"{"outer":{"inner":""#;
    let suffix = r#"","tail":1}}"#;
    let large_len: usize = 140 * 1024;
    let large: String = "a".repeat(large_len);

    let mut input = String::with_capacity(prefix.len() + large.len() + suffix.len());
    input.push_str(prefix);
    input.push_str(&large);
    input.push_str(suffix);

    let split1 = prefix.len() + large_len / 2;
    let split2 = prefix.len() + large_len;

    parser.feed(&input[..split1]).unwrap();
    parser.feed(&input[split1..split2]).unwrap();
    parser.feed(&input[split2..]).unwrap();
    let events = parser
        .finish()
        .expect("large chunked decode with paths should succeed");

    let has_nested_map_path = events.iter().any(|event| {
        matches!(
            event,
            StreamingEvent::MapStart(meta) if meta.path == "$.outer"
        )
    });
    let has_tail_key_path = events.iter().any(|event| {
        matches!(
            event,
            StreamingEvent::MapKey { meta, .. } if meta.path == "$.outer.tail"
        )
    });
    let has_tail_scalar_path = events.iter().any(|event| {
        matches!(
            event,
            StreamingEvent::Scalar { meta, .. } if meta.path == "$.outer.tail"
        )
    });

    assert!(has_nested_map_path);
    assert!(has_tail_key_path);
    assert!(has_tail_scalar_path);
}

#[test]
fn streaming_json_keeps_nested_json_paths_stable() {
    let mut parser =
        treease_core::stream::streaming_json::StreamingParser::with_path_emission(true, true);

    let input = r#"{"outer":"{\"inner\":1,\"tail\":2}"}"#;
    parser.feed(&input[..12]).unwrap();
    parser.feed(&input[12..]).unwrap();
    let events = parser
        .finish()
        .expect("nested json path decode should succeed");

    assert!(
        parser.nested_json_expanded(),
        "nest_json=true should mark nested source expansion when nested content materializes"
    );
    let rewrites = parser.take_source_rewrites();
    assert!(
        rewrites.len() == 1 && rewrites[0].replacement == r#"{"inner":1,"tail":2}"#,
        "nest_json=true should produce one rewrite for the expanded nested JSON, got {rewrites:?}"
    );

    let has_nested_map_path = events.iter().any(|event| {
        matches!(
            event,
            StreamingEvent::MapStart(meta) if meta.path == "$.outer"
        )
    });
    assert!(has_nested_map_path, "expected nested MapStart at $.outer");

    let nested_paths: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            StreamingEvent::MapStart(m) => Some(m.path.clone()),
            StreamingEvent::MapKey { meta, .. } => Some(meta.path.clone()),
            StreamingEvent::Scalar { meta, .. } => Some(meta.path.clone()),
            _ => None,
        })
        .filter(|p| !p.is_empty())
        .collect();

    assert!(
        nested_paths.iter().any(|p| p == "$.outer.tail"),
        "nested expansion should expose the inner tail path, got: {nested_paths:?}"
    );
}

#[test]
fn streaming_json_split_fixture_emits_no_parse_errors_with_path_emission() {
    let mut parser =
        treease_core::stream::streaming_json::StreamingParser::with_path_emission(true, true);

    let split_at = JSON_SPLIT_FIXTURE.len() / 2;
    parser.feed(&JSON_SPLIT_FIXTURE[..split_at]).unwrap();
    parser.feed(&JSON_SPLIT_FIXTURE[split_at..]).unwrap();
    let events = parser
        .finish()
        .expect("split fixture should parse without error");

    let parse_error_count = events
        .iter()
        .filter(|event| matches!(event, StreamingEvent::ParseError { .. }))
        .count();
    assert_eq!(parse_error_count, 0);
}

/// Helper: find the byte index that corresponds to the midpoint of the
/// input when counted in UTF-16 code units (JavaScript `.length`).
fn utf16_half_byte_index(s: &str) -> usize {
    let mut utf16_units: usize = 0;
    for ch in s.chars() {
        utf16_units += ch.len_utf16();
    }
    let half = utf16_units / 2;
    let mut count: usize = 0;
    let mut byte_offset: usize = 0;
    for ch in s.chars() {
        let units = ch.len_utf16();
        if count + units > half {
            return byte_offset;
        }
        count += units;
        byte_offset += ch.len_utf8();
    }
    byte_offset
}

#[test]
fn streaming_json_web_split_fixture_emits_no_parse_errors_with_path_emission() {
    let mut parser =
        treease_core::stream::streaming_json::StreamingParser::with_path_emission(true, true);

    let split_at = utf16_half_byte_index(JSON_SPLIT_FIXTURE);
    parser.feed(&JSON_SPLIT_FIXTURE[..split_at]).unwrap();
    parser.feed(&JSON_SPLIT_FIXTURE[split_at..]).unwrap();
    let events = parser
        .finish()
        .expect("web split fixture should parse without error");

    let parse_error_count = events
        .iter()
        .filter(|event| matches!(event, StreamingEvent::ParseError { .. }))
        .count();
    assert_eq!(parse_error_count, 0);
}

#[test]
fn streaming_json_rejects_comments() {
    let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);
    decoder.feed("//a\n{\"a\":1,/*b*/\"b\":2,}").unwrap();
    assert!(decoder.finish().is_err());
}

#[test]
fn streaming_json_rejects_unescaped_control_chars_in_strings() {
    for input in ["[\"new\nline\"]", "[\"tab\tchar\"]", "[\"ctrl\x01char\"]"] {
        let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);
        decoder.feed(input).unwrap();
        assert!(
            decoder.finish().is_err(),
            "expected unescaped control char to fail: {input:?}"
        );
    }
}

#[test]
fn streaming_json_rejects_invalid_utf8_bytes_in_strings() {
    let cases: &[&[u8]] = &[b"[\"bad \x80 utf8\"]", b"\"bad \xb1\x87 utf8\""];

    for bytes in cases {
        // Use the byte-level parser API to test invalid UTF-8.
        // decode_from_bytes validates UTF-8 and rejects it before parsing.
        let result = treease_core::stream::decode_from_bytes("json", bytes);
        // Invalid UTF-8 is caught by the byte-level validator.
        // If it slips past, the JSON parser should catch it too.
        assert!(
            result.is_err(),
            "expected invalid UTF-8 bytes to fail: {bytes:?}"
        );
    }
}

#[test]
fn streaming_json_handles_split_string_token_across_chunks() {
    let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);

    decoder.feed("{\"a\":\"").unwrap();
    decoder.feed("he").unwrap();
    decoder.feed("llo\"}").unwrap();

    let doc = decoder
        .finish()
        .expect("split string across chunks should succeed");
    let root = doc.store.get(doc.root).unwrap();
    assert_eq!(root.kind, treease_core::core::TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 2);
    let key_node = doc.store.get(root.content[0]).unwrap();
    let value_node = doc.store.get(root.content[1]).unwrap();
    assert_eq!(key_node.kind, treease_core::core::TreeNodeKind::Scalar);
    assert_eq!(value_node.kind, treease_core::core::TreeNodeKind::Scalar);
}

#[test]
fn streaming_json_ignores_empty_chunks() {
    let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);

    decoder.feed("").unwrap();
    decoder.feed("{").unwrap();
    decoder.feed("").unwrap();
    decoder.feed("\"x\":1}").unwrap();
    decoder.feed("").unwrap();

    let doc = decoder.finish().expect("empty chunks should be ignored");
    let root = doc.store.get(doc.root).unwrap();
    assert_eq!(root.kind, treease_core::core::TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 2);
    let key_node = doc.store.get(root.content[0]).unwrap();
    let value_node = doc.store.get(root.content[1]).unwrap();
    assert_eq!(key_node.kind, treease_core::core::TreeNodeKind::Scalar);
    assert_eq!(value_node.kind, treease_core::core::TreeNodeKind::Scalar);
}

#[test]
fn streaming_json_accepts_escape_and_unicode_sequences_split_across_chunks() {
    let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);

    decoder.feed("{\"msg\":\"line\\").unwrap();
    decoder.feed("n\\u4f").unwrap();
    decoder.feed("60\\tend\"}").unwrap();

    let doc = decoder
        .finish()
        .expect("split escape sequences across chunks should succeed");
    let root = doc.store.get(doc.root).unwrap();
    assert_eq!(root.kind, treease_core::core::TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 2);
    assert_eq!(
        doc.store.get(root.content[1]).unwrap().kind,
        treease_core::core::TreeNodeKind::Scalar
    );
    assert!(!doc.store.value_for(root.content[1]).unwrap().is_empty());
}

#[test]
fn streaming_json_accepts_surrogate_pair_escapes() {
    let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);

    decoder.feed("{\"msg\":\"\\uD801\\udc37\"}").unwrap();

    let doc = decoder
        .finish()
        .expect("surrogate pair should decode successfully");
    let root = doc.store.get(doc.root).unwrap();
    assert_eq!(root.kind, treease_core::core::TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 2);
    assert_eq!(
        doc.store.get(root.content[1]).unwrap().kind,
        treease_core::core::TreeNodeKind::Scalar
    );
    assert!(!doc.store.value_for(root.content[1]).unwrap().is_empty());
}

#[test]
fn streaming_json_accepts_number_token_split_at_exponent_boundary() {
    let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);

    decoder.feed("{\"n\":-12.3e").unwrap();
    decoder.feed("+4}").unwrap();

    let doc = decoder
        .finish()
        .expect("split number token at exponent should succeed");
    let root = doc.store.get(doc.root).unwrap();
    assert_eq!(root.kind, treease_core::core::TreeNodeKind::Mapping);
    assert_eq!(root.content.len(), 2);
    assert_eq!(
        doc.store.get(root.content[1]).unwrap().kind,
        treease_core::core::TreeNodeKind::Scalar
    );
    assert!(!doc.store.value_for(root.content[1]).unwrap().is_empty());
}

#[test]
fn streaming_json_reports_invalid_unicode_escape_across_chunk_boundary() {
    let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);

    decoder.feed("{\"msg\":\"\\u12").unwrap();
    decoder.feed("G4\"}").unwrap();

    assert!(decoder.finish().is_err());
}

#[test]
fn stream_decoder_exposes_token_spans_for_close_reuse() {
    let mut decoder = treease_core::stream::streaming_json::StreamDecoder::new(false);
    decoder
        .feed(r#"{"a":"one","b":2}"#)
        .expect("feed should succeed");
    decoder.finish().expect("finish should succeed");

    let spans = decoder.take_token_spans();
    assert!(
        spans.iter().any(|span| span.token_type == 1),
        "object keys should be collected"
    );
    assert!(
        spans.iter().any(|span| span.token_type == 3),
        "string values should be collected"
    );
}
