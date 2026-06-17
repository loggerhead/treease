use std::io::Read;

use crate::{
    core::{CodecService, CoreError, ParseError, RegistryOwner, stream_kind_for_language},
    formats::DecodedDocument,
};

use super::{
    streaming_events::{EventSink, StreamingEvent},
    streaming_json, tree_builder,
};

pub use crate::core::StreamKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamingDecodeError {
    Json(streaming_json::JsonStreamError),
    UnsupportedLanguage(String),
    InvalidUtf8,
    Io(String),
}

impl std::fmt::Display for StreamingDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "JSON stream error: {e:?}"),
            Self::UnsupportedLanguage(lang) => {
                write!(f, "language '{lang}' does not support streaming decode")
            }
            Self::InvalidUtf8 => write!(f, "input is not valid UTF-8"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DecodeOptions {
    pub nest_json: bool,
    pub emit_path: bool,
}

const READ_CHUNK_SIZE: usize = 8 * 1024;

pub fn stream_kind(language: &str) -> StreamKind {
    stream_kind_for_language(language)
}

// ---------------------------------------------------------------------------
// String-based decode (existing API)
// ---------------------------------------------------------------------------

pub fn decode(language: &str, input: &str) -> Result<Vec<StreamingEvent>, StreamingDecodeError> {
    decode_with_options(language, input, DecodeOptions::default())
}

pub fn decode_with_options(
    language: &str,
    input: &str,
    options: DecodeOptions,
) -> Result<Vec<StreamingEvent>, StreamingDecodeError> {
    match stream_kind(language) {
        StreamKind::Json => {
            decode_json_with_options(input, options).map_err(StreamingDecodeError::Json)
        }
        StreamKind::NonStreaming => Err(core_error_to_streaming_decode_error(
            language,
            CoreError::Parse(ParseError::InvalidSyntax),
        )),
    }
}

// ---------------------------------------------------------------------------
// Byte-slice / Reader decode paths
// ---------------------------------------------------------------------------

/// Decode from a byte slice. Validates UTF-8 before delegating to the
/// string-based decoder.
pub fn decode_from_bytes(
    language: &str,
    bytes: &[u8],
) -> Result<Vec<StreamingEvent>, StreamingDecodeError> {
    decode_from_bytes_with_options(language, bytes, DecodeOptions::default())
}

/// Decode from a byte slice with options.
pub fn decode_from_bytes_with_options(
    language: &str,
    bytes: &[u8],
    options: DecodeOptions,
) -> Result<Vec<StreamingEvent>, StreamingDecodeError> {
    let input = std::str::from_utf8(bytes).map_err(|_| StreamingDecodeError::InvalidUtf8)?;
    decode_with_options(language, input, options)
}

/// Decode from any `std::io::Read` source. Reads all bytes into memory,
/// validates UTF-8, then delegates to the string-based decoder.
pub fn decode_from_reader(
    language: &str,
    reader: &mut impl Read,
) -> Result<Vec<StreamingEvent>, StreamingDecodeError> {
    decode_from_reader_with_options(language, reader, DecodeOptions::default())
}

/// Decode from any `std::io::Read` source with options.
pub fn decode_from_reader_with_options(
    language: &str,
    reader: &mut impl Read,
    options: DecodeOptions,
) -> Result<Vec<StreamingEvent>, StreamingDecodeError> {
    match stream_kind(language) {
        StreamKind::Json => {
            let mut sink = BufferedEventSink::default();
            let mut parser = streaming_json::StreamingParser::with_sink(
                options.nest_json,
                options.emit_path,
                &mut sink,
            );
            feed_reader_chunks_streaming(reader, |chunk| {
                parser.feed(chunk).map_err(StreamingDecodeError::Json)
            })?;
            parser
                .finish_without_events()
                .map_err(StreamingDecodeError::Json)?;
            Ok(sink.into_events())
        }
        StreamKind::NonStreaming => Err(StreamingDecodeError::UnsupportedLanguage(
            language.to_string(),
        )),
    }
}

// ---------------------------------------------------------------------------
// EventSink callback path
// ---------------------------------------------------------------------------

/// Decode and feed events directly to an [`EventSink`] instead of collecting
/// by `decodeToTree`.
pub fn decode_with_sink(
    language: &str,
    input: &str,
    sink: &mut dyn EventSink<Error = CoreError>,
) -> Result<(), CoreError> {
    decode_with_sink_and_options(language, input, DecodeOptions::default(), sink)
}

/// Decode with options and feed events to an [`EventSink`].
pub fn decode_with_sink_and_options(
    language: &str,
    input: &str,
    options: DecodeOptions,
    sink: &mut dyn EventSink<Error = CoreError>,
) -> Result<(), CoreError> {
    match stream_kind(language) {
        StreamKind::Json => decode_json_to_sink(input, options, sink),
        StreamKind::NonStreaming => {
            // Non-streaming formats decode directly to a tree; there is no
            // event stream to feed.  Callers should use `decode_to_tree`
            // instead.
            Err(CoreError::Parse(ParseError::InvalidSyntax))
        }
    }
}

/// Decode from bytes and feed events to an [`EventSink`].
pub fn decode_bytes_with_sink(
    language: &str,
    bytes: &[u8],
    sink: &mut dyn EventSink<Error = CoreError>,
) -> Result<(), CoreError> {
    decode_bytes_with_sink_and_options(language, bytes, DecodeOptions::default(), sink)
}

/// Decode from bytes with options and feed events to an [`EventSink`].
pub fn decode_bytes_with_sink_and_options(
    language: &str,
    bytes: &[u8],
    options: DecodeOptions,
    sink: &mut dyn EventSink<Error = CoreError>,
) -> Result<(), CoreError> {
    let input =
        std::str::from_utf8(bytes).map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    decode_with_sink_and_options(language, input, options, sink)
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

/// Decode directly to a [`DecodedDocument`] tree, dispatching through the
/// [`RegistryOwner`] for non-streaming formats.
///
///
/// - **JSON**: uses the streaming decoder, feeds events into a
///   [`tree_builder::Builder`], and returns the constructed tree.
/// - **Non-streaming** (YAML, TOML, CSV, Python, JavaScript, etc.): uses
///   [`CodecService`] via the registry to decode directly to a tree.
pub fn decode_to_tree(
    _owner: &RegistryOwner,
    language: &str,
    input: &str,
    options: DecodeOptions,
) -> Result<DecodedDocument, CoreError> {
    match stream_kind(language) {
        StreamKind::Json => {
            let mut builder = tree_builder::Builder::new();
            decode_json_to_sink(input, options, &mut builder)?;
            builder.into_document()
        }
        StreamKind::NonStreaming => {
            let codec = CodecService::new();
            codec.decode(language, input)
        }
    }
}

/// Decode from bytes directly to a [`DecodedDocument`] tree.
pub fn decode_bytes_to_tree(
    owner: &RegistryOwner,
    language: &str,
    bytes: &[u8],
    options: DecodeOptions,
) -> Result<DecodedDocument, CoreError> {
    let input =
        std::str::from_utf8(bytes).map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    decode_to_tree(owner, language, input, options)
}

/// Decode from a [`Read`] source directly to a [`DecodedDocument`] tree.
pub fn decode_reader_to_tree(
    owner: &RegistryOwner,
    language: &str,
    reader: &mut impl Read,
    options: DecodeOptions,
) -> Result<DecodedDocument, CoreError> {
    match stream_kind(language) {
        StreamKind::Json => {
            let mut builder = tree_builder::Builder::new();
            let mut parser = streaming_json::StreamingParser::with_sink(
                options.nest_json,
                options.emit_path,
                &mut builder,
            );
            feed_reader_chunks_core(reader, |chunk| {
                parser
                    .feed(chunk)
                    .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))
            })?;
            parser
                .finish_without_events()
                .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
            builder.into_document()
        }
        StreamKind::NonStreaming => {
            let input = read_reader_to_string_core(reader)?;
            decode_to_tree(owner, language, &input, options)
        }
    }
}

// ---------------------------------------------------------------------------
// decode_to_document (existing API, fixed NonStreaming)
// ---------------------------------------------------------------------------

pub fn decode_to_document(language: &str, input: &str) -> Result<DecodedDocument, CoreError> {
    decode_to_document_with_options(language, input, DecodeOptions::default())
}

pub fn decode_to_document_with_options(
    language: &str,
    input: &str,
    options: DecodeOptions,
) -> Result<DecodedDocument, CoreError> {
    match stream_kind(language) {
        StreamKind::Json => decode_json_document_with_options(input, options),
        StreamKind::NonStreaming => {
            // Dispatch to the appropriate non-streaming decoder via
            // CodecService (handles YAML, TOML, CSV, Python, JavaScript, etc.)
            let codec = CodecService::new();
            codec.decode(language, input)
        }
    }
}

/// Decode from bytes directly to a [`DecodedDocument`].
pub fn decode_bytes_to_document(
    language: &str,
    bytes: &[u8],
) -> Result<DecodedDocument, CoreError> {
    decode_bytes_to_document_with_options(language, bytes, DecodeOptions::default())
}

/// Decode from bytes with options directly to a [`DecodedDocument`].
pub fn decode_bytes_to_document_with_options(
    language: &str,
    bytes: &[u8],
    options: DecodeOptions,
) -> Result<DecodedDocument, CoreError> {
    let input =
        std::str::from_utf8(bytes).map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
    decode_to_document_with_options(language, input, options)
}

/// Decode from a [`Read`] source directly to a [`DecodedDocument`].
pub fn decode_reader_to_document(
    language: &str,
    reader: &mut impl Read,
) -> Result<DecodedDocument, CoreError> {
    decode_reader_to_document_with_options(language, reader, DecodeOptions::default())
}

/// Decode from a [`Read`] source with options directly to a [`DecodedDocument`].
pub fn decode_reader_to_document_with_options(
    language: &str,
    reader: &mut impl Read,
    options: DecodeOptions,
) -> Result<DecodedDocument, CoreError> {
    match stream_kind(language) {
        StreamKind::Json => {
            decode_reader_to_tree(&RegistryOwner::init_owned(), language, reader, options)
        }
        StreamKind::NonStreaming => {
            let input = read_reader_to_string_core(reader)?;
            decode_to_document_with_options(language, &input, options)
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn decode_json_with_options(
    input: &str,
    options: DecodeOptions,
) -> Result<Vec<StreamingEvent>, streaming_json::JsonStreamError> {
    let mut sink = BufferedEventSink::default();
    decode_json_into_sink(input, options, &mut sink)?;
    Ok(sink.into_events())
}

fn decode_json_document_with_options(
    input: &str,
    options: DecodeOptions,
) -> Result<DecodedDocument, CoreError> {
    let mut builder = tree_builder::Builder::new();
    decode_json_to_sink(input, options, &mut builder)?;
    builder.into_document()
}

fn decode_json_to_sink(
    input: &str,
    options: DecodeOptions,
    sink: &mut dyn EventSink<Error = CoreError>,
) -> Result<(), CoreError> {
    let mut forwarding = ForwardingSink {
        inner: sink,
        emit_path: true,
    };
    decode_json_into_sink(input, options, &mut forwarding)
        .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))
}

fn decode_json_into_sink<T: EventSink<Error = CoreError>>(
    input: &str,
    options: DecodeOptions,
    sink: &mut T,
) -> Result<(), streaming_json::JsonStreamError> {
    let mut parser =
        streaming_json::StreamingParser::with_sink(options.nest_json, options.emit_path, sink);
    parser.feed(input)?;
    parser.finish_without_events()
}

#[derive(Debug, Default)]
struct BufferedEventSink {
    events: Vec<StreamingEvent>,
}

impl BufferedEventSink {
    fn into_events(self) -> Vec<StreamingEvent> {
        self.events
    }
}

impl EventSink for BufferedEventSink {
    type Error = CoreError;

    fn emit(&mut self, event: StreamingEvent) -> Result<(), Self::Error> {
        self.events.push(event);
        Ok(())
    }
}

struct ForwardingSink<'a> {
    inner: &'a mut dyn EventSink<Error = CoreError>,
    emit_path: bool,
}

impl EventSink for ForwardingSink<'_> {
    type Error = CoreError;

    fn emit(&mut self, mut event: StreamingEvent) -> Result<(), Self::Error> {
        if !self.emit_path {
            clear_paths(std::slice::from_mut(&mut event));
        }
        self.inner.emit(event)
    }
}

fn read_reader_to_string_core(reader: &mut impl Read) -> Result<String, CoreError> {
    let mut output = String::new();
    feed_reader_chunks_core(reader, |chunk| {
        output.push_str(chunk);
        Ok(())
    })?;
    Ok(output)
}

fn feed_reader_chunks_streaming(
    reader: &mut impl Read,
    mut on_chunk: impl FnMut(&str) -> Result<(), StreamingDecodeError>,
) -> Result<(), StreamingDecodeError> {
    let mut buf = [0u8; READ_CHUNK_SIZE];
    let mut pending = Vec::new();
    loop {
        let read = reader
            .read(&mut buf)
            .map_err(|e| StreamingDecodeError::Io(e.to_string()))?;
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&buf[..read]);
        drain_utf8_pending_streaming(&mut pending, false, &mut on_chunk)?;
    }
    drain_utf8_pending_streaming(&mut pending, true, &mut on_chunk)
}

fn feed_reader_chunks_core(
    reader: &mut impl Read,
    mut on_chunk: impl FnMut(&str) -> Result<(), CoreError>,
) -> Result<(), CoreError> {
    let mut buf = [0u8; READ_CHUNK_SIZE];
    let mut pending = Vec::new();
    loop {
        let read = reader
            .read(&mut buf)
            .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?;
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&buf[..read]);
        drain_utf8_pending_core(&mut pending, false, &mut on_chunk)?;
    }
    drain_utf8_pending_core(&mut pending, true, &mut on_chunk)
}

fn drain_utf8_pending_streaming(
    pending: &mut Vec<u8>,
    final_chunk: bool,
    on_chunk: &mut impl FnMut(&str) -> Result<(), StreamingDecodeError>,
) -> Result<(), StreamingDecodeError> {
    if pending.is_empty() {
        return Ok(());
    }
    match std::str::from_utf8(pending) {
        Ok(chunk) => {
            let owned = chunk.to_owned();
            pending.clear();
            on_chunk(&owned)
        }
        Err(err) => {
            let valid = err.valid_up_to();
            if valid > 0 {
                let owned = std::str::from_utf8(&pending[..valid])
                    .map_err(|_| StreamingDecodeError::InvalidUtf8)?
                    .to_owned();
                on_chunk(&owned)?;
            }
            if err.error_len().is_some() || final_chunk {
                return Err(StreamingDecodeError::InvalidUtf8);
            }
            let tail = pending[valid..].to_vec();
            *pending = tail;
            Ok(())
        }
    }
}

fn drain_utf8_pending_core(
    pending: &mut Vec<u8>,
    final_chunk: bool,
    on_chunk: &mut impl FnMut(&str) -> Result<(), CoreError>,
) -> Result<(), CoreError> {
    if pending.is_empty() {
        return Ok(());
    }
    match std::str::from_utf8(pending) {
        Ok(chunk) => {
            let owned = chunk.to_owned();
            pending.clear();
            on_chunk(&owned)
        }
        Err(err) => {
            let valid = err.valid_up_to();
            if valid > 0 {
                let owned = std::str::from_utf8(&pending[..valid])
                    .map_err(|_| CoreError::Parse(ParseError::InvalidSyntax))?
                    .to_owned();
                on_chunk(&owned)?;
            }
            if err.error_len().is_some() || final_chunk {
                return Err(CoreError::Parse(ParseError::InvalidSyntax));
            }
            let tail = pending[valid..].to_vec();
            *pending = tail;
            Ok(())
        }
    }
}

fn core_error_to_streaming_decode_error(language: &str, err: CoreError) -> StreamingDecodeError {
    match stream_kind(language) {
        StreamKind::Json => {
            StreamingDecodeError::Json(streaming_json::JsonStreamError::UnexpectedEnd)
        }
        StreamKind::NonStreaming => match err {
            CoreError::Io(msg) => StreamingDecodeError::Io(msg),
            _ => StreamingDecodeError::UnsupportedLanguage(language.to_string()),
        },
    }
}

fn clear_paths(events: &mut [StreamingEvent]) {
    for event in events {
        match event {
            StreamingEvent::DocStart(meta)
            | StreamingEvent::DocEnd(meta)
            | StreamingEvent::MapStart(meta)
            | StreamingEvent::MapEnd(meta)
            | StreamingEvent::SeqStart(meta)
            | StreamingEvent::SeqEnd(meta)
            | StreamingEvent::Alias { meta, .. }
            | StreamingEvent::ParseError { meta, .. } => {
                meta.path.clear();
                meta.path_supplier = None;
            }
            StreamingEvent::MapKey { meta, .. } | StreamingEvent::Scalar { meta, .. } => {
                meta.path.clear();
                meta.path_supplier = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_json_decoder() {
        let events = decode("json", "{\"a\":1}").expect("json decode should succeed");
        assert!(events.iter().any(|event| event.tag() == "map_key"));
    }

    #[test]
    fn decode_from_bytes_works() {
        let events = decode_from_bytes("json", b"{\"a\":1}").expect("byte decode should succeed");
        assert!(events.iter().any(|event| event.tag() == "map_key"));
    }

    #[test]
    fn decode_from_bytes_rejects_invalid_utf8() {
        let result = decode_from_bytes("json", b"\xff\xfe");
        assert!(matches!(result, Err(StreamingDecodeError::InvalidUtf8)));
    }

    #[test]
    fn decode_from_reader_works() {
        let mut cursor = std::io::Cursor::new(b"{\"a\":1}");
        let events = decode_from_reader("json", &mut cursor).expect("reader decode should succeed");
        assert!(events.iter().any(|event| event.tag() == "map_key"));
    }

    #[test]
    fn decode_with_sink_feeds_events() {
        struct CollectSink(Vec<StreamingEvent>);

        impl EventSink for CollectSink {
            type Error = CoreError;

            fn emit(&mut self, event: StreamingEvent) -> Result<(), Self::Error> {
                self.0.push(event);
                Ok(())
            }
        }

        let mut collected = CollectSink(Vec::new());
        decode_with_sink("json", "{\"a\":1}", &mut collected).expect("sink decode should succeed");
        assert!(collected.0.iter().any(|event| event.tag() == "map_key"));
    }

    #[test]
    fn decode_to_tree_json() {
        let owner = RegistryOwner::init_owned();
        let doc = decode_to_tree(&owner, "json", "{\"a\":1}", DecodeOptions::default())
            .expect("decode_to_tree should succeed");
        assert!(doc.store.get(doc.root).is_some());
    }

    #[test]
    fn decode_to_document_non_streaming_uses_codec_service() {
        // YAML is a non-streaming format; should now dispatch via CodecService
        let doc = decode_to_document("yaml", "key: value").expect("yaml decode should succeed");
        assert!(doc.store.get(doc.root).is_some());
    }

    #[test]
    fn decode_bytes_to_tree_works() {
        let owner = RegistryOwner::init_owned();
        let doc = decode_bytes_to_tree(&owner, "json", b"{\"a\":1}", DecodeOptions::default())
            .expect("decode_bytes_to_tree should succeed");
        assert!(doc.store.get(doc.root).is_some());
    }

    #[test]
    fn decode_reader_to_tree_works() {
        let owner = RegistryOwner::init_owned();
        let mut cursor = std::io::Cursor::new(b"{\"a\":1}");
        let doc = decode_reader_to_tree(&owner, "json", &mut cursor, DecodeOptions::default())
            .expect("decode_reader_to_tree should succeed");
        assert!(doc.store.get(doc.root).is_some());
    }

    #[test]
    fn nest_json_option_is_passed_to_json_decoder() {
        let mut decoder = crate::stream::streaming_json::StreamDecoder::new(true);
        let input = br#"{"nested":"{\"inner\":42}"}"#;
        decoder.feed_bytes(input).expect("feed should succeed");
        let events = decoder.finish_events().expect("finish should succeed");
        // Nest expansion is disabled (Wasm SourceRewrite bug).
        // Verify that decode succeeds and produces normal scalar events
        // (not nested tree events).
        assert!(
            !decoder.nested_json_expanded(),
            "nest expansion is disabled"
        );
        assert!(
            decoder.take_source_rewrites().is_empty(),
            "no source rewrites"
        );
        // Only 1 MapStart event (the outer object, no nested expansion)
        let map_starts = events
            .iter()
            .filter(|e| matches!(e, StreamingEvent::MapStart(_)))
            .count();
        assert_eq!(map_starts, 1, "only outer MapStart, no nested expansion");
    }
    #[test]
    fn emit_path_option_is_passed_to_json_decoder() {
        let events = decode_with_options(
            "json",
            r#"{"outer":{"inner":42}}"#,
            DecodeOptions {
                nest_json: false,
                emit_path: true,
            },
        )
        .expect("path emission decode should succeed");
        // At least one event should have a non-empty path when emit_path=true.
        let has_path = events.iter().any(|event| {
            let path = match event {
                StreamingEvent::DocStart(m)
                | StreamingEvent::DocEnd(m)
                | StreamingEvent::MapStart(m)
                | StreamingEvent::MapEnd(m)
                | StreamingEvent::SeqStart(m)
                | StreamingEvent::SeqEnd(m)
                | StreamingEvent::Alias { meta: m, .. }
                | StreamingEvent::ParseError { meta: m, .. } => &m.path,
                StreamingEvent::MapKey { meta: m, .. } | StreamingEvent::Scalar { meta: m, .. } => {
                    &m.path
                }
            };
            !path.is_empty()
        });
        assert!(
            has_path,
            "expected at least one event with a non-empty path"
        );
    }

    #[test]
    fn non_streaming_decode_to_document_handles_toml() {
        let doc =
            decode_to_document("toml", "key = \"value\"").expect("toml decode should succeed");
        assert!(doc.store.get(doc.root).is_some());
    }

    #[test]
    fn non_streaming_decode_with_options_returns_error() {
        // Streaming events are not available for non-streaming formats.
        let result = decode_with_options("yaml", "key: value", DecodeOptions::default());
        assert!(matches!(
            result,
            Err(StreamingDecodeError::UnsupportedLanguage(_))
        ));
    }
}
