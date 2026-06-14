use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let is_wasm = std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32");

    if is_wasm {
        compile_wasm_grammars();
    } else {
        // JavaScript grammar is always needed — JSON uses it for tree-sitter queries.
        compile_tree_sitter_grammar(
            "treease-ts-javascript",
            "../../deps/tree-sitter-javascript/src",
        );
        if !is_lite() {
            compile_tree_sitter_grammar(
                "treease-ts-json-schema",
                "../../deps/tree-sitter-yaml/schema/json/src",
            );
            compile_tree_sitter_grammar("treease-ts-yaml", "../../deps/tree-sitter-yaml/src");
            compile_tree_sitter_grammar("treease-ts-toml", "../../deps/tree-sitter-toml/src");
            compile_tree_sitter_grammar("treease-ts-python", "../../deps/tree-sitter-python/src");
        }
    }
}

// ---------------------------------------------------------------------------
// wasm32 path – compile .c → .o, skip ar entirely
// ---------------------------------------------------------------------------

/// Check if the `lite` Cargo feature is enabled (JSON-only mode).
fn is_lite() -> bool {
    std::env::var("CARGO_FEATURE_LITE").is_ok()
}
#[derive(Clone)]
struct WasmGrammar {
    name: &'static str,
    src_dir: &'static str,
}

/// Non-JSON grammars (excluding JavaScript which JSON depends on).
fn non_json_wasm_grammars() -> &'static [WasmGrammar] {
    &[
        WasmGrammar {
            name: "treease-ts-json-schema",
            src_dir: "../../deps/tree-sitter-yaml/schema/json/src",
        },
        WasmGrammar {
            name: "treease-ts-yaml",
            src_dir: "../../deps/tree-sitter-yaml/src",
        },
        WasmGrammar {
            name: "treease-ts-toml",
            src_dir: "../../deps/tree-sitter-toml/src",
        },
        WasmGrammar {
            name: "treease-ts-python",
            src_dir: "../../deps/tree-sitter-python/src",
        },
        WasmGrammar {
            name: "treease-ts-javascript",
            src_dir: "../../deps/tree-sitter-javascript/src",
        },
    ]
}

fn wasm_grammars() -> Vec<WasmGrammar> {
    if is_lite() {
        // JSON's tree-sitter support uses the JavaScript grammar.
        vec![WasmGrammar {
            name: "treease-ts-javascript",
            src_dir: "../../deps/tree-sitter-javascript/src",
        }]
    } else {
        non_json_wasm_grammars().to_vec()
    }
}
fn compile_wasm_grammars() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ts_src = "../../deps/tree-sitter/lib/src";
    let ts_include = "../../deps/tree-sitter/lib/include";

    // NOTE: We do NOT compile lib.c or wasm stdlib (stdio/stdlib/string) files
    // here — the Rust tree-sitter crate (deps/tree-sitter/binding_rust/build.rs)
    // already compiles them into libtree_sitter.rlib via the `cc` crate.
    // Duplicating these symbols is a hard error with rust-lld on Linux wasm32.

    // Compile isw* function stubs that WASI libc declares but doesn't provide.
    // Must NOT use -I wasm_compat here — wctype.h macros would break the
    // function definitions. The system <wctype.h> provides the necessary types.
    compile_zig_cc_no_compat("wasm_compat/isw_stubs.c", &out_dir.join("isw_stubs.o"));

    let grammars = wasm_grammars();
    for gram in &grammars {
        let parser = format!("{}/parser.c", gram.src_dir);
        compile_zig_cc(
            &parser,
            &out_dir.join(format!("{}-parser.o", gram.name)),
            &[gram.src_dir, ts_src, ts_include],
        );

        let scanner = format!("{}/scanner.c", gram.src_dir);
        if Path::new(&scanner).exists() {
            compile_zig_cc(
                &scanner,
                &out_dir.join(format!("{}-scanner.o", gram.name)),
                &[gram.src_dir, ts_src, ts_include],
            );
        }
    }
}

fn compile_zig_cc(source: &str, dest: &Path, includes: &[&str]) {
    rerun_if_changed(Path::new(source));

    let mut cmd = Command::new("zig");
    cmd.arg("cc")
        .arg("-target")
        .arg("wasm32-wasi")
        .arg("-O2")
        .arg("-std=c11")
        .arg("-c")
        .arg(source)
        .arg("-o")
        .arg(dest);

    // wasm_compat provides wctype.h macros that replace wide-char function
    // calls with inline expressions, avoiding unresolved WASM imports.
    cmd.arg("-I").arg("wasm_compat");

    for inc in includes {
        cmd.arg("-I").arg(inc);
    }

    // Suppress warnings for vendored C code
    cmd.arg("-Wno-unused-parameter");
    cmd.arg("-Wno-unused-but-set-variable");
    cmd.arg("-Wno-unused-value");
    cmd.arg("-Wno-implicit-fallthrough");

    let status = cmd.status().unwrap_or_else(|e| {
        panic!("failed to run zig cc for {source}: {e}");
    });
    if !status.success() {
        panic!("zig cc failed for {source}");
    }

    println!(
        "cargo:rustc-link-arg={}",
        dest.to_str().expect("non-UTF-8 OUT_DIR")
    );
}

fn compile_zig_cc_no_compat(source: &str, dest: &Path) {
    rerun_if_changed(Path::new(source));

    let mut cmd = Command::new("zig");
    cmd.arg("cc")
        .arg("-target")
        .arg("wasm32-wasi")
        .arg("-O2")
        .arg("-std=c11")
        .arg("-c")
        .arg(source)
        .arg("-o")
        .arg(dest);

    let status = cmd.status().unwrap_or_else(|e| {
        panic!("failed to run zig cc for {source}: {e}");
    });
    if !status.success() {
        panic!("zig cc failed for {source}");
    }

    println!(
        "cargo:rustc-link-arg={}",
        dest.to_str().expect("non-UTF-8 OUT_DIR")
    );
}

// ---------------------------------------------------------------------------
// Non-wasm path – use cc::Build (host compiler, works with system ar)
// ---------------------------------------------------------------------------

fn compile_tree_sitter_grammar(name: &str, relative_src: &str) {
    let src_dir = PathBuf::from(relative_src);
    let parser_path = src_dir.join("parser.c");
    let scanner_path = src_dir.join("scanner.c");

    let mut build = cc::Build::new();
    build
        .std("c11")
        .include(&src_dir)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-unused-value")
        .flag_if_supported("-Wno-implicit-fallthrough");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        build
            .include("../../deps/tree-sitter/lib/include")
            .include("../../deps/tree-sitter/lib/src")
            .include("../../deps/tree-sitter/lib/src/wasm");
    }

    build.file(&parser_path);
    rerun_if_changed(&parser_path);

    if scanner_path.exists() {
        build.file(&scanner_path);
        rerun_if_changed(&scanner_path);
    }

    build.compile(name);
}

fn rerun_if_changed(path: &Path) {
    if let Some(path) = path.to_str() {
        println!("cargo:rerun-if-changed={path}");
    }
}
