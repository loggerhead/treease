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
