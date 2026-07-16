//! Language capability seam.
//!
//! This registry owns language-level behaviour only.  In particular it never
//! owns a `DocumentRuntime`, a snapshot, or any freshness state: callers keep
//! document authority and delegate just the language calculation here.

use crate::document::value_edit::{GraphValueEditContext, GraphValueEditPlanner};
use crate::errors::CoreError;
use crate::formats::{
    CsvEncoder, CsvObjectDecoder, PythonEncoder, PythonObjectDecoder, TomlDecoder, TomlEncoder,
    YamlDecoder, YamlEncoder,
};
use crate::formats::{
    Decode, Encode, JavascriptEncoder, JavascriptObjectDecoder, JsonDecoder, JsonEncoder,
};
use crate::language::lang_spec::{CSV_SPEC, PYTHON_SPEC, TOML_SPEC, YAML_SPEC};
use crate::language::lang_spec::{JAVASCRIPT_SPEC, JSON_SPEC, LangSpec};
use crate::tree::{NodeId, TreeStore};

/// The complete language-facing interface.  Each concrete adapter owns all
/// per-language choices; callers never select a decoder, encoder, token path,
/// tree-path rule, or value-edit planner themselves.
pub(crate) trait LanguageAdapter: Send + Sync {
    fn spec(&self) -> &'static LangSpec<'static>;
    fn decode(&self) -> Box<dyn Decode>;
    fn encode(&self, prefs: crate::formats::FormatPreferences) -> Box<dyn Encode>;
    fn semantic_tokens(&self, source: &str) -> Vec<u32>;
    fn graph_value_edit_planner(&self) -> &'static dyn GraphValueEditPlanner;

    fn tree_path_supported(&self) -> bool {
        self.spec().has_structured_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryLoadError {
    EmptyLanguageName,
    DuplicateLanguage(String),
}

impl std::fmt::Display for RegistryLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyLanguageName => write!(f, "language adapter has an empty name"),
            Self::DuplicateLanguage(language) => {
                write!(f, "language adapter already registered: {language}")
            }
        }
    }
}

/// Immutable-at-use registry.  Loading is explicit and validated so a pack
/// failure cannot turn into a fallback decoder or a silently partial adapter.
#[derive(Default)]
pub(crate) struct LanguageCapabilityRegistry {
    adapters: Vec<Box<dyn LanguageAdapter>>,
}

impl LanguageCapabilityRegistry {
    pub fn with_builtin_adapters() -> Self {
        let mut registry = Self::default();
        registry
            .register(Box::new(JsonLanguageAdapter))
            .expect("json adapter is valid");
        registry
            .register(Box::new(YamlLanguageAdapter))
            .expect("yaml adapter is valid");
        registry
            .register(Box::new(StaticLanguageAdapter::toml()))
            .expect("toml adapter is valid");
        registry
            .register(Box::new(StaticLanguageAdapter::javascript()))
            .expect("javascript adapter is valid");
        registry
            .register(Box::new(StaticLanguageAdapter::python()))
            .expect("python adapter is valid");
        registry
            .register(Box::new(StaticLanguageAdapter::csv()))
            .expect("csv adapter is valid");
        registry
    }

    pub fn register(&mut self, adapter: Box<dyn LanguageAdapter>) -> Result<(), RegistryLoadError> {
        let name = adapter.spec().name.trim();
        if name.is_empty() {
            return Err(RegistryLoadError::EmptyLanguageName);
        }
        if self.find(name).is_some() {
            return Err(RegistryLoadError::DuplicateLanguage(name.to_owned()));
        }
        self.adapters.push(adapter);
        Ok(())
    }

    pub fn find(&self, language: &str) -> Option<&dyn LanguageAdapter> {
        self.adapters
            .iter()
            .map(Box::as_ref)
            .find(|adapter| adapter.spec().matches_name(language))
    }

    pub fn find_extension(&self, extension: &str) -> Option<&dyn LanguageAdapter> {
        self.adapters
            .iter()
            .map(Box::as_ref)
            .find(|adapter| adapter.spec().matches_extension(extension))
    }

    pub fn require(
        &self,
        language: &str,
        capability: &'static str,
    ) -> Result<&dyn LanguageAdapter, CoreError> {
        self.find(language)
            .ok_or_else(|| CoreError::CapabilityMissing {
                language: language.trim().to_owned(),
                capability,
            })
    }
}

pub(crate) fn builtin_registry() -> &'static LanguageCapabilityRegistry {
    static REGISTRY: std::sync::OnceLock<LanguageCapabilityRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(LanguageCapabilityRegistry::with_builtin_adapters)
}

pub(crate) fn capability_for_format(
    language: &str,
    capability: &'static str,
) -> Result<&'static dyn LanguageAdapter, CoreError> {
    builtin_registry().require(language, capability)
}

pub fn encode_with_capability(
    language: &str,
    prefs: crate::formats::FormatPreferences,
    store: &TreeStore,
    root: NodeId,
) -> Result<String, CoreError> {
    capability_for_format(language, "encode")?
        .encode(prefs)
        .encode_to_string(store, root)
}

pub fn semantic_tokens_with_capability(
    language: &str,
    source: &str,
) -> Result<Vec<u32>, CoreError> {
    Ok(capability_for_format(language, "semantic_tokens")?.semantic_tokens(source))
}

pub(crate) fn plan_graph_value_edit_with_capability(
    language: &str,
    context: GraphValueEditContext<'_>,
) -> Result<crate::document::protocol::GraphValueEditPlan, CoreError> {
    Ok(capability_for_format(language, "graph_value_edit")?
        .graph_value_edit_planner()
        .plan(context))
}

pub(crate) struct JsonLanguageAdapter;
pub(crate) struct YamlLanguageAdapter;

macro_rules! adapter_impl {
    ($type:ty, $spec:expr, $decoder:expr, $encoder:expr, $planner:expr) => {
        impl LanguageAdapter for $type {
            fn spec(&self) -> &'static LangSpec<'static> {
                &$spec
            }
            fn decode(&self) -> Box<dyn Decode> {
                $decoder
            }
            fn encode(&self, prefs: crate::formats::FormatPreferences) -> Box<dyn Encode> {
                $encoder(prefs)
            }
            fn semantic_tokens(&self, source: &str) -> Vec<u32> {
                crate::language::semantic_tokens::encode_semantic_tokens_direct(
                    self.spec().name,
                    source,
                )
            }
            fn graph_value_edit_planner(&self) -> &'static dyn GraphValueEditPlanner {
                $planner
            }
        }
    };
}

adapter_impl!(
    JsonLanguageAdapter,
    JSON_SPEC,
    Box::new(JsonDecoder::default()),
    |prefs| Box::new(JsonEncoder::new(prefs)),
    crate::document::value_edit::json::planner()
);
adapter_impl!(
    YamlLanguageAdapter,
    YAML_SPEC,
    Box::new(YamlDecoder::default()),
    |prefs| Box::new(YamlEncoder::new(prefs)),
    crate::document::value_edit::yaml::planner()
);

struct StaticLanguageAdapter {
    spec: &'static LangSpec<'static>,
    decoder: fn() -> Box<dyn Decode>,
    encoder: fn(crate::formats::FormatPreferences) -> Box<dyn Encode>,
    planner: &'static dyn GraphValueEditPlanner,
}

impl StaticLanguageAdapter {
    fn toml() -> Self {
        Self {
            spec: &TOML_SPEC,
            decoder: || Box::new(TomlDecoder::default()),
            encoder: |prefs| Box::new(TomlEncoder::new(prefs)),
            planner: crate::document::value_edit::toml::planner(),
        }
    }
    fn javascript() -> Self {
        Self {
            spec: &JAVASCRIPT_SPEC,
            decoder: || Box::new(JavascriptObjectDecoder::default()),
            encoder: |prefs| Box::new(JavascriptEncoder::new(prefs)),
            planner: crate::document::value_edit::javascript::planner(),
        }
    }
    fn python() -> Self {
        Self {
            spec: &PYTHON_SPEC,
            decoder: || Box::new(PythonObjectDecoder::default()),
            encoder: |prefs| Box::new(PythonEncoder::new(prefs)),
            planner: crate::document::value_edit::python::planner(),
        }
    }
    fn csv() -> Self {
        Self {
            spec: &CSV_SPEC,
            decoder: || Box::new(CsvObjectDecoder::default()),
            encoder: |prefs| Box::new(CsvEncoder::new(prefs)),
            planner: crate::document::value_edit::csv::planner(),
        }
    }
}

impl LanguageAdapter for StaticLanguageAdapter {
    fn spec(&self) -> &'static LangSpec<'static> {
        self.spec
    }
    fn decode(&self) -> Box<dyn Decode> {
        (self.decoder)()
    }
    fn encode(&self, prefs: crate::formats::FormatPreferences) -> Box<dyn Encode> {
        (self.encoder)(prefs)
    }
    fn semantic_tokens(&self, source: &str) -> Vec<u32> {
        crate::language::semantic_tokens::encode_semantic_tokens_direct(self.spec.name, source)
    }
    fn graph_value_edit_planner(&self) -> &'static dyn GraphValueEditPlanner {
        self.planner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_adapters_expose_consistent_json_and_yaml_capabilities() {
        let registry = builtin_registry();
        for language in ["json", "yaml"] {
            let adapter = registry.require(language, "contract").unwrap();
            assert_eq!(adapter.spec().name, language);
            assert_eq!(adapter.tree_path_supported(), language == "yaml");
            let source = if language == "json" {
                r#"{"a":1}"#
            } else {
                "a: 1\n"
            };
            let document = adapter.decode().decode_str(source).unwrap();
            assert!(
                adapter
                    .encode(crate::formats::FormatPreferences::default())
                    .encode_to_string(&document.store, document.root)
                    .is_ok()
            );
            assert!(
                !adapter.semantic_tokens(source).is_empty(),
                "{language} adapter must dispatch semantic tokens"
            );

            let analysis = crate::document::snapshot::AnalysisBundle {
                language: language.to_owned(),
                source: source.to_owned(),
                document: Some(document.clone()),
                ..Default::default()
            };
            let request = crate::document::protocol::GraphValueEditRequest {
                document_key: "registry-contract".to_owned(),
                snapshot_id: Default::default(),
                language: language.to_owned(),
                path: vec![crate::document::protocol::GraphPathSeg {
                    tag: 0,
                    key: "a".to_owned(),
                    index: 0,
                }],
                prefer_key: false,
                value: serde_json::json!(2),
            };
            let plan = adapter
                .graph_value_edit_planner()
                .plan(GraphValueEditContext {
                    analysis: &analysis,
                    document: &document,
                    request: &request,
                    path_index: None,
                });
            assert!(
                plan.reason.is_none(),
                "{language} adapter must dispatch graph value edits"
            );
        }
    }

    #[test]
    fn unregistered_language_is_an_explicit_capability_error() {
        assert!(matches!(
            builtin_registry().require("made-up", "decode"),
            Err(CoreError::CapabilityMissing { .. })
        ));
    }

    #[test]
    fn adapter_load_failure_is_not_silently_accepted() {
        let mut registry = LanguageCapabilityRegistry::default();
        registry.register(Box::new(JsonLanguageAdapter)).unwrap();
        assert_eq!(
            registry.register(Box::new(JsonLanguageAdapter)),
            Err(RegistryLoadError::DuplicateLanguage("json".to_owned()))
        );
    }

    #[test]
    fn builtin_registry_exposes_all_language_adapters() {
        assert!(builtin_registry().find("json").is_some());
        assert!(builtin_registry().find("yaml").is_some());
    }
}
