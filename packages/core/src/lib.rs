pub mod analysis;
pub mod compare;
pub mod context;
pub mod core;
pub mod document;
#[path = "wasm/document_analysis_shared.rs"]
pub mod document_analysis_shared;
pub mod errors;
pub mod evaluator;
pub mod expression_pipeline;
pub mod formats;
pub mod graph;
pub mod io;
pub mod language;
pub mod layout;
pub mod operators;
pub mod parser;
pub mod registry;
#[path = "wasm/semantic_tokens_shared.rs"]
pub mod semantic_tokens_shared;
pub mod stream;
#[cfg(test)]
pub mod test_timing;
pub mod tree;
#[path = "wasm/value_json_shared.rs"]
pub mod value_json_shared;
#[cfg(feature = "wasm")]
pub mod wasm;
#[cfg(feature = "wasm")]
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
        pub use crate::analysis::diagnostics;
        pub use crate::analysis::document_analysis;
        pub use crate::analysis::line_index;
        pub use crate::context;
        pub use crate::errors;
        pub use crate::graph::authoritative_graph_service;
        pub use crate::graph::graph_builder;
        pub use crate::graph::graph_builder_preorder;
        pub use crate::graph::graph_delta;
        pub use crate::graph::graph_delta_service;
        pub use crate::graph::graph_fragment_index;
        pub use crate::graph::graph_identity;
        pub use crate::graph::graph_model;
        pub use crate::graph::graph_relayout;
        pub use crate::io::codec_service;
        pub use crate::io::encoding;
        pub use crate::io::io_adapters;
        pub use crate::io::literal_format;
        pub use crate::io::printer;
        pub use crate::io::printer_writer;
        pub use crate::language::lang_spec;
        pub use crate::language::language;
        pub use crate::language::sem_type;
        pub use crate::language::semantic_tokens;
        pub use crate::language::tree_sitter_support;
        pub use crate::operators::core_helpers as utils;
        pub use crate::registry::expression;
        pub use crate::registry::expression_builder;
        pub use crate::registry::format;
        pub use crate::registry::format_registry;
        pub use crate::registry::operation;
        pub use crate::registry::operation_defs;
        pub use crate::registry::operation_prefs;
        pub use crate::registry::operator_registry;
        pub use crate::registry::registry;
        pub use crate::registry::traversal_builder;
        pub use crate::tree::incremental_edit;
        pub use crate::tree::json_block;
        pub use crate::tree::tree_navigator;
        pub use crate::tree::tree_node;
        pub use crate::tree::tree_ops;
        pub use crate::tree::tree_path;
        pub use crate::tree::tree_store;
    }

    pub mod evaluator {
        pub use crate::evaluator::all_at_once_evaluator;
        pub use crate::evaluator::stream_evaluator;
        pub use crate::tree::tree_navigator;
    }

    pub mod formats {
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
        pub use crate::registry::format;
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
        pub use crate::document_analysis_shared;
        pub use crate::language::lang_spec as languages;
        pub use crate::semantic_tokens_shared;
        pub use crate::value_json_shared;
    }

    pub use crate::compare;
    pub use crate::language::tree_sitter_support as tree_sitter;
}
