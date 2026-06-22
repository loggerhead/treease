pub mod compare;
pub mod core;
pub mod document;
pub mod evaluator;
pub mod expression_pipeline;
pub mod formats;
pub mod operators;
pub mod parser;
pub mod stream;
pub mod wasm;
pub mod wasm_document;
pub mod wasm_types;

#[cfg(target_arch = "wasm32")]
mod wasm_wctype_shims {
    // Some vendored tree-sitter grammars call wide-char classification helpers.
    // On wasm these end up as unresolved `env.*` imports unless we provide them.
    #[unsafe(no_mangle)]
    pub extern "C" fn iswalpha(c: u32) -> i32 {
        (((b'a' as u32)..=(b'z' as u32)).contains(&c)
            || ((b'A' as u32)..=(b'Z' as u32)).contains(&c)) as i32
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn iswspace(c: u32) -> i32 {
        matches!(c, 0x20 | 0x09..=0x0d) as i32
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn iswxdigit(c: u32) -> i32 {
        (((b'0' as u32)..=(b'9' as u32)).contains(&c)
            || ((b'a' as u32)..=(b'f' as u32)).contains(&c)
            || ((b'A' as u32)..=(b'F' as u32)).contains(&c)) as i32
    }
}

pub fn init() -> Result<core::RegistryOwner, core::CoreError> {
    Ok(core::RegistryOwner::init_owned())
}

pub fn to_handle(registry: core::Registry) -> core::RegistryHandle {
    core::to_handle(registry)
}

pub fn deinit(owner: &mut core::RegistryOwner) {
    owner.deinit();
}

pub mod internal {
    pub mod core {
        pub use crate::core::authoritative_graph_service;
        pub use crate::core::codec_service;
        pub use crate::core::context;
        pub use crate::core::core_helpers as utils;
        pub use crate::core::diagnostics;
        pub use crate::core::document_analysis;
        pub use crate::core::encoding;
        pub use crate::core::errors;
        pub use crate::core::expression;
        pub use crate::core::expression_builder;
        pub use crate::core::format;
        pub use crate::core::format_registry;
        pub use crate::core::graph_builder;
        pub use crate::core::graph_builder_preorder;
        pub use crate::core::graph_delta;
        pub use crate::core::graph_delta_service;
        pub use crate::core::graph_fragment_index;
        pub use crate::core::graph_identity;
        pub use crate::core::graph_model;
        pub use crate::core::graph_relayout;
        pub use crate::core::incremental_edit;
        pub use crate::core::io_adapters;
        pub use crate::core::json_block;
        pub use crate::core::lang_spec;
        pub use crate::core::language;
        pub use crate::core::line_index;
        pub use crate::core::literal_format;
        pub use crate::core::operation;
        pub use crate::core::operation_defs;
        pub use crate::core::operation_prefs;
        pub use crate::core::operator_registry;
        pub use crate::core::printer;
        pub use crate::core::printer_writer;
        pub use crate::core::registry;
        pub use crate::core::sem_type;
        pub use crate::core::semantic_tokens;
        pub use crate::core::traversal_builder;
        pub use crate::core::tree_navigator;
        pub use crate::core::tree_node;
        pub use crate::core::tree_ops;
        pub use crate::core::tree_path;
        pub use crate::core::tree_sitter_support;
        pub use crate::core::tree_store;
    }

    pub mod evaluator {
        pub use crate::core::tree_navigator;
        pub use crate::evaluator::all_at_once_evaluator;
        pub use crate::evaluator::stream_evaluator;
    }

    pub mod formats {
        pub use crate::core::format;
        pub use crate::formats::decoder_csv;
        pub use crate::formats::decoder_csv_object;
        pub use crate::formats::decoder_json;
        pub use crate::formats::decoder_object;
        pub use crate::formats::decoder_toml;
        pub use crate::formats::decoder_yaml;
        pub use crate::formats::encoder;
        pub use crate::formats::encoder_csv;
        pub use crate::formats::encoder_javascript;
        pub use crate::formats::encoder_json;
        pub use crate::formats::encoder_python;
        pub use crate::formats::encoder_toml;
        pub use crate::formats::encoder_yaml;
        pub use crate::formats::formats_helpers as utils;
        pub use crate::formats::preferences;
    }

    pub mod parser {
        pub use crate::expression_pipeline;
        pub use crate::parser::lexer;
        pub use crate::parser::lexer_participle;
        pub use crate::parser::parser as expression_parser;
    }

    pub mod stream {
        pub use crate::stream::streaming_decoder;
        pub use crate::stream::streaming_events;
        pub use crate::stream::streaming_json;
        pub use crate::stream::tree_builder;
    }

    pub mod wasm {
        pub use crate::core::lang_spec as languages;
        pub use crate::wasm::document_analysis_shared;
        pub use crate::wasm::semantic_tokens_shared;
        pub use crate::wasm::value_json_shared;
    }

    pub use crate::compare;
    pub use crate::core::tree_sitter_support as tree_sitter;
}
