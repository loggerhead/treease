use treease_core::core::{
    CodecService, CoreError, FormatError, FormatLanguage, canonical_format_name,
    language_for_format,
};

#[test]
fn codec_service_resolves_canonical_names_and_languages() {
    assert_eq!(canonical_format_name("yml").unwrap(), "yaml");
    assert_eq!(canonical_format_name("py").unwrap(), "python");
    assert_eq!(canonical_format_name("js").unwrap(), "javascript");
    assert_eq!(language_for_format("json").unwrap(), FormatLanguage::Json);
    assert_eq!(language_for_format("yaml").unwrap(), FormatLanguage::Yaml);
    assert!(matches!(
        canonical_format_name("unknown"),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));
}

#[test]
fn codec_service_decodes_encodes_minifies_and_converts() {
    let service = CodecService::new();
    let decoded = service.decode("json", "{\"a\":1,\"b\":[2,3]}").unwrap();

    let pretty_json = service
        .encode_to_string("json", &decoded.store, decoded.root)
        .unwrap();
    assert!(pretty_json.contains("\"a\""));
    assert!(pretty_json.contains('\n'));

    let minified_json = service
        .minify_to_string("json", &decoded.store, decoded.root)
        .unwrap();
    assert_eq!(minified_json, "{\"a\":1,\"b\":[2,3]}\n");

    let yaml = service
        .convert_string("json", "yaml", "{\"a\":1,\"b\":[2,3]}")
        .unwrap();
    assert!(yaml.contains("a: 1"));
    assert!(yaml.contains("- 2"));
}

#[test]
fn codec_service_reports_unknown_format_errors() {
    let service = CodecService::new();

    assert!(matches!(
        service.decode("nope", "{}"),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));
    assert!(matches!(
        service.preferences_for("nope"),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));
}

#[test]
fn codec_service_exposes_encoder_and_decoder_lookup() {
    let service = CodecService::new();

    assert!(service.get_decoder("json").is_ok());
    assert!(service.get_encoder("json", 2).is_ok());
    assert!(matches!(
        service.get_decoder("nope"),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));
    assert!(matches!(
        service.get_encoder("nope", 2),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));
}

#[test]
fn codec_service_resolves_encoder_and_decoder_by_format_name() {
    let service = CodecService::new();

    let enc = service.get_encoder("yaml", 2);
    assert!(enc.is_ok());

    let dec = service.get_decoder("yaml");
    assert!(dec.is_ok());
}

#[test]
fn codec_service_returns_error_for_unknown_format() {
    let service = CodecService::new();

    assert!(matches!(
        service.get_encoder("unknown", 2),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));
    assert!(matches!(
        service.get_decoder("unknown"),
        Err(CoreError::Format(FormatError::UnknownFormat))
    ));
}

#[test]
fn codec_service_supports_format_aliases() {
    let service = CodecService::new();

    let enc = service.get_encoder("y", 2);
    assert!(enc.is_ok());

    let dec = service.get_decoder("y");
    assert!(dec.is_ok());
}
