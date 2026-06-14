use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};

#[cfg(not(feature = "lite"))]
use crate::formats::{
    CsvDecoder, CsvObjectDecoder, JavascriptObjectDecoder, TomlDecoder, YamlDecoder,
};
use crate::formats::{Decode, DecodedDocument, JsonDecoder};
use crate::wasm_types::{CommonFormatOptions, WasmProtocol};

use super::RustWasmStatus;

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

type DecoderFn = fn(&str) -> Result<DecodedDocument, RustWasmStatus>;

struct FormatEntry {
    decode: DecoderFn,
}

static FORMAT_REGISTRY: LazyLock<Mutex<HashMap<&'static str, FormatEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn lock_format_registry() -> MutexGuard<'static, HashMap<&'static str, FormatEntry>> {
    FORMAT_REGISTRY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Populate the format registry with all enabled decoders.
///
/// checks `enabled && is_format`, and registers each decoder factory into
/// the registry.  Idempotent — subsequent calls are no-ops.
pub(crate) fn ensure_formats() {
    let mut registry = lock_format_registry();
    if !registry.is_empty() {
        return;
    }
    register_format(&mut registry, "json", |source| {
        JsonDecoder
            .decode_str(source)
            .map_err(|_| RustWasmStatus::CoreError)
    });
    #[cfg(not(feature = "lite"))]
    register_format(&mut registry, "yaml", |source| {
        YamlDecoder
            .decode_str(source)
            .map_err(|_| RustWasmStatus::CoreError)
    });
    #[cfg(not(feature = "lite"))]
    register_format(&mut registry, "toml", |source| {
        TomlDecoder
            .decode_str(source)
            .map_err(|_| RustWasmStatus::CoreError)
    });
    #[cfg(not(feature = "lite"))]
    register_format(&mut registry, "python", |source| {
        crate::formats::PythonObjectDecoder
            .decode_str(source)
            .map_err(|_| RustWasmStatus::CoreError)
    });
    #[cfg(not(feature = "lite"))]
    register_format(&mut registry, "javascript", |source| {
        JavascriptObjectDecoder
            .decode_str(source)
            .map_err(|_| RustWasmStatus::CoreError)
    });
    #[cfg(not(feature = "lite"))]
    register_format(&mut registry, "csv", |source| {
        CsvObjectDecoder::default()
            .decode_str(source)
            .or_else(|_| CsvDecoder.decode_str(source))
            .map_err(|_| RustWasmStatus::CoreError)
    });
}

/// Register a single format decoder, skipping if already present.
fn register_format(
    registry: &mut HashMap<&'static str, FormatEntry>,
    name: &'static str,
    decode: DecoderFn,
) {
    if registry.contains_key(name) {
        return;
    }
    registry.insert(name, FormatEntry { decode });
}

// ---------------------------------------------------------------------------
// Decoder dispatch — now goes through the registry
// ---------------------------------------------------------------------------

/// Decode a document via the registry-based format backend.
pub(crate) fn decode_document(
    language: &str,
    source: &str,
) -> Result<DecodedDocument, RustWasmStatus> {
    let protocol = WasmProtocol::from_name(language).ok_or(RustWasmStatus::UnsupportedLanguage)?;
    let canonical = protocol.canonical_name();

    let decode = {
        let registry = lock_format_registry();
        let entry = registry
            .get(canonical)
            .ok_or(RustWasmStatus::UnsupportedLanguage)?;
        entry.decode
    };
    decode(source)
}

pub(crate) fn decode_value_document(
    language: &str,
    source: &str,
) -> Result<DecodedDocument, RustWasmStatus> {
    #[cfg(not(feature = "lite"))]
    {
        match WasmProtocol::from_name(language) {
            Some(WasmProtocol::Toml) => TomlDecoder
                .decode_str(source)
                .map_err(|_| RustWasmStatus::CoreError),
            _ => decode_document(language, source),
        }
    }
    #[cfg(feature = "lite")]
    {
        decode_document(language, source)
    }
}

pub(crate) fn decode_document_for_stream_session(
    language: &str,
    source: &str,
    nest: bool,
) -> Result<DecodedDocument, RustWasmStatus> {
    match WasmProtocol::from_name(language) {
        Some(WasmProtocol::Json) => crate::stream::decode_to_document_with_options(
            language,
            source,
            crate::stream::DecodeOptions {
                nest_json: nest,
                emit_path: false,
            },
        )
        .map_err(|_| RustWasmStatus::CoreError),
        _ => decode_document(language, source),
    }
}

pub(crate) fn normalize_language(language: &str) -> Option<&'static str> {
    WasmProtocol::from_name(language).map(WasmProtocol::canonical_name)
}

pub(crate) fn normalize_output_format(format: &str) -> Option<&'static str> {
    if format.trim().is_empty() {
        Some(WasmProtocol::Json.canonical_name())
    } else {
        WasmProtocol::from_name(format).map(WasmProtocol::canonical_name)
    }
}

pub(crate) fn convert_output_options(
    format: &str,
    options: CommonFormatOptions,
) -> CommonFormatOptions {
    let mut converted = options;
    if format == "toml" {
        converted.indent = 2;
    }
    converted
}
